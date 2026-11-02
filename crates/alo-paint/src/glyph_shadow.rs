/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a colour glyph casts as its text's `text-shadow` (queue item 786):
//! the glyph as it is drawn, as one shape, softened and filled in the
//! shadow's colour.
//!
//! A plain letter casts its outline ([`crate::render`]). A colour glyph has
//! no outline of its own to cast — a picture glyph (`CBDT`, Noto Color
//! Emoji) has none, and a layered one (`COLR` version 0) has a plain one
//! that is never drawn — so it casts what it draws. Measured in Chrome
//! 156.0.8078.76, headless, at one, two and eight, with test faces made
//! for it (`alo-paint/tests/fonts/pictured.ttf` and `layered-deep.ttf`):
//!
//! - **A picture glyph casts its picture's alpha**: a pixel of the picture
//!   at half opacity casts half the shadow's colour, a transparent one
//!   nothing — never the picture's rectangle.
//! - **A layered glyph casts its layers as they are drawn over each
//!   other**: a layer of the palette at half opacity casts half, an opaque
//!   layer over it the whole; the glyph's own plain outline nothing. A
//!   layer in the text's colour casts whole whatever the text colour's own
//!   opacity, as a letter does.
//! - **A glyph is one shape, softened on its own.** Within a layered glyph
//!   the blur sees no seam where one layer meets another (an opaque layer
//!   beside a half one, blurred 16: Chrome's seam within 3 levels of one
//!   shape's, where layers softened apart would be 32 lighter). Between
//!   glyphs it does: each glyph's shadow is softened apart and the shadows
//!   are put over each other — an emoji 8 px from a letter, blurred 16,
//!   meet at 156 where one shape would be 147. (Chrome softens plain
//!   letters apart too, which this engine does not yet: queue item 831.)
//!
//! A glyph's parts are put onto one coverage mask the way layers of one
//! colour are put over each other — each part's alpha over what is there —
//! in the device pixels it is drawn in.

use crate::canvas::Canvas;
use crate::coverage::Coverage;
use crate::drawn_picture::draw_picture;
use crate::path::Path;
use crate::picture_filter::Uploads;
use crate::raster::fill;
use alo_layout::Rect;
use alo_value::{Matrix, Rgba};
use std::sync::Arc;

/// One colour glyph as it casts a shadow, in CSS pixels, already moved by
/// the shadow's offset.
#[derive(Debug, Clone)]
pub enum Cast {
    /// A picture glyph: where its picture is drawn, and its pixels.
    Picture(Rect, Arc<Canvas>),
    /// A layered glyph: each layer, bottom first, with the opacity it is
    /// drawn at — its palette colour's, or one for a layer in the text's
    /// colour.
    Layers(Vec<(Path, f32)>),
}

impl Cast {
    /// What kind of glyph it is, in words: `a picture`, or `3 layers`.
    pub fn kind(&self) -> String {
        match self {
            Cast::Picture(..) => "a picture".to_owned(),
            Cast::Layers(layers) => format!("{} layers", layers.len()),
        }
    }

    /// The same, moved `by` CSS pixels.
    #[must_use]
    pub fn translated(&self, by: (f32, f32)) -> Self {
        match self {
            Cast::Picture(rect, pixels) => Cast::Picture(
                rect.translated(alo_layout::Point::new(by.0, by.1)),
                Arc::clone(pixels),
            ),
            Cast::Layers(layers) => {
                let point = crate::path::Point::new(by.0, by.1);
                Cast::Layers(
                    layers
                        .iter()
                        .map(|(path, opacity)| (path.translated(point), *opacity))
                        .collect(),
                )
            }
        }
    }

    /// Its bounds in CSS pixels, as `(left, top, right, bottom)`; [`None`]
    /// when it casts nothing.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
            Cast::Picture(rect, _) => Some((rect.left(), rect.top(), rect.right(), rect.bottom())),
            Cast::Layers(layers) => {
                layers
                    .iter()
                    .filter_map(|(path, _)| path.bounds())
                    .reduce(|one, other| {
                        (
                            one.0.min(other.0),
                            one.1.min(other.1),
                            one.2.max(other.2),
                            one.3.max(other.3),
                        )
                    })
            }
        }
    }
}

