/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a smoothed picture's pixel meets the pixel beneath it, as Chrome 154
//! puts it there (queue item 600).
//!
//! Split from [`crate::bilinear`], which mixes the texels: this file is only
//! what the [`Mix`] becomes on the page, and it changes when compositing a
//! picture does.
//!
//! # The rule, measured
//!
//! Chrome 154 (headless, its default raster path, screenshots without
//! `--disable-gpu`, the source not read) composites the mix — premultiplied,
//! in sixteenths of a level — onto the eight-bit pixel beneath it in
//! **32-bit floats**, in one fused multiply-add, and writes the level out
//! rounded a half up:
//!
//! ```text
//! s = (colour / 16) / 255      a = (alpha / 16) / 255      d = beneath / 255
//! x = fma(d, 1 − a, s)         level = ⌊x · 255 + ½⌋
//! ```
//!
//! each of `s`, `a`, `d` and `1 − a` a float of its own. Measured
//! (iteration 364's scratchpad `m600/`): a black translucent texel beside a
//! transparent one, every alpha, twenty widths, over four colours —
//! 220,160 samples, every one; every colour level at seven alphas over
//! white, grey and black, 1,076,505 distinct samples, every one; 300 random
//! translucent pairs at seven widths over five colours, 423,685 samples at
//! one and 1,694,670 at two, all but two at one — a column whose position is
//! 0.007 of a 256th from where its weight rounds the other way, as item 594
//! found; and 256 × 256 random texels drawn at their own size over seven
//! colours, 458,752, every one. The blend done in exact arithmetic instead
//! missed 168,747 of the pairs, a product and a sum rounded apart 4,537 of
//! the half-way ones.
//!
//! The floats are what decide a mix exactly half way between two levels. An
//! opaque one, whose `1 − a` is nothing, comes out a half **towards 127.5**
//! — up below 128, down from it — at every level, which is the rule item 594
//! fitted to opaque pictures alone.
//!
//! # What this does not decide
//!
//! A translucent mix over a pixel that is not opaque itself — a picture
//! inside a group drawn at an opacity, onto that group's own transparent
//! canvas — was not measured. Chrome holds such a pixel premultiplied in
//! eight bits too; this engine composites it in floats, as every other
//! drawing onto that canvas is.

use crate::bilinear::Mix;
use alo_value::Rgba;

/// The pixel `mix` makes of the pixel `under` it.
pub fn onto(mix: Mix, under: Rgba) -> Rgba {
    faded_onto(mix, under, u8::MAX)
}

/// The same, the mix drawn at `fade` of 255 — a colour glyph's picture
/// under text of a translucent colour (queue item 830).
///
/// Measured in Chrome 156.0.8078.76 (headless, its default raster path):
/// a `CBDT` glyph's 1,600 random texels, drawn at their own size under
/// eighteen text alphas over white, black and grey, are every one this
/// rule's — the mix's colour and alpha each times `fade / 255`, then
/// composited as an unfaded mix is — with the alpha's byte rounded a half
/// up from the colour's own (0.3 is 77), as every colour's is. At 255 it is
/// [`onto`] exactly: a product with one changes no float.
pub fn faded_onto(mix: Mix, under: Rgba, fade: u8) -> Rgba {
    if mix.alpha == 0 || fade == 0 {
        return under;
    }
    // An opaque mix at full strength keeps none of what is beneath: its `1 −
    // a` is exactly nothing, so the multiply-add below is its colour alone,
    // whatever the pixel beneath holds (queue item 892). Written out without
    // reading that pixel; `an_opaque_mix_is_its_colour_alone_every_bit`
    // holds the two the same for every colour over every level.
    if mix.alpha == Mix::OPAQUE && fade == u8::MAX {
        return whole_strength(mix);
    }
    if (mix.alpha < Mix::OPAQUE || fade < u8::MAX) && under.to_rgba8().3 < u8::MAX {
        return in_floats(mix, under, fade);
    }
    in_bytes(mix, under, fade)
}

