/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which part of a box belongs to which side of its border.
//!
//! A border drawn side by side — any style but a plain `solid` — needs each
//! side's own region, so that where two sides differ the corner is split
//! between them rather than given to one.
//!
//! Where two sides of different colours meet, the corner is split along the
//! line through the border box's corner and the padding box's — a **mitre**.
//! Each side is the **wedge** of the box nearest it, counted in that side's
//! widths (see [`wedge`]), filled inside the ring the border makes: on a square
//! box that is a trapezoid, and on a rounded one it reaches as far into the
//! corner as the curve does, which is further than the padding box's
//! rectangle — a first version that stopped at that rectangle left a hole in
//! every rounded corner. Four overlapping rectangles, which is how a solid
//! border of differing sides is drawn, would give one side the whole corner,
//! and with two tones that is visible.
//!
//! # Joints
//!
//! A dash cut off on a mitre is a new corner on it, and the side across the
//! mitre must have that corner too, or two edges along one line give a seam
//! (see [`Joints`]).

use crate::path::{Path, Point};
use alo_layout::{Edges, Rect};

/// One side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The top.
    Top,
    /// The right.
    Right,
    /// The bottom.
    Bottom,
    /// The left.
    Left,
}

/// The part of the box that belongs to one side.
///
/// A point belongs to the side it is **fewest of that side's widths** from.
/// Between two sides that meet, the line where those are equal is the mitre:
/// it runs through the border box's corner and the padding box's. Between
/// two opposite sides it is a straight line across the box, the depths of the
/// two borders apart in proportion. So the four wedges are the whole box,
/// none overlaps another, and a side with no width has none — its neighbours
/// take its corners.
///
/// Every wedge is wound the same way round as the box, clockwise, so that
/// sides filled together are their union rather than cancelling where they
/// touch.
pub fn wedge(side: Side, outer: Rect, widths: Edges) -> Vec<(f32, f32)> {
    let (left, top, right, bottom) = (outer.left(), outer.top(), outer.right(), outer.bottom());
    // How far a point is in from each side's outer edge.
    let depth = move |of: Side, (x, y): (f32, f32)| match of {
        Side::Top => y - top,
        Side::Right => right - x,
        Side::Bottom => bottom - y,
        Side::Left => x - left,
    };
    let own = width_of(widths, side);
    let mut polygon = vec![(left, top), (right, top), (right, bottom), (left, bottom)];
    for other in SIDES.into_iter().filter(|other| *other != side) {
        let theirs = width_of(widths, other);
        // Nearer this side, in widths, than the other: depth / own is at
        // most depth / theirs, written without dividing by a width of zero.
        polygon = kept(&polygon, |point| {
            own * depth(other, point) - theirs * depth(side, point)
        });
    }
    polygon
}

/// A polygon as a closed path, in the order its corners are given.
pub fn polygon_path(polygon: &[(f32, f32)]) -> Path {
    let mut path = Path::new();
    let mut points = polygon.iter().copied();
    let Some((x, y)) = points.next() else {
        return path;
    };
    path.move_to(Point::new(x, y));
    for (x, y) in points {
        path.line_to(Point::new(x, y));
    }
    path.close();
    path
}

/// The four sides, in the order CSS lists them.
pub const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

/// The part of a convex polygon where a linear measure is not negative.
///
/// One edge of the polygon at a time: a corner on the kept side stays, and an
/// edge that crosses the line where the measure is zero is cut there. The
/// measure is linear, so where along the edge it crosses is exact.
pub fn kept(polygon: &[(f32, f32)], measure: impl Fn((f32, f32)) -> f32) -> Vec<(f32, f32)> {
    let mut kept = Vec::with_capacity(polygon.len() + 1);
    let ends = polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1));
    for (from, to) in ends {
        let (here, there) = (measure(from), measure(to));
        if here >= 0.0 {
            kept.push(from);
        }
        if (here > 0.0 && there < 0.0) || (here < 0.0 && there > 0.0) {
            let share = here / (here - there);
            kept.push((
                from.0 + (to.0 - from.0) * share,
                from.1 + (to.1 - from.1) * share,
            ));
        }
    }
    kept
}

/// The width of one side of a border.
pub fn width_of(widths: Edges, side: Side) -> f32 {
    match side {
        Side::Top => widths.top,
        Side::Right => widths.right,
        Side::Bottom => widths.bottom,
        Side::Left => widths.left,
    }
}

