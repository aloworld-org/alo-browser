/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Shapes into coverage, and strokes into shapes.
//!
//! **This is the only file that names `tiny-skia`.** Filling a path with
//! anti-aliasing is a scanline rasteriser with a great deal of care in it, and
//! ADR 0001 says to rent that kind of thing — so we do, and one rented
//! rasteriser draws every shape this engine has. A glyph and a rounded corner
//! come out of the same code with the same anti-aliasing, which is what stops
//! a letter and the box behind it disagreeing along their shared edge.
//!
//! # Coverage, not colour
//!
//! What comes out is a [`Coverage`]: **how much of each pixel the shape
//! covers**, from zero to 255 — not a colour. Colour is applied when the
//! coverage is composited, which is why the same glyph mask serves black text
//! on white and white text on black, and why a mask can be reused for a
//! shadow. The type itself lives in [`crate::coverage`]; this file only makes
//! them.
//!
//! # Strokes are outlines
//!
//! A stroke is drawn by finding **the shape it covers** — the path offset
//! half a width each way, with its caps and joins, after dashing — and filling
//! that ([`outline`]). That offsetting is the same kind of physics as filling,
//! with a great many degenerate cases in it, and ADR 0022 § 2 rents it from the
//! same crate rather than writing a second rasteriser's worth of geometry.

use crate::coverage::Coverage;
use crate::fill_rule::FillRule;
use crate::path::{Path, Point, Segment};
use crate::stroke::{LineCap, LineJoin, Stroke};

/// Fill a path and report how much of each pixel it covers.
///
/// The non-zero fill rule, which is the one fonts are drawn with and the one
/// CSS uses for everything it fills. Anti-aliased, because a letter with hard
/// edges at these sizes is unreadable.
pub fn fill(path: &Path) -> Coverage {
    covered(path, FillRule::NonZero, None)
}

/// Fill a path by a rule, keeping only the part that lands on a page of this
/// many pixels.
///
/// The coverage outside the page is never drawn, and not making it is the
/// difference between a stranger's `<rect width="60000" height="60000">`
/// costing a page's worth of mask and costing four gigabytes of one. A pixel
/// inside the page is covered exactly as it would have been with the whole
/// shape rasterised, because coverage is a property of each pixel and the
/// shape around it, not of how large the mask is.
pub fn fill_on_page(path: &Path, rule: FillRule, page: (u32, u32)) -> Coverage {
    covered(path, rule, Some(page))
}

fn covered(path: &Path, rule: FillRule, page: Option<(u32, u32)>) -> Coverage {
    let Some((left, top, right, bottom)) = path.bounds() else {
        return Coverage::empty();
    };
    // Whole pixels outwards, so that a shape ending at 10.3 gets the whole of
    // pixel 10 rather than a clipped edge.
    let (mut x0, mut y0, mut x1, mut y1) = (left.floor(), top.floor(), right.ceil(), bottom.ceil());
    if let Some((width, height)) = page {
        x0 = x0.max(0.0);
        y0 = y0.max(0.0);
        x1 = x1.min(f32::from(u16::try_from(width).unwrap_or(u16::MAX)));
        y1 = y1.min(f32::from(u16::try_from(height).unwrap_or(u16::MAX)));
    }
    let width = (x1 - x0).max(0.0);
    let height = (y1 - y0).max(0.0);
    let (Some(width), Some(height)) = (to_pixels(width), to_pixels(height)) else {
        return Coverage::empty();
    };
    if width == 0 || height == 0 {
        return Coverage::empty();
    }

    // Move the shape so its own top-left lands on the mask's, and remember
    // where it was.
    let moved = path.translated(Point::new(-x0, -y0));
    let Some(built) = build(&moved) else {
        return Coverage::empty();
    };
    let Some(mut mask) = tiny_skia::Mask::new(width, height) else {
        return Coverage::empty();
    };
    let rule = match rule {
        FillRule::NonZero => tiny_skia::FillRule::Winding,
        FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    };
    mask.fill_path(&built, rule, true, tiny_skia::Transform::identity());

    Coverage::new(
        width,
        height,
        (to_whole(x0), to_whole(y0)),
        mask.data().to_vec(),
    )
}

