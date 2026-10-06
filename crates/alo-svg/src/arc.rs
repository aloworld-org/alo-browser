/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An elliptical arc, as cubic curves.
//!
//! Path data writes an arc the way a person thinks of drawing one — from here
//! to there, on an ellipse of these radii turned by this much, the long way or
//! the short way round, clockwise or not — and paint draws only lines and
//! curves. SVG 2's implementation notes give the conversion, and this file is
//! those notes in order:
//!
//! - **F.6.2, out-of-range parameters.** An arc that ends where it starts is
//!   not drawn at all; a zero radius makes it a straight line; a negative
//!   radius is its absolute value; and radii too small to reach from one end
//!   to the other are scaled up, keeping their ratio, until they just do.
//! - **F.6.5, endpoint to centre.** Where the ellipse's centre is, and the
//!   angles the arc starts at and sweeps through.
//! - Then the sweep is cut into pieces of at most a quarter turn, each one
//!   cubic curve whose handles are `4/3 · tan(θ/4)` of the way along the
//!   tangent — the standard approximation, within 0.03% of the true ellipse
//!   at a quarter turn and closer for anything shorter.
//!
//! The arithmetic is in `f64`, because squaring an `f32` coordinate can
//! overflow an `f32`, and comes back as `f32` only at the end — where a point
//! that does not fit is refused rather than drawn at infinity.

use alo_paint::{Path, Point};
use core::f64::consts::{FRAC_PI_2, TAU};

/// One arc command's parameters, in absolute user units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    /// Where the pen is.
    pub from: Point,
    /// The ellipse's radius across, before it is turned.
    pub rx: f32,
    /// The ellipse's radius down, before it is turned.
    pub ry: f32,
    /// How far the ellipse is turned, in degrees, clockwise on the page.
    pub rotation: f32,
    /// The long way round rather than the short.
    pub large: bool,
    /// Clockwise on the page (towards increasing angles) rather than not.
    pub sweep: bool,
    /// Where the arc ends.
    pub to: Point,
}

/// The arc as path segments appended to `path`, or `false` if it would put a
/// point outside what an `f32` holds, in which case nothing is appended.
///
/// Appends nothing for an arc that ends where it starts, a single line for an
/// arc with a zero radius, and up to four cubic curves otherwise. The last
/// segment ends exactly at [`Arc::to`], so the path goes on from where its
/// author said it would and not from a rounding of it.
pub fn append(arc: Arc, path: &mut Path) -> bool {
    if arc.from == arc.to {
        return true;
    }
    if arc.rx == 0.0 || arc.ry == 0.0 {
        path.line_to(arc.to);
        return true;
    }
    let mut pieces = Vec::with_capacity(4);
    if !cubics(arc, &mut pieces) {
        return false;
    }
    for (first, second, end) in pieces {
        path.cubic_to(first, second, end);
    }
    true
}