/// A mix onto an opaque pixel, or an opaque mix onto any: in one fused
/// multiply-add per channel onto the level beneath, as measured.
fn in_bytes(mix: Mix, under: Rgba, fade: u8) -> Rgba {
    let (red, green, blue, _) = under.to_rgba8();
    let strength = f32::from(fade) / 255.0;
    let keep = 1.0 - level(mix.alpha) * strength;
    let channel = |colour: u16, beneath: u8| {
        let x = (f32::from(beneath) / 255.0).mul_add(keep, level(colour) * strength);
        written(x)
    };
    Rgba::from_rgba8(
        channel(mix.red, red),
        channel(mix.green, green),
        channel(mix.blue, blue),
        u8::MAX,
    )
}

/// An opaque mix at full strength on the page: each channel its sixteenths
/// written out as a level, the alpha whole.
fn whole_strength(mix: Mix) -> Rgba {
    static WRITTEN: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let table = WRITTEN.get_or_init(|| {
        (0..=Mix::OPAQUE)
            .map(|colour| written(level(colour)))
            .collect()
    });
    let channel = |colour: u16| {
        table
            .get(usize::from(colour))
            .copied()
            .unwrap_or_else(|| written(level(colour)))
    };
    Rgba::from_rgba8(
        channel(mix.red),
        channel(mix.green),
        channel(mix.blue),
        u8::MAX,
    )
}

/// A channel in sixteenths of a level, as a float from zero to one: the
/// sixteenths are exact, and the division by 255 rounds once.
fn level(sixteenths: u16) -> f32 {
    (f32::from(sixteenths) / 16.0) / 255.0
}

/// A float from zero to one written out as a level: times 255, a half up.
/// The product of a float and 255 is exact in an `f64`.
fn written(x: f32) -> u8 {
    let scaled = (f64::from(x) * 255.0 + 0.5).floor().clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 and floored, so a whole number a byte holds"
    )]
    let byte = scaled as u8;
    byte
}

