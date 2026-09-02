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
//! # Two seams, named rather than hidden
//!
//! **Inline formatting.** `taffy` has block, flex and grid, and no inline
//! layout at all. An anonymous box — which exists precisely to hold a run of
//! inline content — is laid out here as a **wrapping flex row**, which gets
//! boxes side by side and wrapping onto new lines. It does not get baselines,
//! or breaking *inside* a run of text. That is queue item 6, which is where
//! shaping and line breaking arrive, and it replaces this by giving the
//! anonymous box a real inline formatting context.
//!
//! **`calc()` with a percentage inside it.** `taffy` carries such a value as
//! an opaque handle that only a tree implementing its own traits can resolve,
//! and this uses `taffy`'s ready-made tree. So `width: calc(100% - 2rem)` is
//! refused and recorded rather than silently becoming something else, and
//! queue item 15 is where it is done properly. A `calc()` of lengths only —
//! `calc(var(--gap) * 2)`, which is what a design system actually writes — is
//! already a plain number by the time it reaches here and works.

use crate::geometry::{Edges, Point, Rect, Size};
use crate::keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
use crate::measure::MeasureText;
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
    MaxTrackSizingFunction, MinTrackSizingFunction, Style, TaffyTree, TrackSizingFunction,
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
        return LayoutTree::from_parts(BTreeMap::new(), issues, viewport);
    };

    let mut taffy: TaffyTree<Option<String>> = TaffyTree::new();
    let mut ours_to_theirs: BTreeMap<BoxId, NodeId> = BTreeMap::new();
    let Some(taffy_root) = build(
        boxes,
        styles,
        root,
        &mut taffy,
        &mut ours_to_theirs,
        &mut issues,
    ) else {
        return LayoutTree::from_parts(BTreeMap::new(), issues, viewport);
    };

    let available = TaffySize {
        width: AvailableSpace::Definite(viewport.width),
        height: AvailableSpace::Definite(viewport.height),
    };
    let computed =
        taffy.compute_layout_with_measure(taffy_root, available, |input, _node, context, style| {
            let Some(Some(text)) = context else {
                return taffy::compute_leaf_layout(
                    input,
                    style,
                    |_, _| 0.0,
                    |_, _| TaffySize::ZERO,
                );
            };
            let text = text.clone();
            taffy::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| {
                    if let (Some(width), Some(height)) = (known.width, known.height) {
                        return TaffySize { width, height };
                    }
                    // `MinContent` and `MaxContent` are the questions "how
                    // narrow can this be" and "how wide would it like to be";
                    // both are answered by measuring with no width at all.
                    let width_to_fit = known.width.or(match available.width {
                        AvailableSpace::Definite(definite) => Some(definite),
                        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                    });
                    let size = measure.measure(&text, width_to_fit);
                    TaffySize {
                        width: size.width,
                        height: size.height,
                    }
                },
            )
        });
    if computed.is_err() {
        // The tree was built by this file, so a failure here is this file
        // being wrong rather than the document. Recorded, and an empty layout
        // is returned rather than a partial one nobody can trust.
        issues.push(StyleIssue {
            kind: IssueKind::UnsupportedStructure,
            source: "the layout engine could not lay out this tree".to_owned(),
            at: Location { line: 0, column: 0 },
        });
        return LayoutTree::from_parts(BTreeMap::new(), issues, viewport);
    }

    let mut geometry = BTreeMap::new();
    read_back(
        &taffy,
        &ours_to_theirs,
        boxes,
        root,
        Point::ZERO,
        &mut geometry,
    );
    LayoutTree::from_parts(geometry, issues, viewport)
}

/// Build one box and everything under it.
fn build(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    taffy: &mut TaffyTree<Option<String>>,
    ours_to_theirs: &mut BTreeMap<BoxId, NodeId>,
    issues: &mut Vec<StyleIssue>,
) -> Option<NodeId> {
    let node = boxes.get(id)?;
    let style = style_for(boxes, styles, id, issues);

    let children: Vec<NodeId> = node
        .children
        .iter()
        .filter_map(|child| build(boxes, styles, *child, taffy, ours_to_theirs, issues))
        .collect();

    let made = match &node.kind {
        BoxKind::Text { text, .. } => taffy.new_leaf_with_context(style, Some(text.clone())),
        _ => taffy.new_with_children(style, &children),
    }
    .ok()?;
    ours_to_theirs.insert(id, made);
    Some(made)
}

