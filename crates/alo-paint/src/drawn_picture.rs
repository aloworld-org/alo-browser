/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A decoded picture, drawn into a rectangle under whatever transform is in
//! force.
//!
//! Split from [`crate::render`] on the iteration that taught it to rotate: the
//! renderer walks a display list, and this is how one kind of item in it turns
//! into pixels — which changes when sampling changes, and for no other reason.
//!
//! # Two paths, and why there are two
//!
//! **Upright.** A transform that only moves and grows the rectangle — no
//! rotation, no skew, no mirror — is drawn in whole pixels: the corners are
//! transformed once and every pixel inside them is filled from the picture
//! with integer arithmetic. That is exact, it cannot lose anything to
//! rounding, and it is every `<img>` almost every page has.
//!
//! **Turned.** Anything else is drawn the way a transformed shape is: the
//! rectangle is transformed *as a shape* and rasterised, so its edges are
//! anti-aliased like any other edge, and each pixel it covers asks where it
//! came from — the pixel's centre, put back through the inverted transform
//! into the rectangle — and takes that pixel of the picture. A mirror goes
//! this way too, because the upright path draws the corners' bounding box and
//! a bounding box has no way to say "the other way round".
//!
//! The upright path is not a shortcut through the turned one. Sampling at a
//! pixel's centre picks different source pixels from sampling at its corner
//! whenever the picture is scaled, so folding the two together would move
//! every scaled picture in the corpus — which is item 179's question, decided
//! when a page asks it, not a side effect of this one.
//!
//! # What is still nearest-neighbour
//!
//! Both paths. One pixel of the picture per pixel of the page: exact at one to
//! one, and coarse when scaled or turned, so a rotated picture's stripes have
//! stepped edges *inside* it while its outline is smooth. Item 179.

use crate::canvas::Canvas;
use crate::path::Path;
use crate::raster::fill;
use crate::render::{pixel_centre, place};
use alo_layout::Rect;
use alo_value::Matrix;

/// Draw a picture into a rectangle in page coordinates, under a transform.
///
/// A picture or a rectangle with no area draws nothing, and so does a
/// transform that flattens the rectangle onto a line — there is nothing it
/// could be drawn into.
pub fn draw_picture(target: &mut Canvas, picture: &Canvas, rect: Rect, transform: Matrix) {
    if picture.width() == 0 || picture.height() == 0 {
        return;
    }
    if keeps_upright(transform) {
        upright(target, picture, rect, transform);
    } else {
        turned(target, picture, rect, transform);
    }
}

/// Whether a transform leaves a rectangle a rectangle the same way round:
/// nothing from one axis reaches the other, and neither axis is reversed.
fn keeps_upright(transform: Matrix) -> bool {
    const NONE: f32 = 1.0e-6;
    transform.b.abs() < NONE && transform.c.abs() < NONE && transform.a > 0.0 && transform.d > 0.0
}

/// The upright path: the transformed corners, filled in whole pixels.
fn upright(target: &mut Canvas, picture: &Canvas, rect: Rect, transform: Matrix) {
    // The corners, transformed once. Everything after this is integers, so that
    // the per-pixel arithmetic cannot lose anything and does not need a cast
    // this crate's lints refuse.
    let (left, top) = transform.apply(rect.left(), rect.top());
    let (right, bottom) = transform.apply(rect.right(), rect.bottom());

    let across = whole(right - left);
    let down = whole(bottom - top);
    let at_x = whole(left);
    let at_y = whole(top);
    let wide = picture.width();
    let tall = picture.height();
    if across == 0 || down == 0 {
        return;
    }

    for y in 0..down {
        // Which row of the picture this row comes from, in whole numbers: the
        // destination row scaled by the picture's height over the box's.
        let from_y = (y * tall / down).min(tall - 1);
        for x in 0..across {
            let from_x = (x * wide / across).min(wide - 1);
            let Some(colour) = picture.at(from_x, from_y) else {
                continue;
            };
            target.blend(at_x + x, at_y + y, colour, 255);
        }
    }
}

/// The turned path: the rectangle as a transformed shape, each covered pixel
/// sampled from where it came from.
fn turned(target: &mut Canvas, picture: &Canvas, rect: Rect, transform: Matrix) {
    let width = rect.right() - rect.left();
    let height = rect.bottom() - rect.top();
    // Written as a refusal of everything that is not a positive size, so that
    // a size that is not a number at all is refused too.
    if !(width > 0.0 && height > 0.0) {
        return;
    }
    let Some(back) = transform.inverted() else {
        return;
    };
    let shape =
        fill(&Path::rectangle(rect.left(), rect.top(), width, height).transformed(transform));
    let (left, top) = shape.origin();
    for row in 0..shape.height() {
        for column in 0..shape.width() {
            let covered = shape.at(column, row);
            if covered == 0 {
                continue;
            }
            let (Some(x), Some(y)) = (place(left, column), place(top, row)) else {
                continue;
            };
            let (local_x, local_y) = back.apply(pixel_centre(x), pixel_centre(y));
            let (Some(from_x), Some(from_y)) = (
                source((local_x - rect.left()) / width, picture.width()),
                source((local_y - rect.top()) / height, picture.height()),
            ) else {
                continue;
            };
            let Some(colour) = picture.at(from_x, from_y) else {
                continue;
            };
            // Partly covered at the outline, like any other shape's edge.
            target.blend(x, y, colour, covered);
        }
    }
}

