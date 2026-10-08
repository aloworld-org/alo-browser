/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The font in force, in numbers.
//!
//! `em` is this element's font size and `rem` is the root's, so before any
//! length that uses them can become a number, the font size has to be one —
//! and a font size can itself be written in `em`, meaning the *parent's*. That
//! chain is what this resolves, once per element, on the way down the tree.
//!
//! It lives beside the cascade rather than in `alo-value` because it needs the
//! parent's answer, and the parent's answer is something only a tree walk has.

use alo_value::{FontMetrics, LengthPercentage, Viewport, parse_length_percentage, parse_number};

/// The font size a document has when nothing says otherwise.
///
/// Sixteen CSS pixels is what every browser uses and what `medium` means, and
/// every keyword size below is a ratio of it rather than a number somebody
/// picked.
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

/// The ratio each absolute-size keyword is of `medium`.
const KEYWORD_SIZES: &[(&str, f32)] = &[
    ("xx-small", 3.0 / 5.0),
    ("x-small", 3.0 / 4.0),
    ("small", 8.0 / 9.0),
    ("medium", 1.0),
    ("large", 6.0 / 5.0),
    ("x-large", 3.0 / 2.0),
    ("xx-large", 2.0),
    ("xxx-large", 3.0),
];

/// The step `smaller` and `larger` take.
const RELATIVE_STEP: f32 = 1.2;

/// What `line-height: normal` is worth, as a multiple of the font size.
///
/// The real answer comes from the font, and queue item 6 is where a font
/// arrives to be asked. Until then this is the ratio every engine falls back
/// to, written here rather than buried so that the day it is wrong it is
/// findable.
const NORMAL_LINE_HEIGHT: f32 = 1.2;

/// Work out the font size of an element from what its style says and what its
/// parent ended up with.
///
/// A font size written in `em` or as a percentage is relative to the
/// **parent's** font size, not to its own — which is the one rule here that
/// surprises people, and the reason this cannot be folded into the generic
/// length resolution.
pub fn resolve_font_size(
    specified: Option<&str>,
    parent: f32,
    root: f32,
    viewport: Option<Viewport>,
) -> f32 {
    font_size_against(
        specified,
        with_window(FontMetrics::estimated(parent, root), viewport),
    )
}

/// Work out the font size of an element against the parent's whole font.
///
/// [`resolve_font_size`] is this with a parent nobody measured. The cascade
/// has measured it, and `font-size: 2ex` is two of the **parent's** `x`, so
/// what it hands here carries the parent's face rather than half its size.
pub(crate) fn font_size_against(specified: Option<&str>, parent_font: FontMetrics) -> f32 {
    let parent = parent_font.font_size;
    let Some(text) = specified else {
        // Nothing said anything, so it is whatever was inherited.
        return parent;
    };
    let text = text.trim();

    for (keyword, ratio) in KEYWORD_SIZES {
        if text.eq_ignore_ascii_case(keyword) {
            return DEFAULT_FONT_SIZE * ratio;
        }
    }
    if text.eq_ignore_ascii_case("smaller") {
        return parent / RELATIVE_STEP;
    }
    if text.eq_ignore_ascii_case("larger") {
        return parent * RELATIVE_STEP;
    }

    // Relative lengths in a font size resolve against the parent's font, so
    // that is the font handed to the resolver — with the window as well:
    // `font-size: clamp(2.4rem, 4vw, 3.5rem)` is a real thing a design system
    // writes, and a font size resolved without a window would silently take
    // the smaller bound.
    match parse_length_percentage(text) {
        // A negative font size is not a font size.
        Some(value) => {
            let pixels = value.to_px(parent_font, parent);
            if pixels.is_finite() && pixels >= 0.0 {
                pixels
            } else {
                parent
            }
        }
        None => parent,
    }
}

/// Work out the line height of an element.
///
/// `line-height: 1.5` is a *number* and means one and a half times this
/// element's font size — and it inherits as the number, so a child with a
/// different font size gets a proportional line height. A length or a
/// percentage inherits as the computed length instead. The difference is real
/// and is why this takes the specified text rather than a number.
pub fn resolve_line_height(
    specified: Option<&str>,
    font_size: f32,
    root: f32,
    viewport: Option<Viewport>,
) -> f32 {
    set_line_height(specified, font_size, root, viewport).unwrap_or(font_size * NORMAL_LINE_HEIGHT)
}

