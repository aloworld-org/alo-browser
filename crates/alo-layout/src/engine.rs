/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The boundary. **This is the only file in the repository that names
//! `taffy`.**
//!
//! ADR 0001 calls `taffy` a judgement call rather than physics: it is a real
//! chunk of engine, taken because it gets us laying out sooner, and meant to
//! be replaced when we have an opinion it does not serve. That is only true if
//! it stays behind one file, so it does, and `scripts/gate.sh` checks it on
//! every run. Everything on the other side of this file is in our own types.
//!
//! # What this file does
//!
//! Translates a box tree and its styles into `taffy`'s tree, runs it, and
//! reads the results back into [`crate::LayoutTree`]. Nothing else. There is
//! no CSS understanding here — that is [`crate::style`] — and no geometry of
//! our own — that is [`crate::geometry`].
//!
//! # Inline formatting is ours
//!
//! `taffy` has block, flex and grid, and no inline layout at all — so a
//! container whose children all sit in a line is given to it as a **leaf**,
//! and [`crate::inline`] lays out what is inside. That is not a workaround:
//! inline layout is a different algorithm from the other three, and every
//! engine has its own.
//!
//! An atomic inline-level box — an `inline-block`, an image, a button — has a
//! size only its own layout can give, so this file lays out that subtree on
//! its own and hands the size to the line box. It is the same code, called
//! again, one formatting context down.
//!
//! # The tree is ours, the algorithms are not
//!
//! This file builds a [`crate::arena::Arena`] — our own nodes, our own cache,
//! our own answers — and hands it to `taffy`'s flexbox, grid and block
//! algorithms. ADR 0004 says why: `taffy` asks the *tree* to resolve a
//! `calc()` against a basis only the running algorithm knows, and its
//! ready-made tree answers zero. So `width: calc(100% - 2rem)` used to be
//! refused; now it is a number.

use crate::arena::{Arena, NodeKind, Unresolved};
use crate::auto_repeat::Axis;
use crate::baseline;
use crate::containing::{self, Containing};
use crate::ellipsis::{self, Ellipses};
use crate::end_padding;
use crate::field_width;
use crate::geometry::{Edges, Point, Rect, Size};
use crate::inline::{self, Fragment, InlineItem, InlineLayout};
use crate::keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
use crate::layout_unit;
use crate::legend;
use crate::marker;
use crate::measure::{MeasureText, TextStyle};
use crate::out_of_flow::{self, Axes};
use crate::placement::{GridLine, GridPlacement};
use crate::scroll_container;
use crate::sizing::{AutoLength, MaxSizing, Sizing};
use crate::style::{self, AxisValues, LayoutStyle};
use crate::text_style::text_style_of;
use crate::track::{RepeatCount, Track, TrackList, TrackListEntry, TrackSize};
use crate::tree::{BoxGeometry, LayoutTree};
use crate::vertical_align::LineAlign;
use crate::viewport;
use alo_box::{BoxId, BoxKind, BoxTree, Inside, Outside};
use alo_css::{IssueKind, Location, StyleIssue};
use alo_style::StyleTree;
use alo_value::{FontMetrics, LengthPercentage};
use std::collections::BTreeMap;
use taffy::prelude::*;
use taffy::util::MaybeResolve;
use taffy::{
    AvailableSpace, Dimension, LengthPercentage as TaffyLengthPercentage, LengthPercentageAuto,
    MaxTrackSizingFunction, MinTrackSizingFunction, Style, TrackSizingFunction,
};

/// Lay out a box tree for a viewport of this size.
///
/// `measure` is how the caller says how big a piece of text is; see
/// [`crate::measure`], which explains why there is no default.
pub fn compute(
    boxes: &BoxTree,
    styles: &StyleTree,
    viewport: Size,
    measure: &impl MeasureText,
) -> LayoutTree {
    let mut issues = Vec::new();
    let Some(root) = boxes.root() else {
        return LayoutTree::from_parts(BTreeMap::new(), BTreeMap::new(), issues, viewport);
    };
    let available = TaffySize {
        width: AvailableSpace::Definite(viewport.width),
        height: AvailableSpace::Definite(viewport.height),
    };
    let laid_out = lay_out_subtree(
        boxes,
        styles,
        Subtree {
            root,
            document: true,
        },
        available,
        measure,
        &mut issues,
    );
    let Some(laid_out) = laid_out else {
        return LayoutTree::from_parts(BTreeMap::new(), BTreeMap::new(), issues, viewport);
    };
    LayoutTree::from_parts(laid_out.geometry, laid_out.fragments, issues, viewport)
        .with_spaces(laid_out.spaces)
        .with_ellipses(laid_out.ellipses)
        .with_scale(styles.scale())
        .with_scroll_containers(scroll_container::scroll_containers(boxes, styles))
        .with_viewport_overflow(
            Some(root),
            boxes
                .ids()
                .filter(|&id| viewport::takes_the_overflow_of(boxes, styles, id))
                .collect(),
        )
}

/// One formatting context, laid out on its own.
struct LaidOut {
    size: Size,
    /// How far below the top of its border box the baseline it stands on in
    /// a line sits, or `None` when it has no line to stand on; see
    /// [`crate::baseline`].
    baseline: Option<f32>,
    geometry: BTreeMap<BoxId, BoxGeometry>,
    fragments: BTreeMap<BoxId, Vec<Fragment>>,
    /// Every space with no letters a line goes on past, by its box; see
    /// [`crate::inline::LineBox::spaces`].
    spaces: BTreeMap<BoxId, Vec<Fragment>>,
    /// What `text-overflow` hid; see [`crate::ellipsis`].
    ellipses: Ellipses,
}

/// Where everything in one formatting context ended up, filled in as it is
/// placed.
#[derive(Default)]
pub(crate) struct Placed {
    /// Every box's rectangle.
    pub(crate) geometry: BTreeMap<BoxId, BoxGeometry>,
    /// Every piece of every box laid out in a line.
    pub(crate) fragments: BTreeMap<BoxId, Vec<Fragment>>,
    /// Every space with no letters a line goes on past, by its box. Not a
    /// piece: it draws nothing and is not where its box is.
    pub(crate) spaces: BTreeMap<BoxId, Vec<Fragment>>,
    /// Where the baselines of each inline formatting context's first and
    /// last lines stand; see [`crate::baseline`].
    pub(crate) lines: BTreeMap<BoxId, baseline::Lines>,
    /// The boxes out of flow written in a line, and which of their axes
    /// that line places.
    pub(crate) in_line: BTreeMap<BoxId, Axes>,
    /// Where lines end in "…", and what that hid; see [`crate::ellipsis`].
    pub(crate) ellipses: Ellipses,
}

/// What is being laid out on its own.
#[derive(Clone, Copy)]
struct Subtree {
    /// The box at its top.
    root: BoxId,
    /// Whether that is the document's root box, above which is the initial
    /// containing block, rather than an atomic inline box, above which is the
    /// rest of the page.
    document: bool,
}

/// The engine's tree as it is built: which node each box became, and the
/// absolutely positioned boxes handed to a node other than their parent's.
#[derive(Default)]
struct Built {
    ours_to_theirs: BTreeMap<BoxId, usize>,
    displaced: BTreeMap<BoxId, Displaced>,
}

/// An absolutely positioned box `taffy` lays out under its containing block
/// rather than under its parent, because `taffy` places an absolute child
/// against the box it is a child of and nothing else. See
/// [`crate::containing`].
struct Displaced {
    /// Its node in the engine's tree.
    node: usize,
    /// The box it is placed against, or [`None`] for the initial containing
    /// block.
    against: Option<BoxId>,
    /// What stands in for it where it was, when an axis needs that.
    probe: Option<Probe>,
    /// Which of its axes take their place from the line it was written in,
    /// when it was written in one. Its parent then holds lines and is no
    /// node `taffy` lays children out in, so no probe can stand there: the
    /// line's [`inline::Anchor`] says where it would have stood instead.
    in_line: Option<Axes>,
}

/// An empty absolutely positioned node left in a displaced box's parent.
///
/// An axis whose two insets are both `auto` puts the box at its **static
/// position** — where it would have been in its parent's flow (CSS 2
/// § 10.3.7) — and only its parent's layout knows where that is. An empty
/// box with no insets is placed exactly there, so the box takes that axis
/// from it.
#[derive(Clone, Copy)]
struct Probe {
    node: usize,
    /// Whether the box takes its horizontal position from here.
    across: bool,
    /// Whether the box takes its vertical position from here.
    down: bool,
}