/// Which of `count` pixels a fraction of the way across falls in.
///
/// Held to the picture's own edge: a pixel the outline only partly covers has
/// its centre just outside the rectangle, and the nearest pixel of the picture
/// is the right one to show there rather than none. [`None`] only for a
/// fraction that is not a number.
fn source(fraction: f32, count: u32) -> Option<u32> {
    if !fraction.is_finite() || count == 0 {
        return None;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a picture is at most sixty-four megapixels, far inside f32's exact integers on a side"
    )]
    let span = count as f32;
    let at = (fraction * span).floor().clamp(0.0, span - 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped above to a whole number between zero and the picture's last pixel"
    )]
    let index = at as u32;
    Some(index)
}

/// A length as a whole number of pixels, never negative.
///
/// The same shape as `alo_renderer`'s: a float-to-integer cast is what this
/// crate's lints refuse, and counting up to the value is the way that needs no
/// cast. Called a handful of times per picture rather than per pixel, which is
/// what makes counting acceptable.
fn whole(value: f32) -> u32 {
    let clamped = value.round().clamp(0.0, 16_384.0);
    let mut whole = 0u32;
    while f32::from(u16::try_from(whole).unwrap_or(u16::MAX)) + 1.0 <= clamped {
        whole += 1;
    }
    whole
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    const RED: Rgba = Rgba {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    const GREEN: Rgba = Rgba {
        red: 0.0,
        green: 1.0,
        blue: 0.0,
        alpha: 1.0,
    };
    const BLUE: Rgba = Rgba {
        red: 0.0,
        green: 0.0,
        blue: 1.0,
        alpha: 1.0,
    };

    /// Four quarters, no two alike, so that every way of turning it round is
    /// a different picture: red top left, green top right, blue bottom left,
    /// black bottom right.
    fn quarters() -> Canvas {
        let mut picture = Canvas::new(2, 2, Rgba::TRANSPARENT);
        picture.blend(0, 0, RED, 255);
        picture.blend(1, 0, GREEN, 255);
        picture.blend(0, 1, BLUE, 255);
        picture.blend(1, 1, Rgba::BLACK, 255);
        picture
    }

    /// The square every test draws into: twenty pixels a side, at (10, 10),
    /// so its centre is (20, 20).
    fn square() -> Rect {
        Rect::new(10.0, 10.0, 20.0, 20.0)
    }

    /// A transform about the square's centre, which is what
    /// `transform-origin`'s default makes of one.
    fn about_the_centre(a: f32, b: f32, c: f32, d: f32) -> Matrix {
        Matrix::translation(-20.0, -20.0)
            .then(Matrix {
                a,
                b,
                c,
                d,
                e: 0.0,
                f: 0.0,
            })
            .then(Matrix::translation(20.0, 20.0))
    }

    fn drawn(transform: Matrix) -> Canvas {
        let mut canvas = Canvas::new(40, 40, Rgba::WHITE);
        draw_picture(&mut canvas, &quarters(), square(), transform);
        canvas
    }

    /// The four quarters' colours, read well inside each: top left, top
    /// right, bottom left, bottom right.
    fn corners_of(canvas: &Canvas) -> [Rgba; 4] {
        let at = |x, y| canvas.at(x, y).expect("on the canvas");
        [at(14, 14), at(25, 14), at(14, 25), at(25, 25)]
    }

    #[test]
    fn an_upright_picture_is_drawn_quarter_by_quarter() {
        assert_eq!(
            corners_of(&drawn(Matrix::IDENTITY)),
            [RED, GREEN, BLUE, Rgba::BLACK]
        );
    }

    #[test]
    fn a_quarter_turn_moves_every_quarter_one_corner_clockwise() {
        // `rotate(90deg)`: across becomes down, down becomes back across.
        let canvas = drawn(about_the_centre(0.0, 1.0, -1.0, 0.0));
        assert_eq!(corners_of(&canvas), [BLUE, RED, Rgba::BLACK, GREEN]);
    }

    #[test]
    fn a_half_turn_is_the_picture_upside_down() {
        let canvas = drawn(about_the_centre(-1.0, 0.0, 0.0, -1.0));
        assert_eq!(corners_of(&canvas), [Rgba::BLACK, BLUE, GREEN, RED]);
    }

    #[test]
    fn a_mirror_is_drawn_the_other_way_round() {
        // `scaleX(-1)`: the bounding box of a mirrored rectangle is the same
        // rectangle, which is how the old path drew it unmirrored.
        let canvas = drawn(about_the_centre(-1.0, 0.0, 0.0, 1.0));
        assert_eq!(corners_of(&canvas), [GREEN, RED, Rgba::BLACK, BLUE]);
    }

    #[test]
    fn an_eighth_of_a_turn_is_a_diamond_rather_than_its_bounding_box() {
        let (sine, cosine) = core::f32::consts::FRAC_PI_4.sin_cos();
        let canvas = drawn(about_the_centre(cosine, sine, -sine, cosine));

        // The square's own corners are outside the diamond: the old path
        // filled them, because it drew the corners' bounding box.
        for (x, y) in [(10, 10), (29, 10), (10, 29), (29, 29)] {
            assert_eq!(canvas.at(x, y), Some(Rgba::WHITE), "({x}, {y})");
        }
        // The diamond's points reach past the square, half a diagonal from
        // the centre — just over fourteen pixels.
        for (x, y) in [(20, 7), (7, 20), (32, 20), (20, 32)] {
            assert_ne!(canvas.at(x, y), Some(Rgba::WHITE), "({x}, {y})");
        }
        // Turned an eighth clockwise, the top-left quarter has swung up to
        // the top point and the bottom-right one down to the bottom point.
        assert_eq!(canvas.at(20, 10), Some(RED));
        assert_eq!(canvas.at(20, 30), Some(Rgba::BLACK));
        assert_eq!(canvas.at(29, 20), Some(GREEN));
        assert_eq!(canvas.at(11, 20), Some(BLUE));
    }

    #[test]
    fn a_turned_outline_is_anti_aliased_like_any_other_edge() {
        let (sine, cosine) = core::f32::consts::FRAC_PI_4.sin_cos();
        let canvas = drawn(about_the_centre(cosine, sine, -sine, cosine));
        // Every pixel well inside is exactly one quarter's colour, none of
        // which has both red and green in it. A pixel that does is the
        // picture partly over the white page: the outline, blended.
        let mut partial = 0;
        for x in 0..40 {
            for y in 0..40 {
                let pixel = canvas.at(x, y).expect("on the canvas");
                if pixel != Rgba::WHITE && pixel.red > 0.05 && pixel.green > 0.05 {
                    partial += 1;
                }
            }
        }
        assert!(
            partial > 20,
            "an outline pixel is a blend of the picture and the page, found {partial}"
        );
    }

    #[test]
    fn an_upright_scale_still_fills_whole_pixels() {
        // Twice the size about the centre: from (0, 0) to (40, 40), every
        // pixel fully one quarter or another, with no blended edge.
        let canvas = drawn(about_the_centre(2.0, 0.0, 0.0, 2.0));
        assert_eq!(canvas.at(0, 0), Some(RED));
        assert_eq!(canvas.at(39, 0), Some(GREEN));
        assert_eq!(canvas.at(0, 39), Some(BLUE));
        assert_eq!(canvas.at(39, 39), Some(Rgba::BLACK));
    }

    #[test]
    fn a_transform_that_flattens_the_picture_draws_nothing() {
        for flat in [
            about_the_centre(0.0, 0.0, 0.0, 0.0),
            about_the_centre(1.0, 1.0, 1.0, 1.0),
        ] {
            let canvas = drawn(flat);
            assert!(
                canvas.pixels().iter().all(|pixel| *pixel == Rgba::WHITE),
                "{flat}"
            );
        }
    }

    #[test]
    fn a_transform_that_is_not_a_number_draws_nothing_and_does_not_fail() {
        for broken in [f32::NAN, f32::INFINITY] {
            let canvas = drawn(about_the_centre(broken, 1.0, -1.0, 0.0));
            assert!(canvas.pixels().iter().all(|pixel| *pixel == Rgba::WHITE));
        }
    }

    #[test]
    fn a_picture_turned_partly_off_the_page_draws_what_is_on_it() {
        let mut canvas = Canvas::new(20, 20, Rgba::WHITE);
        let rect = Rect::new(-10.0, -10.0, 20.0, 20.0);
        let half_turn = Matrix {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            e: 0.0,
            f: 0.0,
        };
        draw_picture(&mut canvas, &quarters(), rect, half_turn);
        // About the page's own corner, the picture lands back over (-10, -10)
        // to (10, 10) upside down: its red quarter is now bottom right.
        assert_eq!(canvas.at(5, 5), Some(RED));
        assert_eq!(canvas.at(15, 15), Some(Rgba::WHITE));
    }

    #[test]
    fn a_fraction_names_a_pixel_inside_the_picture() {
        assert_eq!(source(0.0, 4), Some(0));
        assert_eq!(source(0.49, 4), Some(1));
        assert_eq!(source(0.999, 4), Some(3));
        assert_eq!(source(-0.01, 4), Some(0));
        assert_eq!(source(1.01, 4), Some(3));
        assert_eq!(source(f32::NAN, 4), None);
        assert_eq!(source(0.5, 0), None);
    }
}
