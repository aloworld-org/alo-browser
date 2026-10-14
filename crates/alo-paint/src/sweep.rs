/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The shape a square-ended stroke covers along a piece of a line, made as
//! what a stroke is: every point of the piece's normals, half the stroke's
//! width either side of it.
//!
//! The rented stroker ([`crate::raster::outline`]) offsets a curve by half
//! the width each way and fills between. Where the width is more than the
//! curve's radius, the inner offset turns inside out, and what it leaves
//! open is the very middle of a tight corner, which a stroke's normals do
//! cover. A dashed border's dashes are drawn 2.2 times as wide as its
//! widest side, round corners whose radius is often less than that, and
//! Chrome 154 covers them: a 22 px box with a 10 px dashed border is a
//! solid disc there with a hole only as large as the padding box's, where
//! the offsets left a hole half as wide as the box.
//!
//! So the shape is built here from [`Station`]s along the piece: between
//! each two, the quadrilateral their normals bound on the outside, and the
//! one they bound on the inside — split at the point where the two inside
//! normals cross, when they cross within the stroke, into the triangle
//! before that point and the triangle past it. Every part is wound the same
//! way, so the non-zero rule fills their union. Where two stations stand on
//! one point facing different ways — a square corner — the outside is
//! filled to the mitre, or bevelled past a mitre four times the stroke's
//! half width, as a stroke joins.

use crate::contour::Station;
use crate::path::{Path, Point};

/// How long a mitre may be, in half widths, before it is bevelled.
const MITRE_LIMIT: f32 = 4.0;

/// How far apart, in the line's units, two stations may stand and still be
/// one point.
const SAME_POINT: f32 = 1.0e-4;

/// The shape a stroke `half` wide each side covers along `stations`, with
/// square ends at the first and last.
pub fn swept(stations: &[Station], half: f32) -> Path {
    let mut path = Path::new();
    if !(half.is_finite() && half > 0.0) {
        return path;
    }
    for (first, second) in stations.iter().zip(stations.iter().skip(1)) {
        let (a, b) = (first.point, second.point);
        if (b.x - a.x).hypot(b.y - a.y) <= SAME_POINT {
            join(a, first.normal, second.normal, half, &mut path);
            continue;
        }
        let out =
            |at: Point, (nx, ny): (f32, f32), by: f32| Point::new(at.x + nx * by, at.y + ny * by);
        add(
            &[
                a,
                b,
                out(b, second.normal, half),
                out(a, first.normal, half),
            ],
            &mut path,
        );
        let (inner_a, inner_b) = (out(a, first.normal, -half), out(b, second.normal, -half));
        match crossing(a, first.normal, b, second.normal) {
            Some((along_a, along_b)) if along_a < half && along_b < half => {
                let crossed = out(a, first.normal, -along_a);
                add(&[a, crossed, b], &mut path);
                add(&[crossed, inner_a, inner_b], &mut path);
            }
            _ => add(&[a, inner_a, inner_b, b], &mut path),
        }
    }
    path
}

/// A square corner's outside, from the normal before it to the normal after.
fn join(at: Point, before: (f32, f32), after: (f32, f32), half: f32, path: &mut Path) {
    let dot = before.0 * after.0 + before.1 * after.1;
    // The same way, or a turn back on itself: nothing to fill.
    if !(-1.0 + 1.0e-6..=1.0 - 1.0e-6).contains(&dot) {
        return;
    }
    let edge = |(nx, ny): (f32, f32)| Point::new(at.x + nx * half, at.y + ny * half);
    // The mitre's tip is along the two normals' sum, 1 / cos(θ / 2) half
    // widths out, which is the sum over 1 + cos θ.
    let over = 1.0 + dot;
    let tip = ((before.0 + after.0) / over, (before.1 + after.1) / over);
    if tip.0.hypot(tip.1) <= MITRE_LIMIT {
        add(&[at, edge(before), edge(tip), edge(after)], path);
    } else {
        add(&[at, edge(before), edge(after)], path);
    }
}

/// How far along each of two inward normals, from `a` and from `b`, the
/// lines they lie on cross; [`None`] when they are parallel or cross behind
/// either point.
fn crossing(a: Point, at_a: (f32, f32), b: Point, at_b: (f32, f32)) -> Option<(f32, f32)> {
    // a - s·n_a = b - u·n_b, solved for s and u.
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let det = at_a.0 * at_b.1 - at_a.1 * at_b.0;
    if det.abs() <= f32::EPSILON {
        return None;
    }
    let s = (dy * at_b.0 - dx * at_b.1) / det;
    let u = (dy * at_a.0 - dx * at_a.1) / det;
    (s > 0.0 && u > 0.0).then_some((s, u))
}

