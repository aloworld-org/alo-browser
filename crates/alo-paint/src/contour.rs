/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A rounded rectangle as one closed line, measured the way Chrome measures
//! it, and the points along it between two distances, with which way is out
//! at each.
//!
//! Where a dash of a rounded border falls depends on how long the line round
//! the box is taken to be, and on which point of a curve is a given distance
//! along it. Chrome 154 takes both from a coarse measure: each corner is a
//! quarter-ellipse drawn as a **conic** (a quadratic whose middle point is
//! weighted by √½, which is exactly an ellipse), halved until the middle of
//! each piece is within **half a device pixel** of its chord, and its length
//! is the sum of the chords. A distance that falls inside a chord is found
//! on the curve by going as far through the chord's share of the curve's
//! parameter as through its length. So a corner a few device pixels across
//! is measured a little short, and its dashes sit a little off where an
//! exact arc length would put them — by half a pixel at one on a 10 px
//! border — and less so the larger the screen's scale. Measured on dashed
//! and dotted borders at one, two and eight against this rule and against
//! an exact one, not read from its source; this is the rule that fitted.
//!
//! Lengths and positions are kept in `f32` throughout, as the measure is.

use core::cmp::Ordering;

use crate::corner::Corners;
use crate::path::Point;
use alo_layout::Rect;

/// The weight that makes a conic a quarter of an ellipse: √½.
const QUARTER: f32 = core::f32::consts::FRAC_1_SQRT_2;

/// How fine the parameter of a curve is cut: one less than 2³⁰.
const FINEST: u32 = 0x3FFF_FFFF;

/// How far, in device pixels, the middle of a piece of curve may be from
/// its chord before it is halved again.
const TOLERANCE: f32 = 0.5;

/// How short, in device pixels, a part of the line is to be left out of a
/// piece cut from it: a 4096th. A sliver of curve that short has no
/// direction worth the name, and a square-ended stroke that kept it would
/// turn a sharp corner on it.
const TEENY: f32 = 1.0 / 4096.0;

/// How far a curve turns, in degrees, between one [`Station`] and the next.
const STEP: f32 = 1.0;

/// A point on the line, and the way out of the shape it goes round there:
/// the unit normal on the outside of a clockwise line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Station {
    /// Where.
    pub point: Point,
    /// Which way is out, one unit long.
    pub normal: (f32, f32),
}

/// One piece of the line: straight, or a quarter-ellipse as a conic.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    Line(Point, Point),
    Conic(Point, Point, Point),
}

/// Where the measure has reached: the length so far, at the end of a chord
/// that ends on this piece at this point of its parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mark {
    distance: f32,
    piece: usize,
    t: f32,
}

/// A closed line round a rounded rectangle, measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pieces: Vec<Piece>,
    marks: Vec<Mark>,
    /// [`TEENY`] in the line's own units.
    teeny: f32,
}