/// The shape a stroke along a path covers, to be filled by the non-zero rule.
///
/// Dashed first when the stroke has dashes, and then outlined, in the path's
/// own coordinates. `resolution` is how many pixels one of those units will
/// become, so a curve drawn small and scaled up is offset finely enough to
/// stay smooth.
///
/// [`None`] when the stroke covers nothing: a width that is not a positive
/// finite number, a path with no length that has no caps to draw, a dash
/// pattern that is not one (an odd count, a negative length, a sum of
/// nothing, a non-finite offset — the maker is meant to have settled those),
/// or an outline whose points would not be finite.
///
/// A dash pattern is **not** bounded here. A maker that hands a stranger's
/// pattern to this function must have counted the dashes first: the rented
/// dasher gives up only past a million, which is far more than a page should
/// be allowed to make a renderer build.
pub fn outline(path: &Path, stroke: &Stroke, resolution: f32) -> Option<Path> {
    if !(stroke.width.is_finite() && stroke.width > 0.0) {
        return None;
    }
    let resolution = if resolution.is_finite() && resolution > 0.0 {
        resolution
    } else {
        1.0
    };
    let mut built = build(path)?;
    if let Some(dashes) = &stroke.dashes {
        let pattern = tiny_skia::StrokeDash::new(dashes.lengths.clone(), dashes.offset)?;
        built = built.dash(&pattern, resolution)?;
    }
    let rented = tiny_skia::Stroke {
        width: stroke.width,
        // A limit under one is a miter that is always beveled, which is what
        // the rented stroker does with one anyway; held here so a stranger's
        // value means one thing.
        miter_limit: if stroke.miter_limit.is_finite() {
            stroke.miter_limit.max(1.0)
        } else {
            4.0
        },
        line_cap: match stroke.cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        },
        line_join: match stroke.join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        },
        dash: None,
    };
    let stroked = built.stroke(&rented, resolution)?;
    let mut outline = Path::new();
    for segment in stroked.segments() {
        match segment {
            tiny_skia::PathSegment::MoveTo(to) => outline.move_to(ours(to)?),
            tiny_skia::PathSegment::LineTo(to) => outline.line_to(ours(to)?),
            tiny_skia::PathSegment::QuadTo(control, to) => {
                outline.quad_to(ours(control)?, ours(to)?);
            }
            tiny_skia::PathSegment::CubicTo(first, second, to) => {
                outline.cubic_to(ours(first)?, ours(second)?, ours(to)?);
            }
            tiny_skia::PathSegment::Close => outline.close(),
        }
    }
    (!outline.is_empty()).then_some(outline)
}

/// A rented point as ours, if it is a point at all.
fn ours(point: tiny_skia::Point) -> Option<Point> {
    (point.x.is_finite() && point.y.is_finite()).then(|| Point::new(point.x, point.y))
}

fn build(path: &Path) -> Option<tiny_skia::Path> {
    let mut builder = tiny_skia::PathBuilder::new();
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) => builder.move_to(to.x, to.y),
            Segment::LineTo(to) => builder.line_to(to.x, to.y),
            Segment::QuadTo(control, to) => builder.quad_to(control.x, control.y, to.x, to.y),
            Segment::CubicTo(first, second, to) => {
                builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
            }
            Segment::Close => builder.close(),
        }
    }
    builder.finish()
}

/// A size in pixels, refusing anything a raster could not hold.
///
/// A shape ten million pixels across is a broken value rather than a picture,
/// and turning it into a raster would ask for a great deal of memory on the
/// strength of a typo.
fn to_pixels(value: f32) -> Option<u32> {
    const LIMIT: f32 = 65_536.0;
    if !value.is_finite() || !(0.0..=LIMIT).contains(&value) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked above to be finite and within zero..=65536"
    )]
    let pixels = value as u32;
    Some(pixels)
}

