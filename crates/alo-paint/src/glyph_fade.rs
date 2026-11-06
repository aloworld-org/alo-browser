/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A colour glyph under text of a translucent colour (queue item 830).
//!
//! A colour glyph is drawn in its own colours, not the text's — but Chrome
//! 156 draws it at the text colour's own **opacity**: under
//! `color: rgba(42, 157, 143, 0.3)` a palette red `#e63946` is 247, 195,
//! 199 over white, and under `color: transparent` nothing at all. Measured
//! (Chrome 156.0.8078.76, headless, its default raster path) on a `COLR`
//! version 0 glyph and a `CBDT` one under eighteen text alphas, over
//! white, black and grey:
//!
//! - **A layered glyph is faded as one group.** Its layers are put over
//!   each other first — a layer in the text's colour at that colour's own
//!   opacity — and what they make is drawn at the text's opacity once:
//!   the text-coloured foot over the palette's leg is the leg's red moved
//!   towards the faint teal and *then* faded, 195, 181, 181 at 0.5 over
//!   white, where layers faded one by one would be 192, 156, 162. The
//!   group is made as Chrome's software rasteriser makes a glyph's picture,
//!   in eight bits premultiplied — each colour premultiplied to a level
//!   (`round(c × a / 255)`), and each layer put over what is there as
//!   `s + ⌊d × (256 − sₐ) / 256⌋` — then faded as a picture is
//!   ([`crate::mix_onto::faded_onto`]). All 108 samples of the foot over
//!   the leg and over the half-opaque side, at one and at two, eighteen
//!   alphas over three grounds, are this rule's; the group held in floats
//!   was a level off in 11 of them.
//! - **A picture glyph is its picture faded** — see
//!   [`crate::drawn_picture::draw_faded_picture`].
//! - **Its shadow is not faded** ([`crate::glyph_shadow`], queue item
//!   786), as a letter's is not.
//!
//! The text alpha is the colour's own byte, a half up from its float as
//! [`Rgba::to_rgba8`] gives it — `0.3` is 77 and `0.7` 179, both Chrome's —
//! and so is a layer's own colour inside the group: the foot at 0.3 drawn
//! at 0.3 itself left 768 pixels of the test page a level off.
//!
//! A layer's edge that covers part of a pixel is put over the group with
//! its colour scaled by the coverage as Chrome's software blitter scales it
//! (`⌊c × (coverage + 1) / 256⌋`); every edge of the measured glyphs is on a
//! whole pixel, so that is read from the rasteriser's rule, not measured.

use crate::bilinear::Mix;
use crate::canvas::Canvas;
use crate::coverage::Coverage;
use crate::mix_onto::faded_onto;
use crate::path::Path;
use crate::raster::fill;
use alo_value::{Matrix, Rgba};

/// How strongly a colour glyph under text of `color` is drawn, of 255: the
/// colour's own alpha as a byte. 255 draws it as it is; 0 not at all.
pub fn fade_of(color: Rgba) -> u8 {
    color.to_rgba8().3
}

/// Draw a layered glyph's `layers` — each an outline and its colour,
/// bottom first, in CSS pixels — under `transform` onto `target`, put over
/// each other on a surface of their own and faded once by `fade` of 255,
/// keeping at each pixel only as much as `inside` lets through (255 all of
/// it, as with no clip).
pub fn draw_faded_layers(
    target: &mut Canvas,
    layers: &[(Path, Rgba)],
    fade: u8,
    transform: Matrix,
    inside: &dyn Fn(u32, u32) -> u8,
) {
    if fade == 0 {
        return;
    }
    // Each colour as Chrome holds a colour, a level a channel,
    // premultiplied to a level.
    let shapes: Vec<(Coverage, [u16; 4])> = layers
        .iter()
        .map(|(path, colour)| (fill(&path.transformed(transform)), premultiplied(*colour)))
        .filter(|(coverage, colour)| !coverage.is_empty() && colour[3] > 0)
        .collect();
    let Some(((left, top), (width, height))) = reach(shapes.iter().map(|(shape, _)| shape)) else {
        return;
    };
    // The group: transparent, just large enough, eight bits premultiplied.
    let row = width as usize;
    let mut group = vec![[0_u16; 4]; row.saturating_mul(height as usize)];
    for (shape, colour) in &shapes {
        let (x, y) = shape.origin();
        for down in 0..shape.height() {
            for across in 0..shape.width() {
                let (Some(column), Some(line)) = (offset(x - left, across), offset(y - top, down))
                else {
                    continue;
                };
                if let Some(pixel) = group.get_mut(line as usize * row + column as usize) {
                    *pixel = over(covered(*colour, shape.at(across, down)), *pixel);
                }
            }
        }
    }
    put_held(target, (left, top), width, &group, fade, inside);
}