impl Contour {
    /// The line round `rect` with `corners`, clockwise from where the
    /// top-left corner's curve meets the top, measured for a screen of
    /// `scale` device pixels to the unit.
    pub fn round(rect: Rect, corners: Corners, scale: f32) -> Self {
        let corners = corners.fitted_to(rect.size);
        let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
        let (tl, tr, br, bl) = (
            corners.top_left,
            corners.top_right,
            corners.bottom_right,
            corners.bottom_left,
        );
        let at = Point::new;
        let pieces = vec![
            Piece::Line(at(left + tl.0, top), at(right - tr.0, top)),
            Piece::Conic(at(right - tr.0, top), at(right, top), at(right, top + tr.1)),
            Piece::Line(at(right, top + tr.1), at(right, bottom - br.1)),
            Piece::Conic(
                at(right, bottom - br.1),
                at(right, bottom),
                at(right - br.0, bottom),
            ),
            Piece::Line(at(right - br.0, bottom), at(left + bl.0, bottom)),
            Piece::Conic(
                at(left + bl.0, bottom),
                at(left, bottom),
                at(left, bottom - bl.1),
            ),
            Piece::Line(at(left, bottom - bl.1), at(left, top + tl.1)),
            Piece::Conic(at(left, top + tl.1), at(left, top), at(left + tl.0, top)),
        ];
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let tolerance = TOLERANCE / scale;
        let mut marks = Vec::new();
        let mut distance = 0.0;
        for (index, piece) in pieces.iter().enumerate() {
            match *piece {
                Piece::Line(from, to) => {
                    let next = distance + length(from, to);
                    if next > distance {
                        distance = next;
                        marks.push(Mark {
                            distance,
                            piece: index,
                            t: 1.0,
                        });
                    }
                }
                Piece::Conic(from, control, to) => {
                    let conic = (from, control, to);
                    distance = chords(
                        conic,
                        (0, from),
                        (FINEST, to),
                        tolerance,
                        Chords {
                            piece: index,
                            distance,
                            marks: &mut marks,
                        },
                    );
                }
            }
        }
        Self {
            pieces,
            marks,
            teeny: TEENY / scale,
        }
    }

    /// How long the line is, as measured.
    pub fn length(&self) -> f32 {
        self.marks.last().map_or(0.0, |mark| mark.distance)
    }

    /// The point `distance` along the line, or [`None`] past its ends.
    pub fn point_at(&self, distance: f32) -> Option<Point> {
        let (piece, t) = self.find(distance)?;
        Some(eval(*self.pieces.get(piece)?, t))
    }

    /// The part of the line from `from` to `to` along it, as [`Station`]s:
    /// its ends exactly, a curve in steps of no more than [`STEP`] of its
    /// turn, and a square corner as two stations on its point, one facing
    /// each way. A part shorter than [`TEENY`] is left out, so a sliver of
    /// curve with no direction worth the name never turns a corner.
    ///
    /// Clamped to the line's ends; empty when nothing of it is between.
    pub fn stations(&self, from: f32, to: f32) -> Vec<Station> {
        let mut stations = Vec::new();
        let (from, to) = (from.max(0.0), to.min(self.length()));
        if from.partial_cmp(&to).is_none_or(Ordering::is_gt) {
            return stations;
        }
        let (Some((first, start)), Some((last, stop))) = (self.find(from), self.find(to)) else {
            return stations;
        };
        let Some(begin) = self.pieces.get(first).map(|piece| eval(*piece, start)) else {
            return stations;
        };
        let mut at = begin;
        for index in first..=last {
            let (t0, t1) = (
                if index == first { start } else { 0.0 },
                if index == last { stop } else { 1.0 },
            );
            self.part(index, t0, t1, &mut at, &mut stations);
        }
        stations
    }

    /// The piece a distance falls on and the point of its parameter there:
    /// as far between the chord's ends as the distance is along it.
    fn find(&self, distance: f32) -> Option<(usize, f32)> {
        if !(0.0..=self.length()).contains(&distance) {
            return None;
        }
        let index = self
            .marks
            .iter()
            .position(|mark| mark.distance >= distance)?;
        let mark = self.marks.get(index)?;
        let (start_distance, start_t) = match index.checked_sub(1).and_then(|i| self.marks.get(i)) {
            Some(before) if before.piece == mark.piece => (before.distance, before.t),
            Some(before) => (before.distance, 0.0),
            None => (0.0, 0.0),
        };
        let span = mark.distance - start_distance;
        let share = if span > 0.0 {
            (distance - start_distance) / span
        } else {
            0.0
        };
        Some((mark.piece, start_t + (mark.t - start_t) * share))
    }