/// The cubic curves an arc is, into `out`; `false` if any point is not finite.
fn cubics(arc: Arc, out: &mut Vec<(Point, Point, Point)>) -> bool {
    let (x1, y1) = (f64::from(arc.from.x), f64::from(arc.from.y));
    let (x2, y2) = (f64::from(arc.to.x), f64::from(arc.to.y));
    let mut rx = f64::from(arc.rx).abs();
    let mut ry = f64::from(arc.ry).abs();
    let (sin, cos) = f64::from(arc.rotation).to_radians().sin_cos();

    // F.6.5.1: the start point in the ellipse's own, unturned axes, measured
    // from the chord's middle.
    let (half_x, half_y) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let x1p = cos * half_x + sin * half_y;
    let y1p = -sin * half_x + cos * half_y;

    // F.6.6.2: radii that cannot reach are scaled up until they just do.
    let reach = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if reach > 1.0 {
        let scale = reach.sqrt();
        rx *= scale;
        ry *= scale;
    }

    // F.6.5.2: the centre, in the same axes. Rounding can make the quantity
    // under the root a hair below zero when the radii were just scaled to fit,
    // and zero is what it means.
    let (rx2, ry2) = (rx * rx, ry * ry);
    let (x1p2, y1p2) = (x1p * x1p, y1p * y1p);
    let denominator = rx2 * y1p2 + ry2 * x1p2;
    let mut factor = ((rx2 * ry2 - denominator) / denominator).max(0.0).sqrt();
    if arc.large == arc.sweep {
        factor = -factor;
    }
    let cxp = factor * rx * y1p / ry;
    let cyp = -factor * ry * x1p / rx;

    // F.6.5.3: back to the page.
    let cx = cos * cxp - sin * cyp + f64::midpoint(x1, x2);
    let cy = sin * cxp + cos * cyp + f64::midpoint(y1, y2);

    // F.6.5.5 and F.6.5.6: where the arc starts and how far it turns.
    let start = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut turn = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if arc.sweep && turn < 0.0 {
        turn += TAU;
    } else if !arc.sweep && turn > 0.0 {
        turn -= TAU;
    }

    // At most a quarter turn per curve; the small allowance keeps a sweep of
    // exactly a half turn, give or take rounding, in two pieces and not three.
    let quarters = (turn.abs() / FRAC_PI_2 - 1e-9).ceil().clamp(1.0, 4.0);
    let step = turn / quarters;
    let handle = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |at: f64| {
        let (s, c) = at.sin_cos();
        (
            cx + rx * c * cos - ry * s * sin,
            cy + rx * c * sin + ry * s * cos,
        )
    };
    let tangent = |at: f64| {
        let (s, c) = at.sin_cos();
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };

    // `quarters` is a whole number from one to four, or not a number at all
    // when the parameters were extreme enough to lose one — and then every
    // point below is refused as not finite.
    let count = (1..=4_u8)
        .find(|count| f64::from(*count) >= quarters)
        .unwrap_or(4);
    let mut at = start;
    for piece in 0..count {
        let next = at + step;
        let (ax, ay) = point(at);
        let (bx, by) = point(next);
        let (adx, ady) = tangent(at);
        let (bdx, bdy) = tangent(next);
        let first = finite(ax + handle * adx, ay + handle * ady);
        let second = finite(bx - handle * bdx, by - handle * bdy);
        let end = if piece + 1 == count {
            Some(arc.to)
        } else {
            finite(bx, by)
        };
        let (Some(first), Some(second), Some(end)) = (first, second, end) else {
            out.clear();
            return false;
        };
        out.push((first, second, end));
        at = next;
    }
    true
}

/// The signed angle from one vector to another, in radians.
fn angle(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    (ux * vy - uy * vx).atan2(ux * vx + uy * vy)
}