/// Put a glyph's picture — `pixels`, premultiplied levels, rows `width`
/// long, its corner at `(left, top)` on the page — onto `target` at `fade`
/// of 255, as a picture is faded ([`faded_onto`]), keeping at each pixel
/// only as much as `inside` lets through: a layered glyph's group, and a
/// painted one's (queue item 785).
pub fn put_held(
    target: &mut Canvas,
    (left, top): (i32, i32),
    width: u32,
    pixels: &[[u16; 4]],
    fade: u8,
    inside: &dyn Fn(u32, u32) -> u8,
) {
    let row = width.max(1) as usize;
    for (index, pixel) in pixels.iter().enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        let (Ok(across), Ok(down)) = (u32::try_from(index % row), u32::try_from(index / row))
        else {
            continue;
        };
        let (Some(x), Some(y)) = (offset(left, across), offset(top, down)) else {
            continue;
        };
        let kept = inside(x, y);
        if kept == 0 {
            continue;
        }
        let mix = Mix {
            red: pixel[0] * 16,
            green: pixel[1] * 16,
            blue: pixel[2] * 16,
            alpha: pixel[3] * 16,
        };
        target.replace(x, y, |under| {
            let drawn = faded_onto(mix, under, fade);
            between(under, drawn, kept)
        });
    }
}

/// A colour as a level a channel, premultiplied to a level as Chrome's
/// rasteriser premultiplies one: `round(c × a / 255)`.
fn premultiplied(colour: Rgba) -> [u16; 4] {
    let (red, green, blue, alpha) = colour.to_rgba8();
    let alpha = u16::from(alpha);
    let times = |channel: u8| (u16::from(channel) * alpha + 127) / 255;
    [times(red), times(green), times(blue), alpha]
}

/// A premultiplied colour at `coverage` of 255, as Chrome's software
/// blitter scales one: `⌊c × (coverage + 1) / 256⌋`.
fn covered(colour: [u16; 4], coverage: u8) -> [u16; 4] {
    if coverage == u8::MAX {
        return colour;
    }
    let scale = u16::from(coverage) + 1;
    colour.map(|channel| channel * scale / 256)
}

/// `source` put over `under`, both premultiplied levels, as Chrome's
/// software blitter puts it: `s + ⌊d × (256 − sₐ) / 256⌋`.
fn over(source: [u16; 4], under: [u16; 4]) -> [u16; 4] {
    let keep = 256 - source[3];
    [0, 1, 2, 3].map(|channel| {
        let (s, d) = (
            source.get(channel).copied().unwrap_or(0),
            under.get(channel).copied().unwrap_or(0),
        );
        (s + d * keep / 256).min(255)
    })
}

/// `was`, moved towards `drawn` by `kept` of 255: what a clip's soft edge
/// keeps, as a fill under it keeps.
fn between(was: Rgba, drawn: Rgba, kept: u8) -> Rgba {
    if kept == u8::MAX {
        return drawn;
    }
    let part = f32::from(kept) / 255.0;
    let mix = |from: f32, to: f32| (to - from).mul_add(part, from);
    Rgba {
        red: mix(was.red, drawn.red),
        green: mix(was.green, drawn.green),
        blue: mix(was.blue, drawn.blue),
        alpha: mix(was.alpha, drawn.alpha),
    }
}

/// The device pixels every shape reaches together, as its corner on the
/// page (never left of or above it) and its size; [`None`] for none.
fn reach<'a>(shapes: impl Iterator<Item = &'a Coverage>) -> Option<((i32, i32), (u32, u32))> {
    let (left, top, right, bottom) = shapes
        .map(|shape| {
            let (x, y) = shape.origin();
            (
                i64::from(x),
                i64::from(y),
                i64::from(x) + i64::from(shape.width()),
                i64::from(y) + i64::from(shape.height()),
            )
        })
        .reduce(|one, other| {
            (
                one.0.min(other.0),
                one.1.min(other.1),
                one.2.max(other.2),
                one.3.max(other.3),
            )
        })?;
    let (left, top) = (left.max(0), top.max(0));
    if right <= left || bottom <= top {
        return None;
    }
    Some((
        (i32::try_from(left).ok()?, i32::try_from(top).ok()?),
        (
            u32::try_from(right - left).ok()?,
            u32::try_from(bottom - top).ok()?,
        ),
    ))
}

