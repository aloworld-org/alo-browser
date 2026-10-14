/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A dashed or dotted border with rounded corners: its pieces laid along the
//! line through the middle of the border, all the way round the box.
//!
//! Chrome 154 does not place a rounded border's dashes or dots side by side,
//! as [`crate::pattern`] does for a square one. It draws them along the
//! **centre line** — the border box's shape brought in by half of each
//! side's width — as one closed line, starting where the top-left corner's
//! curve meets the top side and going clockwise, so a dash or a dot bends
//! round a corner with the curve. Measured at one, two and eight on dashed
//! and dotted borders 1 to 10 px wide, of one width and of two, not read
//! from its source:
//!
//! - **A dash is twice the side's width, and a gap about one width** —
//!   three widths and two on a side under three device pixels wide. The
//!   gap is stretched or squeezed so that a whole number of dashes and gaps
//!   goes round the line exactly: of the two counts nearest one width a
//!   gap, the one whose gap is nearer it ([`spacing`]). A line no longer
//!   than two dashes is drawn whole.
//! - **A dot is a round dot as wide as the side**, spaced the same way with
//!   the dot's own width as both dash and gap, less a hundredth of a device
//!   pixel so the last dot is never lost to rounding. A side no more than
//!   three device pixels wide has square dots instead: dashes as long as the
//!   side is wide, a width apart, not fitted to the line.
//! - **The pattern starts at the line's start, but its first dash is drawn
//!   last**, joined to the last one when that reaches the end, so a dash cut
//!   by the end of the line carries on past its start unbroken.
//! - **Each side draws the whole line in its own pattern**, and keeps the
//!   part on its own side of the mitres ([`crate::mitre`]), inside the ring.
//!   So where sides differ in width, a corner holds the end of one side's
//!   pattern and the start of the other's, cut on the mitre. A dash is drawn
//!   2.2 times as wide as the widest of its side and the two beside it, so
//!   that whatever of the ring is on its side of the mitre is covered.
//!
//! The line, its length and where along it a distance falls are
//! [`crate::contour`]'s, which measures it as coarsely as Chrome does. The
//! pieces are made here as the shape they cover — a dash as the sweep of
//! its normals ([`crate::sweep`]), a dot as a circle — and filled like any
//! other shape.
//!
//! # A bound on the count
//!
//! The line's length and the width are the page's, so a page could ask for
//! a hair-thin dotted border round a huge box. As for a square border
//! ([`pattern::MAX_PIECES`]), past that many pieces they are spaced further
//! apart rather than more of them being made.

use crate::contour::{Contour, Station};
use crate::corner::{Corners, rounded_rectangle};
use crate::path::Path;
use crate::pattern;
use crate::sweep::swept;
use alo_layout::{Edges, Rect};

/// How much wider than the widest side beside it a dash is drawn, so that
/// it covers its whole share of the ring once clipped to it.
const OVERREACH: f32 = 2.2;

/// How much shorter than a dot and its gap the step between dots is, in
/// device pixels, so that the last one lands inside the line's end rather
/// than past it.
const DOT_SLACK: f32 = 0.01;

/// How wide a side is, in device pixels, below which its dashes are longer
/// for their width, and at or below which its dots are square.
const THIN: f32 = 3.0;

/// The line through the middle of a border, measured for a screen at
/// `scale`: the border box brought in by half of each side's width, its
/// corners' radii less the same.
pub fn centre_line(outer: Rect, corners: Corners, widths: Edges, scale: f32) -> Contour {
    let half = Edges {
        top: widths.top / 2.0,
        right: widths.right / 2.0,
        bottom: widths.bottom / 2.0,
        left: widths.left / 2.0,
    };
    Contour::round(outer.shrunk_by(half), corners.inside(half), scale)
}

/// The shape a dashed side `width` wide covers along `line`, on a screen at
/// `scale`, drawn [`OVERREACH`] times `reach` wide.
///
/// [`None`] when nothing is drawn: a line or a width that is not a positive
/// finite number.
pub fn dashes(line: &Contour, width: f32, reach: f32, scale: f32) -> Option<Path> {
    let (dash, gap) = asked(width, scale);
    let length = line.length();
    if !measurable(length, dash) {
        return None;
    }
    if length <= 2.0 * dash {
        return across(&[line.stations(0.0, length)], reach);
    }
    let (dash, gap) = spacing(length, dash, gap)?;
    across(&laid(line, dash, gap), reach)
}

