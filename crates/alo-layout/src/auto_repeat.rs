/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! **Delete this file the day `taffy` counts a calculated track.** It exists
//! because one condition upstream is narrower than CSS: `taffy` 0.14's
//! `compute_explicit_grid_size_in_axis` accepts an `auto-fill` or `auto-fit`
//! repetition only when every track `has_fixed_component`, which asks
//! `is_length_or_percentage()` of each end — a tag test a `calc()` handle
//! fails — and discards the whole template when it does not. ADR 0047.
//!
//! CSS Grid 1 § 7.2.3 allows the track: a `<fixed-breadth>` is any
//! `<length-percentage>`, `min()` included, and alo Sites' blog grid is
//! `repeat(auto-fit, minmax(min(100%, 18rem), 1fr))`. So, where the grid's
//! size in an axis is definite — by `taffy`'s own rule for it, its
//! `outer_node_size` — every calculated track size in that axis's template
//! is resolved against the content box and handed to `taffy` as the plain
//! length it is. That is the one case in which the basis `taffy` counts the
//! repetitions against and the basis it sizes percentage tracks against are
//! the same number, so resolving once is exact; Chrome 154, measured,
//! behaves exactly so (ADR 0047, cases A–D). Where the size is not definite
//! the template is left as written, `taffy` still discards it, and the axis
//! is said to be refused so that the engine can record it (cases E–G, and
//! queue item 424).
//!
//! The agreement tests in `tests/auto_repeat.rs` compare a lowered grid with
//! `taffy`'s handling of the plain-length grid it is equal to. They still
//! pass with this file deleted once `taffy` accepts the calculation, which is
//! how an upgrade that makes it unnecessary is noticed.
//!
//! **The node's style is never changed.** This makes a copy, for one
//! `compute_grid_layout` call, and `crate::arena` holds it for exactly that
//! long. The copy is a function of the call's inputs, and the inputs are
//! what `taffy`'s cache is keyed by.

use taffy::{
    BoxSizing, GridTemplateComponent, LayoutInput, MaxTrackSizingFunction, MaybeMath, MaybeResolve,
    MinTrackSizingFunction, Overflow, ResolveOrZero, Size, SizingMode, Style, TrackSizingFunction,
};

/// One of a grid's two axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Axis {
    /// `grid-template-columns`, against the content box's width.
    Columns,
    /// `grid-template-rows`, against the content box's height.
    Rows,
}

/// What a grid is to be laid out with, for one call.
#[derive(Debug, Default)]
pub(crate) struct Lowering {
    /// A copy of the style with its calculated tracks resolved, or `None`
    /// when the node's own style is what `taffy` should read.
    pub(crate) style: Option<Style>,
    /// The axes whose template holds an auto repetition and a calculated
    /// track, in which the grid's size is not definite: `taffy` discards
    /// those templates.
    pub(crate) refused: Vec<Axis>,
}

/// The style to lay a grid out with, given the inputs of this call.
///
/// `resolve` is the arena's own answer to a `calc()` handle — the same
/// function `taffy` reaches through `resolve_calc_value` — so a track
/// resolved here and the same track resolved by `taffy` on a template it
/// accepts cannot disagree.
pub(crate) fn lower(
    style: &Style,
    inputs: &LayoutInput,
    resolve: impl Fn(*const (), f32) -> f32,
) -> Lowering {
    let wanted: Vec<Axis> = [Axis::Columns, Axis::Rows]
        .into_iter()
        .filter(|axis| counts_a_calculation(template(style, *axis)))
        .collect();
    // Nearly every grid: nothing to copy, nothing to say.
    if wanted.is_empty() {
        return Lowering::default();
    }
    let content = content_box(style, inputs, &resolve);
    let mut lowering = Lowering::default();
    for axis in wanted {
        let basis = match axis {
            Axis::Columns => content.width,
            Axis::Rows => content.height,
        };
        let Some(basis) = basis else {
            lowering.refused.push(axis);
            continue;
        };
        let copy = lowering.style.get_or_insert_with(|| style.clone());
        let tracks = match axis {
            Axis::Columns => &mut copy.grid_template_columns,
            Axis::Rows => &mut copy.grid_template_rows,
        };
        for component in tracks.iter_mut() {
            match component {
                GridTemplateComponent::Single(track) => {
                    *track = resolved(*track, basis, &resolve);
                }
                GridTemplateComponent::Repeat(repetition) => {
                    for track in &mut repetition.tracks {
                        *track = resolved(*track, basis, &resolve);
                    }
                }
            }
        }
    }
    lowering
}