/// Lay out a subtree as its own formatting context, with its own engine tree.
///
/// Called once for the document, and again for each atomic inline-level box —
/// an `inline-block`, an image, a button — because such a box's size is
/// whatever its own layout says, and its own layout is this.
fn lay_out_subtree(
    boxes: &BoxTree,
    styles: &StyleTree,
    subtree: Subtree,
    available: TaffySize<AvailableSpace>,
    measure: &impl MeasureText,
    issues: &mut Vec<StyleIssue>,
) -> Option<LaidOut> {
    let root = subtree.root;
    let mut built = Built::default();
    let mut arena = Arena::new(boxes, styles, measure);
    let root_node = build(boxes, styles, root, &mut arena, &mut built, subtree, issues)?;
    let initial = hand_over(&mut arena, &built, available);
    // Never to whole pixels: `taffy`'s rounding is a pass over a trait this
    // engine's tree does not implement, so it cannot happen. A box rounded
    // down to 96 while its text measures 96.16 wraps a word early, which is
    // how "Remember me" once became two lines in a box wide enough for one.
    // Boxes are placed on Chrome's grid of 1/64 px instead, as they are read
    // back (ADR 0054).
    arena.compute(root_node, available);
    if let Some(initial) = initial {
        arena.compute(initial, available);
    }
    record_refused_templates(&arena, &built, boxes, styles, issues);

    let mut placed = Placed::default();
    let mut unplaced = BTreeMap::new();
    let mut reading = Reading {
        arena: &arena,
        built: &built,
        boxes,
        out: &mut placed.geometry,
        unplaced: &mut unplaced,
    };
    reading.read_back(root, Origins::ZERO);
    // A box out of flow in a line is under a box no walk of the engine's
    // tree reaches, so it is read back on its own, against what it is placed
    // against; its line moves it to its static position once lines exist.
    for (id, displaced) in &built.displaced {
        let Some(axes) = displaced.in_line else {
            continue;
        };
        placed.in_line.insert(*id, axes);
        if !placed.geometry.contains_key(id) {
            let mut reading = Reading {
                arena: &arena,
                built: &built,
                boxes,
                out: &mut placed.geometry,
                unplaced: &mut unplaced,
            };
            reading.read_back(*id, Origins::ZERO);
        }
    }
    // Before a scroll container's reach is read into the document's: Chrome
    // pads its own content, not what spills out of what it holds (queue
    // items 674 and 675).
    end_padding::text_areas(boxes, &mut placed.geometry, |id| {
        holds_a_text_areas_lines(boxes, id)
    });
    let containers = scroll_containers_held(boxes, &arena, &built, &placed.geometry);
    end_padding::scroll_containers(&mut placed.geometry, &containers);
    reach_of_the_initial_block(&built, root, &mut placed.geometry);
    // Before anything is placed inside a box, and after every box has a
    // rectangle: a fieldset's legend sits in its block-start border rather
    // than under it, and the lines of text in it have to be laid out against
    // where it ends up. See `crate::legend`.
    legend::raise(boxes, styles, &mut placed.geometry, issues);
    place_inline_content(boxes, styles, root, measure, &mut placed, issues);
    // Once every line is placed: a list item's marker stands beside its
    // first line and took no room in it (ADR 0044 § 4).
    marker::place(boxes, styles, root, measure, &mut placed, issues);

    let root_geometry = placed.geometry.get(&root).copied().unwrap_or_default();
    Some(LaidOut {
        size: root_geometry.border_box.size,
        baseline: baseline::of_atomic(boxes, styles, root, &placed.lines, &placed.geometry),
        geometry: placed.geometry,
        fragments: placed.fragments,
        spaces: placed.spaces,
        ellipses: placed.ellipses,
    })
}

/// Each scroll container this layout placed and the boxes it holds, for
/// [`end_padding::scroll_containers`]: those `taffy` laid out in it, and
/// those positioned against it from further down.
///
/// A scroll container here is what `taffy` pads — a node whose `overflow`
/// scrolls on either axis. The document's root is left out, its scrolling
/// area being the viewport's ([`crate::viewport`]), and so is a control,
/// whose inner box this engine builds and whose padding was measured on its
/// own: a textarea's by [`end_padding::text_areas`], and a button's and a
/// field's not at all.
fn scroll_containers_held<M: MeasureText>(
    boxes: &BoxTree,
    arena: &Arena<'_, M>,
    built: &Built,
    geometry: &BTreeMap<BoxId, BoxGeometry>,
) -> Vec<end_padding::Container> {
    let style_of = |id: BoxId| {
        built
            .ours_to_theirs
            .get(&id)
            .and_then(|node| arena.style_of(*node))
    };
    let held = |id: BoxId, in_flow: bool| {
        let overflow = style_of(id).map_or(TaffyPoint::<taffy::Overflow>::default(), |style| {
            style.overflow
        });
        let scrolls = overflow.x.is_scroll_container() || overflow.y.is_scroll_container();
        end_padding::Held {
            id,
            in_flow,
            moved: built
                .ours_to_theirs
                .get(&id)
                .map_or((0.0, 0.0), |node| moved_by(arena, *node)),
            spills: (
                !scrolls && overflow.x == taffy::Overflow::Visible,
                !scrolls && overflow.y == taffy::Overflow::Visible,
            ),
        }
    };
    // Grouped once: a page can hold a great many boxes of either kind.
    let mut positioned: BTreeMap<BoxId, Vec<BoxId>> = BTreeMap::new();
    for (child, displaced) in &built.displaced {
        if let Some(against) = displaced.against
            && geometry.contains_key(child)
        {
            positioned.entry(against).or_default().push(*child);
        }
    }
    geometry
        .keys()
        .copied()
        .filter(|id| Some(*id) != boxes.root())
        .filter(|id| {
            style_of(*id).is_some_and(|style| {
                style.overflow.x.is_scroll_container() || style.overflow.y.is_scroll_container()
            })
        })
        .filter(|id| {
            !boxes
                .children(*id)
                .any(|child| is_a_controls_inner_box(boxes, child))
        })
        .map(|id| {
            let mut inside: Vec<end_padding::Held> = boxes
                .children(id)
                .filter(|child| {
                    geometry.contains_key(child) && !built.displaced.contains_key(child)
                })
                .map(|child| {
                    let in_flow =
                        style_of(child).is_some_and(|style| style.position != Position::Absolute);
                    held(child, in_flow)
                })
                .collect();
            inside.extend(
                positioned
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .map(|child| held(*child, false)),
            );
            end_padding::Container { id, held: inside }
        })
        .collect()
}

/// How far `position: relative` moved a box across and down from where it
/// stood, as `taffy` moves it: `left` over `right`, `top` over `bottom`.
///
/// A percentage, or a calculation holding one, is resolved against the
/// basis `taffy` placed the box with ([`Arena::basis_of`]) — a block's
/// content box, its height only where definite; a flex container's content
/// box where definite; a grid item's area — and a side whose percentage has
/// no basis is `auto`, so the other side moves the box, as in Chrome (queue
/// item 695).
fn moved_by<M: MeasureText>(arena: &Arena<'_, M>, node: usize) -> (f32, f32) {
    let Some(style) = arena.style_of(node) else {
        return (0.0, 0.0);
    };
    if style.position != Position::Relative {
        return (0.0, 0.0);
    }
    let basis = arena.basis_of(node);
    let calc = |handle: *const (), basis: f32| arena.calc(handle, basis);
    let side = |value: LengthPercentageAuto, basis: Option<f32>| value.maybe_resolve(basis, calc);
    let axis = |start: Option<f32>, end: Option<f32>| start.or(end.map(|end| -end)).unwrap_or(0.0);
    (
        axis(
            side(style.inset.left, basis.width),
            side(style.inset.right, basis.width),
        ),
        axis(
            side(style.inset.top, basis.height),
            side(style.inset.bottom, basis.height),
        ),
    )
}

/// Whether a box is the inner box this engine builds inside a control.
fn is_a_controls_inner_box(boxes: &BoxTree, id: BoxId) -> bool {
    matches!(
        boxes.get(id).map(|node| &node.kind),
        Some(BoxKind::Anonymous {
            purpose: alo_box::Purpose::Control { .. },
            ..
        })
    )
}

/// Say which grids `taffy` laid out, in some call, without the template
/// their author wrote.
///
/// A calculated track in an `auto-fill` or `auto-fit` repetition is counted
/// only where the grid's size in that axis is definite (ADR 0047); elsewhere
/// `taffy` discards the template, the grid has no explicit tracks, and its
/// items fall into implicit ones. A grid sized by its content is measured
/// that way, and then laid out in the size the measurement gave — handed to
/// it, so definite by `taffy`'s rule, and lowered. Neither is what Chrome
/// does, which sizes such a track against a basis it finds while sizing the
/// grid (queue item 424), and a page that looks wrong for a reason nobody
/// can see is the worst kind of wrong, so each grid and axis is one issue,
/// naming the template.
fn record_refused_templates<M: MeasureText>(
    arena: &Arena<'_, M>,
    built: &Built,
    boxes: &BoxTree,
    styles: &StyleTree,
    issues: &mut Vec<StyleIssue>,
) {
    let mut refused = arena.refused().peekable();
    if refused.peek().is_none() {
        return;
    }
    let theirs_to_ours: BTreeMap<usize, BoxId> = built
        .ours_to_theirs
        .iter()
        .map(|(ours, theirs)| (*theirs, *ours))
        .collect();
    for (node, axis) in refused {
        let Some(computed) = theirs_to_ours
            .get(&node)
            .and_then(|id| boxes.get(*id))
            .and_then(|held| held.kind.node())
            .and_then(|source| styles.get(source))
        else {
            continue;
        };
        // Read again only for its text: what reading it had to say was said
        // when the box was built.
        let grid = style::read(computed, &mut Vec::new()).grid;
        let (property, list, size) = match axis {
            Axis::Columns => ("grid-template-columns", grid.template_columns, "width"),
            Axis::Rows => ("grid-template-rows", grid.template_rows, "height"),
        };
        issues.push(StyleIssue {
            kind: IssueKind::UnsupportedValue,
            source: format!(
                "{property}: {list} — a calculated track in an auto repetition is counted \
                 only against a definite {size}: this grid was measured without one, and \
                 without its template"
            ),
            at: Location { line: 0, column: 0 },
        });
    }
}

/// Work out what is on each line of an inline formatting context, to size
/// the box that holds them.
///
/// A field's `::placeholder` takes no part: a browser sizes a field by its
/// own width or its `size`, never by its hint, and a field that shrank when
/// somebody started typing would be wrong in a way they would see (ADR 0043
/// § 3). It is laid out in its line when the field's content is placed.
pub(crate) fn measure_inline(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    available_width: Option<f32>,
    narrowest: bool,
    measure: &impl MeasureText,
) -> InlineLayout {
    let mut issues = Vec::new();
    let mut items = collect_inline_items(boxes, styles, id, available_width, measure, &mut issues);
    items.retain(|item| !is_a_placeholder(boxes, item));
    let (alignment, strut) = (
        alignment_of(boxes, styles, id),
        text_style_for(boxes, styles, id),
    );
    if holds_a_text_areas_lines(boxes, id) {
        inline::lay_out_a_text_area(&items, available_width, narrowest, &strut, measure)
    } else if narrowest {
        inline::lay_out_narrowest(&items, alignment, &strut, measure)
    } else {
        inline::lay_out_aligned(&items, available_width, alignment, &strut, measure)
    }
}

/// Whether an item is the run of a field's `::placeholder`.
fn is_a_placeholder(boxes: &BoxTree, item: &InlineItem) -> bool {
    matches!(item, InlineItem::Text { box_id, .. } if boxes.is_placeholder(*box_id))
}

