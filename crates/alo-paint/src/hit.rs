/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which box a person's pointer is over: the hit test (ADR 0024 § 5, queue
//! item 298).
//!
//! It reads the display list, because which box is on top at a point is
//! **paint order** — a positioned box over the flow, a later block over an
//! earlier one it overlaps, a negative `z-index` under its parent's content
//! — under the clips and transforms around each box, and the list is where
//! all of that is decided once ([`crate::build`]). A second walk of the boxes
//! deciding the same order would drift from the first, and a click would land
//! on a box a person cannot see.
//!
//! Each box says where it is with a [`DisplayItem::Area`], which draws
//! nothing: a box with no background is still somewhere a person can click.
//!
//! # The rule, measured
//!
//! Measured in Chrome 154 with `elementFromPoint` and with mouse events sent
//! over the `DevTools` protocol, which agree; its source was not read.
//!
//! - **A point is the pixel that starts at it.** A box is under the point
//!   `(x, y)` when it overlaps the square from `(x, y)` to `(x + 1, y + 1)`
//!   by some area, not merely at an edge. A box from 10 to 110 is found by
//!   a point at 9.05 and not at 9, by 109.99 and not by 110 — and a box at
//!   10.25 is found by 9.3, not 9.25.
//! - **The last box drawn there wins**, which is the reverse of paint order.
//! - **A transform moves where a box is found**, as it moves where the box
//!   is drawn: a square turned 45° about its middle is not found at its old
//!   corner.
//! - **A clip hides what it hides from a pointer too**, rounded corners
//!   included, and so do a box's own rounded corners.
//!
//! # Where Chrome differs
//!
//! An absolutely positioned box with `overflow: hidden` and square corners
//! lets Chrome find its in-flow content under its border, where the content
//! is not drawn. The same box in the flow, or with rounded corners, clips at
//! its padding box as it draws. This follows what is drawn (queue item 536).
//!
//! # Shapes
//!
//! Every shape the builder records around an area — a box's border box, a
//! clip's padding box — is a rounded rectangle, which is **convex**, and an
//! affine transform keeps it so. That is what lets the square be cut down by
//! each shape in turn (Sutherland and Hodgman's clipping). A contour that is
//! not convex is taken as its bounding rectangle, which finds a box at least
//! wherever the contour is.

use crate::display::{DisplayItem, DisplayList};
use crate::path::{Path, Point, Segment};
use alo_box::BoxId;
use alo_value::Matrix;

/// The least overlap, in square pixels, that counts as a box being under a
/// point. Above the rounding of a pixel cut by a shape put through a
/// transform, and below the least Chrome can find, a corner of 1/64 px on
/// each side (2.4 × 10⁻⁴).
const LEAST_OVERLAP: f32 = 1.0e-5;

/// How many straight pieces a curve may become at most. A quarter of a circle
/// a thousand pixels across, in 64 pieces, is out by less than a tenth of a
/// pixel.
const MOST_PIECES: u16 = 64;

/// How long one straight piece of a curve may be, roughly, before it is cut
/// again.
const LONGEST_PIECE: f32 = 2.0;

/// A closed polygon, its corners in order.
type Polygon = Vec<(f32, f32)>;

/// The box drawn last over `at` in `list`, a point in the list's own
/// coordinates — the page's CSS pixels — or [`None`] when no box is there.
pub fn box_at(list: &DisplayList, at: Point) -> Option<BoxId> {
    let pixel = vec![
        (at.x, at.y),
        (at.x + 1.0, at.y),
        (at.x + 1.0, at.y + 1.0),
        (at.x, at.y + 1.0),
    ];
    // What is left of the pixel inside every clip open so far, innermost
    // last, so an area is cut by one shape rather than by every clip again.
    let mut left: Vec<Vec<Polygon>> = vec![vec![pixel]];
    let mut transforms: Vec<Matrix> = Vec::new();
    let mut found = None;
    for item in list.items() {
        let current = transforms.last().copied().unwrap_or(Matrix::IDENTITY);
        match item {
            DisplayItem::PushTransform { matrix, .. } => transforms.push(matrix.then(current)),
            DisplayItem::PopTransform => {
                transforms.pop();
            }
            DisplayItem::PushClip { path, .. } => {
                let inside = left
                    .last()
                    .map(|region| cut(region, &path.transformed(current)))
                    .unwrap_or_default();
                left.push(inside);
            }
            // The first entry is the whole pixel, which no clip opened, so a
            // list that closes more clips than it opens never loses it.
            DisplayItem::PopClip => {
                if left.len() > 1 {
                    left.pop();
                }
            }
            DisplayItem::Area { box_id, path } => {
                let Some(region) = left.last() else {
                    continue;
                };
                if area(&cut(region, &path.transformed(current))) > LEAST_OVERLAP {
                    found = Some(*box_id);
                }
            }
            DisplayItem::Fill { .. }
            | DisplayItem::Shadow { .. }
            | DisplayItem::PushGroup { .. }
            | DisplayItem::PopGroup
            | DisplayItem::Picture { .. }
            | DisplayItem::Text { .. } => {}
        }
    }
    found
}

