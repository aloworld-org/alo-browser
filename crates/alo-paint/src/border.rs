/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A border drawn side by side: each side the mitred wedge of the box that is
//! its own ([`crate::mitre`]), so that a corner splits where two sides differ.
//!
//! Every border style but a plain `solid` one is drawn this way. Four of them
//! are two tones of one colour — `inset`, `outset`, `groove` and `ridge`,
//! whose colours are [`crate::tone`]'s — and three are a pattern along the
//! side — `dashed`, `dotted` and `double`, whose spacing is
//! [`crate::pattern`]'s. This file is what each side is drawn as, and in
//! what order.
//!
//! # The toned four
//!
//! Each side is its wedge, filled inside the ring the border makes. Sides of
//! the same colour are filled as **one** shape, so that the diagonal
//! between, say, an `inset` border's top and left is not a seam. Where two
//! different colours meet on the diagonal, each edge is anti-aliased on its
//! own, and the page shows faintly through that one line of pixels.
//!
//! A groove or ridge's outer half is clipped to the ring **half as thick** —
//! so the line between the halves follows a rounded corner's curve rather
//! than cutting across it.
//!
//! # The patterned three
//!
//! - A **dash** is the side's wedge cut across at the dash's two ends, so the
//!   first and last dash of a side are the corners' mitred halves.
//! - A **dot** is a circle on the line through the middle of the side,
//!   clipped to the wedges of every dotted side of its colour together — so a
//!   dot centred on a corner's mitre, which is where two sides of one width
//!   both put one, is one round dot rather than two halves with a seam.
//! - A **`double`** side is its wedge clipped to two rings each a third as
//!   thick as the border: the outer third, and the inner third, whose outer
//!   edge's corners are the border box's less two thirds of the border — so
//!   both lines follow a rounded corner.
//!
//! Along a rounded corner a dash or a dot is still placed on the straight
//! side, and the curve clips it; spacing them along the curve itself is not
//! done.

use crate::corner::{Corners, ring, rounded_rectangle};
use crate::display::DisplayItem;
use crate::fill_rule::FillRule;
use crate::mitre::{Joints, kept, polygon_path, wedge, width_of};
use crate::paint::Paint;
use crate::path::Path;
use crate::pattern;
use crate::tone::colors_of;
use alo_box::BoxId;
use alo_layout::{Edges, Rect};
use alo_value::Rgba;

/// The kinds of line this engine draws a border side with.
///
/// `none` and `hidden` draw nothing, so neither is here, and a side with one
/// of them — or with a keyword this engine does not know — is left undrawn
/// rather than drawn as something else.
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
    /// Dashes, starting and ending on one.
    Dashed,
    /// Round dots, starting and ending on one.
    Dotted,
    /// Two lines, a third of the width each.
    Double,
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
            "dashed" => Some(Self::Dashed),
            "dotted" => Some(Self::Dotted),
            "double" => Some(Self::Double),
            _ => None,
        }
    }
}

pub use crate::mitre::Side;

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