/// Where the lines of a formatting context sit in it.
///
/// `text-align` inherits, and a box's own value is already the inherited one by
/// the time it is asked — so this asks the box that holds the lines, which is
/// the box whose width they are aligned in.
///
/// A box nobody wrote — the anonymous block a container wraps a run of inline
/// boxes in — has no style of its own and **inherits** from the box it is in,
/// so it asks the nearest element above it. Reading only its own style once
/// left every such line at the start: alo's offline screen is a `text-align:
/// center` page whose picture and button each sit in one, and both were drawn
/// against the left edge.
fn alignment_of(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> inline::TextAlignment {
    // The box a form control holds what it shows in is anonymous too, but it
    // does not inherit: a button's label sits in the middle of it and a
    // field's at the start whatever the page around them says, so the box
    // tree's own word for why it exists is the answer.
    if let Some(BoxKind::Anonymous {
        purpose: alo_box::Purpose::Control { centred, .. },
        ..
    }) = boxes.get(id).map(|node| &node.kind)
    {
        return if *centred {
            inline::TextAlignment::Center
        } else {
            inline::TextAlignment::Start
        };
    }
    boxes
        .nearest_style(styles, id)
        .and_then(|style| style.get("text-align"))
        .and_then(inline::TextAlignment::parse)
        .unwrap_or_default()
}

/// The things on the lines of an inline formatting context, in order.
///
/// A nested inline box contributes its own children rather than itself, so
/// that a sentence spread over several `<span>`s is one sentence and breaks
/// between any two of its words. An atomic inline-level box contributes
/// itself, at whatever size its own layout gives it.
fn collect_inline_items(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    available_width: Option<f32>,
    measure: &impl MeasureText,
    issues: &mut Vec<StyleIssue>,
) -> Vec<InlineItem> {
    let mut items = Vec::new();
    // A list item's marker joins none of its lines (ADR 0044 § 4).
    let children: Vec<BoxId> = boxes
        .children(id)
        .filter(|child| !boxes.is_marker(*child))
        .collect();
    for child in children {
        let Some(node) = boxes.get(child) else {
            continue;
        };
        match &node.kind {
            BoxKind::Text { text, .. } => items.push(InlineItem::Text {
                box_id: child,
                text: text.clone(),
                style: text_style_for(boxes, styles, child),
            }),
            // Out of flow: no room here, only a place it would have stood.
            _ if boxes.is_out_of_flow(child) => {
                items.push(InlineItem::OutOfFlow { box_id: child });
            }
            // A `<br>`: the line ends here. It has no children to bracket.
            _ if boxes.is_forced_break(child) => items.push(InlineItem::Break {
                box_id: child,
                style: text_style_for(boxes, styles, child),
            }),
            _ if holds_only_what_sits_in_a_line(boxes, child) => {
                if is_atomic(boxes, child) {
                    items.push(atomic_item(
                        boxes,
                        styles,
                        child,
                        available_width,
                        measure,
                        issues,
                    ));
                } else {
                    // A nested inline box: its children join this line, with
                    // its own edges bracketing them. The brackets are what
                    // give it a box of its own — a background, a border and a
                    // padding — rather than only a place where its text sits.
                    let (border, padding) = inline_edges(boxes, styles, child, issues);
                    let edges = added(border, padding);
                    items.push(InlineItem::Open {
                        box_id: child,
                        edge: edges.left,
                        style: text_style_for(boxes, styles, child),
                        over: edges.top,
                        under: edges.bottom,
                        align: line_align(boxes, styles, child),
                    });
                    items.extend(collect_inline_items(
                        boxes,
                        styles,
                        child,
                        available_width,
                        measure,
                        issues,
                    ));
                    items.push(InlineItem::Close {
                        box_id: child,
                        edge: edges.right,
                    });
                }
            }
            _ => items.push(atomic_item(
                boxes,
                styles,
                child,
                available_width,
                measure,
                issues,
            )),
        }
    }
    items
}

/// The style of the box a form control holds what it shows in.
///
/// It fills the control and, unless the control is a `<button>` element, is
/// **one line tall at least**, which is why an empty field is still a field
/// rather than a hairline. A `<button>` is what it holds and no more: an
/// `<svg>` arrow 15 pixels tall makes a button 15 tall inside, as browsers
/// draw alo's Agenda (queue item 437). A button's label sits in the middle
/// of it; a field's sits at the start.
///
/// A date or a time is held in segments with [`SEGMENT_EDGE`] above and below
/// its line, which is padding on this box and so makes the field that much
/// taller than a text field with the same line (queue item 507).
///
/// None of this is in the user-agent style sheet, and it cannot be: a rule
/// that centred a button's label would also centre the children of a button an
/// author had made a flex container, and an author cannot override a rule they
/// cannot see.
fn control_content_style(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    centred: bool,
    at_least_a_line: bool,
    in_segments: bool,
    measure: &impl MeasureText,
) -> Style {
    let edge = if in_segments { SEGMENT_EDGE } else { 0.0 };
    let line = if at_least_a_line {
        control_line(boxes, styles, id, measure)
    } else {
        0.0
    };
    Style {
        display: taffy::Display::Flex,
        flex_grow: 1.0,
        // It **fills** the control, which is what puts a tall button's label
        // in the middle rather than at the top. Against a control whose own
        // height is `auto` this resolves to `auto` too, and the minimum below
        // is then what makes an empty field one line tall.
        size: TaffySize {
            width: Dimension::percent(1.0),
            height: Dimension::percent(1.0),
        },
        // The box is sized by its border box, so the minimum is the line and
        // the segments' edges, and the line is laid out inside them.
        min_size: TaffySize {
            width: LengthPercentageAuto::auto(),
            height: LengthPercentageAuto::length(line + edge * 2.0),
        },
        padding: taffy::Rect {
            left: TaffyLengthPercentage::length(0.0),
            right: TaffyLengthPercentage::length(0.0),
            top: TaffyLengthPercentage::length(edge),
            bottom: TaffyLengthPercentage::length(edge),
        },
        align_items: Some(taffy::AlignItems::CENTER),
        justify_content: centred.then_some(taffy::JustifyContent::CENTER),
        ..Style::default()
    }
}

/// What a date or a time is held in has this much above and below its line:
/// Chrome 154's `::-webkit-datetime-edit-fields-wrapper` padding, `1px 0`.
///
/// Measured over `date`, `month`, `week`, `time` and `datetime-local` at 10,
/// 13.33, 16, 17, 24 and 40 px, at `normal`, `1.6` and `30px`, in
/// `DejaVu Sans` and `DejaVu Sans Mono`, empty and filled: the field is 2
/// px taller than a text field whenever its line is taller than the
/// calendar's indicator, and exactly as tall once a page takes that padding
/// off (queue item 507).
const SEGMENT_EDGE: f32 = 1.0;

/// The one line the box a control holds what it shows in is at least as
/// tall as: a line holding only the control's strut, which is the strut its
/// own lines are laid out with.
///
/// That is the line height the control set, on the grid of 1/64 px, or with
/// `normal` its font's ascent, descent and line gap, each a whole pixel
/// (ADR 0050). It was the computed `line-height`, which answers `normal`
/// with 1.2 em: a 13 px field was 15.59 tall inside where Chrome 154's is
/// 15, and a 14 px button 16.8 where Chrome's is 16 (queue item 467).
fn control_line(boxes: &BoxTree, styles: &StyleTree, id: BoxId, measure: &impl MeasureText) -> f32 {
    inline::empty_line_height(&text_style_for(boxes, styles, id), measure)
}

/// How far down a one-line field's line starts in its content box: half of
/// what the box has beyond the one line it is at least as tall as.
///
/// A field taller than its line holds its text in the middle of it, as
/// browsers draw one: alo's sign-in fields are 46 tall with no padding, and
/// their text at the top was 15 above where a browser draws it, found when
/// the email field's hint was drawn (queue item 389). It is the field's
/// *line* that is centred, not its text, so a field exactly one line tall —
/// nearly every field, whose height comes from its padding — is not moved,
/// and its text stands where it did, empty or filled.
fn field_line_offset(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    content: Rect,
    measure: &impl MeasureText,
) -> f32 {
    ((content.size.height - control_line(boxes, styles, id, measure)) / 2.0).max(0.0)
}

/// An inline box's own border and padding, on each side.
///
/// The horizontal ones take room on the line; the vertical ones draw without
/// changing the line's height. Both come from the same place as a block box's,
/// so a `border` on a `<span>` and a `border` on a `<div>` mean the same
/// number.
fn inline_edges(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    issues: &mut Vec<StyleIssue>,
) -> (Edges, Edges) {
    let Some(ours) = boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        .map(|computed| style::read(computed, issues))
    else {
        return (Edges::ZERO, Edges::ZERO);
    };
    let metrics = ours.metrics;
    // A percentage padding on an inline box is of the containing block's
    // *width*, which this file does not have here — and a wrong number is a
    // wrong pixel, so it is zero and says so.
    let mut side = |value: &LengthPercentage, name: &str| {
        if value.is_percentage() {
            issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source: format!("{value} on the {name} of an inline box"),
                at: Location { line: 0, column: 0 },
            });
            return 0.0;
        }
        pixels(value, metrics)
    };
    (
        Edges {
            top: side(&ours.border.top, "top border"),
            right: side(&ours.border.right, "right border"),
            bottom: side(&ours.border.bottom, "bottom border"),
            left: side(&ours.border.left, "left border"),
        },
        Edges {
            top: side(&ours.padding.top, "top padding"),
            right: side(&ours.padding.right, "right padding"),
            bottom: side(&ours.padding.bottom, "bottom padding"),
            left: side(&ours.padding.left, "left padding"),
        },
    )
}

/// The two sets of edges added together — what actually takes room.
fn added(left: Edges, right: Edges) -> Edges {
    Edges {
        top: left.top + right.top,
        right: left.right + right.right,
        bottom: left.bottom + right.bottom,
        left: left.left + right.left,
    }
}

