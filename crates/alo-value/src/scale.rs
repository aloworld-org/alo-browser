/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A screen's scale, and a number of its device pixels (ADR 0057).
//!
//! Layout's numbers are CSS pixels, and every whole-pixel rule rounds
//! `v·s` and divides back (ADR 0057 § 2). At a scale that is a power of two
//! — one, two, a half — a float holds every grid line of the screen
//! exactly, 1/64 of a device pixel included, so a number a float makes of a
//! sum of them lands on a line or is truly off it. **At any other scale it
//! does not**: at 1.5 a device pixel is 0.666… CSS pixels, which no float
//! holds, and a line height of 20 less a content area of 22 and 6 device
//! pixels is 1.3333334, whose half is 0.99999994 device pixels where Chrome,
//! counting in whole sixty-fourths of a device pixel, has exactly 1 (queue
//! item 593). So at such a scale a number within a float's error of a
//! sixty-fourth of a device pixel — a whole pixel, a half — is on it.

/// A screen's scale as ADR 0057 § 3 admits one: any finite factor above
/// zero, and 1 for anything else.
pub fn device_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Whether a float holds every sixty-fourth of a device pixel of a screen
/// at `scale` exactly, in CSS pixels: whether the scale is a power of two.
pub fn holds_its_grid(scale: f32) -> bool {
    let units = 64.0 * device_scale(scale);
    units.to_bits().trailing_zeros() >= 23
}

/// How many grid lines a device pixel holds: layout's unit is a
/// sixty-fourth of one (ADR 0054, ADR 0057 § 2).
const LINES_PER_PX: f32 = 64.0;

/// How near a grid line a number of sixty-fourths is taken to be on it, at
/// a scale whose grid a float cannot hold: a sixteenth of one.
///
/// Chrome's numbers there are whole sixty-fourths of a device pixel, so no
/// number it means is nearer a line than a sixty-fourth without being on
/// it; a float's error in a sum of CSS pixels a page could hold is a
/// hundredth of that.
const SLACK: f32 = 1.0 / 16.0;

/// `pixels` CSS pixels as device pixels of a screen at `scale`, moved onto
/// the sixty-fourth of a device pixel they are within a float's error of,
/// where the scale is not a power of two (see the module).
///
/// At 1.5, `1.3333334` CSS pixels are 2 device pixels and `1.6666666` are
/// 2.5; at one and at two a number is only multiplied, so nothing measured
/// there moves.
pub fn device_pixels(pixels: f32, scale: f32) -> f32 {
    let scale = device_scale(scale);
    let device = pixels * scale;
    if holds_its_grid(scale) {
        return device;
    }
    let lines = device * LINES_PER_PX;
    let nearest = lines.round();
    if (lines - nearest).abs() <= SLACK {
        nearest / LINES_PER_PX
    } else {
        device
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_is_a_finite_factor_above_zero() {
        for (given, scale) in [
            (2.0_f32, 2.0_f32),
            (1.5, 1.5),
            (0.0, 1.0),
            (-2.0, 1.0),
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
        ] {
            assert_eq!(
                device_scale(given).to_bits(),
                f32::to_bits(scale),
                "{given}"
            );
        }
    }

    #[test]
    fn a_float_holds_the_grid_only_at_a_power_of_two() {
        for scale in [1.0, 2.0, 4.0, 0.5, 0.25] {
            assert!(holds_its_grid(scale), "{scale}");
        }
        for scale in [1.5, 1.25, 3.0, 1.1, 2.5] {
            assert!(!holds_its_grid(scale), "{scale}");
        }
    }

    #[test]
    fn a_float_short_of_a_pixel_at_one_and_a_half_is_on_it() {
        // A 20 px line less 22 and 6 device pixels of font, halved.
        let leading = 20.0_f32 - (22.0 / 1.5 + 6.0 / 1.5);
        let half = leading / 2.0 * 1.5;
        assert!(half < 1.0, "the float this guards against: {half}");
        assert_eq!(
            device_pixels(leading / 2.0, 1.5).to_bits(),
            1.0_f32.to_bits()
        );
        // A half, which a snapped edge rounds up from.
        assert_eq!(device_pixels(2.5 / 1.5, 1.5).to_bits(), 2.5_f32.to_bits());
        // A hundred-and-twenty-eighth short of a pixel is short of it.
        let short = device_pixels(127.0 / 128.0 / 1.5, 1.5);
        assert!((short - 127.0 / 128.0).abs() < 1e-6, "{short}");
    }

    #[test]
    fn at_a_power_of_two_a_number_is_only_multiplied() {
        let near = 1.0 - 1.0 / 4096.0;
        assert_eq!(device_pixels(near, 1.0).to_bits(), near.to_bits());
        assert_eq!(device_pixels(near / 2.0, 2.0).to_bits(), near.to_bits());
    }
}