/// The display items for a border drawn as mitred sides.
///
/// `widths` are the border's own on every side, including those not drawn: a
/// side that is not drawn still has its width, and the sides beside it still
/// stop where it begins. A side with no width is not drawn whatever it asks
/// for, and its neighbours take the corner.
///
/// In four layers, each one clip and one fill per colour: the sides drawn
/// across their whole width (solid, the toned sides' inner colour, and
/// dashes); the toned sides' outer halves; the two lines of `double`; and
/// the dots.
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
    let whole = ring(border_box, corners, widths);

    // Across the whole width, inside the ring the border makes: the clip is
    // what makes a wedge a side. Where a dash is cut off on a mitre, the
    // point is put into both wedges that meet there (see `joined`).
    let joints = joints(&sides, border_box, widths);
    let mut across = Vec::new();
    for drawn in &sides {
        let own = wedge(drawn.side, border_box, widths);
        let shape = match drawn.line {
            Line::Dashed => dashed(drawn.side, &own, &joints, border_box, widths),
            Line::Dotted | Line::Double => continue,
            _ => polygon_path(&joints.join(drawn.side, &own, border_box, widths)),
        };
        add(&mut across, colors_of(*drawn).1, &shape);
    }
    clipped(box_id, &whole, &across, out);

    // The toned sides' outer halves over that, where they are a different
    // colour. The clip is what makes it a half: the ring half as thick.
    let mut outer = Vec::new();
    for drawn in &sides {
        let (outside, inside) = colors_of(*drawn);
        if outside != inside {
            let shape = polygon_path(&wedge(drawn.side, border_box, widths));
            add(&mut outer, outside, &shape);
        }
    }
    let half = scaled_edges(widths, 0.5);
    clipped(box_id, &ring(border_box, corners, half), &outer, out);

    // The two lines of `double`: the outer third ring and the inner one, as
    // one clip.
    let mut double = Vec::new();
    for drawn in sides.iter().filter(|drawn| drawn.line == Line::Double) {
        let shape = polygon_path(&wedge(drawn.side, border_box, widths));
        add(&mut double, drawn.color, &shape);
    }
    if !double.is_empty() {
        let third = pattern::third(widths);
        let two_thirds = scaled_edges(widths, 2.0 / 3.0);
        let mut lines = ring(border_box, corners, third);
        lines.extend(&ring(
            border_box.shrunk_by(two_thirds),
            corners.inside(two_thirds),
            third,
        ));
        clipped(box_id, &lines, &double, out);
    }

    // The dots, inside the ring and inside the wedges of their colour.
    let mut dots: Vec<(Rgba, Path, Path)> = Vec::new();
    for drawn in sides.iter().filter(|drawn| drawn.line == Line::Dotted) {
        let shape = dotted(drawn.side, border_box, widths);
        let owned = polygon_path(&wedge(drawn.side, border_box, widths));
        if let Some((_, held, wedges)) = dots.iter_mut().find(|(color, ..)| *color == drawn.color) {
            held.extend(&shape);
            wedges.extend(&owned);
        } else {
            dots.push((drawn.color, shape, owned));
        }
    }
    if !dots.is_empty() {
        out.push(DisplayItem::PushClip {
            box_id,
            path: whole,
        });
        for (color, shape, wedges) in dots {
            clipped(box_id, &wedges, &[(color, shape)], out);
        }
        out.push(DisplayItem::PopClip);
    }
}

/// A shape added to the fill of its colour, or as a new one.
///
/// In the order each colour first appears, so the display list is the same
/// every time. An empty shape adds nothing.
fn add(fills: &mut Vec<(Rgba, Path)>, color: Rgba, shape: &Path) {
    if shape.is_empty() {
        return;
    }
    if let Some((_, path)) = fills.iter_mut().find(|(held, _)| *held == color) {
        path.extend(shape);
    } else {
        fills.push((color, shape.clone()));
    }
}

/// One fill per colour, inside a clip — or nothing at all, clip included,
/// when there is nothing to fill.
fn clipped(box_id: BoxId, clip: &Path, fills: &[(Rgba, Path)], out: &mut Vec<DisplayItem>) {
    if fills.is_empty() {
        return;
    }
    out.push(DisplayItem::PushClip {
        box_id,
        path: clip.clone(),
    });
    for (color, path) in fills {
        out.push(DisplayItem::Fill {
            rule: FillRule::NonZero,
            box_id,
            path: path.clone(),
            paint: Paint::Solid(*color),
        });
    }
    out.push(DisplayItem::PopClip);
}

/// Where a side starts, how long it is, and how far along it a point is —
/// across for the top and bottom, down for the left and right.
fn run_of(side: Side, outer: Rect) -> (f32, f32, impl Fn((f32, f32)) -> f32) {
    let across = matches!(side, Side::Top | Side::Bottom);
    let (start, length) = if across {
        (outer.left(), outer.size.width)
    } else {
        (outer.top(), outer.size.height)
    };
    (
        start,
        length,
        move |(x, y): (f32, f32)| {
            if across { x } else { y }
        },
    )
}