/// The font a box's text is set in, from the nearest element that has a style.
///
/// A text box came from a text node, which has no style of its own — text
/// inherits everything from the element that holds it, so that is the element
/// to ask. Passing this to the measurer per box rather than once per document
/// is what makes a heading and a caption on the same page different sizes.
///
/// A `<textarea>`'s text, and the inside that holds its lines, keep every
/// space and newline whatever its `white-space` says, as the box tree kept
/// them (`alo_box::WhiteSpace::in_a_text_area`); its `::placeholder` is a
/// hint with a style of its own, and is left to it.
fn text_style_for(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> TextStyle {
    let mut style = boxes
        .nearest_style(styles, id)
        .map_or_else(TextStyle::default, text_style_of);
    let in_a_text_area = holds_a_text_areas_lines(boxes, id)
        || boxes
            .get(id)
            .and_then(|node| node.parent)
            .is_some_and(|parent| holds_a_text_areas_lines(boxes, parent));
    if in_a_text_area && !boxes.is_placeholder(id) {
        style.white_space = style.white_space.in_a_text_area();
    }
    style
}

/// Whether an inline-level box is laid out on its own rather than joining the
/// line around it.
///
/// `inline flow` is a box whose content joins the line — a `<span>`, an `<a>`.
/// Anything else inline-level establishes its own formatting context and is
/// placed on the line whole: an `inline-block`, an `inline-flex`, an image.
///
/// A **replaced** box is atomic whatever its `display` says: `img { display:
/// inline }` is still a picture, and an `<svg>` an author made inline has a
/// size of its own rather than a line of children to join.
fn is_atomic(boxes: &BoxTree, id: BoxId) -> bool {
    boxes.natural_size(id).is_some()
        || boxes
            .get(id)
            .is_some_and(|node| !matches!(node.kind.inside(), Inside::Flow))
}

/// Lay out an atomic inline-level box on its own and turn it into an item.
fn atomic_item(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    available_width: Option<f32>,
    measure: &impl MeasureText,
    issues: &mut Vec<StyleIssue>,
) -> InlineItem {
    let available = TaffySize {
        width: available_width.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite),
        height: AvailableSpace::MaxContent,
    };
    let laid_out = lay_out_subtree(
        boxes,
        styles,
        Subtree {
            root: id,
            document: false,
        },
        available,
        measure,
        issues,
    );
    let margin = laid_out
        .as_ref()
        .and_then(|held| held.geometry.get(&id))
        .map_or(Edges::ZERO, |held| held.margin);
    let size = laid_out.as_ref().map_or(Size::ZERO, |held| held.size);
    // The baseline is measured from the top of the margin box, which is what
    // sits on the line: its last line's, or with no line its bottom margin
    // edge — where an empty inline-block, a picture and a scroll container
    // stand.
    let baseline = laid_out
        .as_ref()
        .and_then(|held| held.baseline)
        .map_or(margin.top + size.height + margin.bottom, |line| {
            margin.top + line
        });
    InlineItem::Atomic {
        box_id: id,
        size,
        margin,
        baseline,
        align: line_align(boxes, styles, id),
    }
}

