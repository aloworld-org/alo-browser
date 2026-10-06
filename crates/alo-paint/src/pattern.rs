/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the pieces of a patterned border side go: the dashes of `dashed`,
//! the dots of `dotted`, and the two lines of `double`.
//!
//! This file is the spacing alone — numbers along a side, measured from its
//! start — and [`crate::border`] turns them into shapes inside each side's
//! mitred wedge. A change to how far apart dots are is this file; a change to
//! which part of a corner a side owns is that one.
//!
//! CSS leaves the spacing to the browser. Ours:
//!
//! - **Dashes** are about three of the border's widths long, with gaps the
//!   same length, and a side **starts and ends on a dash**: the dashes and
//!   gaps are stretched or squeezed together until a whole number of them
//!   fits exactly. The first and last dash take the corners.
//! - **Dots** are round, as wide as the border, with about one width of space
//!   between them, and a side starts and ends on one: the first and last are
//!   centred half a width in from the side's ends, which is in the corner, and
//!   the rest are spaced evenly between.
//! - **`double`** is two lines a third of the width each, a third apart —
//!   which is how CSS's "the two lines and the space between them add up to
//!   the width" comes out when nothing else is said. A border thinner than
//!   three pixels is still split in thirds, and comes out as two faint lines,
//!   because that is what was asked for.
//!
//! # A bound on the count
//!
//! A border's width and a side's length both come from the page, so a page
//! could ask for a hair-thin dotted border round a box a million pixels
//! long. [`MAX_PIECES`] bounds how many pieces one side gets; past it the
//! pieces are spaced further apart rather than more of them being made. A
//! one-pixel dotted border reaches it only on a side longer than about
//! 32 000 pixels.

use alo_layout::Edges;

/// The most dashes or dots one side is drawn with.
pub const MAX_PIECES: u16 = 16_384;

/// How long a dash is, and how long a gap, in the border's widths, before
/// either is stretched to fit the side.
const DASH: f32 = 3.0;

/// Where the dashes go along a side `length` long, of a border `width` thick:
/// each as `(start, end)`, measured from the side's start.
///
/// Empty for a side with no length or no width, or with either not a number.
/// A side too short for two dashes and a gap is one dash the whole length,
/// which is what a dashed border on a very small box looks like.
pub fn dashes(length: f32, width: f32) -> Vec<(f32, f32)> {
    if !measurable(length, width) {
        return Vec::new();
    }
    // n dashes and n - 1 gaps, all one length: n = (length + gap) / (dash +
    // gap), to the nearest whole number.
    let count = ((length + DASH * width) / (2.0 * DASH * width))
        .round()
        .clamp(1.0, f32::from(MAX_PIECES));
    let unit = length / (2.0 * count - 1.0);
    pieces(count)
        .map(|index| (2.0 * index * unit, (2.0 * index + 1.0) * unit))
        .collect()
}

/// Where the dots' centres go along a side `length` long, of a border `width`
/// thick, measured from the side's start. Each dot is `width` across.
///
/// Empty for a side with no length or no width, or with either not a number.
/// A side too short for two dots is one dot, in its middle.
pub fn dots(length: f32, width: f32) -> Vec<f32> {
    if !measurable(length, width) {
        return Vec::new();
    }
    // The first and last centres are half a width in from the ends, and the
    // rest about two widths apart: one for the dot, one for the space.
    let span = length - width;
    let gaps = (span / (2.0 * width))
        .round()
        .min(f32::from(MAX_PIECES) - 1.0);
    if gaps < 1.0 {
        return vec![length / 2.0];
    }
    let step = span / gaps;
    pieces(gaps + 1.0)
        .map(|index| width / 2.0 + index * step)
        .collect()
}

/// A third of a border's width on every side: how thick each of a `double`
/// border's lines is, and the space between them.
pub fn third(widths: Edges) -> Edges {
    Edges {
        top: widths.top / 3.0,
        right: widths.right / 3.0,
        bottom: widths.bottom / 3.0,
        left: widths.left / 3.0,
    }
}

/// Whether a side can be measured at all.
fn measurable(length: f32, width: f32) -> bool {
    length.is_finite() && width.is_finite() && length > 0.0 && width > 0.0
}