/// The shape `cast` makes under `transform`, in device pixels, before it
/// is softened: a picture's alpha, or each layer's coverage at its opacity
/// put over the others'.
pub fn coverage(cast: &Cast, transform: Matrix) -> Coverage {
    match cast {
        Cast::Picture(rect, pixels) => alpha_of(pixels, *rect, transform),
        Cast::Layers(layers) => {
            let parts = layers
                .iter()
                .map(|(path, opacity)| {
                    let moved = if transform.is_identity() {
                        path.clone()
                    } else {
                        path.transformed(transform)
                    };
                    (fill(&moved), opacity.clamp(0.0, 1.0))
                })
                .collect::<Vec<_>>();
            over_each_other(&parts)
        }
    }
}

/// A picture's alpha where it is drawn into `rect` under `transform`, as
/// coverage: drawn as any picture is ([`draw_picture`]), onto a clear
/// canvas just large enough, so it is scaled and smoothed as the glyph
/// itself is.
fn alpha_of(pixels: &Arc<Canvas>, rect: Rect, transform: Matrix) -> Coverage {
    let corners = [
        transform.apply(rect.left(), rect.top()),
        transform.apply(rect.right(), rect.top()),
        transform.apply(rect.left(), rect.bottom()),
        transform.apply(rect.right(), rect.bottom()),
    ];
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in corners {
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    // A pixel's worth of room on every side, so an edge rounded outwards
    // is kept; whole pixels, so moving onto the small canvas moves nothing
    // within a pixel.
    let (Some(origin_x), Some(origin_y), Some(width), Some(height)) = (
        whole(left.floor() - 1.0),
        whole(top.floor() - 1.0),
        span(right.ceil() - left.floor() + 2.0),
        span(bottom.ceil() - top.floor() + 2.0),
    ) else {
        return Coverage::empty();
    };
    let mut canvas = Canvas::new(width, height, Rgba::TRANSPARENT);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a whole pixel of a page, at most a few thousand from its corner"
    )]
    let onto = transform.then(Matrix::translation(-(origin_x as f32), -(origin_y as f32)));
    // The glyph's own draw decides how its picture is uploaded; this one is
    // the shadow's, and leaves that alone.
    draw_picture(&mut canvas, pixels, rect, onto, &mut Uploads::default());
    let data = canvas
        .pixels()
        .iter()
        .map(|pixel| pixel.to_rgba8().3)
        .collect();
    Coverage::new(width, height, (origin_x, origin_y), data)
}

/// `value`, a whole number of pixels, as one — [`None`] off any page.
fn whole(value: f32) -> Option<i32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked to be a whole number within a page's reach first"
    )]
    let whole = (value.is_finite() && value.abs() < 1.0e7).then_some(value as i32);
    whole
}

/// `value`, a whole number of pixels across, as a count of them — [`None`]
/// for none, or more than a canvas is.
fn span(value: f32) -> Option<u32> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to be a whole positive number within a canvas's reach first"
    )]
    let span = (value.is_finite() && (1.0..65_536.0).contains(&value)).then_some(value as u32);
    span
}

