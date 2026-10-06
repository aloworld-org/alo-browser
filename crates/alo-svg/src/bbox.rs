/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A shape's **object bounding box**: the tightest rectangle around its
//! geometry, in its own user space, with no stroke in it.
//!
//! SVG 2 calls it the *fill box*, and `transform-box: fill-box` measures
//! `transform-origin` and a `translate`'s percentages against it (item 287).
//! It is not [`alo_paint::Path::bounds`], which takes in a curve's control
//! points so that a mask is never too small: a circle's control points sit on
//! its bounding square and come out right, but `M0 0 C0 -10 10 -10 10 0`
//! peaks at `-7.5` and not at `-10`, and a box ten units tall would put a
//! `center` origin in the wrong place. So each curve's turning points are
//! found and only the points the curve passes through are counted.
//!
//! A subpath that is only a move draws nothing and is not counted; a `Z` back
//! to where its subpath began is, which is what makes `M5 5Z` a box of no size
//! at `(5, 5)` rather than nothing.

use alo_paint::{Path, Point, Segment};

/// A rectangle in user units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// How far across.
    pub width: f32,
    /// How far down.
    pub height: f32,
}

/// The tightest rectangle every point the path passes through fits inside, or
/// [`None`] for a path that draws nothing at all.
pub fn object_bounding_box(path: &Path) -> Option<Rect> {
    let mut seen = Seen::default();
    let mut at = Point::new(0.0, 0.0);
    let mut start = at;
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) => {
                at = to;
                start = to;
            }
            Segment::LineTo(to) => {
                seen.add(at);
                seen.add(to);
                at = to;
            }
            Segment::QuadTo(control, to) => {
                seen.add(at);
                seen.add(to);
                for t in quad_turns(at.x, control.x, to.x)
                    .into_iter()
                    .chain(quad_turns(at.y, control.y, to.y))
                    .flatten()
                {
                    seen.add(Point::new(
                        quad(at.x, control.x, to.x, t),
                        quad(at.y, control.y, to.y, t),
                    ));
                }
                at = to;
            }
            Segment::CubicTo(first, second, to) => {
                seen.add(at);
                seen.add(to);
                for t in cubic_turns(at.x, first.x, second.x, to.x)
                    .into_iter()
                    .chain(cubic_turns(at.y, first.y, second.y, to.y))
                    .flatten()
                {
                    seen.add(Point::new(
                        cubic(at.x, first.x, second.x, to.x, t),
                        cubic(at.y, first.y, second.y, to.y, t),
                    ));
                }
                at = to;
            }
            Segment::Close => {
                seen.add(at);
                seen.add(start);
                at = start;
            }
        }
    }
    seen.rect()
}

#[derive(Default)]
struct Seen {
    found: Option<(f32, f32, f32, f32)>,
}

impl Seen {
    fn add(&mut self, point: Point) {
        self.found = Some(match self.found {
            None => (point.x, point.y, point.x, point.y),
            Some((left, top, right, bottom)) => (
                left.min(point.x),
                top.min(point.y),
                right.max(point.x),
                bottom.max(point.y),
            ),
        });
    }

    fn rect(&self) -> Option<Rect> {
        let (left, top, right, bottom) = self.found?;
        Some(Rect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }
}

fn quad(p0: f32, p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * p0 + 2.0 * u * t * p1 + t * t * p2
}

fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

/// Where along a quadratic, strictly between its ends, it turns back on one
/// axis: where its derivative, a straight line, crosses zero.
fn quad_turns(p0: f32, p1: f32, p2: f32) -> [Option<f32>; 1] {
    let bend = p0 - 2.0 * p1 + p2;
    [inside((p0 - p1) / bend)]
}

/// Where along a cubic, strictly between its ends, it turns back on one axis:
/// the roots of its derivative, a quadratic `a·t² + b·t + c` (over three).
fn cubic_turns(p0: f32, p1: f32, p2: f32, p3: f32) -> [Option<f32>; 2] {
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    // Next to the other two terms an `a` this small is rounding, and the
    // derivative is the straight line it would be with none.
    let scale = a.abs().max(b.abs()).max(c.abs());
    if a.abs() <= scale * 1.0e-6 {
        return [inside(-c / b), None];
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return [None, None];
    }
    let root = discriminant.sqrt();
    [
        inside((-b + root) / (2.0 * a)),
        inside((-b - root) / (2.0 * a)),
    ]
}

/// A parameter strictly inside the curve. A division by zero, which is a
/// curve that never turns on this axis, is not a number and is not inside.
fn inside(t: f32) -> Option<f32> {
    (t > 0.0 && t < 1.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rounded(rect: Rect) -> (f32, f32, f32, f32) {
        let round = |value: f32| (value * 1000.0).round() / 1000.0;
        (
            round(rect.x),
            round(rect.y),
            round(rect.width),
            round(rect.height),
        )
    }

    fn of(path: &Path) -> (f32, f32, f32, f32) {
        rounded(object_bounding_box(path).expect("a box"))
    }

    #[test]
    fn a_rectangle_is_its_own_box() {
        assert_eq!(
            of(&Path::rectangle(3.0, 4.0, 10.0, 6.0)),
            (3.0, 4.0, 10.0, 6.0)
        );
    }

    #[test]
    fn a_curve_is_measured_where_it_goes_not_where_its_handles_are() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.cubic_to(
            Point::new(0.0, -10.0),
            Point::new(10.0, -10.0),
            Point::new(10.0, 0.0),
        );
        assert_eq!(of(&path), (0.0, -7.5, 10.0, 7.5));
        assert_eq!(
            path.bounds(),
            Some((0.0, -10.0, 10.0, 0.0)),
            "paint's is looser"
        );

        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.quad_to(Point::new(5.0, 10.0), Point::new(10.0, 0.0));
        assert_eq!(of(&path), (0.0, 0.0, 10.0, 5.0));
    }

    #[test]
    fn a_cubic_that_turns_twice_is_measured_at_both_turns() {
        // An S: out past its start, then back past its end.
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.cubic_to(
            Point::new(-10.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(10.0, 0.0),
        );
        let (x, _, width, _) = of(&path);
        assert!(x < 0.0 && x + width > 10.0, "{x} {width}");
    }

    #[test]
    fn a_move_alone_draws_nothing_and_a_close_counts() {
        let mut path = Path::new();
        path.move_to(Point::new(50.0, 50.0));
        assert_eq!(object_bounding_box(&path), None);
        path.move_to(Point::new(5.0, 5.0));
        path.close();
        assert_eq!(of(&path), (5.0, 5.0, 0.0, 0.0));
        assert_eq!(object_bounding_box(&Path::new()), None);
    }

    #[test]
    fn a_straight_cubic_has_no_turns_and_no_division_by_zero() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.cubic_to(
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(3.0, 3.0),
        );
        assert_eq!(of(&path), (0.0, 0.0, 3.0, 3.0));
    }
}
