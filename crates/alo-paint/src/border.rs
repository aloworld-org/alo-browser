/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The borders that are more than one colour: `inset`, `outset`, `groove` and
//! `ridge`.
//!
//! Each is one colour drawn as two tones, a darker and a lighter, and which
//! side gets which is the whole of what makes it look raised or sunk. The
//! light is taken to come from the top left, the convention bevelled
//! interfaces have long used:
//!
//! - **`inset`** is sunk into the page: its top and left are in shadow (the
//!   darker tone), its bottom and right catch the light (the lighter one).
//! - **`outset`** is raised: the other way round.
//! - **`groove`** is a channel cut into the page: its **outer half** is drawn
//!   as `inset` and its **inner half** as `outset`.
//! - **`ridge`** is a groove turned inside out: outer half `outset`, inner
//!   half `inset`.
//!
//! So no two of the four are the same picture, and a test can say so.
//!
//! # The two tones
//!
//! CSS leaves the exact colours to the browser. Ours, in [`tones`]: the
//! darker tone takes a third off the brightest channel and scales the others
//! with it, so a blue border goes dark blue rather than grey; the lighter tone
//! adds a third, up to white. Black has no hue to keep, and lightens to a grey
//! a third of the way to white. The two tones are **never the same colour**,
//! for any colour — a bevel whose halves came out equal would be a solid
//! border nobody asked for.
//!
//! # The shape
//!
//! Where two sides of different colours meet, the corner is split along the
//! line through the border box's corner and the padding box's — a **mitre**.
//! Each side is the **wedge** of the box nearest it, counted in that side's
//! widths (see `wedge`), filled inside the ring the border makes: on a square
//! box that is a trapezoid, and on a rounded one it reaches as far into the
//! corner as the curve does, which is further than the padding box's
//! rectangle — a first version that stopped at that rectangle left a hole in
//! every rounded corner. Four overlapping rectangles, which is how a solid
//! border of differing sides is drawn, would give one side the whole corner,
//! and with two tones that is visible.
//!
//! Sides of the same colour are filled as **one** shape, so that the diagonal
//! between, say, an `inset` border's top and left is not a seam. Where two
//! different colours meet on the diagonal, each edge is anti-aliased on its
//! own, and the page shows faintly through that one line of pixels.
//!
//! A groove or ridge's outer half is clipped to the ring **half as thick** —
//! so the line between the halves follows a rounded corner's curve rather
//! than cutting across it.

use crate::corner::{Corners, ring};
use crate::display::DisplayItem;
use crate::paint::Paint;
use crate::path::{Path, Point};
use alo_box::BoxId;
use alo_layout::{Edges, Rect};
use alo_value::Rgba;

/// How far either tone moves from the colour it is made of, as a share of the
/// whole range of a channel.
const SHIFT: f32 = 1.0 / 3.0;

/// The kinds of line this engine draws a border side with.
///
/// `none` and `hidden` draw nothing, and `dashed`, `dotted` and `double` are
/// not implemented (queue item 266) — so none of them is here, and a side
/// with one of them is left undrawn rather than drawn as something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// One colour.
    Solid,
    /// Sunk: darker at the top and left.
    Inset,
    /// Raised: lighter at the top and left.
    Outset,
    /// Cut in: an `inset` outer half and an `outset` inner one.
    Groove,
    /// Standing up: an `outset` outer half and an `inset` inner one.
    Ridge,
}

impl Line {
    /// The line a `border-style` keyword asks for, if this engine draws it.
    ///
    /// The keyword is expected lowercased, which is how the style arrives.
    pub fn of(keyword: &str) -> Option<Self> {
        match keyword {
            "solid" => Some(Self::Solid),
            "inset" => Some(Self::Inset),
            "outset" => Some(Self::Outset),
            "groove" => Some(Self::Groove),
            "ridge" => Some(Self::Ridge),
            _ => None,
        }
    }
}

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

impl Side {
    /// Whether this side faces the light, which comes from the top left.
    fn faces_the_light(self) -> bool {
        matches!(self, Self::Top | Self::Left)
    }
}