/// A box's `vertical-align`, its lengths made numbers against its own font.
///
/// A box nobody wrote has no style and stands on the baseline: the property
/// is not inherited, so there is nothing for it to take from above.
fn line_align(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> LineAlign {
    boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        .map_or(LineAlign::Baseline, |computed| {
            // A value this engine cannot read was recorded when the box's own
            // style was read for its edges or its layout; reading it again
            // here is not a second finding.
            let ours = style::read(computed, &mut Vec::new());
            ours.vertical_align.resolve(ours.metrics)
        })
}

/// Once the engine has placed every block, put the inline content inside the
/// boxes that hold it.
fn place_inline_content(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    measure: &impl MeasureText,
    out: &mut Placed,
    issues: &mut Vec<StyleIssue>,
) {
    if boxes.children(id).next().is_none() {
        // Nothing under it: only an empty field has a line to record.
        record_an_empty_field(boxes, styles, id, measure, out);
        return;
    }
    if is_inline_formatting_context(boxes, id) {
        let Some(container) = out.geometry.get(&id).copied() else {
            return;
        };
        let content = container.content_box();
        let items =
            collect_inline_items(boxes, styles, id, Some(content.size.width), measure, issues);
        // A field's hint is one line, however narrow the field: it was given
        // no room when the field was sized, so wrapping it to the field would
        // give the field lines it is not tall enough for. Where it is longer
        // than its field it runs past it, until it is clipped (item 390).
        let room =
            (!items.iter().any(|item| is_a_placeholder(boxes, item))).then_some(content.size.width);
        // The strut is the font of the box that holds the lines, and it is
        // the same one `measure_inline` sized this box with — two different
        // struts would give the box one height and its lines another.
        let strut = text_style_for(boxes, styles, id);
        // A `<textarea>`'s text ending in a newline holds a last, empty line
        // after it, which it stands on and scrolls to (queue item 670).
        let layout = if holds_a_text_areas_lines(boxes, id) {
            inline::lay_out_a_text_area(&items, room, false, &strut, measure)
        } else {
            inline::lay_out_aligned(
                &items,
                room,
                alignment_of(boxes, styles, id),
                &strut,
                measure,
            )
        };

        let down = lines_start_down(boxes, styles, id, content, layout.size.height, measure);
        let origin = Point::new(content.origin.x, content.origin.y + down);
        record_lines(boxes, styles, id, &layout, origin.y, measure, out);
        let lines = ellipsis::Lines {
            items: &items,
            layout: &layout,
            origin,
            strut: &strut,
        };
        // Before the lines are ended: an ellipsis hides the spaces past it.
        for space in layout.spaces() {
            out.spaces.entry(space.box_id).or_default().push(Fragment {
                rect: space.rect.translated(origin),
                ..space.clone()
            });
        }
        ellipsis::place(boxes, styles, id, &lines, measure, out);
        for fragment in layout.fragments() {
            let placed = Fragment {
                rect: fragment.rect.translated(origin),
                ..fragment.clone()
            };
            out.fragments
                .entry(placed.box_id)
                .or_default()
                .push(placed.clone());
        }
        // Every box inside this context gets a rectangle as well. A nested
        // inline box has pieces of its own now — one per line, its own edges
        // included — so its rectangle is the union of those, and it carries
        // the border and padding that paint needs to draw them. A box with no
        // pieces at all falls back to the union of what is under it, which is
        // the answer to "where is this" for something that draws nothing.
        for inner in boxes.descendants(id) {
            let own = out
                .fragments
                .get(&inner)
                .and_then(|pieces| pieces.iter().map(|piece| piece.rect).reduce(union_rects));
            let Some(rect) = own.or_else(|| union_of_subtree(boxes, inner, &out.fragments)) else {
                continue;
            };
            let (border, padding) = inline_edges(boxes, styles, inner, issues);
            out.geometry.entry(inner).or_insert(BoxGeometry {
                border_box: rect,
                border,
                padding,
                ..BoxGeometry::default()
            });
        }
        place_atomics(boxes, styles, &items, measure, out, issues);
        for anchor in &layout.anchors {
            out_of_flow::stand_at_static_position(boxes, anchor, origin, out);
            place_inline_content(boxes, styles, anchor.box_id, measure, out, issues);
        }
        return;
    }

    let children: Vec<BoxId> = boxes.children(id).collect();
    for child in children {
        place_inline_content(boxes, styles, child, measure, out, issues);
    }
}

/// Each atomic box among `items` brought a whole layout of its own with it;
/// place it where its line put it.
fn place_atomics(
    boxes: &BoxTree,
    styles: &StyleTree,
    items: &[InlineItem],
    measure: &impl MeasureText,
    out: &mut Placed,
    issues: &mut Vec<StyleIssue>,
) {
    for item in items {
        if let InlineItem::Atomic { box_id, margin, .. } = item
            && let Some(placed) = out.fragments.get(box_id).and_then(|pieces| pieces.first())
        {
            let offset = placed.rect.origin;
            // The room it is given is its margin box: a block takes its
            // margins out of the room it is given, so handing it only its
            // border box would shrink it by its own margins.
            if let Some(sub) = lay_out_subtree(
                boxes,
                styles,
                Subtree {
                    root: *box_id,
                    document: false,
                },
                TaffySize {
                    width: AvailableSpace::Definite(placed.rect.size.width + margin.horizontal()),
                    height: AvailableSpace::Definite(placed.rect.size.height + margin.vertical()),
                },
                measure,
                issues,
            ) {
                for (inner, mut held) in sub.geometry {
                    held.border_box = held.border_box.translated(offset);
                    out.geometry.insert(inner, held);
                }
                for (inner, pieces) in sub.fragments {
                    out.fragments.insert(inner, moved(pieces, offset));
                }
                for (inner, spaces) in sub.spaces {
                    out.spaces.insert(inner, moved(spaces, offset));
                }
                // Relative to the boxes they end, which moved above.
                out.ellipses.extend(sub.ellipses);
            }
        }
    }
}

/// Record where the first and last lines of the inline formatting context
/// `id` stand, `down` being where its lines start, for the atomic box this
/// context may hold the baseline of; see `crate::baseline`.
fn record_lines(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    layout: &inline::InlineLayout,
    down: f32,
    measure: &impl MeasureText,
    out: &mut Placed,
) {
    if let (Some(first), Some(last)) = (layout.lines.first(), layout.lines.last()) {
        out.lines.insert(
            id,
            baseline::Lines {
                first: down + first.top + first.baseline,
                last: down + last.top + last.baseline,
            },
        );
    } else {
        // A field whose value is nothing worth a line, such as a space.
        record_an_empty_field(boxes, styles, id, measure, out);
    }
}

/// How far down its content box a formatting context's lines start, when
/// they are `height` tall together.
///
/// A control's content box centres what is in it, down as well as across —
/// which is what puts a tall button's label in the middle rather than along
/// its top edge — and a one-line field centres its line. Everything else
/// starts at the top.
fn lines_start_down(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    content: Rect,
    height: f32,
    measure: &impl MeasureText,
) -> f32 {
    if centres_its_lines(boxes, id) {
        ((content.size.height - height) / 2.0).max(0.0)
    } else if holds_one_line(boxes, id) {
        field_line_offset(boxes, styles, id, content, measure)
    } else {
        0.0
    }
}

/// Whether this box holds its lines in the middle of itself.
///
/// True of exactly one thing: the box a button holds its label in. It is
/// anonymous and has no style, so the box tree's own word for why it exists is
/// what answers. A one-line field centres its *line* instead
/// ([`field_line_offset`]), and a `<textarea>` starts its lines at its top.
fn centres_its_lines(boxes: &BoxTree, id: BoxId) -> bool {
    matches!(
        boxes.get(id).map(|node| &node.kind),
        Some(BoxKind::Anonymous {
            purpose: alo_box::Purpose::Control { centred: true, .. },
            ..
        })
    )
}

/// Record the line a field with nothing typed in it would hold: the baseline
/// of a line holding only its strut, where a line it held would start, so
/// that it stands where its text will and does not drop when it is typed
/// into. See `crate::baseline`.
///
/// A one-line field's line is centred in it ([`field_line_offset`]); a
/// `<textarea>`'s first line starts at the top of its content box. Chrome 155
/// stands an empty `<textarea>` under `overflow: clip` level with one holding
/// a line, at a `line-height` of 20 and of `normal`, at the sheet's size and
/// under padding (queue item 669, measured). Under the sheet's `auto` it is a
/// scroll container and the line is never asked for. Any other box with no
/// line records none — an empty `<button>` among them.
fn record_an_empty_field(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    measure: &impl MeasureText,
    out: &mut Placed,
) {
    let one_line = holds_one_line(boxes, id);
    if !one_line && !holds_a_text_areas_lines(boxes, id) {
        return;
    }
    let Some(content) = out.geometry.get(&id).copied().map(BoxGeometry::content_box) else {
        return;
    };
    let strut = text_style_for(boxes, styles, id);
    let down = if one_line {
        field_line_offset(boxes, styles, id, content, measure)
    } else {
        0.0
    };
    out.lines.insert(
        id,
        baseline::Lines::one(
            content.origin.y + down + inline::empty_line_baseline(&strut, measure),
        ),
    );
}

/// Whether this box is the inside of a one-line field, which stands on the
/// line it would hold even when it holds none. See `alo_box::Purpose`.
fn holds_one_line(boxes: &BoxTree, id: BoxId) -> bool {
    matches!(
        boxes.get(id).map(|node| &node.kind),
        Some(BoxKind::Anonymous {
            purpose: alo_box::Purpose::Control { one_line: true, .. },
            ..
        })
    )
}

/// Whether this box is the inside of a `<textarea>`: a field that is not one
/// line, which holds its lines from its top. See `alo_box::Purpose`.
pub(crate) fn holds_a_text_areas_lines(boxes: &BoxTree, id: BoxId) -> bool {
    matches!(
        boxes.get(id).map(|node| &node.kind),
        Some(BoxKind::Anonymous {
            purpose: alo_box::Purpose::Control {
                centred: false,
                one_line: false,
                ..
            },
            ..
        })
    )
}

/// Pieces placed in an atomic box's own layout, moved to where the box
/// stands.
fn moved(pieces: Vec<Fragment>, offset: Point) -> Vec<Fragment> {
    pieces
        .into_iter()
        .map(|piece| Fragment {
            rect: piece.rect.translated(offset),
            ..piece
        })
        .collect()
}

/// The rectangle a box and everything under it were drawn in.
fn union_of_subtree(
    boxes: &BoxTree,
    id: BoxId,
    fragments: &BTreeMap<BoxId, Vec<Fragment>>,
) -> Option<Rect> {
    let mut found: Option<Rect> = None;
    let mut extend = |rect: Rect| {
        found = Some(match found {
            None => rect,
            Some(held) => union_rects(held, rect),
        });
    };
    for piece in fragments.get(&id).into_iter().flatten() {
        extend(piece.rect);
    }
    for descendant in boxes.descendants(id) {
        for piece in fragments.get(&descendant).into_iter().flatten() {
            extend(piece.rect);
        }
    }
    found
}

fn union_rects(left: Rect, right: Rect) -> Rect {
    let x = left.left().min(right.left());
    let y = left.top().min(right.top());
    Rect::new(
        x,
        y,
        left.right().max(right.right()) - x,
        left.bottom().max(right.bottom()) - y,
    )
}

/// Build one box and everything under it.
///
/// A box whose children all sit in a line becomes a **leaf**: the engine is
/// told how big it is and nothing about what is inside, because what is inside
/// is a line box and that is [`crate::inline`]'s. Its children are therefore
/// not in the engine's tree at all, and are positioned by this file afterwards.
///
/// An absolutely positioned child whose containing block is not this box is
/// built here but not made this box's child in the engine's tree: it is
/// recorded in `built` to be handed to its containing block once that exists
/// ([`hand_over`]), and an empty [`Probe`] takes its place here when it needs
/// its static position.
fn build<M: MeasureText>(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    arena: &mut Arena<'_, M>,
    built: &mut Built,
    subtree: Subtree,
    issues: &mut Vec<StyleIssue>,
) -> Option<usize> {
    let node = boxes.get(id)?;
    let measure = arena.measure();
    let style = style_for(boxes, styles, id, measure, arena.unresolved(), issues);

    if is_inline_formatting_context(boxes, id) {
        let made = arena.push(style, NodeKind::InlineFormatting(id), Vec::new());
        built.ours_to_theirs.insert(id, made);
        for held in out_of_flow_in_line(boxes, id) {
            build_out_of_flow_in_line(boxes, styles, held, id, arena, built, subtree, issues);
        }
        return Some(made);
    }

    let mut children = Vec::with_capacity(node.children.len());
    // A list item's marker is no node of the engine's: it takes no room,
    // and `crate::marker` stands it beside the item's first line.
    for child in node
        .children
        .iter()
        .filter(|child| !boxes.is_marker(**child))
    {
        let Some(made) = build(boxes, styles, *child, arena, built, subtree, issues) else {
            continue;
        };
        let against = match containing::of(boxes, styles, *child, subtree.root, subtree.document) {
            Containing::Parent => {
                children.push(made);
                continue;
            }
            Containing::Beyond => {
                // Laid out against the atomic box it is in, which is the
                // nearest this layout can see, and said.
                issues.push(StyleIssue {
                    kind: IssueKind::UnsupportedValue,
                    source: "position: absolute inside an inline-block or other atomic inline \
                             box with nothing positioned between: placed against that box \
                             rather than its containing block outside it"
                        .to_owned(),
                    at: Location { line: 0, column: 0 },
                });
                children.push(made);
                continue;
            }
            Containing::Ancestor(ancestor) => Some(ancestor),
            Containing::Initial => None,
        };
        let probe = probe_for(arena, made);
        if let Some(probe) = probe {
            children.push(probe.node);
            if matches!(node.kind.inside(), Inside::Flex | Inside::Grid) {
                // A flex or grid container places an absolute child with no
                // insets by its alignment, which counts the child's own size;
                // the probe has none.
                issues.push(StyleIssue {
                    kind: IssueKind::UnsupportedValue,
                    source: "position: absolute with both insets auto in an axis, in a flex or \
                             grid container that is not its containing block: its static \
                             position is taken as an empty box's"
                        .to_owned(),
                    at: Location { line: 0, column: 0 },
                });
            }
        }
        built.displaced.insert(
            *child,
            Displaced {
                node: made,
                against,
                probe,
                in_line: None,
            },
        );
    }

    let kind = match (&node.kind, boxes.natural_size(id)) {
        (BoxKind::Text { text, .. }, _) => {
            NodeKind::Text(text.clone(), text_style_for(boxes, styles, id))
        }
        // Before the empty case, deliberately: a replaced box has no children
        // and would otherwise be measured as nothing — which is what an `<img>`
        // did until it had a size of its own.
        (_, Some(natural)) => NodeKind::Replaced(natural),
        _ if children.is_empty() => NodeKind::Empty,
        _ => NodeKind::Container,
    };
    let made = arena.push(style, kind, children);
    built.ours_to_theirs.insert(id, made);
    Some(made)
}

/// Build a box out of flow that was written in the lines of `lines`, and
/// hand it to its containing block.
///
/// `lines` is a leaf of the engine's tree — its lines are
/// [`crate::inline`]'s — so the box can never be its child there: it is
/// always displaced, and the axes whose insets are both `auto` take its
/// static position from its line rather than from a probe.
///
/// A containing block inside the lines — `lines` itself, positioned, or a
/// positioned inline box in them — is no node the engine places children
/// against. The box is placed against the initial containing block instead,
/// and said; the axes that take the static position are still right.
#[expect(
    clippy::too_many_arguments,
    reason = "the same tree, styles, arena and record `build` threads through"
)]
fn build_out_of_flow_in_line<M: MeasureText>(
    boxes: &BoxTree,
    styles: &StyleTree,
    held: BoxId,
    lines: BoxId,
    arena: &mut Arena<'_, M>,
    built: &mut Built,
    subtree: Subtree,
    issues: &mut Vec<StyleIssue>,
) {
    let Some(node) = build(boxes, styles, held, arena, built, subtree, issues) else {
        return;
    };
    let in_the_lines = |at: BoxId| {
        let mut chain = boxes.get(held).and_then(|node| node.parent);
        while let Some(up) = chain {
            if up == at {
                return true;
            }
            if up == lines {
                return false;
            }
            chain = boxes.get(up).and_then(|node| node.parent);
        }
        false
    };
    let against = match containing::of(boxes, styles, held, subtree.root, subtree.document) {
        Containing::Initial => None,
        Containing::Ancestor(at) if !in_the_lines(at) => Some(at),
        Containing::Beyond if subtree.root != lines => {
            issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source: "position: absolute inside an inline-block or other atomic inline \
                         box with nothing positioned between: placed against that box \
                         rather than its containing block outside it"
                    .to_owned(),
                at: Location { line: 0, column: 0 },
            });
            Some(subtree.root)
        }
        Containing::Parent | Containing::Ancestor(_) | Containing::Beyond => {
            issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source: "position: absolute in a line whose containing block is the block \
                         holding the line or an inline box in it: placed against the \
                         initial containing block, its static position kept"
                    .to_owned(),
                at: Location { line: 0, column: 0 },
            });
            None
        }
    };
    let in_line = arena.style_of(node).map(|style| Axes {
        across: style.inset.left.is_auto() && style.inset.right.is_auto(),
        down: style.inset.top.is_auto() && style.inset.bottom.is_auto(),
    });
    built.displaced.insert(
        held,
        Displaced {
            node,
            against,
            probe: None,
            in_line,
        },
    );
}