/// The points where pieces of a side are cut off on the mitres at its ends,
/// kept so that both wedges at each mitre share them.
///
/// Two sides of one colour are filled as one shape, and where they meet on
/// a mitre their edges cancel — but only if they are the same edges. A top
/// dash cut off at one point on the mitre and a right dash cut off at
/// another are two different edges along one line, and the rasteriser's
/// rounding of each puts a line of the page's colour between them: the first
/// version of this drew exactly that seam in every dashed corner. So every
/// point where a piece is cut off on a mitre is computed once, by
/// [`Joints::new`], and is both a corner of the piece cut there
/// ([`Joints::snap`]) and a corner of whatever lies across the mitre
/// ([`Joints::join`]).
///
/// Each mitre's points are kept in order along it, so a piece takes only the
/// ones beside it: the first version put every point into every wedge
/// before cutting it, and a hair-thin dashed border round a large box took
/// over a minute to draw.
#[derive(Debug, Clone, Default)]
pub struct Joints {
    /// For each corner, in [`CORNERS`]'s order: how far along the mitre and
    /// where, in order along it.
    corners: [Vec<(f32, (f32, f32))>; 4],
}

impl Joints {
    /// The joints of every cut: for each side, positions along it — across
    /// for the top and bottom, down for the left and right.
    pub fn new(cuts: &[(Side, Vec<f32>)], outer: Rect, widths: Edges) -> Self {
        let mut corners: [Vec<(f32, (f32, f32))>; 4] = Default::default();
        for (side, positions) in cuts {
            for corner in corners_of_side(*side) {
                let Some(mitre) = Mitre::of(corner, outer, widths) else {
                    continue;
                };
                let Some(held) = corner_index(corner).and_then(|index| corners.get_mut(index))
                else {
                    continue;
                };
                held.extend(
                    positions
                        .iter()
                        .map(|cut| mitre.joint(*side == corner.0, *cut)),
                );
            }
        }
        for held in &mut corners {
            held.sort_by(|a, b| a.0.total_cmp(&b.0));
            held.dedup_by(|a, b| a.1 == b.1);
        }
        Self { corners }
    }

    /// A polygon of `side`'s, with the joints that lie inside its edges on
    /// the mitres at that side's ends put into them.
    pub fn join(
        &self,
        side: Side,
        polygon: &[(f32, f32)],
        outer: Rect,
        widths: Edges,
    ) -> Vec<(f32, f32)> {
        joined(polygon, &self.beside(side, polygon, outer, widths))
    }

    /// The joints on the mitres at `side`'s ends that are as far along them
    /// as the polygon reaches, which is all a polygon of that side could
    /// have on its edges.
    fn beside(
        &self,
        side: Side,
        polygon: &[(f32, f32)],
        outer: Rect,
        widths: Edges,
    ) -> Vec<(f32, f32)> {
        let mut nearby = Vec::new();
        for corner in corners_of_side(side) {
            let (Some(mitre), Some(held)) = (
                Mitre::of(corner, outer, widths),
                corner_index(corner).and_then(|index| self.corners.get(index)),
            ) else {
                continue;
            };
            // Only the joints as far along the mitre as the polygon reaches
            // along its own side: across for the top and bottom, down for
            // the left and right. Measured the other way, a piece of the
            // right side reaches as far across as its wedge goes deep, and
            // takes every joint there is. A hundredth of a pixel either way
            // is rounding.
            let across = side == corner.0;
            let margin = 0.01 / if across { mitre.step_x } else { mitre.step_y }.abs();
            let (low, high) = polygon
                .iter()
                .map(|point| mitre.along(across, *point))
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), at| {
                    (low.min(at), high.max(at))
                });
            let first = held.partition_point(|(at, _)| *at < low - margin);
            let last = held.partition_point(|(at, _)| *at <= high + margin);
            if let Some(found) = held.get(first..last) {
                nearby.extend(found.iter().map(|(_, point)| *point));
            }
        }
        nearby
    }

    /// A piece cut from `side`'s wedge across at `cut`, with the corner the
    /// cut made on a mitre moved onto that mitre's joint exactly.
    ///
    /// Cutting a polygon finds where an edge crosses by arithmetic that
    /// rounds, so the corner it makes is near the joint rather than on it.
    pub fn snap(side: Side, cut: f32, piece: &mut [(f32, f32)], outer: Rect, widths: Edges) {
        for corner in corners_of_side(side) {
            let Some(mitre) = Mitre::of(corner, outer, widths) else {
                continue;
            };
            let (_, exact) = mitre.joint(side == corner.0, cut);
            for point in piece.iter_mut() {
                if (point.0 - exact.0).abs() < 0.001 && (point.1 - exact.1).abs() < 0.001 {
                    *point = exact;
                }
            }
        }
    }
}

