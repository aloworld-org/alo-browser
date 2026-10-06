/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The border of a box whose block-start border has something sitting in it:
//! a `<fieldset>` showing a legend, which is the only such box in CSS.
//!
//! The border is drawn **through** the band rather than above it, so the
//! three ordinary sides start at the line the block-start border is on
//! rather than at the top of the border box — the box above that line is the
//! legend's, and a border either side of it would box the legend in.
//!
//! And the block-start border is drawn **either side of the legend**, which
//! is the whole point of the exercise: a group of controls with its name
//! written into the line around them. Either piece can be nothing — a legend
//! wide enough leaves no border along the top at all, and one at the very
//! edge leaves only the piece on the other side.
//!
//! # Solid, and everything else
//!
//! A border that is `solid` wherever it is drawn is five rectangles: the
//! three ordinary sides, and the block-start side in the two pieces the
//! legend leaves.
//!
//! Any other border — `groove`, which is what the user-agent sheet gives a
//! fieldset, `ridge`, `inset` and `outset`, and the patterns `dashed`,
//! `dotted` and `double` — is drawn as the mitred sides every other such
//! border is ([`draw_mitred`]), inside a clip with the legend's part of the
//! block-start stroke cut out of it. That is how other engines cut it too:
//! the legend's span, across the stroke's depth, is left out of the
//! border's painting.
//!
//! So **a pattern is laid along the whole side and then cut**, never spaced
//! afresh on each piece. A side's dashes are spaced so that it starts and
//! ends on one at its corners, and the legend moves neither corner; spacing
//! each piece instead would make every dash on the line depend on how long
//! the legend's words are. A dash or dot the legend's edge falls on is cut
//! off there, as a dash running under any other box in front of it would be.
//!
//! The cut **stops at the side borders' inner edges**. A legend pulled into a
//! corner does not take the corner with it, exactly as the solid pieces
//! leave the left and right sides whole whatever the legend's margin says.
//!
//! # What is not drawn here
//!
//! **A radius is ignored**, where every other border follows one. A rounded
//! corner is a curve between two sides, and one of these sides has a hole in
//! it; the shape that answers that properly is queue item 19's kind of work,
//! and drawing an approximation of it would be a wrong pixel on the one
//! element this code exists for.

use crate::border::{DrawnSide, Line, draw_mitred};
use crate::corner::Corners;
use crate::display::DisplayItem;
use crate::mitre::{Side, polygon_path};
use crate::paint::Paint;
use crate::path::Path;
use alo_box::BoxId;
use alo_layout::{Band, Edges, Rect};

/// The display items for the border of a box with a band in it.
///
/// `border` is the box's border widths as laid out; the block-start one is
/// the band's stroke instead, which is the width the style asked for. `sides`
/// are the sides the style draws, in any order.
pub fn draw(
    box_id: BoxId,
    border_box: Rect,
    band: Band,
    border: Edges,
    sides: &[DrawnSide],
    out: &mut Vec<DisplayItem>,
) {
    let inset = band.inset();
    let area = Rect::new(
        border_box.left(),
        border_box.top() + inset,
        border_box.size.width,
        (border_box.size.height - inset).max(0.0),
    );
    let widths = Edges {
        top: band.stroke,
        ..border
    };
    if sides.iter().all(|drawn| drawn.line == Line::Solid) {
        draw_solid(box_id, area, widths, band.gap, sides, out);
        return;
    }

    let mut drawn = Vec::new();
    draw_mitred(box_id, area, Corners::SQUARE, widths, sides, &mut drawn);
    if drawn.is_empty() {
        return;
    }
    let Some(hole) = hole(area, widths, band.gap) else {
        out.extend(drawn);
        return;
    };
    out.push(DisplayItem::PushClip {
        box_id,
        path: around(area, hole),
    });
    out.extend(drawn);
    out.push(DisplayItem::PopClip);
}