/// A dashed side: its wedge, cut across at each dash's ends.
///
/// Each piece's corners on a mitre are the joints there, so that a piece
/// and whatever lies across the mitre share their edge (see
/// [`Joints`]).
fn dashed(side: Side, own: &[(f32, f32)], joints: &Joints, outer: Rect, widths: Edges) -> Path {
    let (start, length, along) = run_of(side, outer);
    let mut path = Path::new();
    for (from, to) in pattern::dashes(length, width_of(widths, side)) {
        let (from, to) = (start + from, start + to);
        let piece = kept(own, |point| along(point) - from);
        let mut piece = kept(&piece, |point| to - along(point));
        Joints::snap(side, from, &mut piece, outer, widths);
        Joints::snap(side, to, &mut piece, outer, widths);
        path.extend(&polygon_path(&joints.join(side, &piece, outer, widths)));
    }
    path
}

/// Every point where a dash is cut off on a mitre, for both wedges that
/// meet there to share.
fn joints(sides: &[DrawnSide], outer: Rect, widths: Edges) -> Joints {
    let cuts: Vec<(Side, Vec<f32>)> = sides
        .iter()
        .filter(|drawn| drawn.line == Line::Dashed)
        .map(|drawn| {
            let (start, length, _) = run_of(drawn.side, outer);
            let cuts = pattern::dashes(length, width_of(widths, drawn.side))
                .into_iter()
                .flat_map(|(from, to)| [start + from, start + to])
                .collect();
            (drawn.side, cuts)
        })
        .collect();
    Joints::new(&cuts, outer, widths)
}

/// A dotted side: a circle as wide as the side for each dot, centred on the
/// line through the middle of the side.
fn dotted(side: Side, outer: Rect, widths: Edges) -> Path {
    let width = width_of(widths, side);
    let radius = width / 2.0;
    let (start, length, _) = run_of(side, outer);
    let mut path = Path::new();
    for at in pattern::dots(length, width) {
        let (x, y) = match side {
            Side::Top => (start + at, outer.top() + radius),
            Side::Bottom => (start + at, outer.bottom() - radius),
            Side::Left => (outer.left() + radius, start + at),
            Side::Right => (outer.right() - radius, start + at),
        };
        path.extend(&rounded_rectangle(
            Rect::new(x - radius, y - radius, width, width),
            Corners::all(radius),
        ));
    }
    path
}