/// `0, 1, 2, …` up to but not including `count`, never more than
/// [`MAX_PIECES`] of them.
fn pieces(count: f32) -> impl Iterator<Item = f32> {
    (0..MAX_PIECES)
        .map(f32::from)
        .take_while(move |index| *index < count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn a_dashed_side_starts_and_ends_on_a_dash() {
        // 100 long, 2 wide: (100 + 6) / 12 rounds to 9 dashes and 8 gaps,
        // seventeen pieces of 100 / 17 each.
        let found = dashes(100.0, 2.0);
        assert_eq!(found.len(), 9);
        let unit = 100.0 / 17.0;
        assert!(
            found
                .first()
                .is_some_and(|(start, end)| near(*start, 0.0) && near(*end, unit))
        );
        assert!(
            found
                .last()
                .is_some_and(|(start, end)| near(*start, 100.0 - unit) && near(*end, 100.0))
        );
        // Every dash and every gap the same length.
        for ((start, end), (next, _)) in found.iter().zip(found.iter().skip(1)) {
            assert!(
                near(end - start, unit) && near(next - end, unit),
                "{start} {next}"
            );
        }
    }

    #[test]
    fn a_dash_is_about_three_widths_long() {
        for (length, width) in [(300.0, 1.0), (240.0, 4.0), (1000.0, 7.0)] {
            let found = dashes(length, width);
            let (start, end) = found.first().copied().unwrap_or_default();
            let ratio = (end - start) / width;
            assert!((2.0..=4.5).contains(&ratio), "{length} {width}: {ratio}");
        }
    }

    #[test]
    fn a_short_dashed_side_is_one_dash() {
        assert_eq!(dashes(10.0, 2.0), vec![(0.0, 10.0)]);
    }

    #[test]
    fn dots_are_centred_in_the_corners_and_spaced_evenly() {
        // 42 long, 2 wide: the centres run from 1 to 41, a span of 40, in
        // ten gaps of 4.
        let found = dots(42.0, 2.0);
        assert_eq!(found.len(), 11);
        for (index, centre) in found.iter().enumerate() {
            let expected = 1.0 + 4.0 * f32::from(u8::try_from(index).unwrap_or(0));
            assert!(near(*centre, expected), "{index}: {centre}");
        }
    }

    #[test]
    fn dots_never_overlap() {
        for length in 1..200_u8 {
            for width in [1.0, 2.5, 4.0, 9.0] {
                let found = dots(f32::from(length), width);
                for (first, second) in found.iter().zip(found.iter().skip(1)) {
                    assert!(
                        second - first >= width - 0.001,
                        "{length} {width}: {first} {second}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_short_dotted_side_is_one_dot_in_its_middle() {
        assert_eq!(dots(4.0, 3.0), vec![2.0]);
    }

    #[test]
    fn nothing_is_measured_on_a_side_with_no_length_or_width() {
        for (length, width) in [
            (0.0, 2.0),
            (10.0, 0.0),
            (-5.0, 2.0),
            (f32::NAN, 2.0),
            (10.0, f32::INFINITY),
        ] {
            assert!(dashes(length, width).is_empty(), "{length} {width}");
            assert!(dots(length, width).is_empty(), "{length} {width}");
        }
    }

    #[test]
    fn a_page_cannot_ask_for_more_than_the_bound() {
        // A hair-thin border round a box far longer than any page.
        assert_eq!(dashes(1.0e30, 0.001).len(), usize::from(MAX_PIECES));
        assert_eq!(dots(1.0e30, 0.001).len(), usize::from(MAX_PIECES));
        assert_eq!(dots(1.0e9, 0.01).len(), usize::from(MAX_PIECES));
        // And the pieces are spaced out to the end of the side rather than
        // stopping short of it.
        let last = dashes(1.0e9, 0.01).last().map(|(_, end)| *end);
        assert!(
            last.is_some_and(|end| (end - 1.0e9).abs() < 1.0e3),
            "{last:?}"
        );
    }

    #[test]
    fn a_double_border_is_split_in_thirds() {
        let found = third(Edges {
            top: 3.0,
            right: 6.0,
            bottom: 9.0,
            left: 1.5,
        });
        assert_eq!(
            (found.top, found.right, found.bottom, found.left),
            (1.0, 2.0, 3.0, 0.5)
        );
    }
}
