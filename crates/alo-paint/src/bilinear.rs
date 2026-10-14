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
//! 3. **Rounded towards the middle.** The mix is a level and some 65,536ths,
//!    and becomes a level by rounding half up after being moved a
//!    thirty-second of a level towards 127.5: up below 128, down from it.
//!    A plain rounding disagreed with Chrome on 27,094 of the samples, and
//!    this on those 59.
//!
//! The colours are mixed as they are written, in sRGB, not in linear light:
//! black and white halfway is 128.
//!
//! # Translucent texels
//!
//! A pixel mixed from any texel that is not opaque is mixed with the same
//! weights, its colours premultiplied by their alpha so that a transparent
//! texel lends nothing, and composited in floats like every other drawing
//! on the canvas. Chrome rounds such a pixel once more somewhere, and the
//! two differ by a level in 6 of 147 pixels measured over white, along a
//! row of a red texel beside a transparent one at one and at two: queue
//! item 600.
//!
//! # What this does not decide
//!
//! A picture drawn smaller than it is, on either axis, is not sampled here:
//! Chrome filters it the same way at three quarters of its size and more
//! (16 texels to 12, 40 to 30) and through mipmaps by 16 to 10, which is
//! item 179's. Nor is a turned picture,
//! nor `image-rendering`, which this engine does not read yet (601).

use crate::canvas::Canvas;
use alo_value::Rgba;

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

/// The colour the picture gives the pixel sampled at `across` and `down`,
/// or [`None`] for a tap off the picture.
pub fn sample(picture: &Canvas, across: Tap, down: Tap) -> Option<Rgba> {
    let texels = [
        picture.at(across.near, down.near)?.to_rgba8(),
        picture.at(across.far, down.near)?.to_rgba8(),
        picture.at(across.near, down.far)?.to_rgba8(),
        picture.at(across.far, down.far)?.to_rgba8(),
    ];
    let (x, y) = (u64::from(across.weight), u64::from(down.weight));
    let shares = [(256 - x) * (256 - y), x * (256 - y), (256 - x) * y, x * y];
    if texels.iter().all(|(_, _, _, alpha)| *alpha == u8::MAX) {
        let mixed = |channel: fn(&(u8, u8, u8, u8)) -> u8| {
            rounded(
                texels
                    .iter()
                    .zip(shares)
                    .map(|(texel, share)| share * u64::from(channel(texel)))
                    .sum(),
            )
        };
        return Some(Rgba::from_rgba8(
            mixed(|t| t.0),
            mixed(|t| t.1),
            mixed(|t| t.2),
            u8::MAX,
        ));
    }
    Some(translucent(&texels, shares))
}

/// One 65,536th of a level, the unit [`sample`]'s mix is counted in.
const PARTS: u64 = 256 * 256;

/// A mix counted in 65,536ths of a level, as a level: moved a thirty-second
/// of one towards the middle, then rounded half up. Never more than 255,
/// which a mix of levels cannot pass.
fn rounded(mix: u64) -> u8 {
    // In 32nds of a level: a half is 16 of them and the move is one, up
    // below the middle and down from it.
    let half_and_move = if mix < 128 * PARTS { 17 } else { 15 };
    let level = (32 * mix + half_and_move * PARTS) / (32 * PARTS);
    u8::try_from(level).unwrap_or(u8::MAX)
}

/// A mix from texels not all opaque: premultiplied, so a transparent texel
/// lends no colour, and handed back as a colour over its alpha.
fn translucent(texels: &[(u8, u8, u8, u8); 4], shares: [u64; 4]) -> Rgba {
    let alpha: u64 = texels
        .iter()
        .zip(shares)
        .map(|(texel, share)| share * u64::from(texel.3))
        .sum();
    if alpha == 0 {
        return Rgba::TRANSPARENT;
    }
    let premultiplied = |channel: fn(&(u8, u8, u8, u8)) -> u8| -> u64 {
        texels
            .iter()
            .zip(shares)
            .map(|(texel, share)| share * u64::from(channel(texel)) * u64::from(texel.3))
            .sum()
    };
    // Every sum is under 2³², so each is exact in an `f64`; the colour over
    // the alpha is a channel from zero to one.
    let exact = |value: u64| f64::from(u32::try_from(value).unwrap_or(u32::MAX));
    let over_alpha = |value: u64| narrowed(exact(value) / (exact(alpha) * 255.0));
    Rgba {
        red: over_alpha(premultiplied(|t| t.0)),
        green: over_alpha(premultiplied(|t| t.1)),
        blue: over_alpha(premultiplied(|t| t.2)),
        alpha: narrowed(exact(alpha) / (exact(PARTS) * 255.0)),
    }
}

/// A channel from zero to one, as the canvas holds it.
fn narrowed(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a channel between zero and one, where an f32 keeps every level a canvas holds"
    )]
    let narrow = value.clamp(0.0, 1.0) as f32;
    narrow
}

#[cfg(test)]
mod tests {
    use super::*;

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
                sample(picture, across, down)
                    .expect("on the picture")
                    .to_rgba8()
                    .0
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
            .map(|across| sample(&wide, across, down).expect("on it").to_rgba8())
            .collect();
        assert_eq!(seam, [(56, 75, 187, 255), (165, 61, 72, 255)]);
    }

    #[test]
    fn a_mix_is_moved_towards_the_middle_before_it_is_rounded() {
        // A level and 120/256 rounds up below 128, a level and 135/256
        // rounds down from it: what a plain half-up rounding gets wrong.
        assert_eq!(rounded(5 * PARTS + 120 * 256), 6);
        assert_eq!(rounded(5 * PARTS + 119 * 256), 5);
        assert_eq!(rounded(200 * PARTS + 135 * 256), 200);
        assert_eq!(rounded(200 * PARTS + 136 * 256), 201);
        assert_eq!(rounded(255 * PARTS), 255);
        assert_eq!(rounded(0), 0);
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
        let mixed = sample(&pair, across, down).expect("on it").to_rgba8();
        assert_eq!(mixed, (200, 30, 30, 128));
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
        assert_eq!(sample(&clear, across, down), Some(Rgba::TRANSPARENT));
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