fn template(style: &Style, axis: Axis) -> &[GridTemplateComponent<String>] {
    match axis {
        Axis::Columns => &style.grid_template_columns,
        Axis::Rows => &style.grid_template_rows,
    }
}

/// Whether `taffy` would refuse this template for a calculation alone: it
/// holds an `auto-fill` or `auto-fit` repetition, and a track size anywhere
/// in it is a `calc()` handle. `taffy` asks the whole template to be fixed,
/// not only the repeated part (ADR 0047 § 2).
fn counts_a_calculation(template: &[GridTemplateComponent<String>]) -> bool {
    let repeats_to_fit = template
        .iter()
        .any(GridTemplateComponent::is_auto_repetition);
    repeats_to_fit
        && template.iter().any(|component| match component {
            GridTemplateComponent::Single(track) => is_calculated(*track),
            GridTemplateComponent::Repeat(repetition) => {
                repetition.tracks.iter().any(|track| is_calculated(*track))
            }
        })
}

fn is_calculated(track: TrackSizingFunction) -> bool {
    track.min.into_raw().is_calc() || track.max.into_raw().is_calc()
}

/// A track with each calculated end replaced by the length it comes to.
/// Every other end — a length, a percentage, `auto`, a keyword, a fraction —
/// is left exactly as written. Nothing is clamped: a negative result is the
/// number `resolve_calc_value` would have given `taffy` too (ADR 0047 § 2).
fn resolved(
    track: TrackSizingFunction,
    basis: f32,
    resolve: &impl Fn(*const (), f32) -> f32,
) -> TrackSizingFunction {
    let min = track.min.into_raw();
    let max = track.max.into_raw();
    TrackSizingFunction {
        min: if min.is_calc() {
            MinTrackSizingFunction::length(resolve(min.calc_value(), basis))
        } else {
            track.min
        },
        max: if max.is_calc() {
            MaxTrackSizingFunction::length(resolve(max.calc_value(), basis))
        } else {
            track.max
        },
    }
}