    /// One piece from `start` to `stop` of its parameter, onto `stations`,
    /// which have reached `at`; nothing when it ends no further than
    /// [`TEENY`] from `at`, or than the rounding of numbers that size.
    fn part(
        &self,
        index: usize,
        start: f32,
        stop: f32,
        at: &mut Point,
        stations: &mut Vec<Station>,
    ) {
        let Some(piece) = self.pieces.get(index).copied() else {
            return;
        };
        let end = eval(piece, stop);
        // A few of `f32`'s steps at this far from the origin, too: a dash
        // that starts where a curve ends is found on the curve or on the
        // line after it by the last bit of a sum.
        let steps = 4.0 * f32::EPSILON * at.x.abs().max(at.y.abs());
        if start >= stop || length(*at, end) <= self.teeny.max(steps) {
            return;
        }
        *at = end;
        // A quarter-ellipse turns a right angle over its parameter.
        let count = match piece {
            Piece::Line(..) => 1,
            Piece::Conic(..) => {
                let turn = (stop - start) * 90.0 / STEP;
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a count between 1 and 90 / STEP"
                )]
                let count = turn.ceil().clamp(1.0, 90.0 / STEP) as u16;
                count
            }
        };
        for step in 0..=count {
            let t = start + (stop - start) * f32::from(step) / f32::from(count);
            let point = eval(piece, t);
            if let Some(normal) = outward(piece, t) {
                stations.push(Station { point, normal });
            }
        }
    }
}

/// What a curve's chords are added to.
struct Chords<'a> {
    piece: usize,
    distance: f32,
    marks: &'a mut Vec<Mark>,
}

/// A quarter-ellipse's chords from parameter `from` to `to`, halved while
/// the curve's middle is further than `tolerance` from the chord's, in
/// either direction; the length reached.
fn chords(
    conic: (Point, Point, Point),
    from: (u32, Point),
    to: (u32, Point),
    tolerance: f32,
    mut into: Chords<'_>,
) -> f32 {
    let half = from.0.midpoint(to.0);
    let middle = eval(Piece::Conic(conic.0, conic.1, conic.2), parameter(half));
    let off = (middle.x - (from.1.x + to.1.x) * 0.5)
        .abs()
        .max((middle.y - (from.1.y + to.1.y) * 0.5).abs());
    if (to.0 - from.0) >> 10 != 0 && off > tolerance && middle.x.is_finite() && middle.y.is_finite()
    {
        let reached = chords(
            conic,
            from,
            (half, middle),
            tolerance,
            Chords {
                piece: into.piece,
                distance: into.distance,
                marks: into.marks,
            },
        );
        into.distance = reached;
        return chords(conic, (half, middle), to, tolerance, into);
    }
    let next = into.distance + length(from.1, to.1);
    if next > into.distance {
        into.marks.push(Mark {
            distance: next,
            piece: into.piece,
            t: parameter(to.0),
        });
        return next;
    }
    into.distance
}

/// A point of a curve's parameter, from its finest whole step.
fn parameter(step: u32) -> f32 {
    // Exact in f64; the measure's own steps are then rounded once to f32.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a share between 0 and 1, which f32 holds"
    )]
    let share = (f64::from(step) / f64::from(FINEST)) as f32;
    share
}

/// The three weights a conic's points are mixed in at `share` of its
/// parameter — the first, the middle (weighted by [`QUARTER`]) and the last
/// — and how fast each changes there.
fn weights(share: f32) -> ([f32; 3], [f32; 3]) {
    let rest = 1.0 - share;
    (
        [rest * rest, 2.0 * rest * share * QUARTER, share * share],
        [
            -2.0 * rest,
            2.0 * QUARTER * (1.0 - 2.0 * share),
            2.0 * share,
        ],
    )
}

/// Where a piece is at `share` of its parameter.
fn eval(piece: Piece, share: f32) -> Point {
    match piece {
        Piece::Line(from, to) => Point::new(
            from.x + (to.x - from.x) * share,
            from.y + (to.y - from.y) * share,
        ),
        Piece::Conic(from, control, to) => {
            let ([first, middle, last], _) = weights(share);
            let weight = first + middle + last;
            Point::new(
                (first * from.x + middle * control.x + last * to.x) / weight,
                (first * from.y + middle * control.y + last * to.y) / weight,
            )
        }
    }
}