/// `base + step`, when that is on the canvas's side of its edge.
fn offset(base: i32, step: u32) -> Option<u32> {
    u32::try_from(i64::from(base) + i64::from(step)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(left: f32, top: f32, side: f32) -> Path {
        Path::rectangle(left, top, side, side)
    }

    fn drawn(layers: &[(Path, Rgba)], fade: u8, under: Rgba) -> Canvas {
        let mut canvas = Canvas::new(8, 4, under);
        draw_faded_layers(&mut canvas, layers, fade, Matrix::IDENTITY, &|_, _| u8::MAX);
        canvas
    }

    fn at(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8)> {
        canvas.at(x, y).map(|pixel| {
            let (red, green, blue, _) = pixel.to_rgba8();
            (red, green, blue)
        })
    }

    const RED: Rgba = Rgba {
        red: 230.0 / 255.0,
        green: 57.0 / 255.0,
        blue: 70.0 / 255.0,
        alpha: 1.0,
    };

    #[test]
    fn a_palette_red_under_text_at_three_tenths_is_chromes() {
        // Chrome 156: 247, 195, 199 over white; 69, 17, 21 over black.
        let layers = [(square(0.0, 0.0, 4.0), RED)];
        let fade = fade_of(Rgba::from_rgba8(42, 157, 143, 77));
        assert_eq!(fade, 77);
        assert_eq!(
            at(&drawn(&layers, fade, Rgba::WHITE), 1, 1),
            Some((247, 195, 199))
        );
        assert_eq!(
            at(&drawn(&layers, fade, Rgba::BLACK), 1, 1),
            Some((69, 17, 21))
        );
    }

    #[test]
    fn the_layers_are_put_over_each_other_before_they_are_faded() {
        // The leg's red, and over its right half the foot in the text's
        // own colour at half: Chrome 156 draws 195, 181, 181 over white
        // where the foot is, and the leg alone 242, 156, 162.
        let teal = Rgba::from_rgba8(42, 157, 143, 128);
        let layers = [
            (square(0.0, 0.0, 4.0), RED),
            (Path::rectangle(2.0, 0.0, 2.0, 4.0), teal),
        ];
        let canvas = drawn(&layers, 128, Rgba::WHITE);
        assert_eq!(at(&canvas, 0, 1), Some((242, 156, 162)));
        assert_eq!(at(&canvas, 3, 1), Some((195, 181, 181)));
        assert_eq!(at(&canvas, 6, 1), Some((255, 255, 255)), "outside it");
    }

    #[test]
    fn the_group_is_made_in_eight_bits_as_chromes_rasteriser_makes_it() {
        // At 0.7 (179 of 255) Chrome 156 draws the foot over the leg 145,
        // 165, 161 over white and 69, 89, 85 over black: the group's red
        // 29 + ⌊230 × 77 / 256⌋ = 98, where floats make 99 and 146.
        let alpha = Rgba {
            alpha: 0.7,
            ..Rgba::from_rgba8(42, 157, 143, 255)
        };
        assert_eq!(fade_of(alpha), 179);
        let layers = [(square(0.0, 0.0, 4.0), RED), (square(0.0, 0.0, 4.0), alpha)];
        assert_eq!(
            at(&drawn(&layers, 179, Rgba::WHITE), 1, 1),
            Some((145, 165, 161))
        );
        assert_eq!(
            at(&drawn(&layers, 179, Rgba::BLACK), 1, 1),
            Some((69, 89, 85))
        );
    }

    #[test]
    fn a_half_opaque_palette_colour_is_held_premultiplied_then_faded() {
        // The side, `#1d4ed8` at 128 of 255: Chrome 156 draws 221, 228, 249
        // at 0.3 over white, and 113, 120, 141 over grey.
        let blue = Rgba::from_rgba8(0x1d, 0x4e, 0xd8, 128);
        let layers = [(square(0.0, 0.0, 4.0), blue)];
        assert_eq!(
            at(&drawn(&layers, 77, Rgba::WHITE), 1, 1),
            Some((221, 228, 249))
        );
        let grey = Rgba::from_rgba8(128, 128, 128, 255);
        assert_eq!(at(&drawn(&layers, 77, grey), 1, 1), Some((113, 120, 141)));
    }

    #[test]
    fn transparent_text_draws_nothing_and_a_clip_keeps_what_it_lets_through() {
        let layers = [(square(0.0, 0.0, 4.0), RED)];
        let none = drawn(&layers, 0, Rgba::WHITE);
        assert_eq!(at(&none, 1, 1), Some((255, 255, 255)));
        let mut clipped = Canvas::new(8, 4, Rgba::WHITE);
        draw_faded_layers(&mut clipped, &layers, u8::MAX, Matrix::IDENTITY, &|x, _| {
            if x < 2 { u8::MAX } else { 0 }
        });
        assert_eq!(at(&clipped, 1, 1), Some((230, 57, 70)));
        assert_eq!(at(&clipped, 3, 1), Some((255, 255, 255)));
    }
}
