/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Two colours mixed, as `color-mix(in srgb, …)` mixes them.
//!
//! alo Sites writes its shadows and its tinted borders as a fraction of the
//! theme's text colour over nothing — `color-mix(in srgb, var(--text) 12%,
//! transparent)` — so the theme can change and its shadows follow.
//!
//! Mixing **in sRGB** is arithmetic on the channels this engine already holds,
//! which is why it is here while every other space stays refused: there is no
//! conversion to get nearly right. The steps are CSS Color 5's (§ 2):
//!
//! 1. **The shares are normalised.** Neither written is half each; one written
//!    leaves the other the rest of a hundred; two that do not add to a hundred
//!    are scaled until they do, and a total under a hundred is kept as a
//!    multiplier on the result's alpha. Two shares of nothing are no colour.
//! 2. **The channels are mixed premultiplied.** Each colour's channels are
//!    multiplied by its alpha before they are weighed, and the sum divided by
//!    the mixed alpha afterwards. So `transparent` — black at nothing —
//!    contributes opacity and no colour, and a mix with it fades the other
//!    colour rather than darkening it.
//! 3. **The multiplier from step 1 scales the alpha**, last.

use crate::Rgba;

/// One colour in a mix, and the share of it that was written, as a fraction
/// from zero to one — [`None`] where the author wrote none.
pub type Part = (Rgba, Option<f32>);

/// Mix two colours in sRGB.
///
/// [`None`] when the shares are not a mix at all: one outside nothing to a
/// hundred per cent, either not a number, or both zero. CSS calls each of those
/// an invalid `color-mix()`, which leaves the declaration unread.
pub fn mix_srgb(first: Part, second: Part) -> Option<Rgba> {
    let (first_color, first_share) = first;
    let (second_color, second_share) = second;
    let (first_weight, second_weight, alpha_multiplier) = shares(first_share, second_share)?;

    let alpha = first_color.alpha * first_weight + second_color.alpha * second_weight;
    if alpha <= 0.0 {
        return Some(Rgba::TRANSPARENT);
    }
    let channel = |one: f32, other: f32| {
        (one * first_color.alpha * first_weight + other * second_color.alpha * second_weight)
            / alpha
    };
    Some(Rgba::new(
        channel(first_color.red, second_color.red),
        channel(first_color.green, second_color.green),
        channel(first_color.blue, second_color.blue),
        alpha * alpha_multiplier,
    ))
}

/// The two weights, adding to one, and what the alpha is multiplied by.
fn shares(first: Option<f32>, second: Option<f32>) -> Option<(f32, f32, f32)> {
    for share in [first, second].into_iter().flatten() {
        if !(0.0..=1.0).contains(&share) {
            // Out of range, or not a number: `contains` is false for NaN.
            return None;
        }
    }
    let (first, second) = match (first, second) {
        (None, None) => (0.5, 0.5),
        (Some(first), None) => (first, 1.0 - first),
        (None, Some(second)) => (1.0 - second, second),
        (Some(first), Some(second)) => (first, second),
    };
    let total = first + second;
    if total <= 0.0 {
        return None;
    }
    let multiplier = if total < 1.0 { total } else { 1.0 };
    Some((first / total, second / total, multiplier))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = Rgba {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    const BLUE: Rgba = Rgba {
        red: 0.0,
        green: 0.0,
        blue: 1.0,
        alpha: 1.0,
    };

    fn bytes(mixed: Option<Rgba>) -> Option<(u8, u8, u8, u8)> {
        mixed.map(Rgba::to_rgba8)
    }

    #[test]
    fn no_share_written_is_half_each() {
        assert_eq!(
            bytes(mix_srgb((RED, None), (BLUE, None))),
            Some((128, 0, 128, 255))
        );
    }

    #[test]
    fn one_share_written_leaves_the_other_the_rest() {
        let written_first = mix_srgb((RED, Some(0.25)), (BLUE, None));
        let written_second = mix_srgb((RED, None), (BLUE, Some(0.75)));
        assert_eq!(bytes(written_first), Some((64, 0, 191, 255)));
        assert_eq!(bytes(written_first), bytes(written_second));
    }

    #[test]
    fn shares_over_a_hundred_are_scaled_and_keep_the_colour_solid() {
        // 60% and 60% are half each, as if neither had been written.
        assert_eq!(
            bytes(mix_srgb((RED, Some(0.6)), (BLUE, Some(0.6)))),
            Some((128, 0, 128, 255))
        );
    }

    #[test]
    fn shares_under_a_hundred_are_scaled_and_fade_the_result() {
        // 20% and 20% are half each, at forty per cent opacity.
        assert_eq!(
            bytes(mix_srgb((RED, Some(0.2)), (BLUE, Some(0.2)))),
            Some((128, 0, 128, 102))
        );
    }

    #[test]
    fn transparent_fades_a_colour_rather_than_darkening_it() {
        // alo Sites' shadow: its theme's text colour at 12%, over nothing.
        let text = Rgba::from_rgba8(0x17, 0x21, 0x2b, 255);
        let mixed = mix_srgb((text, Some(0.12)), (Rgba::TRANSPARENT, None));
        assert_eq!(bytes(mixed), Some((0x17, 0x21, 0x2b, 31)));
        let alpha = mixed.map(|mixed| mixed.alpha).unwrap_or_default();
        assert!((alpha - 0.12).abs() < 1e-6, "{alpha}");
    }

    #[test]
    fn two_translucent_colours_are_weighed_by_their_opacity() {
        // Red at full opacity and blue at a quarter, half each: the red is
        // four times as present in the mix, and the alpha is the average.
        let faint_blue = Rgba {
            alpha: 0.25,
            ..BLUE
        };
        let mixed = mix_srgb((RED, None), (faint_blue, None));
        assert_eq!(bytes(mixed), Some((204, 0, 51, 159)));
    }

    #[test]
    fn two_colours_of_nothing_mix_to_nothing() {
        assert_eq!(
            mix_srgb((Rgba::TRANSPARENT, None), (Rgba::TRANSPARENT, None)),
            Some(Rgba::TRANSPARENT)
        );
    }

    #[test]
    fn shares_that_are_not_a_mix_are_refused() {
        for (first, second) in [
            (Some(0.0), Some(0.0)),
            (Some(1.5), None),
            (None, Some(-0.1)),
            (Some(f32::NAN), None),
            (Some(f32::INFINITY), Some(0.5)),
        ] {
            assert_eq!(
                mix_srgb((RED, first), (BLUE, second)),
                None,
                "{first:?} and {second:?}"
            );
        }
    }

    #[test]
    fn all_of_one_and_none_of_the_other_is_the_one() {
        assert_eq!(
            bytes(mix_srgb((RED, Some(1.0)), (BLUE, None))),
            Some((255, 0, 0, 255))
        );
        assert_eq!(
            bytes(mix_srgb((RED, Some(0.0)), (BLUE, None))),
            Some((0, 0, 255, 255))
        );
    }
}