/// What of `region` is inside `shape`.
fn cut(region: &[Polygon], shape: &Path) -> Vec<Polygon> {
    let contours: Vec<Polygon> = flattened(shape).into_iter().map(convex).collect();
    let mut out = Vec::new();
    for piece in region {
        for contour in &contours {
            let inside = clipped(piece, contour);
            if signed_area(&inside).abs() > LEAST_OVERLAP {
                out.push(inside);
            }
        }
    }
    out
}

/// How much area the polygons cover, taken as not overlapping one another —
/// which pieces of one region cut by disjoint contours are not.
fn area(region: &[Polygon]) -> f32 {
    region
        .iter()
        .map(|polygon| signed_area(polygon).abs())
        .sum()
}

/// The area a polygon encloses, positive one way round and negative the
/// other. Not a number when a corner is not.
///
/// Measured from the first corner rather than from the page's: a pixel a
/// hundred pixels in has corners whose products are ten thousand, and in an
/// `f32` the area of a sliver of it would be lost in their rounding.
fn signed_area(polygon: &[(f32, f32)]) -> f32 {
    let Some(&(ox, oy)) = polygon.first() else {
        return 0.0;
    };
    let mut twice = 0.0;
    for (index, &(x, y)) in polygon.iter().enumerate() {
        let next = polygon.get(index + 1).or(polygon.first());
        if let Some(&(nx, ny)) = next {
            twice += (x - ox) * (ny - oy) - (nx - ox) * (y - oy);
        }
    }
    twice / 2.0
}