/// The four corners, each as its horizontal side and its vertical one, in
/// the order [`Joints`] keeps them.
const CORNERS: [(Side, Side); 4] = [
    (Side::Top, Side::Left),
    (Side::Top, Side::Right),
    (Side::Bottom, Side::Right),
    (Side::Bottom, Side::Left),
];

fn corner_index(corner: (Side, Side)) -> Option<usize> {
    CORNERS.iter().position(|held| *held == corner)
}

/// One corner's mitre: the border box's corner, and the step inwards along
/// it — the vertical side's width across for the horizontal side's width
/// down.
#[derive(Debug, Clone, Copy)]
struct Mitre {
    x: f32,
    y: f32,
    step_x: f32,
    step_y: f32,
}

impl Mitre {
    /// The mitre at a corner, or `None` where either side has no width and
    /// the mitre lies along the other side's edge.
    fn of(corner: (Side, Side), outer: Rect, widths: Edges) -> Option<Self> {
        let (horizontal, vertical) = corner;
        let across = width_of(widths, vertical);
        let down = width_of(widths, horizontal);
        if across <= 0.0 || down <= 0.0 {
            return None;
        }
        let (x, step_x) = if vertical == Side::Left {
            (outer.left(), across)
        } else {
            (outer.right(), -across)
        };
        let (y, step_y) = if horizontal == Side::Top {
            (outer.top(), down)
        } else {
            (outer.bottom(), -down)
        };
        Some(Self {
            x,
            y,
            step_x,
            step_y,
        })
    }

    /// How many steps along the mitre a point is, by its `x` when `across`
    /// and by its `y` otherwise.
    fn along(self, across: bool, (x, y): (f32, f32)) -> f32 {
        if across {
            (x - self.x) / self.step_x
        } else {
            (y - self.y) / self.step_y
        }
    }

    /// Where a cut crosses the mitre: how many steps along, and the point.
    ///
    /// The cut's own coordinate is kept exactly — a top dash's `x`, a right
    /// dash's `y` — and only the other is worked out.
    fn joint(self, across: bool, cut: f32) -> (f32, (f32, f32)) {
        if across {
            let at = self.along(true, (cut, self.y));
            (at, (cut, self.y + at * self.step_y))
        } else {
            let at = self.along(false, (self.x, cut));
            (at, (self.x + at * self.step_x, cut))
        }
    }
}

/// The two corners at the ends of a side, each as its horizontal side and
/// its vertical one.
fn corners_of_side(side: Side) -> [(Side, Side); 2] {
    match side {
        Side::Top => [(Side::Top, Side::Left), (Side::Top, Side::Right)],
        Side::Bottom => [(Side::Bottom, Side::Left), (Side::Bottom, Side::Right)],
        Side::Left => [(Side::Top, Side::Left), (Side::Bottom, Side::Left)],
        Side::Right => [(Side::Top, Side::Right), (Side::Bottom, Side::Right)],
    }
}

