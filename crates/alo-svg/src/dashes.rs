/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `stroke-dasharray`, and how many dashes it would make.
//!
//! The list is SVG 2's: lengths separated by commas or spaces, each a number
//! in user units, a CSS length, or a percentage of the viewport's diagonal
//! over √2. An odd list is written out twice so that dash and gap alternate;
//! a list that adds up to nothing is a solid stroke; a negative length makes
//! the whole value an error, which is a solid stroke too.
//!
//! **A pattern is counted before it is cut** ([`count`]). The rented dasher
//! would lay a million dashes before giving up, and ADR 0022 § 5 asks for a
//! bound before the work, not inside it.

use crate::bounds::MOST_DASH_LENGTHS;
use crate::length::{Axis, Viewport, user_units};
use alo_paint::{Path, Point, Segment};
use alo_value::FontMetrics;

/// What a `stroke-dasharray` comes to.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// No dashes: `none`, a list that adds up to nothing, or one in error.
    Solid,
    /// Dash, gap, dash, gap…, always an even number of lengths and adding up
    /// to more than nothing.
    Dashed(Vec<f32>),
}

/// Why a `stroke-dasharray` was not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The value is in error, and the stroke is drawn solid.
    Invalid(&'static str),
    /// More lengths than [`MOST_DASH_LENGTHS`]: the drawing is refused.
    TooMany,
}

/// Read a `stroke-dasharray`.
///
/// # Errors
///
/// [`Refusal::Invalid`] for a length that is not one, is negative or is not
/// finite, and [`Refusal::TooMany`] past [`MOST_DASH_LENGTHS`], checked as the
/// list is read so that a million-entry value is not read whole first.
pub fn pattern(text: &str, viewport: Viewport, metrics: FontMetrics) -> Result<Pattern, Refusal> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("none") {
        return Ok(Pattern::Solid);
    }
    let mut lengths = Vec::new();
    for part in text.split(|c: char| c == ',' || c.is_ascii_whitespace()) {
        if part.is_empty() {
            continue;
        }
        if lengths.len() == MOST_DASH_LENGTHS {
            return Err(Refusal::TooMany);
        }
        let length = user_units(part, Axis::Other, viewport, metrics).map_err(Refusal::Invalid)?;
        if length < 0.0 {
            return Err(Refusal::Invalid("a negative length"));
        }
        lengths.push(length);
    }
    if lengths.is_empty() {
        return Err(Refusal::Invalid("no lengths"));
    }
    let sum: f64 = lengths.iter().copied().map(f64::from).sum();
    if !sum.is_finite() || sum <= 0.0 {
        return Ok(Pattern::Solid);
    }
    if lengths.len() % 2 == 1 {
        lengths.extend_from_within(..);
    }
    Ok(Pattern::Dashed(lengths))
}

/// At most how many dashes a pattern lays along a path.
///
/// Never fewer than it will lay: each subpath is measured along its control
/// points, which are never shorter than the curve through them, and its
/// pattern starts again, so each begins with one dash more than its length
/// alone would hold. Infinite for a path whose length overflows, which is
/// what such a path should be counted as.
pub fn count(path: &Path, lengths: &[f32]) -> f64 {
    let interval: f64 = lengths.iter().copied().map(f64::from).sum();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a dash count bounded far below 2^52 lengths"
    )]
    let per_interval = (lengths.len() / 2) as f64;
    let mut total = 0.0;
    for length in subpath_lengths(path) {
        total += (length / interval)
            .ceil()
            .mul_add(per_interval, per_interval);
    }
    total
}