/// One side to draw: which, with what line, in what colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnSide {
    /// Which side.
    pub side: Side,
    /// What kind of line.
    pub line: Line,
    /// The border's colour, before any toning.
    pub color: Rgba,
}

/// The darker and the lighter tone of a colour, in that order.
///
/// Alpha is kept: a translucent border's bevel is as translucent as it is.
pub fn tones(color: Rgba) -> (Rgba, Rgba) {
    let brightest = color.red.max(color.green).max(color.blue);
    if brightest <= 0.0 {
        return (color, Rgba::new(SHIFT, SHIFT, SHIFT, color.alpha));
    }
    let darker = (brightest - SHIFT).max(0.0) / brightest;
    let lighter = (brightest + SHIFT).min(1.0) / brightest;
    (scaled(color, darker), scaled(color, lighter))
}

/// The colours one side is drawn in: its outer half, then its inner half.
///
/// The same colour twice for every line but `groove` and `ridge`.
pub fn colors_of(drawn: DrawnSide) -> (Rgba, Rgba) {
    let (darker, lighter) = tones(drawn.color);
    let (sunk, raised) = if drawn.side.faces_the_light() {
        (darker, lighter)
    } else {
        (lighter, darker)
    };
    match drawn.line {
        Line::Solid => (drawn.color, drawn.color),
        Line::Inset => (sunk, sunk),
        Line::Outset => (raised, raised),
        Line::Groove => (sunk, raised),
        Line::Ridge => (raised, sunk),
    }
}

/// The display items for a border drawn as mitred sides.
///
/// `widths` are the border's own on every side, including those not drawn: a
/// side that is not drawn still has its width, and the sides beside it still
/// stop where it begins. A side with no width is not drawn whatever it asks
/// for, and its neighbours take the corner.
pub fn draw_mitred(
    box_id: BoxId,
    border_box: Rect,
    corners: Corners,
    widths: Edges,
    sides: &[DrawnSide],
    out: &mut Vec<DisplayItem>,
) {
    let sides: Vec<DrawnSide> = sides
        .iter()
        .copied()
        .filter(|drawn| width_of(widths, drawn.side) > 0.0 && !drawn.color.is_invisible())
        .collect();
    if sides.is_empty() {
        return;
    }
    // Every side across its whole width, in the colour of its inner half.
    // The clip is what makes a wedge a side: the ring the border makes.
    let whole: Vec<(Side, Rgba)> = sides
        .iter()
        .map(|drawn| (drawn.side, colors_of(*drawn).1))
        .collect();
    out.push(DisplayItem::PushClip {
        box_id,
        path: ring(border_box, corners, widths),
    });
    fill_by_color(box_id, border_box, widths, &whole, out);
    out.push(DisplayItem::PopClip);

    // Then the outer half over it, where that is a different colour. The
    // clip is what makes it a half: the ring half as thick as the border.
    let outer: Vec<(Side, Rgba)> = sides
        .iter()
        .filter_map(|drawn| {
            let (outer, inner) = colors_of(*drawn);
            (outer != inner).then_some((drawn.side, outer))
        })
        .collect();
    if outer.is_empty() {
        return;
    }
    let half = Edges {
        top: widths.top / 2.0,
        right: widths.right / 2.0,
        bottom: widths.bottom / 2.0,
        left: widths.left / 2.0,
    };
    out.push(DisplayItem::PushClip {
        box_id,
        path: ring(border_box, corners, half),
    });
    fill_by_color(box_id, border_box, widths, &outer, out);
    out.push(DisplayItem::PopClip);
}