/// The unit normal on the outside of a piece at `share` of its parameter:
/// its direction turned a right angle anticlockwise on the screen, which for
/// a line going clockwise round a shape is out of it. [`None`] where the
/// piece has no direction, as a square corner's curve of no size has none.
fn outward(piece: Piece, share: f32) -> Option<(f32, f32)> {
    let (dx, dy) = match piece {
        Piece::Line(from, to) => (to.x - from.x, to.y - from.y),
        Piece::Conic(from, control, to) => {
            // The quotient rule on N / D, with N the weighted points and D
            // the weights; only the direction is wanted, so not over D².
            let (mix, change) = weights(share);
            let weight: f32 = mix.iter().sum();
            let slope: f32 = change.iter().sum();
            let along = |points: [f32; 3]| {
                let mixed: f32 = mix.iter().zip(points).map(|(k, p)| k * p).sum();
                let changed: f32 = change.iter().zip(points).map(|(k, p)| k * p).sum();
                changed * weight - mixed * slope
            };
            (
                along([from.x, control.x, to.x]),
                along([from.y, control.y, to.y]),
            )
        }
    };
    let size = dx.hypot(dy);
    (size > 0.0 && size.is_finite()).then(|| (dy / size, -dx / size))
}

fn length(from: Point, to: Point) -> f32 {
    (to.x - from.x).hypot(to.y - from.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, within: f32) -> bool {
        (a - b).abs() <= within
    }

    /// The centre line of a 100 × 60 box with a 2 px border and corners of
    /// 16: 98 × 58 from (131, 11), corners of 15.
    fn thin() -> Rect {
        Rect::new(131.0, 11.0, 98.0, 58.0)
    }

    #[test]
    fn a_corner_is_measured_short_by_its_chords_and_less_so_on_a_finer_screen() {
        // 2 × 68 + 2 × 28 straight, and four quarters of a circle of 15.
        let exact = 2.0 * 68.0 + 2.0 * 28.0 + 2.0 * core::f32::consts::PI * 15.0;
        let at = |scale| Contour::round(thin(), Corners::all(15.0), scale).length();
        let (one, two, eight) = (at(1.0), at(2.0), at(8.0));
        assert!(
            one < two && two < eight && eight < exact,
            "{one} {two} {eight} {exact}"
        );
        // Each corner a few chords at one: 0.6 short all round.
        assert!(near(exact - one, 0.607, 0.01), "{one}");
        assert!(near(exact - eight, 0.038, 0.01), "{eight}");
    }

    #[test]
    fn the_line_starts_where_the_top_left_curve_meets_the_top_and_goes_clockwise() {
        let line = Contour::round(thin(), Corners::all(15.0), 1.0);
        let start = line.point_at(0.0);
        assert!(start.is_some_and(|at| near(at.x, 146.0, 1e-4) && near(at.y, 11.0, 1e-4)));
        let along = line.point_at(10.0);
        assert!(along.is_some_and(|at| near(at.x, 156.0, 1e-4) && near(at.y, 11.0, 1e-4)));
        let end = line.point_at(line.length());
        assert!(end.is_some_and(|at| near(at.x, 146.0, 1e-3) && near(at.y, 11.0, 1e-3)));
        assert_eq!(line.point_at(-1.0), None);
        assert_eq!(line.point_at(line.length() + 1.0), None);
    }

    #[test]
    fn a_straight_part_is_its_two_ends_facing_out() {
        let line = Contour::round(thin(), Corners::all(15.0), 1.0);
        let found = line.stations(2.0, 12.0);
        assert_eq!(found.len(), 2, "{found:?}");
        let ends: Vec<(f32, f32)> = found.iter().map(|at| (at.point.x, at.point.y)).collect();
        assert!(
            near(ends[0].0, 148.0, 1e-4) && near(ends[1].0, 158.0, 1e-4),
            "{ends:?}"
        );
        // Out of the top is up the screen.
        assert!(found.iter().all(|at| at.normal == (0.0, -1.0)), "{found:?}");
    }

    #[test]
    fn a_curve_is_stepped_a_degree_at_a_time_and_turns_its_normal_with_it() {
        let line = Contour::round(thin(), Corners::all(15.0), 8.0);
        // From the end of the top to the start of the right: the top-right
        // corner, whole.
        let top = 68.0;
        let corner = line.stations(top, top + core::f32::consts::FRAC_PI_2 * 15.0);
        assert!(corner.len() >= 91, "{}", corner.len());
        let (Some(first), Some(last)) = (corner.first(), corner.last()) else {
            panic!("the corner has stations");
        };
        assert!(near(first.normal.0, 0.0, 1e-3) && near(first.normal.1, -1.0, 1e-3));
        assert!(near(last.normal.0, 1.0, 1e-2) && near(last.normal.1, 0.0, 1e-1));
        for (one, next) in corner.iter().zip(corner.iter().skip(1)) {
            assert!(near(one.normal.0.hypot(one.normal.1), 1.0, 1e-5));
            // No more than about a degree between two stations.
            let turn = (one.normal.0 * next.normal.0 + one.normal.1 * next.normal.1).acos();
            assert!(turn < 1.2_f32.to_radians(), "{turn}");
        }
    }

    #[test]
    fn a_square_corner_is_two_stations_on_one_point_facing_each_way() {
        let corners = Corners {
            top_right: (0.0, 0.0),
            ..Corners::all(10.0)
        };
        let line = Contour::round(Rect::new(0.0, 0.0, 50.0, 40.0), corners, 1.0);
        // The top runs from 10 to the corner at 50, 40 long.
        let found = line.stations(35.0, 45.0);
        let at_corner: Vec<&Station> = found
            .iter()
            .filter(|at| near(at.point.x, 50.0, 1e-4) && near(at.point.y, 0.0, 1e-4))
            .collect();
        assert_eq!(at_corner.len(), 2, "{found:?}");
        assert_eq!(at_corner[0].normal, (0.0, -1.0));
        assert_eq!(at_corner[1].normal, (1.0, 0.0));
    }

    #[test]
    fn a_sliver_of_curve_is_left_out() {
        // At eight, a dash of the 1 px border round a 100 × 60 box starts
        // on the last bit of the bottom-right curve: what is left of the
        // curve is shorter than a 4096th of a device pixel, and the first
        // station is on the bottom, facing down.
        let line = Contour::round(Rect::new(370.5, 170.5, 99.0, 59.0), Corners::all(15.5), 8.0);
        let found = line.stations(144.674_99, 146.674_99);
        assert!(
            found
                .first()
                .is_some_and(|at| at.normal == (0.0, 1.0) && near(at.point.x, 454.0, 1e-3)),
            "{found:?}"
        );
    }

    #[test]
    fn nothing_is_measured_round_a_rectangle_that_is_not_a_number() {
        let line = Contour::round(Rect::new(f32::NAN, 0.0, 10.0, 10.0), Corners::all(2.0), 1.0);
        assert!(near(line.length(), 0.0, 0.0));
        assert!(line.stations(0.0, 5.0).is_empty());
        // A side of no end is measured, and every station on it faces a
        // way that is a number.
        let line = Contour::round(
            Rect::new(0.0, 0.0, f32::INFINITY, 10.0),
            Corners::all(2.0),
            1.0,
        );
        assert!(
            line.stations(0.0, 5.0)
                .iter()
                .all(|at| { at.normal.0.is_finite() && at.normal.1.is_finite() })
        );
        let line = Contour::round(Rect::new(0.0, 0.0, 0.0, 0.0), Corners::all(2.0), 1.0);
        assert!(near(line.length(), 0.0, 0.0));
        assert!(line.stations(0.0, 1.0).is_empty());
        assert_eq!(line.point_at(0.0), None);
    }
}