/// The line height an element was given, or [`None`] for `normal`.
///
/// The two are not the same thing at the same number. `normal` leaves a line
/// as tall as its font says it is, while a line height that was set puts half
/// of the difference above the text and half below it. Layout has to know
/// which of them it was given. A value this engine cannot read is `normal`,
/// and so is one that comes out negative or not finite: `1e38` times a font
/// size is no line height, and handing layout an infinity would be a page
/// that cannot be laid out.
pub(crate) fn set_line_height(
    specified: Option<&str>,
    font_size: f32,
    root: f32,
    viewport: Option<Viewport>,
) -> Option<f32> {
    set_line_height_in(
        specified,
        with_window(FontMetrics::estimated(font_size, root), viewport),
    )
}

/// The line height an element was given, against its own measured font, or
/// [`None`] for `normal`.
///
/// [`set_line_height`] is this with a font nobody measured; the cascade has
/// measured it, so `line-height: 3ex` is three of this face's `x`.
pub(crate) fn set_line_height_in(specified: Option<&str>, font: FontMetrics) -> Option<f32> {
    let font_size = font.font_size;
    let text = specified?.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("normal") {
        return None;
    }
    let usable = |pixels: f32| pixels.is_finite() && pixels >= 0.0;
    if let Some(multiple) = parse_number(text) {
        return Some(font_size * multiple).filter(|pixels| usable(*pixels));
    }
    // A percentage line height is a percentage of the font size.
    parse_length_percentage(text)
        .map(|value| value.to_px(font, font_size))
        .filter(|pixels| usable(*pixels))
}

/// The line height an element ends up with, against its own measured font.
pub(crate) fn line_height_in(specified: Option<&str>, font: FontMetrics) -> f32 {
    set_line_height_in(specified, font).unwrap_or(font.font_size * NORMAL_LINE_HEIGHT)
}

/// The same metrics, in a window when there is one.
///
/// [`None`] rather than a default size: a viewport unit resolved against a
/// window nobody supplied is a guess, and `alo_value::FontMetrics` says so by
/// answering zero rather than a plausible number.
fn with_window(metrics: FontMetrics, viewport: Option<Viewport>) -> FontMetrics {
    match viewport {
        Some(viewport) => metrics.in_viewport(viewport),
        None => metrics,
    }
}