/// A polygon onto `path`, wound clockwise on the screen whichever way it
/// was given.
fn add(polygon: &[Point], path: &mut Path) {
    let twice_area: f32 = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(p, q)| p.x * q.y - q.x * p.y)
        .sum();
    if twice_area == 0.0 || !twice_area.is_finite() {
        return;
    }
    let mut points: Vec<Point> = polygon.to_vec();
    if twice_area < 0.0 {
        points.reverse();
    }
    let mut corners = points.into_iter();
    if let Some(first) = corners.next() {
        path.move_to(first);
        for corner in corners {
            path.line_to(corner);
        }
        path.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contour::Contour;
    use crate::corner::Corners;
    use crate::path::Segment;
    use alo_layout::Rect;

    fn station(x: f32, y: f32, normal: (f32, f32)) -> Station {
        Station {
            point: Point::new(x, y),
            normal,
        }
    }

    /// How many times the path winds round a point, every outline being
    /// straight lines.
    fn winding(path: &Path, x: f32, y: f32) -> i32 {
        let mut total = 0;
        let (mut start, mut at) = (Point::new(0.0, 0.0), Point::new(0.0, 0.0));
        let mut cross = |from: Point, to: Point| {
            if (from.y <= y) != (to.y <= y) {
                let across = from.x + (y - from.y) / (to.y - from.y) * (to.x - from.x);
                if across > x {
                    total += if to.y > from.y { 1 } else { -1 };
                }
            }
        };
        for segment in path.segments() {
            match *segment {
                Segment::MoveTo(to) => {
                    start = to;
                    at = to;
                }
                Segment::LineTo(to) => {
                    cross(at, to);
                    at = to;
                }
                Segment::Close => {
                    cross(at, start);
                    at = start;
                }
                Segment::QuadTo(..) | Segment::CubicTo(..) => panic!("a sweep is straight lines"),
            }
        }
        total
    }

    #[test]
    fn a_straight_piece_is_a_rectangle_square_at_its_ends() {
        let shape = swept(
            &[
                station(0.0, 0.0, (0.0, -1.0)),
                station(10.0, 0.0, (0.0, -1.0)),
            ],
            2.0,
        );
        for (x, y) in [(5.0, -1.9), (5.0, 1.9), (0.1, 0.0), (9.9, 0.0)] {
            assert_ne!(winding(&shape, x, y), 0, "({x}, {y}) is covered");
        }
        for (x, y) in [(5.0, -2.1), (5.0, 2.1), (-0.1, 0.0), (10.1, 0.0)] {
            assert_eq!(winding(&shape, x, y), 0, "({x}, {y}) is not");
        }
    }

    #[test]
    fn every_part_is_wound_one_way_so_overlaps_never_cancel() {
        // Round a tight corner, wider than its radius, every point is
        // covered once or more and never wound back to nothing.
        let line = Contour::round(Rect::new(0.0, 0.0, 12.0, 12.0), Corners::all(6.0), 1.0);
        let shape = swept(&line.stations(0.0, line.length()), 11.0);
        for y in 0..12_u8 {
            for x in 0..12_u8 {
                let found = winding(&shape, f32::from(x) + 0.5, f32::from(y) + 0.5);
                assert!(found > 0, "({x}, {y}) winds {found}");
            }
        }
    }

    #[test]
    fn a_stroke_wider_than_its_curve_covers_the_curves_middle() {
        // A quarter of a circle of 6 round (6, 6), from the top round to
        // the right, stroked 11 each side: its normals pass through the
        // centre and 5 beyond it, the other way.
        let line = Contour::round(Rect::new(0.0, 0.0, 12.0, 12.0), Corners::all(6.0), 8.0);
        let quarter = core::f32::consts::FRAC_PI_2 * 6.0;
        let shape = swept(&line.stations(0.0, quarter), 11.0);
        assert_ne!(winding(&shape, 6.0, 6.0), 0, "the centre is covered");
        // Past the centre, on the line from the corner's middle through it.
        assert_ne!(winding(&shape, 4.0, 8.0), 0, "the far side is covered");
        // But not across the stroke's square ends.
        assert_eq!(winding(&shape, 5.0, -3.0), 0);
    }

    #[test]
    fn a_square_corner_is_filled_to_its_mitre() {
        let shape = swept(
            &[
                station(0.0, 0.0, (0.0, -1.0)),
                station(10.0, 0.0, (0.0, -1.0)),
                station(10.0, 0.0, (1.0, 0.0)),
                station(10.0, 10.0, (1.0, 0.0)),
            ],
            2.0,
        );
        assert_ne!(winding(&shape, 11.9, -1.9), 0, "the mitre is filled");
        assert_eq!(winding(&shape, 12.1, -1.0), 0);
        assert_eq!(winding(&shape, 11.0, -2.1), 0);
    }

    #[test]
    fn a_corner_sharper_than_the_limit_is_bevelled() {
        // Turning back by 170°: the mitre would be over 11 half widths long.
        let (sin, cos) = 170.0_f32.to_radians().sin_cos();
        let mut path = Path::new();
        join(
            Point::new(0.0, 0.0),
            (0.0, -1.0),
            (sin, -cos),
            1.0,
            &mut path,
        );
        let reach = path.bounds().map(|(left, top, right, bottom)| {
            left.abs().max(top.abs()).max(right.abs()).max(bottom.abs())
        });
        assert!(reach.is_some_and(|reach| reach <= 1.0 + 1e-4), "{reach:?}");
        // And a right angle is mitred, √2 half widths out.
        let mut path = Path::new();
        join(
            Point::new(0.0, 0.0),
            (0.0, -1.0),
            (1.0, 0.0),
            1.0,
            &mut path,
        );
        assert_eq!(path.bounds(), Some((0.0, -1.0, 1.0, 0.0)));
    }

    #[test]
    fn nothing_is_swept_with_no_width_or_one_that_is_not_a_number() {
        let stations = [
            station(0.0, 0.0, (0.0, -1.0)),
            station(10.0, 0.0, (0.0, -1.0)),
        ];
        for half in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(swept(&stations, half).is_empty(), "{half}");
        }
        assert!(swept(&stations[..1], 2.0).is_empty());
    }
}
