/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a picture drawn larger than its own size is smoothed across its
//! texels, as Chrome 154 smooths it (queue item 594).
//!
//! Split from [`crate::drawn_picture`], which decides which pixels a picture
//! covers: this file is only what colour each of them takes, and it changes
//! when the filter does.
//!
//! # The filter, measured
//!
//! Chrome 154 (headless, its default raster path, the source not read) was
//! measured on 37 pictures drawn no smaller than they are at one and at two, a
//! single dark texel among light ones, two-colour halves, ramps and boxes at
//! tenths of a pixel, every pixel of each, and on 1,470,000 channel samples
//! of 400 random colour pairs at sixteen widths. Every opaque pixel of the
//! probes is this, and every sample of the pairs but one column's 59, whose
//! position lies a seventieth of a 256th from where its weight would round
//! the other way — Chrome works the position out in floats:
//!
//! 1. **Bilinear, onto the box as drawn.** A destination pixel's centre is
//!    put back into the picture across the content box as it is drawn on
//!    whole device pixels (`crate::snap`), not as it was laid out: the
//!    pixel `i` of `n` across a picture `w` texels wide samples at
//!    `(i + ½) · w / n − ½` texels, between the two texels either side.
//!    Past the first or the last texel's centre it takes that texel alone,
//!    so the picture's edge is never blended with what is beside it.
//! 2. **The weight in 256ths.** The far texel's share is that position's
//!    fraction rounded to the nearest 1/256, and the two rows of a pair are
//!    mixed the same way down. A ramp of black into white drawn 40 wide is
//!    6, 19, 32, 45, 58 … at one: an exact fraction gives 57 for the fifth.
//! 3. **Held in sixteenths of a level.** The mix is a level and some
//!    65,536ths, and is kept to the nearest sixteenth of a level, a half up:
//!    a [`Mix`]. What it becomes on the page is [`crate::mix_onto`]'s, and for
//!    an opaque mix that is the sixteenths to a level, a half towards 127.5 —
//!    which is the rule iteration 358 first wrote here as "a thirty-second of
//!    a level towards the middle, then a half up", and which agrees with it
//!    on every mix. A plain rounding disagreed with Chrome on 27,094 of the
//!    pairs' samples, and this on those 59.
//!
//! The colours are mixed as they are written, in sRGB, not in linear light:
//! black and white halfway is 128.
//!
//! # Translucent texels
//!
//! A texel that is not opaque is **premultiplied to a level first** — its
//! colour times its alpha over 255, to the nearest level — so a transparent
//! texel lends nothing, and its premultiplied colour and its alpha are each
//! mixed with the same weights and held in sixteenths like an opaque
//! colour (queue item 600). Measured on Chrome 154 over black, where what
//! is drawn is the premultiplied mix alone: every colour level at seven
//! alphas beside a transparent texel, at twenty widths, 2,118,144 channels,
//! every one; and with the same over white and grey, through
//! [`crate::mix_onto`], 1,076,505 distinct samples, every one.
//! Premultiplying in floats instead missed 33,275 of 478,659 channels of
//! random translucent pairs over black.
//!
//! # What this does not decide
//!
//! Which texels are mixed when a picture is drawn smaller on both axes:
//! Chrome mixes two of its mipmap levels then, and [`crate::picture_filter`]
//! decides which (queue item 179), sampling each level with these taps and
//! [`mixed`]. Nor a turned picture, nor `image-rendering`, which this engine
//! does not read yet (601).

use crate::canvas::Canvas;

/// Where a column or a row of the drawing samples the picture: the texel
/// before its position and the texel after, and the second's share in
/// 256ths. At the picture's edges both are the edge texel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tap {
    /// The texel before the position.
    pub near: u32,
    /// The texel after it.
    pub far: u32,
    /// How much of the mix is `far`'s, from 0 to 256.
    pub weight: u32,
}