/// A polygon with every joint that lies inside one of its edges put into
/// that edge, in order along it.
///
/// A joint that is not on an edge — on another mitre, or beyond where this
/// wedge's mitre ends — is left out, so the shape is the same shape with
/// more corners.
pub fn joined(polygon: &[(f32, f32)], joints: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if joints.is_empty() {
        return polygon.to_vec();
    }
    let mut out = Vec::with_capacity(polygon.len() + joints.len());
    let ends = polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1));
    for (from, to) in ends {
        out.push(from);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 0.0 {
            continue;
        }
        let mut inside: Vec<(f32, (f32, f32))> = joints
            .iter()
            .filter_map(|point| {
                let (px, py) = (point.0 - from.0, point.1 - from.1);
                let along = (px * dx + py * dy) / length_squared;
                // How far off the edge's line, in pixels: a thousandth is
                // rounding, anything more is a point on some other line.
                let off = (px * dy - py * dx).abs() / length_squared.sqrt();
                (off < 0.001 && along > 0.0 && along < 1.0 && *point != to)
                    .then_some((along, *point))
            })
            .collect();
        inside.sort_by(|a, b| a.0.total_cmp(&b.0));
        inside.dedup_by(|a, b| a.1 == b.1);
        out.extend(inside.into_iter().map(|(_, point)| point));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twice a polygon's area, positive when it is wound clockwise on a page
    /// whose y grows downwards.
    fn twice_area(points: &[(f32, f32)]) -> f32 {
        let ends = points.iter().zip(points.iter().cycle().skip(1));
        ends.map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum()
    }

    fn near(points: &[(f32, f32)], expected: &[(f32, f32)]) -> bool {
        points.len() == expected.len()
            && points
                .iter()
                .zip(expected)
                .all(|(a, b)| (a.0 - b.0).abs() < 0.001 && (a.1 - b.1).abs() < 0.001)
    }

    const UNEVEN: Edges = Edges {
        top: 4.0,
        right: 6.0,
        bottom: 8.0,
        left: 10.0,
    };

    #[test]
    fn a_side_is_cut_by_the_mitres_at_its_ends_and_by_the_side_opposite() {
        let outer = Rect::new(0.0, 0.0, 100.0, 50.0);
        // The top's mitres run from (0, 0) through (10, 4), and from
        // (100, 0) through (94, 4). The top and the bottom split the height
        // four to eight, at 16.67.
        let top = wedge(Side::Top, outer, UNEVEN);
        assert!(
            near(
                &top,
                &[
                    (0.0, 0.0),
                    (100.0, 0.0),
                    (75.0, 50.0 / 3.0),
                    (125.0 / 3.0, 50.0 / 3.0)
                ],
            ),
            "{top:?}",
        );
        // The left's mitres, from (0, 50) through (10, 42) and from (0, 0)
        // through (10, 4); the left and the right split the width ten to
        // six, at 62.5 — which is further in than the mitres meet.
        let left = wedge(Side::Left, outer, UNEVEN);
        assert!(
            left.contains(&(0.0, 50.0)) && left.contains(&(0.0, 0.0)),
            "{left:?}"
        );
    }

    #[test]
    fn the_four_sides_are_the_whole_box_and_none_overlaps_another() {
        for (outer, widths) in [
            (Rect::new(0.0, 0.0, 100.0, 50.0), UNEVEN),
            (Rect::new(3.0, 7.0, 40.0, 30.0), Edges::all(5.0)),
            // Borders deeper than the box is wide: the cut between the left
            // and the right is what keeps their wedges apart.
            (
                Rect::new(0.0, 0.0, 30.0, 200.0),
                Edges {
                    top: 2.0,
                    right: 15.0,
                    bottom: 2.0,
                    left: 15.0,
                },
            ),
        ] {
            let total: f32 = SIDES
                .iter()
                .map(|side| twice_area(&wedge(*side, outer, widths)) / 2.0)
                .sum();
            let whole = outer.size.width * outer.size.height;
            assert!((total - whole).abs() < 0.01, "{total} of {whole}");
        }
    }

    #[test]
    fn every_side_is_wound_clockwise() {
        let outer = Rect::new(0.0, 0.0, 40.0, 30.0);
        for widths in [Edges::all(5.0), UNEVEN] {
            for side in SIDES {
                let points = wedge(side, outer, widths);
                assert!(twice_area(&points) > 0.0, "{side:?}: {points:?}");
            }
        }
    }

    #[test]
    fn a_side_with_no_width_gives_its_corners_to_its_neighbours() {
        let outer = Rect::new(0.0, 0.0, 40.0, 30.0);
        let widths = Edges {
            top: 0.0,
            ..Edges::all(5.0)
        };
        // With no top, the left's mitre at the top left is along the top
        // edge, so the left reaches the box's very corner.
        let left = wedge(Side::Left, outer, widths);
        assert!(left.contains(&(0.0, 0.0)), "{left:?}");
        let top = wedge(Side::Top, outer, widths);
        assert!(
            twice_area(&top).abs() < 0.001,
            "the top has nothing: {top:?}"
        );
    }

    #[test]
    fn a_joint_on_an_edge_becomes_a_corner_of_it_and_one_elsewhere_does_not() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let found = joined(&square, &[(4.0, 0.0), (2.0, 0.0), (5.0, 5.0), (10.0, 0.0)]);
        // Both points on the top, in order along it; the one in the middle
        // and the one already a corner left out.
        assert_eq!(
            found,
            [
                (0.0, 0.0),
                (2.0, 0.0),
                (4.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0)
            ]
        );
    }

    #[test]
    fn a_cut_on_one_side_is_a_joint_on_both_wedges_at_its_mitre() {
        let outer = Rect::new(0.0, 0.0, 100.0, 50.0);
        let widths = Edges::all(5.0);
        // A cut 97 across the top lands on the top right mitre at (97, 3).
        let joints = Joints::new(&[(Side::Top, vec![97.0])], outer, widths);
        let top = joints.join(Side::Top, &wedge(Side::Top, outer, widths), outer, widths);
        let right = joints.join(
            Side::Right,
            &wedge(Side::Right, outer, widths),
            outer,
            widths,
        );
        assert!(top.contains(&(97.0, 3.0)), "{top:?}");
        assert!(right.contains(&(97.0, 3.0)), "{right:?}");
        // The left and bottom are nowhere near it.
        let left = joints.join(Side::Left, &wedge(Side::Left, outer, widths), outer, widths);
        assert_eq!(left, wedge(Side::Left, outer, widths));
    }

    #[test]
    fn a_piece_cut_on_a_mitre_is_snapped_onto_the_joint() {
        let outer = Rect::new(0.0, 0.0, 100.0, 50.0);
        let widths = Edges {
            top: 3.0,
            right: 7.0,
            ..Edges::all(5.0)
        };
        let cut = 95.3;
        let mut piece = kept(&wedge(Side::Top, outer, widths), |(x, _)| x - cut);
        Joints::snap(Side::Top, cut, &mut piece, outer, widths);
        let joints = Joints::new(&[(Side::Top, vec![cut])], outer, widths);
        let right = joints.join(
            Side::Right,
            &wedge(Side::Right, outer, widths),
            outer,
            widths,
        );
        // The very same point, to the bit, in the piece and across the mitre.
        let shared: Vec<&(f32, f32)> = piece.iter().filter(|point| right.contains(point)).collect();
        assert!(
            shared.iter().any(|point| (point.0 - cut).abs() < 0.001),
            "{piece:?} {right:?}"
        );
    }

    #[test]
    fn a_side_with_no_width_has_no_mitre_to_join() {
        let outer = Rect::new(0.0, 0.0, 100.0, 50.0);
        let widths = Edges {
            right: 0.0,
            ..Edges::all(5.0)
        };
        let joints = Joints::new(&[(Side::Top, vec![3.0, 97.0])], outer, widths);
        // Only the top left mitre: the right has no width.
        let top = joints.join(Side::Top, &wedge(Side::Top, outer, widths), outer, widths);
        assert_eq!(
            top.len(),
            wedge(Side::Top, outer, widths).len() + 1,
            "{top:?}"
        );
    }

    #[test]
    fn a_piece_takes_only_the_joints_beside_it() {
        // Every side cut every three pixels round a box 100 000 square, with
        // mitres running to its middle: a piece from the middle of the right
        // side is beside a couple of joints at most, not the thousands on
        // its mitres. Taking all of them was what made a hair-thin dashed
        // border take over a minute.
        let outer = Rect::new(0.0, 0.0, 100_000.0, 100_000.0);
        let widths = Edges::all(0.01);
        let cuts: Vec<f32> = (0..30_000_u16).map(|step| f32::from(step) * 3.0).collect();
        let joints = Joints::new(&SIDES.map(|side| (side, cuts.clone())), outer, widths);
        for side in SIDES {
            let along = |point: (f32, f32)| match side {
                Side::Top | Side::Bottom => point.0,
                Side::Left | Side::Right => point.1,
            };
            let piece = kept(&wedge(side, outer, widths), |point| along(point) - 40_000.0);
            let piece = kept(&piece, |point| 40_003.0 - along(point));
            let beside = joints.beside(side, &piece, outer, widths);
            assert!(beside.len() <= 8, "{side:?}: {} joints", beside.len());
        }
    }
}