/// `subject`, kept to the inside of the convex polygon `clip`: Sutherland and
/// Hodgman's clipping, one edge of `clip` at a time.
///
/// A `clip` with no area — a box scaled to nothing, or one with a corner
/// that is not a number — keeps nothing.
fn clipped(subject: &[(f32, f32)], clip: &[(f32, f32)]) -> Polygon {
    let turn = signed_area(clip);
    if turn.is_nan() || turn.abs() <= LEAST_OVERLAP {
        return Vec::new();
    }
    // Which side of each edge is inside depends on which way round the
    // polygon goes, and a transform that mirrors a box turns it over.
    let side = turn.signum();
    let mut out: Polygon = subject.to_vec();
    for (index, &from) in clip.iter().enumerate() {
        let Some(&to) = clip.get(index + 1).or(clip.first()) else {
            break;
        };
        let inside = |point: (f32, f32)| {
            side * ((to.0 - from.0) * (point.1 - from.1) - (to.1 - from.1) * (point.0 - from.0))
                >= 0.0
        };
        let crossing = |a: (f32, f32), b: (f32, f32)| {
            let ex = to.0 - from.0;
            let ey = to.1 - from.1;
            let da = ex * (a.1 - from.1) - ey * (a.0 - from.0);
            let db = ex * (b.1 - from.1) - ey * (b.0 - from.0);
            let t = da / (da - db);
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        };
        let input = core::mem::take(&mut out);
        for (at, &current) in input.iter().enumerate() {
            let Some(&previous) = (if at == 0 {
                input.last()
            } else {
                input.get(at - 1)
            }) else {
                break;
            };
            match (inside(previous), inside(current)) {
                (true, true) => out.push(current),
                (true, false) => out.push(crossing(previous, current)),
                (false, true) => {
                    out.push(crossing(previous, current));
                    out.push(current);
                }
                (false, false) => {}
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// `contour` if it is convex, or else its bounding rectangle.
fn convex(contour: Polygon) -> Polygon {
    if is_convex(&contour) {
        return contour;
    }
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for &(x, y) in &contour {
        bounds = Some(match bounds {
            None => (x, y, x, y),
            Some((left, top, right, bottom)) => {
                (left.min(x), top.min(y), right.max(x), bottom.max(y))
            }
        });
    }
    match bounds {
        Some((left, top, right, bottom)) => {
            vec![(left, top), (right, top), (right, bottom), (left, bottom)]
        }
        None => Vec::new(),
    }
}

/// Whether every turn along `contour` goes the same way. A straight run, a
/// corner repeated and a turn too small to measure turn neither way.
fn is_convex(contour: &[(f32, f32)]) -> bool {
    let count = contour.len();
    // Whether the turns found so far go one way, once one has been found.
    let mut way: Option<bool> = None;
    for (index, &(x, y)) in contour.iter().enumerate() {
        let (Some(&(nx, ny)), Some(&(ax, ay))) = (
            contour.get((index + 1) % count.max(1)),
            contour.get((index + 2) % count.max(1)),
        ) else {
            return false;
        };
        let turn = (nx - x) * (ay - ny) - (ny - y) * (ax - nx);
        if turn.abs() <= f32::EPSILON {
            continue;
        }
        match way {
            None => way = Some(turn > 0.0),
            Some(left) if left != (turn > 0.0) => return false,
            Some(_) => {}
        }
    }
    true
}

/// `path` as closed polygons, one per contour, its curves cut into straight
/// pieces.
fn flattened(path: &Path) -> Vec<Polygon> {
    let mut contours = Vec::new();
    let mut contour: Polygon = Vec::new();
    let mut pen = (0.0, 0.0);
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) => {
                if contour.len() > 2 {
                    contours.push(core::mem::take(&mut contour));
                }
                contour.clear();
                pen = (to.x, to.y);
                contour.push(pen);
            }
            Segment::LineTo(to) => {
                pen = (to.x, to.y);
                contour.push(pen);
            }
            Segment::QuadTo(control, to) => {
                let (from, c, end) = (pen, (control.x, control.y), (to.x, to.y));
                let steps = pieces(&[from, c, end]);
                for step in 1..=steps {
                    let t = f32::from(step) / f32::from(steps);
                    let u = 1.0 - t;
                    contour.push((
                        u * u * from.0 + 2.0 * u * t * c.0 + t * t * end.0,
                        u * u * from.1 + 2.0 * u * t * c.1 + t * t * end.1,
                    ));
                }
                pen = end;
            }
            Segment::CubicTo(first, second, to) => {
                let (from, a, b, end) =
                    (pen, (first.x, first.y), (second.x, second.y), (to.x, to.y));
                let steps = pieces(&[from, a, b, end]);
                for step in 1..=steps {
                    let t = f32::from(step) / f32::from(steps);
                    let u = 1.0 - t;
                    contour.push((
                        u * u * u * from.0
                            + 3.0 * u * u * t * a.0
                            + 3.0 * u * t * t * b.0
                            + t * t * t * end.0,
                        u * u * u * from.1
                            + 3.0 * u * u * t * a.1
                            + 3.0 * u * t * t * b.1
                            + t * t * t * end.1,
                    ));
                }
                pen = end;
            }
            Segment::Close => {
                if let Some(&start) = contour.first() {
                    pen = start;
                }
            }
        }
    }
    if contour.len() > 2 {
        contours.push(contour);
    }
    contours
}

/// How many straight pieces a curve with these control points becomes: one
/// for every [`LONGEST_PIECE`] of its control polygon, doubled up to
/// [`MOST_PIECES`]. A curve whose points are not numbers is one piece.
fn pieces(points: &[(f32, f32)]) -> u16 {
    let length: f32 = points
        .windows(2)
        .filter_map(|pair| match pair {
            [a, b] => Some(((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()),
            _ => None,
        })
        .sum();
    let mut steps = 1u16;
    while steps < MOST_PIECES && length / f32::from(steps) > LONGEST_PIECE {
        steps *= 2;
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corner::{Corners, rounded_rectangle};
    use crate::paint::Paint;
    use alo_layout::Rect;
    use alo_value::Rgba;

    fn id(index: usize) -> BoxId {
        BoxId::from_index_for_tests(index)
    }

    fn area_of(index: usize, x: f32, y: f32, width: f32, height: f32) -> DisplayItem {
        DisplayItem::Area {
            box_id: id(index),
            path: Path::rectangle(x, y, width, height),
        }
    }

    fn list(items: Vec<DisplayItem>) -> DisplayList {
        let mut list = DisplayList::default();
        for item in items {
            list.push(item);
        }
        list
    }

    fn at(list: &DisplayList, x: f32, y: f32) -> Option<BoxId> {
        box_at(list, Point::new(x, y))
    }

    #[test]
    fn a_point_is_the_pixel_that_starts_at_it() {
        // Chrome 154: a box from 10 to 110 across and 10 to 60 down.
        let page = list(vec![area_of(1, 10.0, 10.0, 100.0, 50.0)]);
        assert_eq!(at(&page, 9.05, 30.0), Some(id(1)));
        assert_eq!(at(&page, 9.0, 30.0), None, "touching is not overlapping");
        assert_eq!(at(&page, 109.99, 59.99), Some(id(1)));
        assert_eq!(at(&page, 110.0, 30.0), None);
        assert_eq!(at(&page, 60.0, 9.05), Some(id(1)));
        assert_eq!(at(&page, 60.0, 60.0), None);
    }

    #[test]
    fn a_box_at_a_fraction_is_found_from_a_pixel_before_it() {
        // Chrome 154: a box at 10.25 is found by 9.3, not 9.25, and ends at
        // 30.75 exactly.
        let page = list(vec![area_of(1, 10.25, 10.75, 20.5, 20.5)]);
        assert_eq!(at(&page, 9.3, 15.0), Some(id(1)));
        assert_eq!(at(&page, 9.25, 15.0), None);
        assert_eq!(at(&page, 30.74, 15.0), Some(id(1)));
        assert_eq!(at(&page, 30.75, 15.0), None);
        assert_eq!(at(&page, 15.0, 9.8), Some(id(1)));
        assert_eq!(at(&page, 15.0, 9.75), None);
    }

    #[test]
    fn the_last_box_drawn_wins() {
        // Chrome 154: two boxes side by side, the second from 10.25; a point
        // at 9.5 has a pixel over both and finds the second.
        let page = list(vec![
            area_of(1, 0.0, 0.0, 10.25, 20.0),
            area_of(2, 10.25, 0.0, 10.0, 20.0),
        ]);
        assert_eq!(at(&page, 9.5, 5.0), Some(id(2)));
        assert_eq!(at(&page, 5.0, 5.0), Some(id(1)));
    }

    #[test]
    fn what_is_drawn_finds_nothing() {
        let page = list(vec![DisplayItem::Fill {
            box_id: id(1),
            path: Path::rectangle(0.0, 0.0, 100.0, 100.0),
            paint: Paint::Solid(Rgba::BLACK),
            rule: crate::fill_rule::FillRule::NonZero,
        }]);
        assert_eq!(at(&page, 50.0, 50.0), None);
        assert_eq!(at(&DisplayList::default(), 0.0, 0.0), None);
    }

    #[test]
    fn a_turned_box_is_found_where_it_was_turned_to() {
        // Chrome 154: 100 square at (100, 100) turned 45° about its middle.
        let turn = Matrix::translation(-150.0, -150.0)
            .then(Matrix {
                a: core::f32::consts::FRAC_1_SQRT_2,
                b: core::f32::consts::FRAC_1_SQRT_2,
                c: -core::f32::consts::FRAC_1_SQRT_2,
                d: core::f32::consts::FRAC_1_SQRT_2,
                e: 0.0,
                f: 0.0,
            })
            .then(Matrix::translation(150.0, 150.0));
        let page = list(vec![
            DisplayItem::PushTransform {
                box_id: id(1),
                matrix: turn,
            },
            area_of(1, 100.0, 100.0, 100.0, 100.0),
            DisplayItem::PopTransform,
        ]);
        assert_eq!(at(&page, 101.0, 101.0), None, "its old corner");
        assert_eq!(at(&page, 150.0, 150.0), Some(id(1)));
        assert_eq!(at(&page, 150.0, 85.0), Some(id(1)));
        assert_eq!(at(&page, 150.0, 78.0), None, "its top corner is at 79.3");
        assert_eq!(at(&page, 215.0, 150.0), Some(id(1)));
        assert_eq!(at(&page, 222.0, 150.0), None);
    }

    #[test]
    fn a_clip_hides_what_is_outside_it_and_ends_where_it_is_popped() {
        let page = list(vec![
            DisplayItem::PushClip {
                box_id: id(1),
                path: Path::rectangle(10.0, 10.0, 100.0, 100.0),
            },
            area_of(2, 10.0, 10.0, 300.0, 300.0),
            DisplayItem::PopClip,
            area_of(3, 200.0, 0.0, 10.0, 10.0),
        ]);
        assert_eq!(at(&page, 109.0, 50.0), Some(id(2)));
        assert_eq!(at(&page, 110.0, 50.0), None);
        assert_eq!(at(&page, 50.0, 150.0), None);
        assert_eq!(at(&page, 205.0, 5.0), Some(id(3)));
    }

    #[test]
    fn clips_inside_clips_keep_only_what_both_keep() {
        let page = list(vec![
            DisplayItem::PushClip {
                box_id: id(1),
                path: Path::rectangle(0.0, 0.0, 200.0, 50.0),
            },
            DisplayItem::PushClip {
                box_id: id(2),
                path: Path::rectangle(10.0, 10.0, 100.0, 100.0),
            },
            area_of(3, 10.0, 10.0, 300.0, 300.0),
            DisplayItem::PopClip,
            DisplayItem::PopClip,
        ]);
        assert_eq!(at(&page, 50.0, 48.5), Some(id(3)));
        assert_eq!(at(&page, 50.0, 50.0), None);
        assert_eq!(at(&page, 150.0, 30.0), None);
    }

    #[test]
    fn rounded_corners_are_not_there_to_point_at() {
        // Chrome 154: a 100 square with a 50 radius.
        let shape = rounded_rectangle(Rect::new(0.0, 0.0, 100.0, 100.0), Corners::all(50.0));
        let page = list(vec![DisplayItem::Area {
            box_id: id(1),
            path: shape,
        }]);
        assert_eq!(at(&page, 5.0, 5.0), None);
        assert_eq!(at(&page, 15.0, 15.0), Some(id(1)));
        assert_eq!(at(&page, 50.0, 1.0), Some(id(1)));
        assert_eq!(at(&page, 99.0, 50.0), Some(id(1)));
        assert_eq!(at(&page, 95.0, 95.0), None);
    }

    #[test]
    fn a_box_scaled_to_nothing_or_moved_by_no_number_is_nowhere() {
        for matrix in [
            Matrix {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                d: 0.0,
                e: 50.0,
                f: 50.0,
            },
            Matrix {
                a: f32::NAN,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            },
            Matrix {
                a: f32::INFINITY,
                b: 0.0,
                c: 0.0,
                d: f32::INFINITY,
                e: 0.0,
                f: 0.0,
            },
        ] {
            let page = list(vec![
                DisplayItem::PushTransform {
                    box_id: id(1),
                    matrix,
                },
                area_of(1, 0.0, 0.0, 100.0, 100.0),
                DisplayItem::PopTransform,
            ]);
            assert_eq!(at(&page, 50.0, 50.0), None, "{matrix:?}");
        }
    }

    #[test]
    fn a_list_that_closes_what_it_never_opened_is_read_without_panicking() {
        let page = list(vec![
            DisplayItem::PopClip,
            DisplayItem::PopTransform,
            DisplayItem::PopGroup,
            area_of(1, 0.0, 0.0, 10.0, 10.0),
            DisplayItem::PushClip {
                box_id: id(2),
                path: Path::new(),
            },
            area_of(2, 0.0, 0.0, 10.0, 10.0),
        ]);
        assert_eq!(
            at(&page, 5.0, 5.0),
            Some(id(1)),
            "an empty clip keeps nothing"
        );
        assert_eq!(at(&page, f32::NAN, 5.0), None);
        assert_eq!(at(&page, f32::MAX, f32::MAX), None);
    }

    #[test]
    fn a_contour_that_is_not_convex_is_found_within_its_bounds() {
        // An L, which no builder records, taken as the rectangle around it.
        let mut shape = Path::new();
        shape.move_to(Point::new(0.0, 0.0));
        shape.line_to(Point::new(100.0, 0.0));
        shape.line_to(Point::new(100.0, 10.0));
        shape.line_to(Point::new(10.0, 10.0));
        shape.line_to(Point::new(10.0, 100.0));
        shape.line_to(Point::new(0.0, 100.0));
        shape.close();
        let page = list(vec![DisplayItem::Area {
            box_id: id(1),
            path: shape,
        }]);
        assert_eq!(at(&page, 50.0, 50.0), Some(id(1)));
        assert_eq!(at(&page, 150.0, 50.0), None);
    }
}