/// The dash and the gap a dashed side `width` wide asks for on a screen at
/// `scale`, before they are fitted to its line: two widths and one, or
/// three and two under [`THIN`] device pixels.
fn asked(width: f32, scale: f32) -> (f32, f32) {
    if width * scale < THIN {
        (3.0 * width, 2.0 * width)
    } else {
        (2.0 * width, width)
    }
}

/// The shape a dotted side `width` wide covers along `line`, on a screen at
/// `scale`; a thin side's square dots are drawn [`OVERREACH`] times `reach`
/// wide, as dashes are.
///
/// [`None`] when nothing is drawn, as for [`dashes`].
pub fn dots(line: &Contour, width: f32, reach: f32, scale: f32) -> Option<Path> {
    let length = line.length();
    if !measurable(length, width) {
        return None;
    }
    if width * scale <= THIN {
        let gap = spread(length, width, width);
        return across(&laid(line, width, gap), reach);
    }
    let (dot, gap) = spacing(length, width, width)?;
    let step = (dot + gap - DOT_SLACK / scale).max(DOT_SLACK / scale);
    let radius = width / 2.0;
    let mut path = Path::new();
    let mut at = 0.0;
    for _ in 0..=pattern::MAX_PIECES {
        if at >= length {
            break;
        }
        if let Some(centre) = line.point_at(at) {
            path.extend(&rounded_rectangle(
                Rect::new(centre.x - radius, centre.y - radius, width, width),
                Corners::all(radius),
            ));
        }
        at += step;
    }
    (!path.is_empty()).then_some(path)
}

/// The dashes `dash` long and `gap` apart round `line`, from its start,
/// each as its stations.
///
/// The first dash is laid last: joined on to the dash before it when that
/// one ran to the line's end, and a dash of its own otherwise.
fn laid(line: &Contour, dash: f32, gap: f32) -> Vec<Vec<Station>> {
    let length = line.length();
    let mut pieces: Vec<Vec<Station>> = Vec::new();
    let mut at = 0.0;
    let mut ended_on = false;
    for index in 0..=2 * u32::from(pattern::MAX_PIECES) + 1 {
        if at >= length {
            break;
        }
        let on = index % 2 == 0;
        ended_on = on && index > 0;
        if ended_on {
            pieces.push(line.stations(at, at + dash));
        }
        at += if on { dash } else { gap };
    }
    let first = line.stations(0.0, dash);
    match pieces.last_mut() {
        Some(last) if ended_on => last.extend(first),
        _ => pieces.push(first),
    }
    pieces
}

/// Dashes along the line, square-ended and wide enough to cover a side
/// `reach` wide and its corners.
fn across(pieces: &[Vec<Station>], reach: f32) -> Option<Path> {
    let half = OVERREACH * reach / 2.0;
    let mut path = Path::new();
    for piece in pieces {
        path.extend(&swept(piece, half));
    }
    (!path.is_empty()).then_some(path)
}

/// A dash and a gap that go round a closed line `length` long a whole number
/// of times, from a dash and a gap asked for.
///
/// Of the two counts nearest the asked-for gap — as many as fit whole, and
/// one more — the one whose gap comes out nearer it; never a gap below
/// nothing. Past [`pattern::MAX_PIECES`] the gap is what spreads that many
/// round the line.
///
/// [`None`] for a length or a dash that is not a positive finite number.
pub fn spacing(length: f32, dash: f32, gap: f32) -> Option<(f32, f32)> {
    if !measurable(length, dash) {
        return None;
    }
    let most = f32::from(pattern::MAX_PIECES);
    let fewer = (length / (dash + gap)).floor().max(1.0);
    if fewer >= most {
        return Some((dash, spread(length, dash, gap)));
    }
    let more = fewer + 1.0;
    let fewer_gap = (length - fewer * dash) / fewer;
    let more_gap = (length - more * dash) / more;
    let chosen = if more_gap <= 0.0 || (fewer_gap - gap).abs() < (more_gap - gap).abs() {
        fewer_gap
    } else {
        more_gap
    };
    Some((dash, chosen.max(0.0)))
}