/// A point, if it fits an `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a finite f64 that is within f32's range rounds to the nearest f32, which is the point"
)]
fn finite(x: f64, y: f64) -> Option<Point> {
    let (x, y) = (x as f32, y as f32);
    (x.is_finite() && y.is_finite()).then_some(Point::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_paint::Segment;

    fn arc(rx: f32, ry: f32, rotation: f32, large: bool, sweep: bool, to: (f32, f32)) -> Arc {
        Arc {
            from: Point::new(0.0, 0.0),
            rx,
            ry,
            rotation,
            large,
            sweep,
            to: Point::new(to.0, to.1),
        }
    }

    fn drawn(arc: Arc) -> Vec<Segment> {
        let mut path = Path::new();
        assert!(append(arc, &mut path), "{arc:?} was refused");
        path.segments().to_vec()
    }

    fn near(left: Point, right: Point) -> bool {
        (left.x - right.x).abs() < 1e-3 && (left.y - right.y).abs() < 1e-3
    }

    fn cubic(segment: Option<&Segment>) -> (Point, Point, Point) {
        match segment {
            Some(Segment::CubicTo(first, second, end)) => (*first, *second, *end),
            other => panic!("not a curve: {other:?}"),
        }
    }

    /// Where a curve is halfway along, at t = 1/2.
    fn middle(start: Point, (one, two, end): (Point, Point, Point)) -> Point {
        Point::new(
            0.125 * start.x + 0.375 * one.x + 0.375 * two.x + 0.125 * end.x,
            0.125 * start.y + 0.375 * one.y + 0.375 * two.y + 0.125 * end.y,
        )
    }

    #[test]
    fn an_arc_that_ends_where_it_starts_is_not_drawn() {
        assert!(drawn(arc(10.0, 10.0, 0.0, true, true, (0.0, 0.0))).is_empty());
    }

    #[test]
    fn a_zero_radius_is_a_straight_line() {
        assert_eq!(
            drawn(arc(0.0, 10.0, 0.0, false, true, (8.0, 6.0))),
            vec![Segment::LineTo(Point::new(8.0, 6.0))]
        );
        assert_eq!(
            drawn(arc(10.0, 0.0, 0.0, false, true, (8.0, 6.0))),
            vec![Segment::LineTo(Point::new(8.0, 6.0))]
        );
    }

    #[test]
    fn a_quarter_circle_is_one_curve_with_the_standard_handles() {
        // From (0, 0) to (10, 10) on a circle of 10, clockwise on the page, the
        // short way: the centre is (0, 10), and the arc runs from straight
        // above the centre to straight right of it.
        let segments = drawn(arc(10.0, 10.0, 0.0, false, true, (10.0, 10.0)));
        assert_eq!(segments.len(), 1);
        let (one, two, end) = cubic(segments.first());
        assert!(near(one, Point::new(5.522_847, 0.0)), "{one}");
        assert!(near(two, Point::new(10.0, 4.477_153)), "{two}");
        assert_eq!(end, Point::new(10.0, 10.0));
    }

    #[test]
    fn the_four_flag_combinations_are_four_different_arcs() {
        // Radius 10 from (0, 0) to (10, 10): the two centres are (0, 10) and
        // (10, 0); each gives a quarter turn one way and three quarters the
        // other.
        let short_clockwise = drawn(arc(10.0, 10.0, 0.0, false, true, (10.0, 10.0)));
        let short_anticlockwise = drawn(arc(10.0, 10.0, 0.0, false, false, (10.0, 10.0)));
        let long_clockwise = drawn(arc(10.0, 10.0, 0.0, true, true, (10.0, 10.0)));
        let long_anticlockwise = drawn(arc(10.0, 10.0, 0.0, true, false, (10.0, 10.0)));
        assert_eq!(short_clockwise.len(), 1);
        assert_eq!(short_anticlockwise.len(), 1);
        assert_eq!(long_clockwise.len(), 3);
        assert_eq!(long_anticlockwise.len(), 3);

        let start = Point::new(0.0, 0.0);
        // The short way round the centre (0, 10) bulges up and right.
        let bulge = middle(start, cubic(short_clockwise.first()));
        assert!(near(bulge, Point::new(7.071, 2.929)), "{bulge}");
        // The short way round the centre (10, 0) bulges down and left.
        let bulge = middle(start, cubic(short_anticlockwise.first()));
        assert!(near(bulge, Point::new(2.929, 7.071)), "{bulge}");
        // The long way clockwise goes round (10, 0) starting leftwards, so its
        // first quarter ends straight below that centre's left side.
        let (_, _, end) = cubic(long_clockwise.first());
        assert!(near(end, Point::new(10.0, -10.0)), "{end}");
        let (_, _, end) = cubic(long_anticlockwise.first());
        assert!(near(end, Point::new(-10.0, 10.0)), "{end}");
    }

    #[test]
    fn radii_too_small_to_reach_are_scaled_up_to_a_half_ellipse() {
        // A chord of 20 and a radius of 1: scaled to 10, and the arc is a
        // half circle about the chord's middle, in two quarter curves.
        let segments = drawn(arc(1.0, 1.0, 0.0, false, true, (20.0, 0.0)));
        assert_eq!(segments.len(), 2);
        let (_, _, top) = cubic(segments.first());
        assert!(near(top, Point::new(10.0, -10.0)), "{top}");
        assert_eq!(cubic(segments.get(1)).2, Point::new(20.0, 0.0));

        // An ellipse keeps its ratio when scaled: 2 by 1 across a chord of 20
        // becomes 10 by 5.
        let segments = drawn(arc(2.0, 1.0, 0.0, false, true, (20.0, 0.0)));
        let (_, _, top) = cubic(segments.first());
        assert!(near(top, Point::new(10.0, -5.0)), "{top}");
    }

    #[test]
    fn a_negative_radius_is_its_size() {
        assert_eq!(
            drawn(arc(-10.0, -10.0, 0.0, false, true, (10.0, 10.0))),
            drawn(arc(10.0, 10.0, 0.0, false, true, (10.0, 10.0)))
        );
    }

    #[test]
    fn rotation_turns_the_ellipse_and_not_the_endpoints() {
        // An ellipse 20 by 10 turned a quarter turn is 10 by 20: the same arc
        // as the unturned one written the other way round.
        let turned = drawn(arc(20.0, 10.0, 90.0, false, true, (10.0, 20.0)));
        let plain = drawn(arc(10.0, 20.0, 0.0, false, true, (10.0, 20.0)));
        assert_eq!(turned.len(), plain.len());
        for (left, right) in turned.iter().zip(&plain) {
            let ((a1, a2, a3), (b1, b2, b3)) = (cubic(Some(left)), cubic(Some(right)));
            assert!(
                near(a1, b1) && near(a2, b2) && near(a3, b3),
                "{left:?} {right:?}"
            );
        }
    }

    #[test]
    fn every_point_of_a_three_quarter_arc_is_on_the_circle() {
        let segments = drawn(arc(10.0, 10.0, 0.0, true, true, (10.0, 10.0)));
        let centre = Point::new(10.0, 0.0);
        let mut start = Point::new(0.0, 0.0);
        for segment in &segments {
            let curve = cubic(Some(segment));
            let halfway = middle(start, curve);
            let radius = (halfway.x - centre.x).hypot(halfway.y - centre.y);
            assert!((radius - 10.0).abs() < 0.003, "{radius}");
            start = curve.2;
        }
    }

    #[test]
    fn an_arc_that_would_reach_past_a_float_is_refused_and_appends_nothing() {
        let mut path = Path::new();
        let huge = arc(3e38, 3e38, 0.0, true, true, (1.0, 0.0));
        assert!(!append(huge, &mut path));
        assert!(path.is_empty());

        // The short way round the same circle stays near its endpoints.
        assert!(append(
            arc(3e38, 3e38, 0.0, false, true, (1.0, 0.0)),
            &mut path
        ));
        assert_eq!(path.segments().len(), 1);
    }

    #[test]
    fn extreme_but_finite_parameters_never_panic() {
        for (rx, ry, rotation) in [
            (f32::MAX, f32::MIN_POSITIVE, 1e30),
            (f32::MIN_POSITIVE, f32::MAX, -1e30),
            (1e-45, 1e-45, 0.0),
            (f32::MAX, f32::MAX, f32::MAX),
        ] {
            for (large, sweep) in [(false, false), (false, true), (true, false), (true, true)] {
                for to in [
                    (1.0, 0.0),
                    (f32::MAX, f32::MAX),
                    (1e-45, 0.0),
                    (-3e38, 3e38),
                ] {
                    let mut path = Path::new();
                    let _ = append(arc(rx, ry, rotation, large, sweep, to), &mut path);
                    for segment in path.segments() {
                        if let Segment::CubicTo(a, b, c) = segment {
                            for p in [a, b, c] {
                                assert!(p.x.is_finite() && p.y.is_finite());
                            }
                        }
                    }
                }
            }
        }
    }
}