/// A border that is solid wherever it is drawn: the three ordinary sides
/// whole, then the block-start side in the two pieces the legend leaves.
fn draw_solid(
    box_id: BoxId,
    area: Rect,
    widths: Edges,
    gap: (f32, f32),
    sides: &[DrawnSide],
    out: &mut Vec<DisplayItem>,
) {
    let mut fill = |rect: Rect, drawn: &DrawnSide| {
        if rect.size.width > 0.0 && rect.size.height > 0.0 && !drawn.color.is_invisible() {
            out.push(DisplayItem::Fill {
                box_id,
                path: Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height),
                paint: Paint::Solid(drawn.color),
            });
        }
    };
    for side in [Side::Right, Side::Bottom, Side::Left] {
        let Some(drawn) = sides.iter().find(|drawn| drawn.side == side) else {
            continue;
        };
        let rect = match side {
            Side::Right => Rect::new(
                area.right() - widths.right,
                area.top(),
                widths.right,
                area.size.height,
            ),
            Side::Bottom => Rect::new(
                area.left(),
                area.bottom() - widths.bottom,
                area.size.width,
                widths.bottom,
            ),
            _ => Rect::new(area.left(), area.top(), widths.left, area.size.height),
        };
        fill(rect, drawn);
    }

    let Some(top) = sides.iter().find(|drawn| drawn.side == Side::Top) else {
        return;
    };
    let (gap_starts, gap_ends) = gap;
    let before = Rect::new(
        area.left(),
        area.top(),
        gap_starts.clamp(0.0, area.size.width),
        widths.top,
    );
    let after_starts = area.left() + gap_ends.clamp(0.0, area.size.width);
    let after = Rect::new(
        after_starts,
        area.top(),
        (area.right() - after_starts).max(0.0),
        widths.top,
    );
    fill(before, top);
    fill(after, top);
}

/// The legend's part of the block-start stroke: the band's gap, across the
/// stroke's depth, and never further out than the side borders' inner edges.
///
/// `None` when that leaves nothing to cut.
fn hole(area: Rect, widths: Edges, gap: (f32, f32)) -> Option<Rect> {
    let starts = gap.0.max(widths.left);
    let ends = gap.1.min(area.size.width - widths.right);
    (ends > starts && widths.top > 0.0)
        .then(|| Rect::new(area.left() + starts, area.top(), ends - starts, widths.top))
}