/// An empty stand-in for a displaced box, when one of its axes has both its
/// insets `auto` and so takes its static position; [`None`] when neither
/// does.
fn probe_for<M: MeasureText>(arena: &mut Arena<'_, M>, displaced: usize) -> Option<Probe> {
    let inset = arena.style_of(displaced)?.inset;
    let across = inset.left.is_auto() && inset.right.is_auto();
    let down = inset.top.is_auto() && inset.bottom.is_auto();
    if !across && !down {
        return None;
    }
    let style = Style {
        display: taffy::Display::Block,
        position: Position::Absolute,
        ..Style::default()
    };
    let node = arena.push(style, NodeKind::Empty, Vec::new());
    Some(Probe { node, across, down })
}

/// Hand every displaced box to its containing block's node, once the whole
/// tree exists; and make the node standing for the initial containing block,
/// when a box is placed against it, returning it to be laid out.
///
/// The initial containing block is the viewport's size at the top left of
/// the page (CSS 2 § 10.1). It is a node of its own, laid out on its own
/// beside the document's root rather than above it: the root element's
/// margins do not collapse (CSS 2 § 8.3.1), and a parent above it would
/// begin collapsing them.
fn hand_over<M: MeasureText>(
    arena: &mut Arena<'_, M>,
    built: &Built,
    available: TaffySize<AvailableSpace>,
) -> Option<usize> {
    let mut initial = Vec::new();
    for displaced in built.displaced.values() {
        match displaced
            .against
            .and_then(|against| built.ours_to_theirs.get(&against))
        {
            Some(theirs) => arena.adopt(*theirs, displaced.node),
            None => initial.push(displaced.node),
        }
    }
    if initial.is_empty() {
        return None;
    }
    let side = |space: AvailableSpace| match space {
        AvailableSpace::Definite(definite) => Dimension::length(definite),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => Dimension::auto(),
    };
    let style = Style {
        display: taffy::Display::Block,
        position: Position::Relative,
        size: TaffySize {
            width: side(available.width),
            height: side(available.height),
        },
        ..Style::default()
    };
    Some(arena.push(style, NodeKind::Container, initial))
}