/// A translucent mix over a pixel that is not opaque either: as a colour
/// over its alpha, at `fade` of 255, composited in floats like every other
/// drawing.
fn in_floats(mix: Mix, under: Rgba, fade: u8) -> Rgba {
    let alpha = f32::from(mix.alpha);
    let over = Rgba {
        red: (f32::from(mix.red) / alpha).min(1.0),
        green: (f32::from(mix.green) / alpha).min(1.0),
        blue: (f32::from(mix.blue) / alpha).min(1.0),
        alpha: alpha / f32::from(Mix::OPAQUE) * (f32::from(fade) / 255.0),
    };
    over.over(under)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(sixteenths: u16) -> Mix {
        Mix {
            red: sixteenths,
            green: sixteenths,
            blue: sixteenths,
            alpha: Mix::OPAQUE,
        }
    }

    fn opaque(sixteenths: u16) -> u8 {
        onto(grey(sixteenths), Rgba::WHITE).to_rgba8().0
    }

    #[test]
    fn an_opaque_half_goes_towards_the_middle() {
        // Exactly half way, at every level: up below 128, down from it.
        for level in 0..255_u16 {
            let expected = if level < 128 { level + 1 } else { level };
            assert_eq!(
                u16::from(opaque(level * 16 + 8)),
                expected,
                "{level} and a half"
            );
        }
    }

    #[test]
    fn an_opaque_mix_off_the_half_is_its_nearest_level() {
        assert_eq!(opaque(5 * 16 + 7), 5);
        assert_eq!(opaque(5 * 16 + 9), 6);
        assert_eq!(opaque(200 * 16 + 7), 200);
        assert_eq!(opaque(200 * 16 + 9), 201);
        assert_eq!(opaque(0), 0);
        assert_eq!(opaque(Mix::OPAQUE), 255);
    }

    #[test]
    fn an_opaque_mix_does_not_care_what_is_beneath() {
        let mix = grey(100 * 16 + 3);
        assert_eq!(onto(mix, Rgba::BLACK), onto(mix, Rgba::TRANSPARENT));
        assert_eq!(onto(mix, Rgba::BLACK).to_rgba8(), (100, 100, 100, 255));
    }

    #[test]
    fn an_opaque_mix_is_its_colour_alone_every_bit() {
        // Queue item 892: every colour an opaque mix can hold, in
        // sixteenths, over every level beneath — the shortcut writes what the
        // multiply-add would, so not reading the pixel beneath moves none.
        for colour in 0..=Mix::OPAQUE {
            let mix = Mix {
                red: colour,
                green: colour,
                blue: colour,
                alpha: Mix::OPAQUE,
            };
            for beneath in 0..=u8::MAX {
                for alpha in [0, 1, 128, 254, 255] {
                    let under = Rgba::from_rgba8(beneath, 255 - beneath, beneath / 2, alpha);
                    assert_eq!(
                        whole_strength(mix),
                        in_bytes(mix, under, u8::MAX),
                        "{colour} over {beneath} at {alpha}"
                    );
                }
            }
        }
    }

    #[test]
    fn nothing_mixed_leaves_the_pixel_beneath() {
        let under = Rgba::from_rgba8(1, 2, 3, 4);
        let clear = Mix {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
        assert_eq!(onto(clear, under), under);
    }

    #[test]
    fn a_translucent_mix_over_white_is_chromes() {
        // A black texel at alpha 128 beside a transparent one, the far
        // weight 2: alpha 128 × 254 / 256 = 127, in sixteenths 2032; over
        // white that is 255 − 127 = 128, and Chrome 154 draws 128.
        let mix = Mix {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 2032,
        };
        assert_eq!(onto(mix, Rgba::WHITE).to_rgba8(), (128, 128, 128, 255));
    }

    #[test]
    fn a_half_way_blend_is_decided_by_the_floats() {
        // Measured: an alpha of 3 3/16 over a ground of 200 is exactly
        // 197.5, and Chrome 154 draws 198; over 140, 6 6/16 is 136.5 and it
        // draws 137 — both above 128, where an opaque half goes down.
        let black = |alpha| Mix {
            red: 0,
            green: 0,
            blue: 0,
            alpha,
        };
        let ground = |level| Rgba::from_rgba8(level, level, level, 255);
        assert_eq!(onto(black(51), ground(200)).to_rgba8().0, 198);
        assert_eq!(onto(black(102), ground(140)).to_rgba8().0, 137);
    }

    #[test]
    fn the_ground_and_the_colour_are_added_in_one_rounding() {
        // Measured: a premultiplied colour of 4 at alpha 100 beside a
        // transparent texel, the far weight 52, over 127 — 51 sixteenths at
        // an alpha of 1275 — is 90.5 and a few millionths, and Chrome 154
        // draws 91. A product rounded to a float before the sum is added
        // lands under the half and gives 90; so does 34 at 1530, 82.
        let mix = |colour, alpha| Mix {
            red: colour,
            green: colour,
            blue: colour,
            alpha,
        };
        let grey = Rgba::from_rgba8(127, 127, 127, 255);
        assert_eq!(onto(mix(51, 1275), grey).to_rgba8().0, 91);
        assert_eq!(onto(mix(34, 1530), grey).to_rgba8().0, 82);
    }

    #[test]
    fn red_beside_a_transparent_texel_over_white_is_chromes() {
        // Iteration 358's six pixels: rgb(200 30 30) beside a transparent
        // texel, drawn 80 wide, at weights 35, 54, 170 and 202. Chrome 154
        // draws these; the floats-throughout blend missed each by a level.
        let premultiplied = |weight: u32| {
            let near = 256 - weight;
            let sixteenths =
                |value: u32| u16::try_from((value * near * 256 + 2048) / 4096).unwrap_or(u16::MAX);
            Mix {
                red: sixteenths(200),
                green: sixteenths(30),
                blue: sixteenths(30),
                alpha: sixteenths(255),
            }
        };
        let drawn = |weight| onto(premultiplied(weight), Rgba::WHITE).to_rgba8();
        assert_eq!(drawn(35), (208, 61, 61, 255));
        assert_eq!(drawn(54), (212, 78, 78, 255));
        assert_eq!(drawn(170), (237, 179, 179, 255));
        assert_eq!(drawn(202), (243, 207, 207, 255));
    }

    #[test]
    fn a_translucent_mix_over_a_translucent_pixel_is_composited_in_floats() {
        let mix = Mix {
            red: 2040,
            green: 0,
            blue: 0,
            alpha: 2040,
        };
        let drawn = onto(mix, Rgba::TRANSPARENT);
        assert!((drawn.alpha - 0.5).abs() < 1e-6, "{drawn}");
        assert!((drawn.red - 1.0).abs() < 1e-6, "{drawn}");
    }
}