/// The taps for each of `span` destination pixels across a picture `count`
/// texels long, or none for a span or a picture of no length.
///
/// Worked in whole numbers, so that a position on exactly a 512th rounds as
/// written rather than as a float happens to land: the pixel `i`'s position
/// is `((2i + 1) · count − span) / (2 · span)` texels.
pub fn taps(span: u32, count: u32) -> Vec<Tap> {
    if span == 0 || count == 0 {
        return Vec::new();
    }
    let last = i64::from(count) - 1;
    let count = i64::from(count);
    let den = 2 * i64::from(span);
    (0..span)
        .map(|i| {
            let num = (2 * i64::from(i) + 1) * count - i64::from(span);
            let before = num.div_euclid(den);
            let rest = num.rem_euclid(den);
            // The fraction rest / den, to the nearest 256th, a half up.
            let weight = (rest * 512 + den) / (2 * den);
            Tap {
                near: texel(before.clamp(0, last)),
                far: texel((before + 1).clamp(0, last)),
                weight: texel(weight),
            }
        })
        .collect()
}

/// A texel's index, which [`taps`] has already held inside the picture.
fn texel(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// A mix of texels, premultiplied, each channel in sixteenths of a level:
/// from 0 to [`Mix::OPAQUE`]. What it becomes on the page is
/// [`crate::mix_onto::onto`]'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mix {
    /// Red, premultiplied by the alpha.
    pub red: u16,
    /// Green, premultiplied by the alpha.
    pub green: u16,
    /// Blue, premultiplied by the alpha.
    pub blue: u16,
    /// How opaque the mix is.
    pub alpha: u16,
}

impl Mix {
    /// A whole level of 255, in sixteenths.
    pub const OPAQUE: u16 = 255 * 16;
}

/// A texel premultiplied by its alpha, each channel a level: red, green,
/// blue and alpha.
pub type Texel = [u8; 4];

/// A texel as it is written, premultiplied to a level (queue item 600).
pub fn premultiplied((red, green, blue, alpha): (u8, u8, u8, u8)) -> Texel {
    let times = |channel: u8| u8::try_from(premultiply(channel, alpha)).unwrap_or(u8::MAX);
    [times(red), times(green), times(blue), alpha]
}

/// What the picture gives the pixel sampled at `across` and `down`, or
/// [`None`] for a tap off the picture.
pub fn sample(picture: &Canvas, across: Tap, down: Tap) -> Option<Mix> {
    let texel = |x, y| {
        picture
            .at(x, y)
            .map(|colour| premultiplied(colour.to_rgba8()))
    };
    mixed(texel, across, down).map(|parts| Mix::held(parts, PARTS))
}

/// The four texels `across` and `down` name, mixed, each channel in
/// 65,536ths of a level and not yet rounded: red, green, blue and alpha. A
/// mix of mipmap levels is mixed again before it is held
/// ([`crate::picture_filter`]). [`None`] for a tap off the texels.
pub fn mixed(
    texel: impl Fn(u32, u32) -> Option<Texel>,
    across: Tap,
    down: Tap,
) -> Option<[u64; 4]> {
    let texels = [
        texel(across.near, down.near)?,
        texel(across.far, down.near)?,
        texel(across.near, down.far)?,
        texel(across.far, down.far)?,
    ];
    let (x, y) = (u64::from(across.weight), u64::from(down.weight));
    let shares = [(256 - x) * (256 - y), x * (256 - y), (256 - x) * y, x * y];
    let mut parts = [0_u64; 4];
    for (texel, share) in texels.iter().zip(shares) {
        for (part, channel) in parts.iter_mut().zip(texel) {
            *part += share * u64::from(*channel);
        }
    }
    Some(parts)
}

impl Mix {
    /// A mix counted in `per_level` parts of a level, each channel held to
    /// the nearest sixteenth of a level, a half up.
    pub fn held([red, green, blue, alpha]: [u64; 4], per_level: u64) -> Self {
        let held = |channel| sixteenths(channel, per_level);
        Self {
            red: held(red),
            green: held(green),
            blue: held(blue),
            alpha: held(alpha),
        }
    }
}

/// A texel's channel times its alpha, to the nearest level. A product of
/// two levels over 255 is never exactly a half, so there is no tie to break.
fn premultiply(channel: u8, alpha: u8) -> u64 {
    (u64::from(channel) * u64::from(alpha) + 127) / 255
}

/// One 65,536th of a level, the unit [`mixed`] counts in.
pub const PARTS: u64 = 256 * 256;