fn scaled_edges(widths: Edges, by: f32) -> Edges {
    Edges {
        top: widths.top * by,
        right: widths.right * by,
        bottom: widths.bottom * by,
        left: widths.left * by,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mitre::SIDES;
    use crate::path::Point;
    use crate::tone::tones;

    #[test]
    fn only_the_lines_drawn_are_recognised() {
        assert_eq!(Line::of("groove"), Some(Line::Groove));
        assert_eq!(Line::of("solid"), Some(Line::Solid));
        assert_eq!(Line::of("dashed"), Some(Line::Dashed));
        assert_eq!(Line::of("dotted"), Some(Line::Dotted));
        assert_eq!(Line::of("double"), Some(Line::Double));
        for keyword in ["none", "hidden", "wavy", "GROOVE", ""] {
            assert_eq!(Line::of(keyword), None, "{keyword}");
        }
    }

    fn grey(level: u8) -> Rgba {
        Rgba::from_rgba8(level, level, level, 255)
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

    /// Each closed outline in a path, as the points its segments end at.
    fn outlines(path: &Path) -> Vec<Vec<(f32, f32)>> {
        use crate::path::Segment;
        let mut found: Vec<Vec<(f32, f32)>> = Vec::new();
        for segment in path.segments() {
            match segment {
                Segment::MoveTo(at) => found.push(vec![(at.x, at.y)]),
                Segment::LineTo(at) | Segment::QuadTo(_, at) | Segment::CubicTo(_, _, at) => {
                    found.last_mut().unwrap().push((at.x, at.y));
                }
                Segment::Close => {}
            }
        }
        found
    }

    fn kinds(out: &[DisplayItem]) -> Vec<&'static str> {
        out.iter()
            .map(|item| match item {
                DisplayItem::PushClip { .. } => "clip",
                DisplayItem::PopClip => "pop",
                DisplayItem::Fill { .. } => "fill",
                _ => "other",
            })
            .collect()
    }

    fn all_sides(line: Line, color: Rgba) -> [DrawnSide; 4] {
        SIDES.map(|side| DrawnSide { side, line, color })
    }

    #[test]
    fn a_dashed_side_is_its_wedge_cut_across_and_its_ends_take_the_corners() {
        let outer = Rect::new(0.0, 0.0, 100.0, 50.0);
        let widths = Edges::all(2.0);
        let own = wedge(Side::Top, outer, widths);
        let pieces = outlines(&dashed(Side::Top, &own, &Joints::default(), outer, widths));
        // 100 long and 2 wide is nine dashes (see `pattern`).
        assert_eq!(pieces.len(), 9);
        let first = corners_of(&dashed(Side::Top, &own, &Joints::default(), outer, widths));
        assert!(
            first.contains(&(0.0, 0.0)),
            "the top left corner: {first:?}"
        );
        assert!(first.contains(&(100.0, 0.0)), "the top right: {first:?}");
        // The first dash is the mitred corner: the wedge's corner up to the
        // dash's end, 100 / 17 along. The ring clips it to the border's
        // depth when it is drawn.
        let unit = 100.0 / 17.0;
        let start = pieces.first().unwrap();
        assert!(
            near(start, &[(0.0, 0.0), (unit, 0.0), (unit, unit)]),
            "{start:?}"
        );
        // Every piece is wound clockwise, like the wedge it was cut from.
        for piece in &pieces {
            assert!(twice_area(piece) > 0.0, "{piece:?}");
        }
    }

    #[test]
    fn dashes_cover_less_than_their_side_and_never_overlap_another() {
        let outer = Rect::new(0.0, 0.0, 120.0, 60.0);
        for widths in [Edges::all(3.0), UNEVEN] {
            let mut total = 0.0;
            for side in SIDES {
                let wedge_area = twice_area(&wedge(side, outer, widths)) / 2.0;
                let own = wedge(side, outer, widths);
                let dash_area: f32 =
                    outlines(&dashed(side, &own, &Joints::default(), outer, widths))
                        .iter()
                        .map(|piece| twice_area(piece) / 2.0)
                        .sum();
                assert!(
                    dash_area > 0.2 * wedge_area && dash_area < 0.8 * wedge_area,
                    "{side:?}: {dash_area} of {wedge_area}",
                );
                total += dash_area;
            }
            assert!(total < outer.size.width * outer.size.height, "{total}");
        }
    }

    #[test]
    fn a_dotted_side_is_one_circle_per_dot_on_its_middle_line() {
        let outer = Rect::new(10.0, 20.0, 42.0, 30.0);
        let path = dotted(Side::Top, outer, Edges::all(2.0));
        // Each circle is a move, four arcs and a close.
        let circles = path
            .segments()
            .iter()
            .filter(|segment| matches!(segment, crate::path::Segment::MoveTo(_)))
            .count();
        assert_eq!(circles, pattern::dots(42.0, 2.0).len());
        assert_eq!(path.bounds(), Some((10.0, 20.0, 52.0, 22.0)));
        let left = dotted(Side::Left, outer, Edges::all(2.0));
        assert_eq!(left.bounds(), Some((10.0, 20.0, 12.0, 50.0)));
    }

    #[test]
    fn dots_are_clipped_to_the_ring_and_then_to_the_wedges_of_their_colour() {
        let mut out = Vec::new();
        let mut sides = all_sides(Line::Dotted, grey(0)).to_vec();
        if let Some(bottom) = sides.get_mut(2) {
            bottom.color = grey(200);
        }
        draw_mitred(
            BoxId::from_index_for_tests(0),
            Rect::new(0.0, 0.0, 40.0, 40.0),
            Corners::all(0.0),
            Edges::all(4.0),
            &sides,
            &mut out,
        );
        assert_eq!(
            kinds(&out),
            ["clip", "clip", "fill", "pop", "clip", "fill", "pop", "pop"]
        );
    }

    #[test]
    fn a_double_border_is_one_fill_inside_two_rings_a_third_thick() {
        let mut out = Vec::new();
        draw_mitred(
            BoxId::from_index_for_tests(0),
            Rect::new(0.0, 0.0, 30.0, 30.0),
            Corners::all(0.0),
            Edges::all(6.0),
            &all_sides(Line::Double, grey(0)),
            &mut out,
        );
        assert_eq!(kinds(&out), ["clip", "fill", "pop"]);
        // Two rings, each an outline and a hole: four outlines, the outer
        // ring from 0 to 2 in and the inner from 4 to 6.
        let Some(DisplayItem::PushClip { path, .. }) = out.first() else {
            panic!("{out:?}");
        };
        let edges: Vec<Option<(f32, f32, f32, f32)>> = outlines(path)
            .iter()
            .map(|outline| {
                let mut each = Path::new();
                each.move_to(Point::new(outline[0].0, outline[0].1));
                for (x, y) in outline {
                    each.line_to(Point::new(*x, *y));
                }
                each.bounds()
            })
            .collect();
        assert_eq!(
            edges,
            [
                Some((0.0, 0.0, 30.0, 30.0)),
                Some((2.0, 2.0, 28.0, 28.0)),
                Some((4.0, 4.0, 26.0, 26.0)),
                Some((6.0, 6.0, 24.0, 24.0)),
            ]
        );
    }

    #[test]
    fn sides_of_every_kind_together_are_drawn_in_their_own_layers() {
        let mut out = Vec::new();
        let color = grey(128);
        draw_mitred(
            BoxId::from_index_for_tests(0),
            Rect::new(0.0, 0.0, 40.0, 40.0),
            Corners::all(0.0),
            Edges::all(6.0),
            &[
                DrawnSide {
                    side: Side::Top,
                    line: Line::Groove,
                    color,
                },
                DrawnSide {
                    side: Side::Right,
                    line: Line::Dashed,
                    color,
                },
                DrawnSide {
                    side: Side::Bottom,
                    line: Line::Double,
                    color,
                },
                DrawnSide {
                    side: Side::Left,
                    line: Line::Dotted,
                    color,
                },
            ],
            &mut out,
        );
        assert_eq!(
            kinds(&out),
            [
                // The groove's inner half and the dashes.
                "clip", "fill", "fill", "pop", // The groove's outer half.
                "clip", "fill", "pop", // The double.
                "clip", "fill", "pop", // The dots.
                "clip", "clip", "fill", "pop", "pop",
            ]
        );
    }

    #[test]
    fn a_dash_cut_off_on_a_mitre_shares_its_corner_to_the_bit_with_the_side_across() {
        // Awkward numbers, so that cutting a wedge rounds: every corner of a
        // top dash that lies on the top right mitre must be exactly a corner
        // of the right side's wedge, or the two edges along the mitre are
        // different edges and the rasteriser leaves a seam between them.
        let outer = Rect::new(0.3, 0.7, 97.1, 41.3);
        let widths = Edges {
            top: 2.7,
            right: 3.3,
            bottom: 1.9,
            left: 4.1,
        };
        let sides = [
            DrawnSide {
                side: Side::Top,
                line: Line::Dashed,
                color: Rgba::BLACK,
            },
            DrawnSide {
                side: Side::Right,
                line: Line::Solid,
                color: Rgba::BLACK,
            },
        ];
        let joints = joints(&sides, outer, widths);
        let top = dashed(
            Side::Top,
            &wedge(Side::Top, outer, widths),
            &joints,
            outer,
            widths,
        );
        let right = joints.join(
            Side::Right,
            &wedge(Side::Right, outer, widths),
            outer,
            widths,
        );
        // The mitre from (97.4, 0.7) inwards by 3.3 across for 2.7 down.
        let on_mitre = |(x, y): (f32, f32)| {
            let across = (97.4 - x) / 3.3;
            let down = (y - 0.7) / 2.7;
            (across - down).abs() < 0.001 && across > 0.0001
        };
        let mut checked = 0;
        for point in outlines(&top)
            .into_iter()
            .flatten()
            .filter(|point| on_mitre(*point))
        {
            assert!(
                right.contains(&point),
                "{point:?} is not a corner of {right:?}"
            );
            checked += 1;
        }
        assert!(checked > 0, "no dash was cut off on the mitre");
    }
}