/// The grid's content box in each axis where its size is definite, by the
/// rule `compute_grid_layout` uses for `outer_node_size`, written out here
/// because `taffy` does not expose it: the size the caller fixed, or else,
/// when the call asks for the inherent size, the style's own `width` or
/// `height` against the parent, adjusted for `box-sizing`; clamped by the
/// minimum and maximum; never less than the padding and border; less the
/// padding and border.
///
/// What `taffy` would fold in and this does not is refused rather than
/// approximated: an aspect ratio on the grid, in both axes, and a scrollbar
/// gutter, in the axis it narrows. Neither is reachable today — this engine
/// gives a grid an aspect ratio only from the style, and every scrollbar a
/// width of zero — and a page that reaches either opens an item.
fn content_box(
    style: &Style,
    inputs: &LayoutInput,
    resolve: &impl Fn(*const (), f32) -> f32,
) -> Size<Option<f32>> {
    if style.aspect_ratio.is_some() {
        return Size::NONE;
    }
    let parent = inputs.parent_size;
    let padding = style.padding.resolve_or_zero(parent.width, resolve);
    let border = style.border.resolve_or_zero(parent.width, resolve);
    let inset = padding + border;
    let inset_size = inset.sum_axes();
    let adjustment = match style.box_sizing {
        BoxSizing::ContentBox => inset_size,
        BoxSizing::BorderBox => Size::ZERO,
    };
    let min = style
        .min_size
        .maybe_resolve(parent, resolve)
        .maybe_add(adjustment);
    let max = style
        .max_size
        .maybe_resolve(parent, resolve)
        .maybe_add(adjustment);
    let preferred = match inputs.sizing_mode {
        SizingMode::InherentSize => style
            .size
            .maybe_resolve(parent, resolve)
            .maybe_add(adjustment),
        SizingMode::ContentSize => Size::NONE,
    };
    let outer = inputs
        .known_dimensions
        .or(preferred)
        .maybe_clamp(min, max)
        .maybe_max(inset_size);
    // A node that scrolls down reserves a gutter across, and the other way
    // round: `taffy` transposes `overflow` for it.
    let gutter = |overflow: Overflow| overflow == Overflow::Scroll && style.scrollbar_width != 0.0;
    Size {
        width: outer
            .width
            .filter(|_| !gutter(style.overflow.y))
            .map(|width| width - inset.horizontal_axis_sum()),
        height: outer
            .height
            .filter(|_| !gutter(style.overflow.x))
            .map(|height| height - inset.vertical_axis_sum()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::{
        AvailableSpace, Dimension, GridTemplateRepetition, LengthPercentage, LengthPercentageAuto,
        Line, Point, Rect, RepetitionCount, RequestedAxis, RunMode,
    };

    /// The handle every test hands out, and what it resolves to: the
    /// `min(100%, 288px)` of alo Sites' blog grid.
    const HANDLE: *const () = 8 as *const ();

    fn smaller_of_all_or_288(handle: *const (), basis: f32) -> f32 {
        if handle == HANDLE {
            basis.min(288.0)
        } else {
            f32::NAN
        }
    }

    /// `repeat(auto-fit, minmax(<calc>, 1fr))`.
    fn the_blog_template() -> Vec<GridTemplateComponent<String>> {
        vec![GridTemplateComponent::Repeat(GridTemplateRepetition {
            count: RepetitionCount::AutoFit,
            tracks: vec![TrackSizingFunction {
                min: MinTrackSizingFunction::calc(HANDLE),
                max: MaxTrackSizingFunction::fr(1.0),
            }],
            line_names: Vec::new(),
        })]
    }

    fn grid(width: Dimension) -> Style {
        Style {
            display: taffy::Display::Grid,
            size: Size {
                width,
                height: Dimension::auto(),
            },
            grid_template_columns: the_blog_template(),
            ..Style::default()
        }
    }

    fn inputs(known_width: Option<f32>) -> LayoutInput {
        LayoutInput {
            run_mode: RunMode::PerformLayout,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: Size {
                width: known_width,
                height: None,
            },
            known_dimensions_are_definite: Size {
                width: true,
                height: true,
            },
            parent_size: Size {
                width: Some(800.0),
                height: Some(600.0),
            },
            available_space: Size {
                width: AvailableSpace::Definite(800.0),
                height: AvailableSpace::Definite(600.0),
            },
            vertical_margins_are_collapsible: Line::FALSE,
        }
    }

    fn lowered_min(lowering: &Lowering) -> Option<f32> {
        let style = lowering.style.as_ref()?;
        match style.grid_template_columns.first()? {
            GridTemplateComponent::Repeat(repetition) => {
                let min = repetition.tracks.first()?.min.into_raw();
                (!min.is_calc()).then(|| min.value())
            }
            GridTemplateComponent::Single(_) => None,
        }
    }

    #[test]
    fn a_grid_of_a_definite_width_has_its_calculation_resolved_against_its_content_box() {
        let lowering = lower(
            &grid(Dimension::length(760.0)),
            &inputs(None),
            smaller_of_all_or_288,
        );
        assert_eq!(lowered_min(&lowering), Some(288.0));
        assert!(lowering.refused.is_empty());
        let narrow = lower(
            &grid(Dimension::length(200.0)),
            &inputs(None),
            smaller_of_all_or_288,
        );
        assert_eq!(
            lowered_min(&narrow),
            Some(200.0),
            "the min() takes the 100 %"
        );
    }

    #[test]
    fn a_width_the_caller_fixed_is_definite_whatever_the_style_says() {
        let lowering = lower(
            &grid(Dimension::auto()),
            &inputs(Some(250.0)),
            smaller_of_all_or_288,
        );
        assert_eq!(lowered_min(&lowering), Some(250.0));
    }

    #[test]
    fn the_basis_is_the_content_box_after_padding_and_border() {
        // Case D: 800 border-box, padding 20, border 3 — a content box 754.
        let mut style = grid(Dimension::length(800.0));
        style.box_sizing = BoxSizing::BorderBox;
        style.padding = Rect {
            left: LengthPercentage::length(20.0),
            right: LengthPercentage::length(20.0),
            top: LengthPercentage::length(20.0),
            bottom: LengthPercentage::length(20.0),
        };
        style.border = Rect {
            left: LengthPercentage::length(3.0),
            right: LengthPercentage::length(3.0),
            top: LengthPercentage::length(3.0),
            bottom: LengthPercentage::length(3.0),
        };
        let wide = |handle: *const (), basis: f32| if handle == HANDLE { basis } else { 0.0 };
        assert_eq!(
            lowered_min(&lower(&style, &inputs(None), wide)),
            Some(754.0)
        );
        style.box_sizing = BoxSizing::ContentBox;
        assert_eq!(
            lowered_min(&lower(&style, &inputs(None), wide)),
            Some(800.0),
            "content-box: the width is the content box"
        );
    }

    #[test]
    fn a_grid_with_no_definite_width_is_refused_and_left_as_written() {
        let mut style = grid(Dimension::auto());
        let lowering = lower(&style, &inputs(None), smaller_of_all_or_288);
        assert!(lowering.style.is_none());
        assert_eq!(lowering.refused, vec![Axis::Columns]);
        // Only a maximum, or only a minimum: counted against one number and
        // sized against another (ADR 0047 cases F and G).
        style.max_size.width = LengthPercentageAuto::length(700.0);
        assert_eq!(
            lower(&style, &inputs(None), smaller_of_all_or_288).refused,
            vec![Axis::Columns]
        );
        style.max_size.width = LengthPercentageAuto::auto();
        style.min_size.width = LengthPercentageAuto::length(700.0);
        assert_eq!(
            lower(&style, &inputs(None), smaller_of_all_or_288).refused,
            vec![Axis::Columns]
        );
    }

    #[test]
    fn a_width_asked_for_as_content_is_not_the_styles() {
        let mut asked = inputs(None);
        asked.sizing_mode = SizingMode::ContentSize;
        let lowering = lower(
            &grid(Dimension::length(760.0)),
            &asked,
            smaller_of_all_or_288,
        );
        assert_eq!(lowering.refused, vec![Axis::Columns]);
    }

    #[test]
    fn an_aspect_ratio_or_a_scrollbar_gutter_is_refused_rather_than_approximated() {
        let mut style = grid(Dimension::length(760.0));
        style.aspect_ratio = Some(2.0);
        assert_eq!(
            lower(&style, &inputs(None), smaller_of_all_or_288).refused,
            vec![Axis::Columns]
        );
        let mut style = grid(Dimension::length(760.0));
        style.overflow = Point {
            x: Overflow::Visible,
            y: Overflow::Scroll,
        };
        style.scrollbar_width = 15.0;
        assert_eq!(
            lower(&style, &inputs(None), smaller_of_all_or_288).refused,
            vec![Axis::Columns]
        );
        style.scrollbar_width = 0.0;
        assert_eq!(
            lowered_min(&lower(&style, &inputs(None), smaller_of_all_or_288)),
            Some(288.0),
            "a gutter of nothing narrows nothing"
        );
    }

    #[test]
    fn a_template_without_an_auto_repetition_is_left_to_taffy() {
        let mut style = grid(Dimension::length(760.0));
        style.grid_template_columns = vec![GridTemplateComponent::Single(TrackSizingFunction {
            min: MinTrackSizingFunction::calc(HANDLE),
            max: MaxTrackSizingFunction::fr(1.0),
        })];
        let lowering = lower(&style, &inputs(None), smaller_of_all_or_288);
        assert!(lowering.style.is_none() && lowering.refused.is_empty());
    }

    #[test]
    fn an_auto_repetition_of_plain_lengths_is_left_to_taffy() {
        let mut style = grid(Dimension::auto());
        style.grid_template_columns = vec![GridTemplateComponent::Repeat(GridTemplateRepetition {
            count: RepetitionCount::AutoFill,
            tracks: vec![TrackSizingFunction {
                min: MinTrackSizingFunction::length(288.0),
                max: MaxTrackSizingFunction::fr(1.0),
            }],
            line_names: Vec::new(),
        })];
        let lowering = lower(&style, &inputs(None), smaller_of_all_or_288);
        assert!(lowering.style.is_none() && lowering.refused.is_empty());
    }

    #[test]
    fn every_calculated_end_is_lowered_and_nothing_else_is() {
        let mut style = grid(Dimension::length(760.0));
        style.grid_template_columns = vec![
            GridTemplateComponent::Single(TrackSizingFunction {
                min: MinTrackSizingFunction::auto(),
                max: MaxTrackSizingFunction::calc(HANDLE),
            }),
            the_blog_template().remove(0),
            GridTemplateComponent::Single(TrackSizingFunction {
                min: MinTrackSizingFunction::percent(0.1),
                max: MaxTrackSizingFunction::max_content(),
            }),
        ];
        let lowering = lower(&style, &inputs(None), smaller_of_all_or_288);
        let Some(lowered) = lowering.style else {
            panic!("a definite width lowers");
        };
        let [
            GridTemplateComponent::Single(first),
            GridTemplateComponent::Repeat(repeated),
            GridTemplateComponent::Single(last),
        ] = lowered.grid_template_columns.as_slice()
        else {
            panic!(
                "the template keeps its shape: {:?}",
                lowered.grid_template_columns
            );
        };
        assert_eq!(first.min, MinTrackSizingFunction::auto());
        assert_eq!(first.max, MaxTrackSizingFunction::length(288.0));
        assert_eq!(repeated.count, RepetitionCount::AutoFit);
        assert_eq!(
            repeated.tracks[0].min,
            MinTrackSizingFunction::length(288.0)
        );
        assert_eq!(repeated.tracks[0].max, MaxTrackSizingFunction::fr(1.0));
        assert_eq!(last.min, MinTrackSizingFunction::percent(0.1));
        assert_eq!(last.max, MaxTrackSizingFunction::max_content());
    }

    #[test]
    fn rows_are_decided_apart_from_columns() {
        let mut style = grid(Dimension::length(760.0));
        style.grid_template_rows = the_blog_template();
        let lowering = lower(&style, &inputs(None), smaller_of_all_or_288);
        assert_eq!(lowering.refused, vec![Axis::Rows], "the height is auto");
        assert_eq!(lowered_min(&lowering), Some(288.0));
        style.size.height = Dimension::length(100.0);
        let both = lower(&style, &inputs(None), smaller_of_all_or_288);
        assert!(both.refused.is_empty());
    }

    #[test]
    fn a_template_of_many_calculated_tracks_is_lowered_without_a_panic() {
        let mut style = grid(Dimension::length(760.0));
        style.grid_template_columns = (0..10_000)
            .map(|_| the_blog_template().remove(0))
            .chain(core::iter::repeat_n(
                GridTemplateComponent::Single(TrackSizingFunction {
                    min: MinTrackSizingFunction::calc(HANDLE),
                    max: MaxTrackSizingFunction::calc(HANDLE),
                }),
                10_000,
            ))
            .collect();
        let lowering = lower(&style, &inputs(None), |_, _| f32::MAX);
        assert_eq!(
            lowering
                .style
                .map(|lowered| lowered.grid_template_columns.len()),
            Some(20_000)
        );
    }
}