/// A mix counted in `per_level` parts of a level, to the nearest sixteenth,
/// a half up. Never more than [`Mix::OPAQUE`], which a mix of levels cannot
/// pass.
fn sixteenths(mix: u64, per_level: u64) -> u16 {
    let doubled = mix.saturating_mul(32).saturating_add(per_level);
    let held = doubled / per_level.max(1).saturating_mul(2);
    u16::try_from(held).map_or(Mix::OPAQUE, |held| held.min(Mix::OPAQUE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mix_onto::onto;
    use alo_value::Rgba;

    fn picture(texels: &[(u8, u8, u8, u8)], width: u32) -> Canvas {
        let height = u32::try_from(texels.len()).expect("small") / width;
        let mut canvas = Canvas::new(width, height, Rgba::TRANSPARENT);
        for (i, (r, g, b, a)) in texels.iter().enumerate() {
            let i = u32::try_from(i).expect("small");
            canvas.blend(i % width, i / width, Rgba::from_rgba8(*r, *g, *b, *a), 255);
        }
        canvas
    }

    /// Black then white, the ramp the weights were read from.
    fn ramp() -> Canvas {
        picture(&[(0, 0, 0, 255), (255, 255, 255, 255)], 2)
    }

    fn row(picture: &Canvas, span: u32) -> Vec<u8> {
        let down = Tap {
            near: 0,
            far: 0,
            weight: 0,
        };
        taps(span, picture.width())
            .into_iter()
            .map(|across| {
                let mix = sample(picture, across, down).expect("on the picture");
                onto(mix, Rgba::WHITE).to_rgba8().0
            })
            .collect()
    }

    #[test]
    fn a_ramp_drawn_forty_wide_is_chromes() {
        // Chrome 154's pixels at one, columns 10 to 29; either side is the
        // edge texel's alone.
        let chrome = [
            6, 19, 32, 45, 58, 70, 83, 96, 109, 122, 133, 146, 159, 172, 185, 197, 210, 223, 236,
            249,
        ];
        let drawn = row(&ramp(), 40);
        assert_eq!(drawn.get(10..30), Some(&chrome[..]));
        assert!(drawn.iter().take(10).all(|level| *level == 0));
        assert!(drawn.iter().skip(30).all(|level| *level == 255));
    }

    #[test]
    fn a_ramp_drawn_seven_wide_is_chromes() {
        assert_eq!(row(&ramp(), 7), [0, 0, 55, 128, 200, 255, 255]);
    }

    #[test]
    fn a_seam_at_once_and_a_half_is_chromes() {
        // `pictures-sized-on-a-line`'s blue and red, their halves 20 texels
        // each, drawn 60 wide: Chrome's two columns of blends where they meet.
        let blue = (29, 78, 216, 255);
        let red = (192, 57, 43, 255);
        let mut texels = vec![blue; 20];
        texels.extend(vec![red; 20]);
        let wide = picture(&texels, 40);
        let down = Tap {
            near: 0,
            far: 0,
            weight: 0,
        };
        let seam: Vec<_> = taps(60, 40)
            .into_iter()
            .skip(29)
            .take(2)
            .map(|across| {
                let mix = sample(&wide, across, down).expect("on it");
                onto(mix, Rgba::WHITE).to_rgba8()
            })
            .collect();
        assert_eq!(seam, [(56, 75, 187, 255), (165, 61, 72, 255)]);
    }

    #[test]
    fn a_mix_is_held_to_the_nearest_sixteenth_of_a_level() {
        // A level and 120/256 is 7.5 sixteenths, a half, and goes up; a
        // level and 119/256 is under it and goes down.
        assert_eq!(sixteenths(5 * PARTS + 120 * 256, PARTS), 5 * 16 + 8);
        assert_eq!(sixteenths(5 * PARTS + 119 * 256, PARTS), 5 * 16 + 7);
        assert_eq!(sixteenths(200 * PARTS + 136 * 256, PARTS), 200 * 16 + 9);
        assert_eq!(sixteenths(255 * PARTS, PARTS), Mix::OPAQUE);
        assert_eq!(sixteenths(0, PARTS), 0);
        assert_eq!(sixteenths(u64::MAX / 2, PARTS), Mix::OPAQUE);
        // Counted in 2^24ths, as two levels mixed in 256ths are: the same
        // half rounds up.
        assert_eq!(
            sixteenths((5 * PARTS + 120 * 256) * 256, PARTS * 256),
            5 * 16 + 8
        );
    }

    #[test]
    fn a_mix_of_opaque_texels_is_the_rule_item_594_fitted() {
        // Iteration 358 wrote it as a thirty-second of a level towards the
        // middle, then a half up: up from 120/256 below 128 and from 136/256
        // above. Held in sixteenths and written out, every mix agrees.
        let earlier = |mix: u64| {
            let half_and_move = if mix < 128 * PARTS { 17 } else { 15 };
            (32 * mix + half_and_move * PARTS) / (32 * PARTS)
        };
        for level in 0..255_u64 {
            for part in 0..256_u64 {
                let mix = level * PARTS + part * 256;
                let held = sixteenths(mix, PARTS);
                let grey = Mix {
                    red: held,
                    green: held,
                    blue: held,
                    alpha: Mix::OPAQUE,
                };
                let now = u64::from(onto(grey, Rgba::BLACK).to_rgba8().0);
                assert_eq!(now, earlier(mix), "{level} and {part}/256");
            }
        }
    }

    #[test]
    fn a_translucent_texel_is_premultiplied_to_a_level_first() {
        // 202 at alpha 89 is 70.5 and a little, so 71; drawn alone at its
        // own size it is that, in sixteenths.
        let one = picture(&[(202, 0, 0, 89)], 1);
        let alone = Tap {
            near: 0,
            far: 0,
            weight: 0,
        };
        let mix = sample(&one, alone, alone).expect("on it");
        assert_eq!((mix.red, mix.alpha), (71 * 16, 89 * 16));
        assert_eq!(premultiply(202, 89), 71);
        assert_eq!(premultiply(255, 255), 255);
        assert_eq!(premultiply(9, 0), 0);
        assert_eq!(premultiplied((202, 9, 255, 89)), [71, 3, 89, 89]);
    }

    #[test]
    fn the_weight_is_the_fraction_to_the_nearest_256th() {
        // Two texels drawn 40 wide: the fifth blended pixel is at 0.225 of
        // the way, 57.6 256ths, which is 58.
        let tap = taps(40, 2).get(14).copied().expect("forty taps");
        assert_eq!(
            tap,
            Tap {
                near: 0,
                far: 1,
                weight: 58
            }
        );
    }

    #[test]
    fn past_the_edge_texels_a_tap_takes_the_edge_alone() {
        let all = taps(10, 2);
        let first = all.first().copied().expect("ten taps");
        let last = all.last().copied().expect("ten taps");
        assert_eq!((first.near, first.far), (0, 0));
        assert_eq!((last.near, last.far), (1, 1));
    }

    #[test]
    fn at_its_own_size_every_tap_is_one_texel() {
        for (i, tap) in (0..5).zip(taps(5, 5)) {
            assert_eq!((tap.near, tap.weight), (i, 0));
        }
    }

    #[test]
    fn nothing_to_sample_has_no_taps() {
        assert!(taps(0, 4).is_empty());
        assert!(taps(4, 0).is_empty());
    }

    #[test]
    fn a_transparent_texel_lends_no_colour() {
        // Red beside a transparent blue: halfway is red at half alpha, not
        // purple.
        let pair = picture(&[(200, 30, 30, 255), (30, 30, 200, 0)], 2);
        let across = Tap {
            near: 0,
            far: 1,
            weight: 128,
        };
        let down = Tap {
            near: 0,
            far: 0,
            weight: 0,
        };
        let mixed = sample(&pair, across, down).expect("on it");
        assert_eq!(
            mixed,
            Mix {
                red: 100 * 16,
                green: 15 * 16,
                blue: 15 * 16,
                alpha: 2040,
            }
        );
    }

    #[test]
    fn wholly_transparent_texels_mix_to_nothing() {
        let clear = picture(&[(0, 0, 0, 0), (9, 9, 9, 0)], 2);
        let across = Tap {
            near: 0,
            far: 1,
            weight: 100,
        };
        let down = Tap {
            near: 0,
            far: 0,
            weight: 0,
        };
        let nothing = Mix {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
        assert_eq!(sample(&clear, across, down), Some(nothing));
    }

    #[test]
    fn a_tap_off_the_picture_samples_nothing() {
        let across = Tap {
            near: 0,
            far: 7,
            weight: 3,
        };
        assert_eq!(sample(&ramp(), across, across), None);
    }
}