/// Read a length from a property's specified text.
///
/// [`None`] for a property that is absent, or whose value this engine cannot
/// read — both of which mean "at its initial value" to the caller.
pub fn length_of(specified: Option<&str>) -> Option<LengthPercentage> {
    parse_length_percentage(specified?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn a_media_querys_em_is_this_initial_size() {
        // Media Queries resolve `em` and `rem` against the initial font size,
        // so the two crates must agree on what that is.
        assert!(close(alo_css::media::QUERY_FONT_SIZE, DEFAULT_FONT_SIZE));
    }

    #[test]
    fn nothing_said_means_whatever_was_inherited() {
        assert!(close(resolve_font_size(None, 20.0, 16.0, None), 20.0));
        assert!(close(resolve_font_size(Some("  "), 20.0, 16.0, None), 20.0));
    }

    #[test]
    fn an_absolute_length_is_itself() {
        assert!(close(
            resolve_font_size(Some("24px"), 16.0, 16.0, None),
            24.0
        ));
        assert!(close(
            resolve_font_size(Some("12pt"), 16.0, 16.0, None),
            16.0
        ));
    }

    #[test]
    fn em_and_a_percentage_in_a_font_size_are_of_the_parents() {
        assert!(
            close(resolve_font_size(Some("2em"), 20.0, 16.0, None), 40.0),
            "two of the parent's, not two of its own",
        );
        assert!(close(
            resolve_font_size(Some("150%"), 20.0, 16.0, None),
            30.0
        ));
        assert!(close(
            resolve_font_size(Some("2rem"), 20.0, 16.0, None),
            32.0
        ));
    }

    #[test]
    fn the_keyword_sizes_are_ratios_of_sixteen_pixels() {
        assert!(close(
            resolve_font_size(Some("medium"), 99.0, 16.0, None),
            16.0
        ));
        assert!(close(
            resolve_font_size(Some("large"), 99.0, 16.0, None),
            19.2
        ));
        assert!(close(
            resolve_font_size(Some("xx-large"), 99.0, 16.0, None),
            32.0
        ));
        assert!(
            close(resolve_font_size(Some("MEDIUM"), 99.0, 16.0, None), 16.0),
            "however it is capitalised",
        );
    }

    #[test]
    fn smaller_and_larger_step_from_the_parent() {
        assert!(close(
            resolve_font_size(Some("larger"), 20.0, 16.0, None),
            24.0
        ));
        assert!(close(
            resolve_font_size(Some("smaller"), 24.0, 16.0, None),
            20.0
        ));
    }

    #[test]
    fn a_font_size_this_engine_cannot_read_leaves_the_inherited_one() {
        for text in ["auto", "banana", "50dvh", "calc(1px + 2)"] {
            assert!(
                close(resolve_font_size(Some(text), 20.0, 16.0, None), 20.0),
                "{text} should have changed nothing",
            );
        }
    }

    #[test]
    fn a_negative_font_size_is_not_a_font_size() {
        assert!(close(
            resolve_font_size(Some("-4px"), 20.0, 16.0, None),
            20.0
        ));
        assert!(close(
            resolve_font_size(Some("calc(4px - 8px)"), 20.0, 16.0, None),
            20.0
        ));
    }

    #[test]
    fn a_calc_font_size_is_resolved_against_the_parents() {
        assert!(close(
            resolve_font_size(Some("calc(1em + 4px)"), 20.0, 16.0, None),
            24.0,
        ));
    }

    #[test]
    fn a_line_height_number_is_a_multiple_of_this_elements_font_size() {
        assert!(close(
            resolve_line_height(Some("1.5"), 20.0, 16.0, None),
            30.0
        ));
        assert!(close(
            resolve_line_height(Some("2"), 10.0, 16.0, None),
            20.0
        ));
    }

    #[test]
    fn a_line_height_length_is_itself_and_a_percentage_is_of_the_font_size() {
        assert!(close(
            resolve_line_height(Some("24px"), 20.0, 16.0, None),
            24.0
        ));
        assert!(close(
            resolve_line_height(Some("150%"), 20.0, 16.0, None),
            30.0
        ));
        assert!(close(
            resolve_line_height(Some("1.5em"), 20.0, 16.0, None),
            30.0
        ));
    }

    #[test]
    fn normal_and_nothing_are_the_same_ratio_until_there_is_a_font_to_ask() {
        assert!(close(resolve_line_height(None, 20.0, 16.0, None), 24.0));
        assert!(close(
            resolve_line_height(Some("normal"), 20.0, 16.0, None),
            24.0
        ));
        assert!(close(
            resolve_line_height(Some("nonsense"), 20.0, 16.0, None),
            24.0
        ));
        assert!(close(
            resolve_line_height(Some("-2"), 20.0, 16.0, None),
            24.0
        ));
    }

    #[test]
    fn a_set_line_height_is_told_apart_from_normal() {
        assert_eq!(set_line_height(None, 20.0, 16.0, None), None);
        assert_eq!(set_line_height(Some(" NORMAL "), 20.0, 16.0, None), None);
        assert_eq!(set_line_height(Some("1.25"), 32.0, 16.0, None), Some(40.0));
        assert_eq!(set_line_height(Some("24px"), 20.0, 16.0, None), Some(24.0));
        assert_eq!(set_line_height(Some("0"), 20.0, 16.0, None), Some(0.0));
        // Hostile and unreadable values are all `normal`: nothing negative,
        // nothing infinite and nothing that is not a number reaches layout.
        for text in [
            "", "nonsense", "-2", "-4px", "1e38", "1e39px", "1e38em", "NaN", "inf", "2 3",
        ] {
            assert_eq!(
                set_line_height(Some(text), 20.0, 16.0, None),
                None,
                "{text}"
            );
            assert!(resolve_line_height(Some(text), 20.0, 16.0, None).is_finite());
        }
    }

    #[test]
    fn a_font_size_in_ex_or_ch_is_of_the_parents_measured_face() {
        let parent = FontMetrics {
            x_height: 11.0,
            zero_width: 12.5,
            ..FontMetrics::estimated(20.0, 16.0)
        };
        assert!(close(font_size_against(Some("2ex"), parent), 22.0));
        assert!(close(font_size_against(Some("2ch"), parent), 25.0));
        assert!(close(font_size_against(Some("2em"), parent), 40.0));
        // And without a measured parent it is still half an em each.
        assert!(close(
            resolve_font_size(Some("2ch"), 20.0, 16.0, None),
            20.0
        ));
    }

    #[test]
    fn a_line_height_in_ex_or_ch_is_of_this_elements_measured_face() {
        let font = FontMetrics {
            x_height: 11.0,
            zero_width: 12.5,
            ..FontMetrics::estimated(20.0, 16.0)
        };
        assert!(close(line_height_in(Some("3ex"), font), 33.0));
        assert!(close(line_height_in(Some("2ch"), font), 25.0));
        assert!(close(line_height_in(Some("1.5"), font), 30.0));
        assert!(close(line_height_in(None, font), 24.0));
        assert_eq!(set_line_height_in(Some("normal"), font), None);
    }

    #[test]
    fn a_length_is_read_from_a_property_or_reported_as_absent() {
        assert!(length_of(Some("16px")).is_some());
        assert!(length_of(Some("50%")).is_some());
        assert_eq!(length_of(None), None);
        assert_eq!(length_of(Some("auto")), None);
    }
}