/// The area with a hole in it: the area wound clockwise and the hole the
/// other way round, so that under the non-zero rule the hole is outside.
fn around(area: Rect, hole: Rect) -> Path {
    let mut path = polygon_path(&[
        (area.left(), area.top()),
        (area.right(), area.top()),
        (area.right(), area.bottom()),
        (area.left(), area.bottom()),
    ]);
    path.extend(&polygon_path(&[
        (hole.left(), hole.top()),
        (hole.left(), hole.bottom()),
        (hole.right(), hole.bottom()),
        (hole.right(), hole.top()),
    ]));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mitre::SIDES;
    use crate::raster::fill;
    use alo_value::Rgba;

    fn area() -> Rect {
        Rect::new(10.0, 20.0, 100.0, 50.0)
    }

    fn band(gap: (f32, f32)) -> Band {
        Band {
            height: 2.0,
            stroke: 2.0,
            gap,
        }
    }

    fn all_sides(line: Line) -> [DrawnSide; 4] {
        SIDES.map(|side| DrawnSide {
            side,
            line,
            color: Rgba::from_rgba8(128, 128, 128, 255),
        })
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

    #[test]
    fn the_hole_is_the_gap_across_the_strokes_depth() {
        let widths = Edges::all(4.0);
        assert_eq!(
            hole(area(), widths, (20.0, 60.0)),
            Some(Rect::new(30.0, 20.0, 40.0, 4.0))
        );
    }

    #[test]
    fn the_hole_stops_at_the_side_borders_inner_edges() {
        let widths = Edges {
            top: 4.0,
            right: 6.0,
            bottom: 4.0,
            left: 8.0,
        };
        // A gap reaching past both edges of the box leaves both corners.
        assert_eq!(
            hole(area(), widths, (-5.0, 120.0)),
            Some(Rect::new(18.0, 20.0, 86.0, 4.0))
        );
        // And one entirely inside a side border cuts nothing.
        assert_eq!(hole(area(), widths, (1.0, 7.0)), None);
    }

    #[test]
    fn an_empty_gap_or_a_stroke_of_nothing_cuts_nothing() {
        assert_eq!(hole(area(), Edges::all(4.0), (40.0, 40.0)), None);
        assert_eq!(hole(area(), Edges::all(4.0), (60.0, 40.0)), None);
        let no_stroke = Edges {
            top: 0.0,
            ..Edges::all(4.0)
        };
        assert_eq!(hole(area(), no_stroke, (20.0, 60.0)), None);
    }

    #[test]
    fn the_clip_covers_the_area_and_leaves_the_hole_out() {
        let hole = Rect::new(30.0, 20.0, 40.0, 4.0);
        let mask = fill(&around(area(), hole));
        // A pixel on the page, read from a mask that starts at its own origin.
        let (left, top) = mask.origin();
        let at = |x: i32, y: i32| {
            let (Ok(x), Ok(y)) = (u32::try_from(x - left), u32::try_from(y - top)) else {
                return 0;
            };
            mask.at(x, y)
        };
        assert_eq!(at(20, 21), 255, "beside the hole");
        assert_eq!(at(50, 21), 0, "in the hole");
        assert_eq!(at(50, 30), 255, "below it");
        assert_eq!(at(5, 30), 0, "outside the area");
    }

    #[test]
    fn a_solid_border_is_five_rectangles_and_no_clip() {
        let mut out = Vec::new();
        draw(
            BoxId::from_index_for_tests(0),
            area(),
            band((20.0, 60.0)),
            Edges::all(2.0),
            &all_sides(Line::Solid),
            &mut out,
        );
        assert_eq!(kinds(&out), ["fill"; 5]);
    }

    #[test]
    fn a_groove_is_its_mitred_sides_inside_one_clip_with_the_hole() {
        let mut out = Vec::new();
        draw(
            BoxId::from_index_for_tests(0),
            area(),
            band((20.0, 60.0)),
            Edges::all(2.0),
            &all_sides(Line::Groove),
            &mut out,
        );
        // The hole's clip, then the inner halves and the outer halves, each
        // in two tones inside their own ring.
        assert_eq!(
            kinds(&out),
            [
                "clip", "clip", "fill", "fill", "pop", "clip", "fill", "fill", "pop", "pop"
            ]
        );
        let Some(DisplayItem::PushClip { path, .. }) = out.first() else {
            panic!("{out:?}");
        };
        assert_eq!(path.bounds(), Some((10.0, 20.0, 110.0, 70.0)));
    }

    #[test]
    fn a_gap_that_cuts_nothing_needs_no_clip() {
        let mut out = Vec::new();
        draw(
            BoxId::from_index_for_tests(0),
            area(),
            band((0.0, 1.0)),
            Edges::all(2.0),
            &all_sides(Line::Groove),
            &mut out,
        );
        assert_eq!(
            kinds(&out),
            ["clip", "fill", "fill", "pop", "clip", "fill", "fill", "pop"]
        );
    }

    #[test]
    fn a_pattern_is_laid_along_the_whole_side_and_cut_by_the_hole() {
        // Inside the hole's clip is exactly the border the box would have
        // with no legend at all: the same dashes, dots or lines, spaced on
        // the whole side, so that only the clip says where the legend is.
        for line in [Line::Dashed, Line::Dotted, Line::Double] {
            let mut out = Vec::new();
            draw(
                BoxId::from_index_for_tests(0),
                area(),
                band((20.0, 60.0)),
                Edges::all(2.0),
                &all_sides(line),
                &mut out,
            );
            let mut whole = Vec::new();
            draw_mitred(
                BoxId::from_index_for_tests(0),
                area(),
                Corners::SQUARE,
                Edges::all(2.0),
                &all_sides(line),
                &mut whole,
            );
            assert!(!whole.is_empty(), "{line:?} draws");

            let Some((DisplayItem::PushClip { path, .. }, rest)) = out.split_first() else {
                panic!("{line:?}: {out:?}");
            };
            assert_eq!(
                *path,
                around(area(), Rect::new(30.0, 20.0, 40.0, 2.0)),
                "{line:?} is cut by the hole",
            );
            let Some((DisplayItem::PopClip, inside)) = rest.split_last() else {
                panic!("{line:?}: {out:?}");
            };
            assert_eq!(format!("{inside:?}"), format!("{whole:?}"), "{line:?}");
        }
    }
}
