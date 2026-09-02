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
use crate::geometry::{Edges, Point, Rect, Size};
use crate::inline::{self, Fragment, InlineItem, InlineLayout};
use crate::keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
use crate::measure::{MeasureText, TextStyle};
use crate::placement::{GridLine, GridPlacement};
use crate::sizing::{AutoLength, Sizing};
use crate::style::{self, LayoutStyle};
use crate::track::{RepeatCount, Track, TrackList, TrackListEntry, TrackSize};
use crate::tree::{BoxGeometry, LayoutTree};
use alo_box::{BoxId, BoxKind, BoxTree, Inside, Outside};
use alo_css::{IssueKind, Location, StyleIssue};
use alo_style::StyleTree;
use alo_value::{FontMetrics, LengthPercentage};
use std::collections::BTreeMap;
use taffy::prelude::*;
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
    let Some(laid_out) = lay_out_subtree(boxes, styles, root, available, measure, &mut issues)
    else {
        return LayoutTree::from_parts(BTreeMap::new(), BTreeMap::new(), issues, viewport);
    };
    LayoutTree::from_parts(laid_out.geometry, laid_out.fragments, issues, viewport)
}

/// One formatting context, laid out on its own.
struct LaidOut {
    size: Size,
    baseline: f32,
    geometry: BTreeMap<BoxId, BoxGeometry>,
    fragments: BTreeMap<BoxId, Vec<Fragment>>,
}

/// Lay out a subtree as its own formatting context, with its own engine tree.
///
/// Called once for the document, and again for each atomic inline-level box —
/// an `inline-block`, an image, a button — because such a box's size is
/// whatever its own layout says, and its own layout is this.
fn lay_out_subtree(
    boxes: &BoxTree,
    styles: &StyleTree,
    root: BoxId,
    available: TaffySize<AvailableSpace>,
    measure: &impl MeasureText,
    issues: &mut Vec<StyleIssue>,
) -> Option<LaidOut> {
    let mut ours_to_theirs: BTreeMap<BoxId, usize> = BTreeMap::new();
    let mut arena = Arena::new(boxes, styles, measure);
    let root_node = build(boxes, styles, root, &mut arena, &mut ours_to_theirs, issues)?;
    // Sub-pixel throughout: `taffy`'s rounding is a pass over a trait this
    // engine's tree does not implement, so it cannot happen. A box rounded
    // down to 96 while its text measures 96.16 wraps a word early, which is
    // how "Remember me" once became two lines in a box wide enough for one.
    arena.compute(root_node, available);

    let mut geometry = BTreeMap::new();
    let mut fragments = BTreeMap::new();
    read_back(
        &arena,
        &ours_to_theirs,
        boxes,
        root,
        Point::ZERO,
        &mut geometry,
    );
    place_inline_content(
        boxes,
        styles,
        root,
        measure,
        &mut geometry,
        &mut fragments,
        issues,
    );

    let root_geometry = geometry.get(&root).copied().unwrap_or_default();
    Some(LaidOut {
        size: root_geometry.border_box.size,
        // A box's baseline is the last line inside it, or its bottom edge when
        // there is no line — which is what CSS says an empty inline-block sits
        // on.
        baseline: root_geometry.border_box.size.height,
        geometry,
        fragments,
    })
}

/// Work out what is on each line of an inline formatting context.
pub(crate) fn measure_inline(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    available_width: Option<f32>,
    measure: &impl MeasureText,
) -> InlineLayout {
    let mut issues = Vec::new();
    let items = collect_inline_items(boxes, styles, id, available_width, measure, &mut issues);
    inline::lay_out_aligned(
        &items,
        available_width,
        alignment_of(boxes, styles, id),
        measure,
    )
}