/// `gap`, or as much more as keeps a line `length` long to
/// [`pattern::MAX_PIECES`] dashes.
fn spread(length: f32, dash: f32, gap: f32) -> f32 {
    gap.max(length / f32::from(pattern::MAX_PIECES) - dash)
}

/// Whether a line this long can be patterned with pieces this long.
fn measurable(length: f32, piece: f32) -> bool {
    length.is_finite() && piece.is_finite() && length > 0.0 && piece > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, within: f32) -> bool {
        (a - b).abs() <= within
    }

    /// A 100 × 60 box at (10, 10) with a border `width` wide all round and
    /// corners of 16.
    fn line_of(width: f32, scale: f32) -> Contour {
        centre_line(
            Rect::new(10.0, 10.0, 100.0, 60.0),
            Corners::all(16.0),
            Edges::all(width),
            scale,
        )
    }

    /// Where each piece starts and ends, as points.
    fn ends(pieces: &[Vec<Station>]) -> Vec<((f32, f32), (f32, f32))> {
        pieces
            .iter()
            .filter_map(|piece| Some((piece.first()?.point, piece.last()?.point)))
            .map(|(a, b)| ((a.x, a.y), (b.x, b.y)))
            .collect()
    }

    #[test]
    fn of_the_two_counts_nearest_the_gap_asked_for_the_nearer_is_taken() {
        // 280 round: 23 dashes of 8 leave gaps of 4.17, 24 leave 3.67.
        let found = spacing(280.0, 8.0, 4.0);
        assert!(
            found.is_some_and(|(dash, gap)| near(dash, 8.0, 0.0) && near(gap, 96.0 / 23.0, 1e-4))
        );
        // 290 round: 24 leave 4.08 and 25 leave 3.6; and 299: 24 leave
        // 4.46, 25 leave 3.96, which is nearer.
        assert!(spacing(290.0, 8.0, 4.0).is_some_and(|(_, gap)| near(gap, 98.0 / 24.0, 1e-4)));
        assert!(spacing(299.0, 8.0, 4.0).is_some_and(|(_, gap)| near(gap, 99.0 / 25.0, 1e-4)));
        // Never a gap below nothing.
        assert!(spacing(9.0, 8.0, 4.0).is_some_and(|(_, gap)| near(gap, 1.0, 1e-4)));
    }

    #[test]
    fn a_dash_starts_where_the_top_left_curve_meets_the_top_and_its_first_is_laid_last() {
        // Chrome 154's 4 px dashed border round this box, at eight: dashes
        // from 26, 38.125 and 50.375 along the top, each 8 long.
        let line = line_of(4.0, 1.0);
        let Some((dash, gap)) = spacing(line.length(), 8.0, 4.0) else {
            panic!("the line is long enough to pattern");
        };
        let pieces = laid(&line, dash, gap);
        assert_eq!(pieces.len(), 23);
        let found = ends(&pieces);
        // Laid last, as a piece of its own: the gap before it ends the line.
        assert!(
            found
                .last()
                .is_some_and(|(from, to)| near(from.0, 26.0, 1e-3)
                    && near(to.0, 34.0, 1e-3)
                    && near(from.1, 12.0, 1e-3))
        );
        let starts: Vec<f32> = found.iter().take(2).map(|(from, _)| from.0).collect();
        assert!(
            near(starts[0], 38.125, 0.125) && near(starts[1], 50.375, 0.125),
            "{starts:?}"
        );
    }

    #[test]
    fn under_three_device_pixels_a_dash_is_three_widths_and_a_gap_two() {
        // Chrome 154: a 2 px border's dashes are 6 and 4 at one, 4 and 2 at
        // two; a 1.5 px border's at two, three device pixels, 3 and 1.5.
        assert_eq!(asked(2.0, 1.0), (6.0, 4.0));
        assert_eq!(asked(2.0, 2.0), (4.0, 2.0));
        assert_eq!(asked(1.5, 2.0), (3.0, 1.5));
        assert_eq!(asked(1.0, 2.0), (3.0, 2.0));
    }

    #[test]
    fn dots_of_three_device_pixels_or_fewer_are_square() {
        let curves = |shape: &Path| {
            shape
                .segments()
                .iter()
                .any(|segment| matches!(segment, crate::path::Segment::CubicTo(..)))
        };
        let line = line_of(1.5, 2.0);
        assert!(dots(&line, 1.5, 1.5, 2.0).is_some_and(|shape| !curves(&shape)));
        let line = line_of(2.0, 2.0);
        assert!(dots(&line, 2.0, 2.0, 2.0).is_some_and(|shape| curves(&shape)));
    }

    #[test]
    fn square_dots_are_not_fitted_to_the_line() {
        // A 2 px dotted border at one: dots 2 long and 2 apart all the way
        // round, whatever is left over before the start.
        let line = line_of(2.0, 1.0);
        let gap = spread(line.length(), 2.0, 2.0);
        assert!(near(gap, 2.0, 0.0), "{gap}");
        let pieces = laid(&line, 2.0, gap);
        let found = ends(&pieces);
        // From 26 along the top, 4 apart.
        for (index, (from, _)) in found.iter().take(10).enumerate() {
            let expected = 30.0 + 4.0 * f32::from(u8::try_from(index).unwrap_or(0));
            assert!(near(from.0, expected, 1e-3), "{index}: {from:?}");
        }
    }

    #[test]
    fn round_dots_are_fitted_and_start_at_the_line_start() {
        let line = line_of(4.0, 1.0);
        let Some(shape) = dots(&line, 4.0, 4.0, 1.0) else {
            panic!("dots are drawn");
        };
        // One circle per dot: 35 of them, each a move and four curves.
        let circles = shape
            .segments()
            .iter()
            .filter(|segment| matches!(segment, crate::path::Segment::MoveTo(_)))
            .count();
        let Some((dot, gap)) = spacing(line.length(), 4.0, 4.0) else {
            panic!("the line is long enough to pattern");
        };
        let expected = (line.length() / (dot + gap - DOT_SLACK)).ceil();
        assert!(near(
            f32::from(u16::try_from(circles).unwrap_or(0)),
            expected,
            0.0
        ));
        assert!(
            shape
                .bounds()
                .is_some_and(|(left, top, ..)| top < 10.5 && left < 13.0)
        );
    }

    #[test]
    fn a_line_no_longer_than_two_dashes_is_drawn_whole() {
        // A 22 px box with a 10 px border and corners of 11: the line is a
        // circle of 6 round, 37.7 long, and two dashes are 40.
        let line = centre_line(
            Rect::new(0.0, 0.0, 22.0, 22.0),
            Corners::all(11.0),
            Edges::all(10.0),
            1.0,
        );
        assert!(line.length() < 40.0);
        let Some(shape) = dashes(&line, 10.0, 10.0, 1.0) else {
            panic!("the side is drawn");
        };
        let Some((left, top, right, bottom)) = shape.bounds() else {
            panic!("the shape has bounds");
        };
        // Swept 11 either side of the circle all the way round.
        assert!(left < -5.0 && top < -5.0 && right > 27.0 && bottom > 27.0);
    }

    #[test]
    fn a_page_cannot_ask_for_more_pieces_than_the_bound() {
        let line = centre_line(
            Rect::new(0.0, 0.0, 1.0e6, 1.0e6),
            Corners::all(10.0),
            Edges::all(0.001),
            1.0,
        );
        assert!(
            laid(&line, 0.001, spread(line.length(), 0.001, 0.001)).len()
                <= usize::from(pattern::MAX_PIECES) + 1
        );
        assert!(
            spacing(line.length(), 0.002, 0.001).is_some_and(|(dash, gap)| {
                line.length() / (dash + gap) <= f32::from(pattern::MAX_PIECES) + 1.0
            })
        );
    }

    #[test]
    fn nothing_is_drawn_with_no_width_or_one_that_is_not_a_number() {
        let line = line_of(4.0, 1.0);
        for width in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert!(dashes(&line, width, 4.0, 1.0).is_none(), "{width}");
            assert!(dots(&line, width, 4.0, 1.0).is_none(), "{width}");
        }
        assert_eq!(spacing(f32::NAN, 8.0, 4.0), None);
        assert_eq!(spacing(100.0, 0.0, 4.0), None);
    }
}