/// The `taffy` style for one box.
fn style_for(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    measure: &impl MeasureText,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) -> Style {
    let Some(node) = boxes.get(id) else {
        return Style::default();
    };
    if let BoxKind::Anonymous {
        purpose:
            alo_box::Purpose::Control {
                centred,
                at_least_a_line,
                in_segments,
                ..
            },
        ..
    } = node.kind
    {
        return control_content_style(
            boxes,
            styles,
            id,
            centred,
            at_least_a_line,
            in_segments,
            measure,
        );
    }
    let mut ours = node
        .kind
        .node()
        .and_then(|source| styles.get(source))
        .map(|computed| style::read(computed, issues))
        .unwrap_or_default();
    // The viewport's `overflow`, taken from this box, leaves it `visible`:
    // a margin collapses through a body of `hidden` under a root of
    // `visible`, and what it holds reaches the page's scrolling area.
    if viewport::takes_the_overflow_of(boxes, styles, id) {
        ours.overflow = AxisValues::default();
    }

    let mut style = Style {
        display: display_for(&node.kind, boxes.natural_size(id).is_some()),
        box_sizing: match ours.box_sizing {
            BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
            BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
        },
        position: match ours.position {
            // `static` and `relative` differ only in whether offsets apply,
            // and `taffy` says that with the inset rather than the position.
            Positioning::Static | Positioning::Relative => Position::Relative,
            Positioning::Absolute => Position::Absolute,
        },
        overflow: TaffyPoint {
            x: overflow_for(ours.overflow.horizontal),
            y: overflow_for(ours.overflow.vertical),
        },
        // Only the ratio an author wrote. A replaced box's own ratio is
        // applied where the box is sized (`crate::replaced`, from the arena's
        // leaf): handed to `taffy` it held a picture at least its width over
        // its ratio tall whatever its style fixed, and carried a
        // `max-height` across into a narrower width (queue item 590).
        aspect_ratio: ours.aspect_ratio,
        flex_direction: match ours.flex.direction {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        flex_wrap: match ours.flex.wrap {
            FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        },
        flex_grow: ours.flex.grow,
        flex_shrink: ours.flex.shrink,
        flex_basis: dimension(&ours.flex.basis, ours.metrics, unresolved, issues),
        align_items: alignment(ours.align.align_items),
        align_self: alignment(ours.align.align_self),
        justify_items: alignment(ours.align.justify_items),
        justify_self: alignment(ours.align.justify_self),
        align_content: distribution(ours.align.align_content),
        justify_content: distribution(ours.align.justify_content),
        grid_auto_flow: match ours.grid.auto_flow {
            GridAutoFlow::Row => taffy::GridAutoFlow::Row,
            GridAutoFlow::Column => taffy::GridAutoFlow::Column,
            GridAutoFlow::RowDense => taffy::GridAutoFlow::RowDense,
            GridAutoFlow::ColumnDense => taffy::GridAutoFlow::ColumnDense,
        },
        grid_row: placement(ours.grid.row),
        grid_column: placement(ours.grid.column),
        ..Style::default()
    };

    box_model_into(&ours, &mut style, unresolved, issues);
    if let Some(characters) = boxes.field_size(id) {
        let text = text_style_for(boxes, styles, id);
        with_a_fields_width(characters, &text, &ours, &mut style, measure, issues);
    }
    if boxes.rendered_legend(id).is_some() {
        // A fieldset showing a legend has a **band** where its block-start
        // border would be, as tall as the legend and with the border drawn
        // through the middle of it — so the border itself takes no room here,
        // or the fieldset would be that much taller than a browser's.
        // `crate::legend` puts the legend in the band afterwards and records
        // the border that belongs there.
        style.border.top = TaffyLengthPercentage::length(0.0);
    }
    style.grid_template_rows = template(&ours.grid.template_rows, ours.metrics, unresolved);
    style.grid_template_columns = template(&ours.grid.template_columns, ours.metrics, unresolved);
    style.grid_auto_rows = auto_tracks(&ours.grid.auto_rows, ours.metrics, unresolved);
    style.grid_auto_columns = auto_tracks(&ours.grid.auto_columns, ours.metrics, unresolved);

    style
}

/// Whether this box's children all sit in a line, and so form an inline
/// formatting context of their own.
///
/// A `<p>` holding text is one; so is the anonymous box the box tree made for
/// a run of inline children beside a block. Both are handed to the layout
/// engine as a leaf, and [`crate::inline`] lays out what is inside.
pub(crate) fn is_inline_formatting_context(boxes: &BoxTree, id: BoxId) -> bool {
    let Some(node) = boxes.get(id) else {
        return false;
    };
    if !matches!(node.kind.inside(), Inside::Flow | Inside::FlowRoot) {
        return false;
    }
    // A list item's marker is in none of its lines, so it says nothing
    // about whether the item holds lines (ADR 0044 § 4).
    let mut children = node
        .children
        .iter()
        .filter(|child| !boxes.is_marker(**child))
        .peekable();
    if children.peek().is_none() {
        return false;
    }
    // A box out of flow among them takes no room in the lines and does not
    // stop them being lines; it alone, with nothing in flow, makes none.
    let in_line = |child: &BoxId| {
        boxes
            .get(*child)
            .is_some_and(|node| node.kind.outside() == Outside::Inline)
    };
    let mut any_in_flow = false;
    for child in children {
        if in_line(child) {
            any_in_flow = true;
        } else if !boxes.is_out_of_flow(*child) {
            return false;
        }
    }
    any_in_flow
}

/// Whether every child of `id` sits in a line or is out of flow, which is
/// true of nothing at all: what a nested inline box holds once the box tree
/// has broken it around every block in it.
///
/// Not [`is_inline_formatting_context`], which asks whether a block makes
/// lines: an `<a>` holding nothing but a box out of flow makes none and is
/// still a bracket in its parent's line, not an atomic box laid out on its
/// own — which is what it was, a whole line wide, until this.
fn holds_only_what_sits_in_a_line(boxes: &BoxTree, id: BoxId) -> bool {
    boxes
        .children(id)
        .filter(|child| !boxes.is_marker(*child))
        .all(|child| {
            boxes.is_out_of_flow(child)
                || boxes
                    .get(child)
                    .is_some_and(|node| node.kind.outside() == Outside::Inline)
        })
}

/// The boxes out of flow written in the lines of `id`, in order: among its
/// children and inside the inline boxes it holds, though not inside an
/// atomic one, which is laid out on its own and finds its own.
pub(crate) fn out_of_flow_in_line(boxes: &BoxTree, id: BoxId) -> Vec<BoxId> {
    let mut found = Vec::new();
    for child in boxes.children(id).filter(|child| !boxes.is_marker(*child)) {
        if boxes.is_out_of_flow(child) {
            found.push(child);
            continue;
        }
        let Some(node) = boxes.get(child) else {
            continue;
        };
        let nested = !matches!(node.kind, BoxKind::Text { .. })
            && !boxes.is_forced_break(child)
            && holds_only_what_sits_in_a_line(boxes, child)
            && !is_atomic(boxes, child);
        if nested {
            found.extend(out_of_flow_in_line(boxes, child));
        }
    }
    found
}

/// The sizes, the four edges and the gap — everything measured in lengths.
fn box_model_into(
    ours: &LayoutStyle,
    style: &mut Style,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) {
    let metrics = ours.metrics;
    style.inset = if ours.position == Positioning::Static {
        // A static box ignores its offsets, which is the whole difference
        // between `static` and `relative`.
        TaffyRect::auto()
    } else {
        TaffyRect {
            top: auto_length(&ours.inset.top, metrics, unresolved),
            right: auto_length(&ours.inset.right, metrics, unresolved),
            bottom: auto_length(&ours.inset.bottom, metrics, unresolved),
            left: auto_length(&ours.inset.left, metrics, unresolved),
        }
    };
    style.size = TaffySize {
        width: dimension(&ours.size.horizontal, metrics, unresolved, issues),
        height: dimension(&ours.size.vertical, metrics, unresolved, issues),
    };
    style.min_size = TaffySize {
        width: min_max(&ours.min_size.horizontal, metrics, unresolved, issues),
        height: min_max(&ours.min_size.vertical, metrics, unresolved, issues),
    };
    style.max_size = TaffySize {
        width: maximum(&ours.max_size.horizontal, metrics, unresolved, issues),
        height: maximum(&ours.max_size.vertical, metrics, unresolved, issues),
    };
    style.margin = TaffyRect {
        top: auto_length(&ours.margin.top, metrics, unresolved),
        right: auto_length(&ours.margin.right, metrics, unresolved),
        bottom: auto_length(&ours.margin.bottom, metrics, unresolved),
        left: auto_length(&ours.margin.left, metrics, unresolved),
    };
    style.padding = TaffyRect {
        top: length(&ours.padding.top, metrics, unresolved),
        right: length(&ours.padding.right, metrics, unresolved),
        bottom: length(&ours.padding.bottom, metrics, unresolved),
        left: length(&ours.padding.left, metrics, unresolved),
    };
    style.border = TaffyRect {
        top: length(&ours.border.top, metrics, unresolved),
        right: length(&ours.border.right, metrics, unresolved),
        bottom: length(&ours.border.bottom, metrics, unresolved),
        left: length(&ours.border.left, metrics, unresolved),
    };
    style.gap = TaffySize {
        width: length(&ours.gap.horizontal, metrics, unresolved),
        height: length(&ours.gap.vertical, metrics, unresolved),
    };
}

/// A text field whose `width` is `auto` made as wide as its `size` says, in
/// its own font ([`crate::field_width`]; queue item 511).
///
/// That is its content box's width, so a field sized by its border box,
/// as Chrome's `search` field is, has its padding and border added: Chrome
/// 154 makes a `search` field exactly as wide as a `text` field with the
/// same `size`. A percentage of either is of a width this does not know
/// yet, so it is counted as nothing and said.
fn with_a_fields_width(
    characters: u32,
    text: &TextStyle,
    ours: &LayoutStyle,
    style: &mut Style,
    measure: &impl MeasureText,
    issues: &mut Vec<StyleIssue>,
) {
    if !matches!(ours.size.horizontal, Sizing::Auto) {
        return;
    }
    let mut width = field_width::of(characters, text, measure);
    if ours.box_sizing == BoxSizing::BorderBox {
        for (value, name) in [
            (&ours.padding.left, "left padding"),
            (&ours.padding.right, "right padding"),
            (&ours.border.left, "left border"),
            (&ours.border.right, "right border"),
        ] {
            if value.is_percentage() {
                issues.push(StyleIssue {
                    kind: IssueKind::UnsupportedValue,
                    source: format!("{value} on the {name} of a field sized by its border box"),
                    at: Location { line: 0, column: 0 },
                });
                continue;
            }
            width += pixels(value, ours.metrics);
        }
    }
    style.size.width = Dimension::length(width);
}

/// How `taffy` is to lay out a box, from what its `display` says inside.
///
/// A **replaced** box whose `display` is a flow is handed over as a
/// `flow-root`: it is a leaf whose content is a picture rather than lines,
/// laid out on its own as an atomic box, and `taffy` stretches a `block` it
/// lays out on its own to the width it was offered. An `img`, `inline` since
/// ADR 0046, would otherwise be drawn as wide as its line.
fn display_for(kind: &BoxKind, replaced: bool) -> taffy::Display {
    match kind {
        BoxKind::Element { display, .. }
            if replaced && matches!(display.inside(), Some(Inside::Flow) | None) =>
        {
            taffy::Display::FlowRoot
        }
        // A `<details>`' content box holds blocks as any block does, and its
        // children's margins collapse through it (ADR 0048 § 1).
        BoxKind::Text { .. }
        | BoxKind::Anonymous {
            purpose: alo_box::Purpose::DetailsContent,
            ..
        } => taffy::Display::Block,
        BoxKind::Anonymous { .. } => taffy::Display::Flex,
        BoxKind::Element { display, .. } => match display.inside() {
            Some(Inside::Flex) => taffy::Display::Flex,
            Some(Inside::Grid) => taffy::Display::Grid,
            Some(Inside::FlowRoot) => taffy::Display::FlowRoot,
            // A box with no `inside` generates no box at all and never reaches
            // here; the box tree removed it.
            Some(Inside::Flow) | None => taffy::Display::Block,
        },
    }
}

fn overflow_for(overflow: Overflow) -> taffy::Overflow {
    match overflow {
        Overflow::Visible => taffy::Overflow::Visible,
        Overflow::Clip => taffy::Overflow::Clip,
        Overflow::Hidden => taffy::Overflow::Hidden,
        // `auto` reserves room for a scrollbar only when one is needed, which
        // is a paint decision; for layout it behaves as `scroll`.
        Overflow::Scroll | Overflow::Auto => taffy::Overflow::Scroll,
    }
}

fn alignment(value: Alignment) -> Option<taffy::AlignItems> {
    match value {
        Alignment::Normal => None,
        Alignment::Start => Some(taffy::AlignItems::START),
        Alignment::End => Some(taffy::AlignItems::END),
        Alignment::FlexStart => Some(taffy::AlignItems::FLEX_START),
        Alignment::FlexEnd => Some(taffy::AlignItems::FLEX_END),
        Alignment::Center => Some(taffy::AlignItems::CENTER),
        Alignment::Baseline => Some(taffy::AlignItems::BASELINE),
        Alignment::Stretch => Some(taffy::AlignItems::STRETCH),
    }
}

fn distribution(value: Distribution) -> Option<taffy::AlignContent> {
    match value {
        Distribution::Normal => None,
        Distribution::Start => Some(taffy::AlignContent::START),
        Distribution::End => Some(taffy::AlignContent::END),
        Distribution::FlexStart => Some(taffy::AlignContent::FLEX_START),
        Distribution::FlexEnd => Some(taffy::AlignContent::FLEX_END),
        Distribution::Center => Some(taffy::AlignContent::CENTER),
        Distribution::SpaceBetween => Some(taffy::AlignContent::SPACE_BETWEEN),
        Distribution::SpaceAround => Some(taffy::AlignContent::SPACE_AROUND),
        Distribution::SpaceEvenly => Some(taffy::AlignContent::SPACE_EVENLY),
        Distribution::Stretch => Some(taffy::AlignContent::STRETCH),
    }
}

fn placement(value: GridPlacement) -> Line<taffy::GridPlacement> {
    Line {
        start: grid_line(value.start),
        end: grid_line(value.end),
    }
}

fn grid_line(line: GridLine) -> taffy::GridPlacement {
    match line {
        GridLine::Auto => taffy::GridPlacement::Auto,
        GridLine::Line(index) => taffy::GridPlacement::from_line_index(index),
        GridLine::Span(count) => taffy::GridPlacement::from_span(count),
    }
}

/// Whether a value has to wait for a basis only the algorithm knows.
///
/// A `calc()` of lengths only is already a plain number by the time it gets
/// here — `calc(var(--gap) * 2)`, which is what a design system actually
/// writes. It is the ones with a percentage inside that have to be carried as
/// a handle and asked about later; see ADR 0004. **A plain percentage is
/// carried the same way**, so that the arena truncates it onto the grid once
/// it is resolved (ADR 0054 § 2): handed over as a percentage, `taffy` would
/// resolve it itself, as `basis × percent`, and nothing could reach it.
fn needs_a_basis(value: &LengthPercentage) -> bool {
    value.is_percentage()
}

/// A length's number, truncated toward zero onto the grid, each length on
/// its own (ADR 0054 § 1). A `calc()` of lengths only is resolved whole and
/// truncated once.
fn pixels(value: &LengthPercentage, metrics: FontMetrics) -> f32 {
    layout_unit::truncated(value.to_px(metrics, 0.0), metrics.scale())
}

/// A length or percentage, as `taffy` spells it.
fn length(
    value: &LengthPercentage,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> TaffyLengthPercentage {
    if needs_a_basis(value) {
        return TaffyLengthPercentage::calc(unresolved.handle(value, metrics));
    }
    TaffyLengthPercentage::length(pixels(value, metrics))
}

fn auto_length(
    value: &AutoLength,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> LengthPercentageAuto {
    match value {
        AutoLength::Auto => LengthPercentageAuto::auto(),
        AutoLength::Length(inner) => {
            if needs_a_basis(inner) {
                return LengthPercentageAuto::calc(unresolved.handle(inner, metrics));
            }
            LengthPercentageAuto::length(pixels(inner, metrics))
        }
    }
}

fn dimension(
    value: &Sizing,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) -> Dimension {
    match value {
        Sizing::Auto => Dimension::auto(),
        Sizing::MinContent => Dimension::min_content(),
        Sizing::MaxContent => Dimension::max_content(),
        Sizing::FitContent(limit) => {
            // `fit-content()` takes a percentage or a length, and the layout
            // algorithms have no `calc()` spelling for its argument — so this
            // one expression is still refused, and says so. For the same
            // reason a percentage limit is `taffy`'s to resolve, and is not
            // truncated onto the grid (ADR 0054 § 2): no page has asked.
            if matches!(limit, LengthPercentage::Calc(_)) && limit.is_percentage() {
                issues.push(StyleIssue {
                    kind: IssueKind::UnsupportedValue,
                    source: format!("fit-content({limit}) — a calc() inside fit-content()"),
                    at: Location { line: 0, column: 0 },
                });
                return Dimension::auto();
            }
            match limit {
                LengthPercentage::Percentage(percent) => {
                    Dimension::fit_content_percent(percent / 100.0)
                }
                other => Dimension::fit_content_px(pixels(other, metrics)),
            }
        }
        Sizing::Length(inner) => {
            if needs_a_basis(inner) {
                return Dimension::calc(unresolved.handle(inner, metrics));
            }
            Dimension::length(pixels(inner, metrics))
        }
    }
}

/// A `max-width` or `max-height`. `taffy` says "no limit" as `auto`, so
/// `none` is that, and a limit is read as a minimum's size is.
fn maximum(
    value: &MaxSizing,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) -> LengthPercentageAuto {
    match value {
        MaxSizing::None => LengthPercentageAuto::auto(),
        MaxSizing::Limit(limit) => min_max(limit, metrics, unresolved, issues),
    }
}

/// A `min-width` or `max-width`.
///
/// The content keywords are not expressible here — `taffy` takes a plain
/// length, a percentage or `auto` for a minimum and a maximum — so
/// `min-width: min-content` becomes `auto`, which is what CSS says the initial
/// value is and is recorded rather than silently substituted.
fn min_max(
    value: &Sizing,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) -> LengthPercentageAuto {
    match value {
        Sizing::Auto => LengthPercentageAuto::auto(),
        Sizing::MinContent | Sizing::MaxContent | Sizing::FitContent(_) => {
            issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source: format!("{value} as a minimum or maximum size"),
                at: Location { line: 0, column: 0 },
            });
            LengthPercentageAuto::auto()
        }
        Sizing::Length(inner) => {
            if needs_a_basis(inner) {
                return LengthPercentageAuto::calc(unresolved.handle(inner, metrics));
            }
            LengthPercentageAuto::length(pixels(inner, metrics))
        }
    }
}

fn template(
    list: &TrackList,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> Vec<GridTemplateComponent<String>> {
    list.entries
        .iter()
        .map(|entry| match entry {
            TrackListEntry::Single(track) => {
                GridTemplateComponent::Single(track_function(track, metrics, unresolved))
            }
            TrackListEntry::Repeat { count, tracks } => {
                GridTemplateComponent::Repeat(taffy::GridTemplateRepetition {
                    count: match count {
                        RepeatCount::Times(times) => taffy::RepetitionCount::Count(*times),
                        RepeatCount::AutoFill => taffy::RepetitionCount::AutoFill,
                        RepeatCount::AutoFit => taffy::RepetitionCount::AutoFit,
                    },
                    tracks: tracks
                        .iter()
                        .map(|track| track_function(track, metrics, unresolved))
                        .collect(),
                    line_names: Vec::new(),
                })
            }
        })
        .collect()
}

fn auto_tracks(
    list: &TrackList,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> Vec<TrackSizingFunction> {
    list.entries
        .iter()
        .filter_map(|entry| match entry {
            TrackListEntry::Single(track) => Some(track_function(track, metrics, unresolved)),
            // `grid-auto-rows` takes a list of track sizes, not repetitions.
            TrackListEntry::Repeat { .. } => None,
        })
        .collect()
}

fn track_function(
    track: &Track,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> TrackSizingFunction {
    TrackSizingFunction {
        min: min_track(&track.min, metrics, unresolved),
        max: max_track(&track.max, metrics, unresolved),
    }
}

fn min_track(
    size: &TrackSize,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> MinTrackSizingFunction {
    match size {
        // A fraction has no minimum of its own; `auto` is what CSS says a
        // `<flex>` minimum becomes, which is the same thing `auto` says.
        TrackSize::Auto | TrackSize::Fraction(_) => MinTrackSizingFunction::auto(),
        TrackSize::MinContent => MinTrackSizingFunction::min_content(),
        TrackSize::MaxContent => MinTrackSizingFunction::max_content(),
        TrackSize::Length(value) => {
            if needs_a_basis(value) {
                return MinTrackSizingFunction::calc(unresolved.handle(value, metrics));
            }
            MinTrackSizingFunction::length(pixels(value, metrics))
        }
    }
}

fn max_track(
    size: &TrackSize,
    metrics: FontMetrics,
    unresolved: &mut Unresolved,
) -> MaxTrackSizingFunction {
    match size {
        TrackSize::Auto => MaxTrackSizingFunction::auto(),
        TrackSize::MinContent => MaxTrackSizingFunction::min_content(),
        TrackSize::MaxContent => MaxTrackSizingFunction::max_content(),
        TrackSize::Fraction(share) => MaxTrackSizingFunction::fr(*share),
        TrackSize::Length(value) => {
            if needs_a_basis(value) {
                return MaxTrackSizingFunction::calc(unresolved.handle(value, metrics));
            }
            MaxTrackSizingFunction::length(pixels(value, metrics))
        }
    }
}

/// Where a box's top left is on the page: where `taffy` put it, and where it
/// was placed on the grid.
#[derive(Clone, Copy)]
struct Origins {
    unplaced: Point,
    placed: Point,
}

impl Origins {
    const ZERO: Self = Self {
        unplaced: Point::ZERO,
        placed: Point::ZERO,
    };
}

/// What reading the engine's tree back needs, and what it fills in.
struct Reading<'r, 'a, M: MeasureText> {
    arena: &'r Arena<'a, M>,
    built: &'r Built,
    boxes: &'r BoxTree,
    /// Every box's geometry, placed on the grid.
    out: &'r mut BTreeMap<BoxId, BoxGeometry>,
    /// Where `taffy` put each box's top left on the page, before it was
    /// placed: a displaced box is measured from its containing block's.
    unplaced: &'r mut BTreeMap<BoxId, Point>,
}

impl<M: MeasureText> Reading<'_, '_, M> {
    /// Walk the tree and turn parent-relative positions into positions on the
    /// page, each box placed on the grid (ADR 0054 §§ 3 and 6).
    ///
    /// A displaced box's position is relative to its containing block's node,
    /// so it is measured from there — the containing block is its ancestor and
    /// was read first — or from the top left of the page for the initial
    /// containing block; and an axis it takes from its [`Probe`] is where the
    /// probe is, in its parent, plus its own margin, as `taffy` places a box at
    /// its static position.
    ///
    /// **A box is placed against where the box it is measured from was
    /// placed**, not against where `taffy` put that box. Its near edge is
    /// truncated toward zero from there and its size is the distance between
    /// its two edges floored ([`layout_unit::edges`]). A box that fills its
    /// parent in `taffy`'s numbers therefore fills it on the grid too: a grid
    /// item whose track `taffy` started at 22,579.2 units and ended at 49,920
    /// is placed at 22,579 and 27,341 wide, and a label filling it is 27,341
    /// wide as well, as in Chrome, where measuring the label from its
    /// parent's unplaced edge would floor it to 27,340.
    fn read_back(&mut self, id: BoxId, parent: Origins) {
        let Some(theirs) = self.built.ours_to_theirs.get(&id) else {
            return;
        };
        let Some(layout) = self.arena.layout(*theirs) else {
            return;
        };
        // What the box is measured from in each axis, and where `taffy` put
        // its top left on the page.
        let (mut from_x, mut from_y) = (parent, parent);
        let mut unplaced = Point::new(
            parent.unplaced.x + layout.location.x,
            parent.unplaced.y + layout.location.y,
        );
        if let Some(displaced) = self.built.displaced.get(&id) {
            let against = displaced.against.map_or(Origins::ZERO, |against| Origins {
                unplaced: self
                    .unplaced
                    .get(&against)
                    .copied()
                    .unwrap_or(parent.unplaced),
                placed: self
                    .out
                    .get(&against)
                    .map_or(parent.placed, |held| held.border_box.origin),
            });
            (from_x, from_y) = (against, against);
            unplaced = Point::new(
                against.unplaced.x + layout.location.x,
                against.unplaced.y + layout.location.y,
            );
            if let Some(probe) = displaced.probe
                && let Some(at) = self.arena.layout(probe.node)
            {
                if probe.across {
                    from_x = parent;
                    unplaced.x = parent.unplaced.x + at.location.x + layout.margin.left;
                }
                if probe.down {
                    from_y = parent;
                    unplaced.y = parent.unplaced.y + at.location.y + layout.margin.top;
                }
            }
        }
        let scale = self.arena.scale();
        let (left, width) =
            layout_unit::edges(unplaced.x - from_x.placed.x, layout.size.width, scale);
        let (top, height) =
            layout_unit::edges(unplaced.y - from_y.placed.y, layout.size.height, scale);
        let origin = Point::new(from_x.placed.x + left, from_y.placed.y + top);
        self.unplaced.insert(id, unplaced);
        self.out.insert(
            id,
            BoxGeometry {
                border_box: Rect {
                    origin,
                    size: Size::new(width, height),
                },
                border: edges(layout.border),
                padding: edges(layout.padding),
                margin: edges(layout.margin),
                scrollable: Size::new(
                    layout.scrollable_overflow_rect.right - layout.scrollable_overflow_rect.left,
                    layout.scrollable_overflow_rect.bottom - layout.scrollable_overflow_rect.top,
                ),
                // `taffy` measures the overflow from the padding box's top
                // left, its scroll origin, and its right and bottom are never
                // below zero: what a scroll could reach.
                reach: Size::new(
                    layout.scrollable_overflow_rect.right.max(0.0),
                    layout.scrollable_overflow_rect.bottom.max(0.0),
                ),
                // Written by `crate::legend`, once every box has a rectangle:
                // a band is where a legend ended up, and nothing here knows
                // that yet.
                band: None,
            },
        );
        let here = Origins {
            unplaced,
            placed: origin,
        };
        let children: Vec<BoxId> = self.boxes.children(id).collect();
        for child in children {
            self.read_back(child, here);
        }
    }
}

/// Count what is placed against the initial containing block in the
/// document's reach.
///
/// The root's [`BoxGeometry::reach`] is what the viewport's scrolling area is
/// made of (ADR 0038 § 4), and CSS Overflow 3 § 2.2 puts a box in the
/// scrollable overflow of each box in its chain of containing blocks — which
/// for these is the viewport's, and not the root element's own. `taffy`
/// laid them out beside the root rather than in it, so the root's reach is
/// extended by hand: a box placed at `top: 2000px` against nothing makes the
/// page that much taller to scroll, as it does in a browser, and one far to
/// the left, alo Sites' skip link, makes it no wider.
fn reach_of_the_initial_block(
    built: &Built,
    root: BoxId,
    geometry: &mut BTreeMap<BoxId, BoxGeometry>,
) {
    let (mut right, mut bottom) = (0.0_f32, 0.0_f32);
    for (id, displaced) in &built.displaced {
        if displaced.against.is_some() {
            continue;
        }
        let Some(held) = geometry.get(id) else {
            continue;
        };
        let padding = held.padding_box();
        right = right
            .max(held.border_box.right())
            .max(padding.origin.x + held.reach.width);
        bottom = bottom
            .max(held.border_box.bottom())
            .max(padding.origin.y + held.reach.height);
    }
    let Some(root) = geometry.get_mut(&root) else {
        return;
    };
    let from = root.padding_box().origin;
    root.reach = Size::new(
        root.reach.width.max(right - from.x),
        root.reach.height.max(bottom - from.y),
    );
}

fn edges(rect: TaffyRect<f32>) -> Edges {
    Edges {
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        left: rect.left,
    }
}

/// `taffy`'s geometry types, named apart from ours so that the difference is
/// visible in this file and impossible anywhere else.
use taffy::geometry::{Point as TaffyPoint, Rect as TaffyRect, Size as TaffySize};