/// Where the lines of a formatting context sit in it.
///
/// `text-align` inherits, and a box's own value is already the inherited one by
/// the time it is asked — so this asks the box that holds the lines, which is
/// the box whose width they are aligned in.
fn alignment_of(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> inline::TextAlignment {
    boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
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
    let children: Vec<BoxId> = boxes.children(id).collect();
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
            _ if is_inline_formatting_context(boxes, child) || node.children.is_empty() => {
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
        value.to_px(metrics, 0.0)
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
fn text_style_for(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> TextStyle {
    let mut current = Some(id);
    while let Some(box_id) = current {
        if let Some(style) = boxes
            .get(box_id)
            .and_then(|node| node.kind.node())
            .and_then(|source| styles.get(source))
        {
            return TextStyle {
                families: style
                    .get("font-family")
                    .map(|value| {
                        value
                            .split(',')
                            .map(|part| {
                                part.trim()
                                    .trim_matches(|c| c == '"' || c == '\'')
                                    .trim()
                                    .to_owned()
                            })
                            .filter(|part| !part.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                size: style.font_size(),
                weight: weight_of(style),
                italic: style
                    .get("font-style")
                    .is_some_and(|value| !value.eq_ignore_ascii_case("normal")),
            };
        }
        current = boxes.get(box_id).and_then(|node| node.parent);
    }
    TextStyle::default()
}

/// `font-weight` as a number, taking the two keywords that are numbers in
/// disguise.
fn weight_of(style: &alo_style::ComputedStyle) -> u16 {
    if let Some(number) = style.number("font-weight") {
        let clamped = number.clamp(1.0, 1000.0).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to one..=1000 and rounded"
        )]
        let weight = clamped as u16;
        return weight;
    }
    match style.get("font-weight") {
        Some(value) if value.eq_ignore_ascii_case("bold") => 700,
        _ => 400,
    }
}

/// Whether an inline-level box is laid out on its own rather than joining the
/// line around it.
///
/// `inline flow` is a box whose content joins the line — a `<span>`, an `<a>`.
/// Anything else inline-level establishes its own formatting context and is
/// placed on the line whole: an `inline-block`, an `inline-flex`, an image.
fn is_atomic(boxes: &BoxTree, id: BoxId) -> bool {
    boxes
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
    let laid_out = lay_out_subtree(boxes, styles, id, available, measure, issues);
    InlineItem::Atomic {
        box_id: id,
        size: laid_out.as_ref().map_or(Size::ZERO, |held| held.size),
        baseline: laid_out.as_ref().map_or(0.0, |held| held.baseline),
    }
}

/// Once the engine has placed every block, put the inline content inside the
/// boxes that hold it.
fn place_inline_content(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    measure: &impl MeasureText,
    geometry: &mut BTreeMap<BoxId, BoxGeometry>,
    fragments: &mut BTreeMap<BoxId, Vec<Fragment>>,
    issues: &mut Vec<StyleIssue>,
) {
    if is_inline_formatting_context(boxes, id) {
        let Some(container) = geometry.get(&id).copied() else {
            return;
        };
        let content = container.content_box();
        let items =
            collect_inline_items(boxes, styles, id, Some(content.size.width), measure, issues);
        let layout = inline::lay_out_aligned(
            &items,
            Some(content.size.width),
            alignment_of(boxes, styles, id),
            measure,
        );

        for fragment in layout.fragments() {
            let placed = Fragment {
                rect: fragment.rect.translated(content.origin),
                ..fragment.clone()
            };
            fragments
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
            let own = fragments
                .get(&inner)
                .and_then(|pieces| pieces.iter().map(|piece| piece.rect).reduce(union_rects));
            let Some(rect) = own.or_else(|| union_of_subtree(boxes, inner, fragments)) else {
                continue;
            };
            let (border, padding) = inline_edges(boxes, styles, inner, issues);
            geometry.entry(inner).or_insert(BoxGeometry {
                border_box: rect,
                border,
                padding,
                ..BoxGeometry::default()
            });
        }
        // An atomic box brought a whole layout of its own with it; place it.
        for item in &items {
            if let InlineItem::Atomic { box_id, .. } = item
                && let Some(placed) = fragments.get(box_id).and_then(|pieces| pieces.first())
            {
                let offset = placed.rect.origin;
                if let Some(sub) = lay_out_subtree(
                    boxes,
                    styles,
                    *box_id,
                    TaffySize {
                        width: AvailableSpace::Definite(placed.rect.size.width),
                        height: AvailableSpace::Definite(placed.rect.size.height),
                    },
                    measure,
                    issues,
                ) {
                    for (inner, mut held) in sub.geometry {
                        held.border_box = held.border_box.translated(offset);
                        geometry.insert(inner, held);
                    }
                    for (inner, pieces) in sub.fragments {
                        let moved: Vec<Fragment> = pieces
                            .into_iter()
                            .map(|piece| Fragment {
                                rect: piece.rect.translated(offset),
                                ..piece
                            })
                            .collect();
                        fragments.insert(inner, moved);
                    }
                }
            }
        }
        return;
    }

    let children: Vec<BoxId> = boxes.children(id).collect();
    for child in children {
        place_inline_content(boxes, styles, child, measure, geometry, fragments, issues);
    }
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
fn build<M: MeasureText>(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    arena: &mut Arena<'_, M>,
    ours_to_theirs: &mut BTreeMap<BoxId, usize>,
    issues: &mut Vec<StyleIssue>,
) -> Option<usize> {
    let node = boxes.get(id)?;
    let style = style_for(boxes, styles, id, arena.unresolved(), issues);

    if is_inline_formatting_context(boxes, id) {
        let made = arena.push(style, NodeKind::InlineFormatting(id), Vec::new());
        ours_to_theirs.insert(id, made);
        return Some(made);
    }

    let children: Vec<usize> = node
        .children
        .iter()
        .filter_map(|child| build(boxes, styles, *child, arena, ours_to_theirs, issues))
        .collect();

    let kind = match &node.kind {
        BoxKind::Text { text, .. } => {
            NodeKind::Text(text.clone(), text_style_for(boxes, styles, id))
        }
        _ if children.is_empty() => NodeKind::Empty,
        _ => NodeKind::Container,
    };
    let made = arena.push(style, kind, children);
    ours_to_theirs.insert(id, made);
    Some(made)
}

/// The `taffy` style for one box.
fn style_for(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    unresolved: &mut Unresolved,
    issues: &mut Vec<StyleIssue>,
) -> Style {
    let Some(node) = boxes.get(id) else {
        return Style::default();
    };
    let ours = node
        .kind
        .node()
        .and_then(|source| styles.get(source))
        .map(|computed| style::read(computed, issues))
        .unwrap_or_default();

    let mut style = Style {
        display: display_for(&node.kind),
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
    if node.children.is_empty() {
        return false;
    }
    node.children.iter().all(|child| {
        boxes
            .get(*child)
            .is_some_and(|child| child.kind.outside() == Outside::Inline)
    })
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
        width: min_max(&ours.max_size.horizontal, metrics, unresolved, issues),
        height: min_max(&ours.max_size.vertical, metrics, unresolved, issues),
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

fn display_for(kind: &BoxKind) -> taffy::Display {
    match kind {
        BoxKind::Text { .. } => taffy::Display::Block,
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
/// a handle and asked about later; see ADR 0004.
fn needs_a_basis(value: &LengthPercentage) -> bool {
    matches!(value, LengthPercentage::Calc(_)) && value.is_percentage()
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
    match value {
        LengthPercentage::Percentage(percent) => TaffyLengthPercentage::percent(percent / 100.0),
        other => TaffyLengthPercentage::length(other.to_px(metrics, 0.0)),
    }
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
            match inner {
                LengthPercentage::Percentage(percent) => {
                    LengthPercentageAuto::percent(percent / 100.0)
                }
                other => LengthPercentageAuto::length(other.to_px(metrics, 0.0)),
            }
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
            // one expression is still refused, and says so.
            if needs_a_basis(limit) {
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
                other => Dimension::fit_content_px(other.to_px(metrics, 0.0)),
            }
        }
        Sizing::Length(inner) => {
            if needs_a_basis(inner) {
                return Dimension::calc(unresolved.handle(inner, metrics));
            }
            match inner {
                LengthPercentage::Percentage(percent) => Dimension::percent(percent / 100.0),
                other => Dimension::length(other.to_px(metrics, 0.0)),
            }
        }
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
            match inner {
                LengthPercentage::Percentage(percent) => {
                    LengthPercentageAuto::percent(percent / 100.0)
                }
                other => LengthPercentageAuto::length(other.to_px(metrics, 0.0)),
            }
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
            match value {
                LengthPercentage::Percentage(percent) => {
                    MinTrackSizingFunction::percent(percent / 100.0)
                }
                other => MinTrackSizingFunction::length(other.to_px(metrics, 0.0)),
            }
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
            match value {
                LengthPercentage::Percentage(percent) => {
                    MaxTrackSizingFunction::percent(percent / 100.0)
                }
                other => MaxTrackSizingFunction::length(other.to_px(metrics, 0.0)),
            }
        }
    }
}

/// Walk the tree and turn parent-relative positions into positions on the page.
fn read_back<M: MeasureText>(
    arena: &Arena<'_, M>,
    ours_to_theirs: &BTreeMap<BoxId, usize>,
    boxes: &BoxTree,
    id: BoxId,
    parent_origin: Point,
    out: &mut BTreeMap<BoxId, BoxGeometry>,
) {
    let Some(theirs) = ours_to_theirs.get(&id) else {
        return;
    };
    let Some(layout) = arena.layout(*theirs) else {
        return;
    };
    let origin = Point::new(
        parent_origin.x + layout.location.x,
        parent_origin.y + layout.location.y,
    );
    out.insert(
        id,
        BoxGeometry {
            border_box: Rect {
                origin,
                size: Size::new(layout.size.width, layout.size.height),
            },
            border: edges(layout.border),
            padding: edges(layout.padding),
            margin: edges(layout.margin),
            scrollable: Size::new(
                layout.scrollable_overflow_rect.right - layout.scrollable_overflow_rect.left,
                layout.scrollable_overflow_rect.bottom - layout.scrollable_overflow_rect.top,
            ),
        },
    );
    let children: Vec<BoxId> = boxes.children(id).collect();
    for child in children {
        read_back(arena, ours_to_theirs, boxes, child, origin, out);
    }
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
