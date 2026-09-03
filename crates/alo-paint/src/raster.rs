/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Shapes into coverage.
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

use crate::coverage::Coverage;
use crate::path::{Path, Point, Segment};

/// Fill a path and report how much of each pixel it covers.
///
/// The non-zero fill rule, which is the one fonts are drawn with and the one
/// CSS uses for everything it fills. Anti-aliased, because a letter with hard
/// edges at these sizes is unreadable.
pub fn fill(path: &Path) -> Coverage {
    let Some((left, top, right, bottom)) = path.bounds() else {
        return Coverage::empty();
    };
    // Whole pixels outwards, so that a shape ending at 10.3 gets the whole of
    // pixel 10 rather than a clipped edge.
    let x0 = left.floor();
    let y0 = top.floor();
    let width = (right.ceil() - x0).max(0.0);
    let height = (bottom.ceil() - y0).max(0.0);
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
    mask.fill_path(
        &built,
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::identity(),
    );

    Coverage::new(
        width,
        height,
        (to_whole(x0), to_whole(y0)),
        mask.data().to_vec(),
    )
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
}