/// The `taffy` style for one box.
fn style_for(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
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
        flex_basis: dimension(&ours.flex.basis, ours.metrics, issues),
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

    box_model_into(&ours, &mut style, issues);
    style.grid_template_rows = template(&ours.grid.template_rows, ours.metrics, issues);
    style.grid_template_columns = template(&ours.grid.template_columns, ours.metrics, issues);
    style.grid_auto_rows = auto_tracks(&ours.grid.auto_rows, ours.metrics, issues);
    style.grid_auto_columns = auto_tracks(&ours.grid.auto_columns, ours.metrics, issues);

    // The seam named in this file's header. A container whose children all sit
    // in a line *is* an inline formatting context — an anonymous box is one
    // that the box tree made for a run of them, and a `<p>` holding only text
    // is one that nobody had to make. Until item 6 brings a real inline
    // context, both are laid out as a row that wraps: boxes go side by side
    // and wrap, and text is as wide as it needs rather than as wide as its
    // parent.
    if needs_a_line_of_its_own(boxes, id) {
        style.display = taffy::Display::Flex;
        style.flex_direction = taffy::FlexDirection::Row;
        style.flex_wrap = taffy::FlexWrap::Wrap;
        style.align_items = Some(taffy::AlignItems::START);
        style.align_content = Some(taffy::AlignContent::START);
    }
    style
}

/// Whether this box holds several things that sit in a line, and so needs
/// them arranged along one.
///
/// The rule, stated because it is a stand-in and not the specification:
///
/// - **Several inline children** — `<a>All</a> <a>Due</a>` — become a wrapping
///   flex row, so they sit beside each other and wrap onto a new line.
/// - **One text child** — a paragraph, a heading, a label — is left as a block
///   child, so it fills the container and the measurer is asked to wrap it
///   inside that width. That is what a paragraph does, and it is the shape
///   most of an interface is.
///
/// Neither gets baselines, and neither breaks a line at the right place
/// between two different inline boxes. Queue item 6 replaces both with one
/// real inline formatting context.
fn needs_a_line_of_its_own(boxes: &BoxTree, id: BoxId) -> bool {
    let Some(node) = boxes.get(id) else {
        return false;
    };
    if !matches!(node.kind.inside(), Inside::Flow | Inside::FlowRoot) {
        return false;
    }
    if node.children.len() < 2 {
        return false;
    }
    node.children.iter().all(|child| {
        boxes
            .get(*child)
            .is_some_and(|child| child.kind.outside() == Outside::Inline)
    })
}