/// One fill per colour, each the union of that colour's sides.
///
/// In the order each colour first appears, so the display list is the same
/// every time.
fn fill_by_color(
    box_id: BoxId,
    outer: Rect,
    widths: Edges,
    sides: &[(Side, Rgba)],
    out: &mut Vec<DisplayItem>,
) {
    let mut groups: Vec<(Rgba, Path)> = Vec::new();
    for (side, color) in sides {
        let shape = wedge(*side, outer, widths);
        if let Some((_, path)) = groups.iter_mut().find(|(held, _)| held == color) {
            path.extend(&shape);
        } else {
            groups.push((*color, shape));
        }
    }
    for (color, path) in groups {
        out.push(DisplayItem::Fill {
            box_id,
            path,
            paint: Paint::Solid(color),
        });
    }
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
fn wedge(side: Side, outer: Rect, widths: Edges) -> Path {
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

    let mut path = Path::new();
    let mut points = polygon.into_iter();
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
const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

/// The part of a convex polygon where a linear measure is not negative.
///
/// One edge of the polygon at a time: a corner on the kept side stays, and an
/// edge that crosses the line where the measure is zero is cut there. The
/// measure is linear, so where along the edge it crosses is exact.
fn kept(polygon: &[(f32, f32)], measure: impl Fn((f32, f32)) -> f32) -> Vec<(f32, f32)> {
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

fn width_of(widths: Edges, side: Side) -> f32 {
    match side {
        Side::Top => widths.top,
        Side::Right => widths.right,
        Side::Bottom => widths.bottom,
        Side::Left => widths.left,
    }
}

fn scaled(color: Rgba, by: f32) -> Rgba {
    Rgba::new(
        color.red * by,
        color.green * by,
        color.blue * by,
        color.alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: [Line; 4] = [Line::Inset, Line::Outset, Line::Groove, Line::Ridge];
    fn grey(level: u8) -> Rgba {
        Rgba::from_rgba8(level, level, level, 255)
    }

    #[test]
    fn the_two_tones_are_never_the_same_colour() {
        for level in 0..=255_u8 {
            let (darker, lighter) = tones(grey(level));
            assert_ne!(darker, lighter, "grey {level}");
        }
        for color in [
            Rgba::from_rgba8(255, 0, 0, 255),
            Rgba::from_rgba8(0, 0, 255, 255),
            Rgba::from_rgba8(1, 0, 0, 255),
            Rgba::from_rgba8(0, 128, 255, 128),
        ] {
            let (darker, lighter) = tones(color);
            assert_ne!(darker, lighter, "{color:?}");
        }
    }

    #[test]
    fn the_tones_are_a_third_either_way_and_keep_the_hue() {
        let (darker, lighter) = tones(grey(153)); // 0.6
        assert_eq!(darker.to_rgba8(), (68, 68, 68, 255));
        assert_eq!(lighter.to_rgba8(), (238, 238, 238, 255));

        // Blue stays blue: the channels keep their proportions.
        let (darker, lighter) = tones(Rgba::from_rgba8(0, 51, 204, 255));
        assert_eq!(darker.to_rgba8(), (0, 30, 119, 255));
        assert_eq!(lighter.to_rgba8(), (0, 64, 255, 255));
    }

    #[test]
    fn black_lightens_to_a_grey_and_white_darkens_to_one() {
        assert_eq!(
            tones(Rgba::BLACK),
            (Rgba::BLACK, Rgba::new(SHIFT, SHIFT, SHIFT, 1.0))
        );
        let (darker, lighter) = tones(Rgba::WHITE);
        assert_eq!(darker.to_rgba8(), (170, 170, 170, 255));
        assert_eq!(lighter, Rgba::WHITE);
    }

    #[test]
    fn a_translucent_border_keeps_its_alpha() {
        let (darker, lighter) = tones(Rgba::from_rgba8(100, 150, 200, 64));
        assert_eq!(darker.to_rgba8().3, 64);
        assert_eq!(lighter.to_rgba8().3, 64);
    }

    #[test]
    fn inset_is_dark_at_the_top_left_and_outset_the_other_way() {
        let color = grey(128);
        let (darker, lighter) = tones(color);
        let of = |line, side| colors_of(DrawnSide { side, line, color });
        for side in [Side::Top, Side::Left] {
            assert_eq!(of(Line::Inset, side), (darker, darker));
            assert_eq!(of(Line::Outset, side), (lighter, lighter));
            assert_eq!(of(Line::Groove, side), (darker, lighter));
            assert_eq!(of(Line::Ridge, side), (lighter, darker));
        }
        for side in [Side::Bottom, Side::Right] {
            assert_eq!(of(Line::Inset, side), (lighter, lighter));
            assert_eq!(of(Line::Outset, side), (darker, darker));
            assert_eq!(of(Line::Groove, side), (lighter, darker));
            assert_eq!(of(Line::Ridge, side), (darker, lighter));
        }
        assert_eq!(of(Line::Solid, Side::Top), (color, color));
    }

    #[test]
    fn no_two_lines_are_drawn_alike() {
        let color = grey(128);
        let picture = |line| SIDES.map(|side| colors_of(DrawnSide { side, line, color }));
        for (index, first) in LINES.iter().enumerate() {
            for second in LINES.iter().skip(index + 1) {
                assert_ne!(picture(*first), picture(*second), "{first:?} {second:?}");
            }
        }
    }

    #[test]
    fn only_the_lines_drawn_are_recognised() {
        assert_eq!(Line::of("groove"), Some(Line::Groove));
        assert_eq!(Line::of("solid"), Some(Line::Solid));
        for keyword in ["none", "hidden", "dashed", "dotted", "double", "GROOVE"] {
            assert_eq!(Line::of(keyword), None, "{keyword}");
        }
    }

    fn corners_of(path: &Path) -> Vec<(f32, f32)> {
        path.segments()
            .iter()
            .filter_map(|segment| match segment {
                crate::path::Segment::MoveTo(at) | crate::path::Segment::LineTo(at) => {
                    Some((at.x, at.y))
                }
                _ => None,
            })
            .collect()
    }

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
        let top = corners_of(&wedge(Side::Top, outer, UNEVEN));
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
        let left = corners_of(&wedge(Side::Left, outer, UNEVEN));
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
                .map(|side| twice_area(&corners_of(&wedge(*side, outer, widths))) / 2.0)
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
                let points = corners_of(&wedge(side, outer, widths));
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
        let left = corners_of(&wedge(Side::Left, outer, widths));
        assert!(left.contains(&(0.0, 0.0)), "{left:?}");
        let top = corners_of(&wedge(Side::Top, outer, widths));
        assert!(
            twice_area(&top).abs() < 0.001,
            "the top has nothing: {top:?}"
        );
    }

    #[test]
    fn a_side_with_no_width_or_no_colour_is_not_drawn() {
        let mut out = Vec::new();
        let widths = Edges {
            top: 0.0,
            right: 2.0,
            bottom: 2.0,
            left: 2.0,
        };
        draw_mitred(
            BoxId::from_index_for_tests(0),
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Corners::all(0.0),
            widths,
            &[
                DrawnSide {
                    side: Side::Top,
                    line: Line::Inset,
                    color: Rgba::BLACK,
                },
                DrawnSide {
                    side: Side::Right,
                    line: Line::Inset,
                    color: Rgba::TRANSPARENT,
                },
            ],
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn a_groove_is_its_inner_colour_then_its_outer_half_clipped() {
        let mut out = Vec::new();
        let color = grey(128);
        let (darker, lighter) = tones(color);
        draw_mitred(
            BoxId::from_index_for_tests(0),
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Corners::all(0.0),
            Edges::all(4.0),
            &SIDES.map(|side| DrawnSide {
                side,
                line: Line::Groove,
                color,
            }),
            &mut out,
        );
        let kinds: Vec<String> = out
            .iter()
            .map(|item| match item {
                DisplayItem::Fill {
                    paint: Paint::Solid(color),
                    ..
                } if *color == darker => "dark".to_owned(),
                DisplayItem::Fill {
                    paint: Paint::Solid(color),
                    ..
                } if *color == lighter => "light".to_owned(),
                DisplayItem::PushClip { path, .. } => format!("clip {:?}", path.bounds()),
                DisplayItem::PopClip => "pop".to_owned(),
                other => format!("{other:?}"),
            })
            .collect();
        // Inner halves, inside the border's ring: top and left lighter,
        // right and bottom darker. Then the outer halves the other way
        // round, inside the half-thick ring.
        assert_eq!(
            kinds,
            [
                "clip Some((0.0, 0.0, 20.0, 20.0))",
                "light",
                "dark",
                "pop",
                "clip Some((0.0, 0.0, 20.0, 20.0))",
                "dark",
                "light",
                "pop",
            ],
        );
    }
}