/// Each coverage, at its opacity, put over the others: where two parts
/// cover a pixel, the second covers what the first left uncovered.
fn over_each_other(parts: &[(Coverage, f32)]) -> Coverage {
    let drawn = parts
        .iter()
        .filter(|(coverage, opacity)| !coverage.is_empty() && *opacity > 0.0);
    let Some((left, top, right, bottom)) = drawn
        .clone()
        .map(|(coverage, _)| {
            let (x, y) = coverage.origin();
            (
                i64::from(x),
                i64::from(y),
                i64::from(x) + i64::from(coverage.width()),
                i64::from(y) + i64::from(coverage.height()),
            )
        })
        .reduce(|one, other| {
            (
                one.0.min(other.0),
                one.1.min(other.1),
                one.2.max(other.2),
                one.3.max(other.3),
            )
        })
    else {
        return Coverage::empty();
    };
    let (Ok(width), Ok(height), Ok(origin_x), Ok(origin_y)) = (
        u32::try_from(right - left),
        u32::try_from(bottom - top),
        i32::try_from(left),
        i32::try_from(top),
    ) else {
        return Coverage::empty();
    };
    let row = width as usize;
    let mut alpha = vec![0.0_f32; row.saturating_mul(height as usize)];
    for (coverage, opacity) in drawn {
        let (x, y) = coverage.origin();
        let (Ok(across), Ok(down)) = (
            usize::try_from(i64::from(x) - left),
            usize::try_from(i64::from(y) - top),
        ) else {
            continue;
        };
        for (index, value) in coverage.data().iter().enumerate() {
            let column = across + index % coverage.width() as usize;
            let line = down + index / coverage.width() as usize;
            if let Some(under) = alpha.get_mut(line * row + column) {
                let over = f32::from(*value) / 255.0 * opacity;
                *under = over + *under * (1.0 - over);
            }
        }
    }
    let data = alpha
        .into_iter()
        .map(|value| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "an opacity from zero to one, as a byte"
            )]
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            byte
        })
        .collect();
    Coverage::new(width, height, (origin_x, origin_y), data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(left: f32, top: f32, side: f32) -> Path {
        Path::rectangle(left, top, side, side)
    }

    #[test]
    fn a_layer_at_half_casts_half_and_an_opaque_one_over_it_the_whole() {
        let cast = Cast::Layers(vec![
            (square(0.0, 0.0, 4.0), 0.5),
            (square(2.0, 0.0, 4.0), 1.0),
        ]);
        let shape = coverage(&cast, Matrix::IDENTITY);
        assert_eq!(shape.origin(), (0, 0));
        assert_eq!((shape.width(), shape.height()), (6, 4));
        assert_eq!(shape.at(0, 1), 128, "the half layer alone");
        assert_eq!(shape.at(3, 1), 255, "the opaque one over it");
        assert_eq!(shape.at(5, 1), 255, "the opaque one alone");
    }

    #[test]
    fn two_halves_over_each_other_cast_three_quarters() {
        let cast = Cast::Layers(vec![
            (square(0.0, 0.0, 2.0), 0.5),
            (square(0.0, 0.0, 2.0), 0.5),
        ]);
        assert_eq!(coverage(&cast, Matrix::IDENTITY).at(1, 1), 191);
    }

    #[test]
    fn a_picture_casts_its_alpha_and_not_its_rectangle() {
        // Two by one: an opaque pixel, then a clear one.
        let mut pixels = Canvas::new(2, 1, Rgba::TRANSPARENT);
        pixels.blend(0, 0, Rgba::from_rgba8(230, 57, 70, 255), 255);
        let cast = Cast::Picture(Rect::new(3.0, 5.0, 4.0, 2.0), Arc::new(pixels));
        // At two, the picture is 8 × 4 device pixels from (6, 10).
        let shape = coverage(&cast, Matrix::scaling(2.0));
        let (x, y) = shape.origin();
        let at = |column: i32, row: i32| match (u32::try_from(column - x), u32::try_from(row - y)) {
            (Ok(column), Ok(row)) => shape.at(column, row),
            _ => 0,
        };
        assert_eq!(at(6, 11), 255, "the opaque pixel's left edge");
        assert_eq!(at(13, 11), 0, "the clear pixel's right edge");
        assert_eq!(at(5, 11), 0, "left of the picture");
        assert!(at(9, 12) > 0 && at(9, 12) < 255, "smoothed between them");
    }

    #[test]
    fn its_bounds_hold_every_layer_and_it_moves_whole() {
        let layers = Cast::Layers(vec![
            (square(0.0, 0.0, 2.0), 1.0),
            (square(10.0, -3.0, 1.0), 0.5),
        ]);
        assert_eq!(layers.bounds(), Some((0.0, -3.0, 11.0, 2.0)));
        assert_eq!(
            layers.translated((1.0, 2.0)).bounds(),
            Some((1.0, -1.0, 12.0, 4.0))
        );
        let picture = Cast::Picture(
            Rect::new(4.0, 4.0, 2.0, 2.0),
            Arc::new(Canvas::new(1, 1, Rgba::TRANSPARENT)),
        );
        assert_eq!(
            picture.translated((0.0, 50.0)).bounds(),
            Some((4.0, 54.0, 6.0, 56.0))
        );
        assert_eq!(Cast::Layers(Vec::new()).bounds(), None);
    }
}