/// How long each subpath is, along its control points.
fn subpath_lengths(path: &Path) -> Vec<f64> {
    let mut lengths = Vec::new();
    let mut start = Point::new(0.0, 0.0);
    let mut at = start;
    let mut length = 0.0_f64;
    let mut open = false;
    let step = |from: Point, to: Point| -> f64 {
        f64::from(to.x - from.x).hypot(f64::from(to.y - from.y))
    };
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) => {
                if open {
                    lengths.push(length);
                }
                (start, at, length, open) = (to, to, 0.0, true);
            }
            Segment::LineTo(to) => {
                length += step(at, to);
                at = to;
            }
            Segment::QuadTo(control, to) => {
                length += step(at, control) + step(control, to);
                at = to;
            }
            Segment::CubicTo(first, second, to) => {
                length += step(at, first) + step(first, second) + step(second, to);
                at = to;
            }
            Segment::Close => {
                length += step(at, start);
                at = start;
            }
        }
    }
    if open {
        lengths.push(length);
    }
    lengths
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Viewport = Viewport {
        width: 30.0,
        height: 40.0,
    };

    fn read(text: &str) -> Result<Pattern, Refusal> {
        pattern(
            text,
            VIEW,
            FontMetrics {
                font_size: 10.0,
                ..FontMetrics::default()
            },
        )
    }

    #[test]
    fn a_list_is_read_by_commas_or_spaces_and_an_odd_one_is_doubled() {
        assert_eq!(read("4 2"), Ok(Pattern::Dashed(vec![4.0, 2.0])));
        assert_eq!(
            read(" 4,2 , 1 "),
            Ok(Pattern::Dashed(vec![4.0, 2.0, 1.0, 4.0, 2.0, 1.0]))
        );
        assert_eq!(read("1em 10%"), Ok(Pattern::Dashed(vec![10.0, 3.535_534])));
        assert_eq!(read("3"), Ok(Pattern::Dashed(vec![3.0, 3.0])));
        assert_eq!(read("0 4"), Ok(Pattern::Dashed(vec![0.0, 4.0])), "dots");
    }

    #[test]
    fn nothing_and_a_list_of_nothing_are_solid() {
        assert_eq!(read("none"), Ok(Pattern::Solid));
        assert_eq!(read("NONE"), Ok(Pattern::Solid));
        assert_eq!(read("0 0 0"), Ok(Pattern::Solid));
        assert_eq!(
            read("3e38 3e38"),
            Ok(Pattern::Dashed(vec![3.0e38, 3.0e38])),
            "a sum past a float is still a sum, counted in doubles",
        );
    }

    #[test]
    fn a_length_in_error_makes_the_whole_list_an_error() {
        for text in ["4 -2", "4 dashes", "", ",", "1e99999 1", "inf", "NaN 2"] {
            assert!(matches!(read(text), Err(Refusal::Invalid(_))), "{text:?}");
        }
    }

    #[test]
    fn a_list_longer_than_the_bound_is_refused_and_one_at_it_is_not() {
        let at = vec!["1"; MOST_DASH_LENGTHS].join(" ");
        assert!(
            matches!(read(&at), Ok(Pattern::Dashed(lengths)) if lengths.len() == MOST_DASH_LENGTHS)
        );
        let past = vec!["1"; MOST_DASH_LENGTHS + 1].join(" ");
        assert_eq!(read(&past), Err(Refusal::TooMany));
        let enormous = "1,".repeat(1_000_000);
        assert_eq!(read(&enormous), Err(Refusal::TooMany));
    }

    fn line(length: f32) -> Path {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(length, 0.0));
        path
    }

    /// A count that is a whole number, as that number.
    fn whole(count: f64) -> Option<u32> {
        (count.fract() == 0.0 && (0.0..1.0e6).contains(&count)).then(|| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked above to be whole and between nothing and a million"
            )]
            let whole = count as u32;
            whole
        })
    }

    #[test]
    fn a_count_is_never_fewer_than_the_dashes_laid() {
        // Ten units of 2-on, 3-off is dashes at 0, 5: two, counted as three.
        assert_eq!(whole(count(&line(10.0), &[2.0, 3.0])), Some(3));
        // Two subpaths each start the pattern again.
        let mut two = line(10.0);
        two.move_to(Point::new(0.0, 5.0));
        two.line_to(Point::new(10.0, 5.0));
        assert_eq!(whole(count(&two, &[2.0, 3.0])), Some(6));
        // A closed square is measured all the way round.
        let square = Path::rectangle(0.0, 0.0, 10.0, 10.0);
        assert_eq!(whole(count(&square, &[5.0, 5.0])), Some(5));
        // Three dashes in each interval of a six-length pattern.
        assert_eq!(
            whole(count(&line(12.0), &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0])),
            Some(9)
        );
    }

    #[test]
    fn a_curve_is_measured_along_its_handles_which_is_never_short() {
        let mut quarter = Path::new();
        quarter.move_to(Point::new(10.0, 0.0));
        quarter.cubic_to(
            Point::new(10.0, 5.5),
            Point::new(5.5, 10.0),
            Point::new(0.0, 10.0),
        );
        let lengths = subpath_lengths(&quarter);
        let arc = core::f64::consts::FRAC_PI_2 * 10.0;
        assert!(lengths.iter().all(|length| *length >= arc), "{lengths:?}");
    }

    #[test]
    fn a_tiny_dash_along_a_long_path_counts_as_millions() {
        let counted = count(&line(10_000.0), &[0.0001, 0.0001]);
        assert!(counted > 1.0e7, "{counted}");
        let overflowing = count(&line(3.0e38), &[1.0e-38, 1.0e-38]);
        assert!(overflowing > 1.0e18, "{overflowing}");
    }
}