/// The sizes, the four edges and the gap — everything measured in lengths.
fn box_model_into(ours: &LayoutStyle, style: &mut Style, issues: &mut Vec<StyleIssue>) {
    let metrics = ours.metrics;
    style.inset = if ours.position == Positioning::Static {
        // A static box ignores its offsets, which is the whole difference
        // between `static` and `relative`.
        TaffyRect::auto()
    } else {
        TaffyRect {
            top: auto_length(&ours.inset.top, metrics, issues),
            right: auto_length(&ours.inset.right, metrics, issues),
            bottom: auto_length(&ours.inset.bottom, metrics, issues),
            left: auto_length(&ours.inset.left, metrics, issues),
        }
    };
    style.size = TaffySize {
        width: dimension(&ours.size.horizontal, metrics, issues),
        height: dimension(&ours.size.vertical, metrics, issues),
    };
    style.min_size = TaffySize {
        width: min_max(&ours.min_size.horizontal, metrics, issues),
        height: min_max(&ours.min_size.vertical, metrics, issues),
    };
    style.max_size = TaffySize {
        width: min_max(&ours.max_size.horizontal, metrics, issues),
        height: min_max(&ours.max_size.vertical, metrics, issues),
    };
    style.margin = TaffyRect {
        top: auto_length(&ours.margin.top, metrics, issues),
        right: auto_length(&ours.margin.right, metrics, issues),
        bottom: auto_length(&ours.margin.bottom, metrics, issues),
        left: auto_length(&ours.margin.left, metrics, issues),
    };
    style.padding = TaffyRect {
        top: length(&ours.padding.top, metrics, issues),
        right: length(&ours.padding.right, metrics, issues),
        bottom: length(&ours.padding.bottom, metrics, issues),
        left: length(&ours.padding.left, metrics, issues),
    };
    style.border = TaffyRect {
        top: length(&ours.border.top, metrics, issues),
        right: length(&ours.border.right, metrics, issues),
        bottom: length(&ours.border.bottom, metrics, issues),
        left: length(&ours.border.left, metrics, issues),
    };
    style.gap = TaffySize {
        width: length(&ours.gap.horizontal, metrics, issues),
        height: length(&ours.gap.vertical, metrics, issues),
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

/// A `calc()` with a percentage in it cannot be handed to the layout engine.
/// Recorded, and the property falls back to its initial value.
fn refuse_calc(value: &LengthPercentage, issues: &mut Vec<StyleIssue>) -> bool {
    let is_unexpressible = matches!(value, LengthPercentage::Calc(_)) && value.is_percentage();
    if is_unexpressible {
        issues.push(StyleIssue {
            kind: IssueKind::UnsupportedValue,
            source: format!("{value} — a calc() mixing percentages, see queue item 15"),
            at: Location { line: 0, column: 0 },
        });
    }
    is_unexpressible
}

/// A length or percentage, as `taffy` spells it.
fn length(
    value: &LengthPercentage,
    metrics: FontMetrics,
    issues: &mut Vec<StyleIssue>,
) -> TaffyLengthPercentage {
    if refuse_calc(value, issues) {
        return TaffyLengthPercentage::length(0.0);
    }
    match value {
        LengthPercentage::Percentage(percent) => TaffyLengthPercentage::percent(percent / 100.0),
        other => TaffyLengthPercentage::length(other.to_px(metrics, 0.0)),
    }
}

fn auto_length(
    value: &AutoLength,
    metrics: FontMetrics,
    issues: &mut Vec<StyleIssue>,
) -> LengthPercentageAuto {
    match value {
        AutoLength::Auto => LengthPercentageAuto::auto(),
        AutoLength::Length(inner) => {
            if refuse_calc(inner, issues) {
                return LengthPercentageAuto::auto();
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

fn dimension(value: &Sizing, metrics: FontMetrics, issues: &mut Vec<StyleIssue>) -> Dimension {
    match value {
        Sizing::Auto => Dimension::auto(),
        Sizing::MinContent => Dimension::min_content(),
        Sizing::MaxContent => Dimension::max_content(),
        Sizing::FitContent(limit) => {
            if refuse_calc(limit, issues) {
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
            if refuse_calc(inner, issues) {
                return Dimension::auto();
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
            if refuse_calc(inner, issues) {
                return LengthPercentageAuto::auto();
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
    issues: &mut Vec<StyleIssue>,
) -> Vec<GridTemplateComponent<String>> {
    list.entries
        .iter()
        .map(|entry| match entry {
            TrackListEntry::Single(track) => {
                GridTemplateComponent::Single(track_function(track, metrics, issues))
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
                        .map(|track| track_function(track, metrics, issues))
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
    issues: &mut Vec<StyleIssue>,
) -> Vec<TrackSizingFunction> {
    list.entries
        .iter()
        .filter_map(|entry| match entry {
            TrackListEntry::Single(track) => Some(track_function(track, metrics, issues)),
            // `grid-auto-rows` takes a list of track sizes, not repetitions.
            TrackListEntry::Repeat { .. } => None,
        })
        .collect()
}

fn track_function(
    track: &Track,
    metrics: FontMetrics,
    issues: &mut Vec<StyleIssue>,
) -> TrackSizingFunction {
    TrackSizingFunction {
        min: min_track(&track.min, metrics, issues),
        max: max_track(&track.max, metrics, issues),
    }
}

fn min_track(
    size: &TrackSize,
    metrics: FontMetrics,
    issues: &mut Vec<StyleIssue>,
) -> MinTrackSizingFunction {
    match size {
        // A fraction has no minimum of its own; `auto` is what CSS says a
        // `<flex>` minimum becomes, which is the same thing `auto` says.
        TrackSize::Auto | TrackSize::Fraction(_) => MinTrackSizingFunction::auto(),
        TrackSize::MinContent => MinTrackSizingFunction::min_content(),
        TrackSize::MaxContent => MinTrackSizingFunction::max_content(),
        TrackSize::Length(value) => {
            if refuse_calc(value, issues) {
                return MinTrackSizingFunction::auto();
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
    issues: &mut Vec<StyleIssue>,
) -> MaxTrackSizingFunction {
    match size {
        TrackSize::Auto => MaxTrackSizingFunction::auto(),
        TrackSize::MinContent => MaxTrackSizingFunction::min_content(),
        TrackSize::MaxContent => MaxTrackSizingFunction::max_content(),
        TrackSize::Fraction(share) => MaxTrackSizingFunction::fr(*share),
        TrackSize::Length(value) => {
            if refuse_calc(value, issues) {
                return MaxTrackSizingFunction::auto();
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
fn read_back(
    taffy: &TaffyTree<Option<String>>,
    ours_to_theirs: &BTreeMap<BoxId, NodeId>,
    boxes: &BoxTree,
    id: BoxId,
    parent_origin: Point,
    out: &mut BTreeMap<BoxId, BoxGeometry>,
) {
    let Some(theirs) = ours_to_theirs.get(&id) else {
        return;
    };
    let Ok(layout) = taffy.layout(*theirs) else {
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
        read_back(taffy, ours_to_theirs, boxes, child, origin, out);
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