fn to_whole(value: f32) -> i32 {
    let clamped = value.clamp(-1.0e6, 1.0e6);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped to a range i32 represents exactly"
    )]
    let whole = clamped as i32;
    whole
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Point;
    use crate::stroke::{LineCap, LineJoin, Stroke};

    #[test]
    fn nothing_covers_nothing() {
        let coverage = fill(&Path::new());
        assert!(coverage.is_empty());
        assert_eq!(coverage.width(), 0);
        assert_eq!(coverage.at(0, 0), 0);
        assert!(coverage.data().is_empty());
    }

    #[test]
    fn a_whole_pixel_rectangle_is_covered_completely() {
        let coverage = fill(&Path::rectangle(0.0, 0.0, 4.0, 3.0));
        assert_eq!((coverage.width(), coverage.height()), (4, 3));
        assert_eq!(coverage.origin(), (0, 0));
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(coverage.at(x, y), 255, "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn a_half_covered_pixel_is_half_covered() {
        // A rectangle two pixels wide and half a pixel tall: the top row is
        // half covered rather than on or off.
        let coverage = fill(&Path::rectangle(0.0, 0.0, 2.0, 0.5));
        assert_eq!(coverage.height(), 1);
        let value = coverage.at(0, 0);
        assert!(
            (120..=135).contains(&value),
            "expected about half coverage, got {value}",
        );
    }

    #[test]
    fn the_origin_says_where_the_shape_was() {
        let coverage = fill(&Path::rectangle(10.0, -20.0, 4.0, 4.0));
        assert_eq!(coverage.origin(), (10, -20));
        assert_eq!((coverage.width(), coverage.height()), (4, 4));
    }

    #[test]
    fn a_shape_between_pixels_covers_the_pixels_it_touches() {
        let coverage = fill(&Path::rectangle(0.5, 0.5, 1.0, 1.0));
        assert_eq!(
            (coverage.width(), coverage.height()),
            (2, 2),
            "it touches four pixels, so the mask is two by two",
        );
        assert_eq!(coverage.origin(), (0, 0));
        // A quarter of the shape in each of the four pixels.
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let value = coverage.at(x, y);
            assert!(
                (50..=80).contains(&value),
                "expected about a quarter at {x},{y}, got {value}",
            );
        }
    }

    #[test]
    fn asking_outside_the_covered_area_is_answered_with_nothing() {
        let coverage = fill(&Path::rectangle(0.0, 0.0, 2.0, 2.0));
        assert_eq!(coverage.at(2, 0), 0);
        assert_eq!(coverage.at(0, 2), 0);
        assert_eq!(coverage.at(u32::MAX, u32::MAX), 0);
    }

    #[test]
    fn a_shape_with_no_area_covers_nothing() {
        let mut line = Path::new();
        line.move_to(Point::new(0.0, 0.0));
        line.line_to(Point::new(10.0, 0.0));
        line.close();
        assert!(fill(&line).is_empty(), "a horizontal line has no inside");
    }

    #[test]
    fn a_shape_too_large_to_raster_is_refused_rather_than_asked_for() {
        let enormous = Path::rectangle(0.0, 0.0, 1.0e9, 1.0e9);
        assert!(
            fill(&enormous).is_empty(),
            "a typo should not ask for a terabyte",
        );
    }

    #[test]
    fn a_hole_is_a_hole() {
        // A square with a smaller square wound the other way inside it: the
        // non-zero rule leaves the middle empty.
        let mut path = Path::rectangle(0.0, 0.0, 10.0, 10.0);
        path.move_to(Point::new(3.0, 3.0));
        path.line_to(Point::new(3.0, 7.0));
        path.line_to(Point::new(7.0, 7.0));
        path.line_to(Point::new(7.0, 3.0));
        path.close();

        let coverage = fill(&path);
        assert_eq!(coverage.at(1, 1), 255, "the ring is filled");
        assert_eq!(coverage.at(5, 5), 0, "and the middle is not");
    }

    fn star() -> Path {
        // A five-pointed star in one outline, crossing itself: the pentagon in
        // its middle is wound twice.
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(16.0, 20.0));
        path.line_to(Point::new(0.0, 7.0));
        path.line_to(Point::new(20.0, 7.0));
        path.line_to(Point::new(4.0, 20.0));
        path.close();
        path
    }

    #[test]
    fn even_odd_leaves_a_hole_where_non_zero_fills_one() {
        let page = (64, 64);
        let nonzero = fill_on_page(&star(), FillRule::NonZero, page);
        let evenodd = fill_on_page(&star(), FillRule::EvenOdd, page);
        assert_eq!(
            nonzero.at(10, 11),
            255,
            "the middle, wound twice, is filled"
        );
        assert_eq!(evenodd.at(10, 11), 0, "and crossed twice, is not");
        assert_eq!(nonzero.at(10, 5), 255, "a point is inside either way");
        assert_eq!(evenodd.at(10, 5), 255);
    }

    #[test]
    fn a_shape_larger_than_the_page_is_covered_only_on_the_page() {
        let enormous = Path::rectangle(-5.0e6, -5.0e6, 1.0e7, 1.0e7);
        let coverage = fill_on_page(&enormous, FillRule::NonZero, (32, 16));
        assert_eq!((coverage.width(), coverage.height()), (32, 16));
        assert_eq!(coverage.origin(), (0, 0));
        assert_eq!(coverage.at(31, 15), 255);
        assert!(
            fill(&enormous).is_empty(),
            "without a page the same shape is too large to raster at all, \
             where on a page it is a page of mask",
        );
    }

    #[test]
    fn a_page_changes_no_pixel_it_keeps() {
        let path = Path::rectangle(-2.5, 1.25, 10.0, 3.5);
        let whole = fill(&path);
        let kept = fill_on_page(&path, FillRule::NonZero, (6, 6));
        assert_eq!(kept.origin(), (0, 1));
        for y in 0..kept.height() {
            for x in 0..kept.width() {
                assert_eq!(kept.at(x, y), whole.at(x + 3, y), "pixel {x},{y}");
            }
        }
        assert!(fill_on_page(&path, FillRule::NonZero, (0, 0)).is_empty());
        let off = Path::rectangle(100.0, 100.0, 4.0, 4.0);
        assert!(fill_on_page(&off, FillRule::NonZero, (6, 6)).is_empty());
    }

    fn line(from: (f32, f32), to: (f32, f32)) -> Path {
        let mut path = Path::new();
        path.move_to(Point::new(from.0, from.1));
        path.line_to(Point::new(to.0, to.1));
        path
    }

    fn stroked(width: f32, cap: LineCap, join: LineJoin) -> Stroke {
        Stroke {
            width,
            cap,
            join,
            ..Stroke::default()
        }
    }

    fn bounds_of(path: &Path, stroke: &Stroke) -> (f32, f32, f32, f32) {
        outline(path, stroke, 1.0)
            .and_then(|outline| outline.bounds())
            .expect("an outline")
    }

    fn near(found: (f32, f32, f32, f32), wanted: (f32, f32, f32, f32)) -> bool {
        let close = |a: f32, b: f32| (a - b).abs() < 0.01;
        close(found.0, wanted.0)
            && close(found.1, wanted.1)
            && close(found.2, wanted.2)
            && close(found.3, wanted.3)
    }

    #[test]
    fn a_line_is_outlined_half_its_width_each_side_and_capped_by_its_cap() {
        let path = line((0.0, 5.0), (10.0, 5.0));
        let butt = bounds_of(&path, &stroked(2.0, LineCap::Butt, LineJoin::Miter));
        assert!(near(butt, (0.0, 4.0, 10.0, 6.0)), "{butt:?}");
        // Round and square both reach a half width past each end.
        for cap in [LineCap::Round, LineCap::Square] {
            let capped = bounds_of(&path, &stroked(2.0, cap, LineJoin::Miter));
            assert!(near(capped, (-1.0, 4.0, 11.0, 6.0)), "{cap}: {capped:?}");
        }
        // And they differ at the corner: a square cap covers it, a round one
        // only part of it.
        let corner = |cap| {
            let outline =
                outline(&path, &stroked(4.0, cap, LineJoin::Miter), 1.0).expect("an outline");
            page_at(&outline, 10, 3)
        };
        assert_eq!(corner(LineCap::Square), 255);
        assert!(corner(LineCap::Round) < 255);
        assert_eq!(corner(LineCap::Butt), 0);
    }

    #[test]
    fn a_corner_is_mitered_rounded_or_beveled() {
        // A right angle at (10, 2), turning down: the outer corner of a
        // two-wide stroke is (11, 1) when mitered.
        let mut path = line((0.0, 2.0), (10.0, 2.0));
        path.line_to(Point::new(10.0, 12.0));
        let at_corner = |join| {
            let outline =
                outline(&path, &stroked(2.0, LineCap::Butt, join), 1.0).expect("an outline");
            page_at(&outline, 10, 1)
        };
        assert_eq!(at_corner(LineJoin::Miter), 255, "a miter fills the corner");
        let round = at_corner(LineJoin::Round);
        let bevel = at_corner(LineJoin::Bevel);
        assert!(bevel < round && round < 255, "bevel {bevel}, round {round}");
        let mitered = bounds_of(&path, &stroked(2.0, LineCap::Butt, LineJoin::Miter));
        assert!(near(mitered, (0.0, 1.0, 11.0, 12.0)), "{mitered:?}");
    }

    #[test]
    fn a_miter_past_its_limit_is_beveled() {
        // A spike turning back at twenty degrees: its miter is about
        // 1 / sin(10°) = 5.8 widths long, past a limit of four.
        let mut path = line((0.0, 0.0), (20.0, 0.0));
        path.line_to(Point::new(0.0, 7.28));
        let reach = |limit| {
            let stroke = Stroke {
                width: 2.0,
                miter_limit: limit,
                ..Stroke::default()
            };
            bounds_of(&path, &stroke).2
        };
        let beveled = reach(4.0);
        let mitered = reach(10.0);
        assert!(beveled < 21.0, "{beveled}");
        assert!(mitered > 25.0, "{mitered}");
        assert!(
            (reach(0.5) - beveled).abs() < 0.01,
            "a limit under one is one"
        );
    }

    #[test]
    fn dashes_alternate_from_the_start_of_each_subpath() {
        let mut path = line((0.0, 2.0), (12.0, 2.0));
        path.move_to(Point::new(0.0, 8.0));
        path.line_to(Point::new(12.0, 8.0));
        let stroke = Stroke {
            width: 2.0,
            dashes: Some(crate::stroke::Dashes {
                lengths: vec![3.0, 3.0],
                offset: 0.0,
            }),
            ..Stroke::default()
        };
        let outline = outline(&path, &stroke, 1.0).expect("dashes");
        for y in [1, 7] {
            for (x, inked) in [(1, true), (4, false), (7, true), (10, false)] {
                assert_eq!(page_at(&outline, x, y) == 255, inked, "pixel {x},{y}");
            }
        }
        // An offset moves the pattern along.
        let moved = Stroke {
            dashes: Some(crate::stroke::Dashes {
                lengths: vec![3.0, 3.0],
                offset: 3.0,
            }),
            ..stroke
        };
        let moved = outline_or_nothing(&path, &moved);
        assert_eq!(page_at(&moved, 1, 1), 0);
        assert_eq!(page_at(&moved, 4, 1), 255);
    }

    /// How much of the page's pixel `(x, y)` a shape covers.
    fn page_at(path: &Path, x: i32, y: i32) -> u8 {
        let coverage = fill_on_page(path, FillRule::NonZero, (32, 32));
        let (left, top) = coverage.origin();
        match (u32::try_from(x - left), u32::try_from(y - top)) {
            (Ok(x), Ok(y)) => coverage.at(x, y),
            _ => 0,
        }
    }

    fn outline_or_nothing(path: &Path, stroke: &Stroke) -> Path {
        outline(path, stroke, 1.0).unwrap_or_default()
    }

    #[test]
    fn a_dash_of_nothing_under_a_round_cap_is_a_dot() {
        let path = line((2.0, 4.0), (14.0, 4.0));
        let dotted = Stroke {
            width: 2.0,
            cap: LineCap::Round,
            dashes: Some(crate::stroke::Dashes {
                lengths: vec![0.0, 4.0],
                offset: 0.0,
            }),
            ..Stroke::default()
        };
        let dots = outline_or_nothing(&path, &dotted);
        // Dots at 2, 6, 10 and 14, a unit round, and nothing between them.
        for x in [1, 2, 5, 6, 9, 10] {
            assert!(page_at(&dots, x, 3) > 100, "a dot at {x}");
        }
        assert_eq!(page_at(&dots, 4, 3), 0);
        assert_eq!(page_at(&dots, 8, 3), 0);
    }

    #[test]
    fn a_subpath_of_no_length_is_a_dot_only_under_a_cap() {
        let mut point = Path::new();
        point.move_to(Point::new(4.0, 4.0));
        point.line_to(Point::new(4.0, 4.0));
        let mut closed = Path::new();
        closed.move_to(Point::new(4.0, 4.0));
        closed.close();
        for path in [&point, &closed] {
            assert!(outline(path, &stroked(2.0, LineCap::Butt, LineJoin::Miter), 1.0).is_none());
            let round = bounds_of(path, &stroked(2.0, LineCap::Round, LineJoin::Miter));
            assert!(near(round, (3.0, 3.0, 5.0, 5.0)), "{round:?}");
            let square = bounds_of(path, &stroked(2.0, LineCap::Square, LineJoin::Miter));
            assert!(near(square, (3.0, 3.0, 5.0, 5.0)), "{square:?}");
        }
        let mut alone = Path::new();
        alone.move_to(Point::new(4.0, 4.0));
        assert!(
            outline(&alone, &stroked(2.0, LineCap::Round, LineJoin::Miter), 1.0).is_none(),
            "a move on its own is not a subpath to stroke",
        );
    }

    #[test]
    fn a_stroke_that_is_not_one_covers_nothing() {
        let path = line((0.0, 0.0), (10.0, 0.0));
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(outline(&path, &stroked(width, LineCap::Butt, LineJoin::Miter), 1.0).is_none());
        }
        for (lengths, offset) in [
            (vec![1.0], 0.0),
            (vec![1.0, 2.0, 3.0], 0.0),
            (vec![1.0, -1.0], 0.0),
            (vec![0.0, 0.0], 0.0),
            (vec![1.0, f32::INFINITY], 0.0),
            (vec![1.0, 1.0], f32::NAN),
        ] {
            let stroke = Stroke {
                dashes: Some(crate::stroke::Dashes { lengths, offset }),
                ..Stroke::default()
            };
            assert!(outline(&path, &stroke, 1.0).is_none());
        }
        assert!(outline(&Path::new(), &Stroke::default(), 1.0).is_none());
    }

    #[test]
    fn hostile_strokes_never_panic_and_never_make_a_point_that_is_not_one() {
        let paths = [
            line((0.0, 0.0), (1.0e30, 1.0e30)),
            line((-3.0e38, 0.0), (3.0e38, 0.0)),
            line((0.0, 0.0), (1.0e-30, 0.0)),
        ];
        for path in &paths {
            for width in [1.0e-30, 1.0, 1.0e30, f32::MAX] {
                for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
                    for join in [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel] {
                        for resolution in [0.0, -1.0, f32::NAN, 1.0e-30, 1.0e30] {
                            let stroke = Stroke {
                                width,
                                cap,
                                join,
                                miter_limit: f32::MAX,
                                dashes: None,
                            };
                            if let Some(outline) = outline(path, &stroke, resolution) {
                                assert!(
                                    outline.segments().iter().all(|segment| match *segment {
                                        Segment::MoveTo(p) | Segment::LineTo(p) => {
                                            p.x.is_finite() && p.y.is_finite()
                                        }
                                        Segment::QuadTo(a, b) => [a, b]
                                            .iter()
                                            .all(|p| p.x.is_finite() && p.y.is_finite()),
                                        Segment::CubicTo(a, b, c) => [a, b, c]
                                            .iter()
                                            .all(|p| p.x.is_finite() && p.y.is_finite()),
                                        Segment::Close => true,
                                    })
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
