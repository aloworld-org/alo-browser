/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How wide a border is once it is a number: whole device pixels.
//!
//! CSS Values 4 *snap as a border width*: a computed border width of at least
//! one device pixel is floored to a whole number of them, and a smaller one
//! that is not zero is one. So `border-left: .3rem` is 4 wide, not 4.8, and
//! everything inside it starts 4 in — which is what Chrome lays out and draws,
//! and what alo Sites' editorial sections are measured against.
//!
//! **A device pixel is the screen's** (ADR 0057 § 2): at a scale of two the
//! same `.3rem` snaps to 4.5 CSS pixels, as Chrome 154 started with
//! `--force-device-scale-factor=2` snaps it, and a width under a device pixel
//! is half a CSS pixel there.

/// A border width in CSS pixels, snapped as a border width on a screen of
/// `scale` device pixels to the CSS pixel.
///
/// Zero stays zero, and so does anything that is not a positive number: a
/// negative width is not valid CSS, and snapping one up to a pixel would draw
/// a border nobody declared.
pub(crate) fn snap(width: f32, scale: f32) -> f32 {
    if width.is_nan() || width <= 0.0 {
        return width;
    }
    let scale = alo_value::device_scale(scale);
    let device = width * scale;
    let snapped = if device < 1.0 { 1.0 } else { device.floor() };
    snapped / scale
}

#[cfg(test)]
mod tests {
    use super::snap as snapped;

    /// Snapped at a scale of one.
    fn snap(width: f32) -> f32 {
        snapped(width, 1.0)
    }

    /// Equal to within far less than a pixel.
    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn a_width_of_a_pixel_or_more_is_floored_to_whole_pixels() {
        assert!(close(snap(4.8), 4.0));
        assert!(close(snap(5.6), 5.0));
        assert!(close(snap(1.0), 1.0));
        assert!(close(snap(1.999), 1.0));
    }

    #[test]
    fn a_whole_width_is_left_as_it_is() {
        assert!(close(snap(2.0), 2.0));
        assert!(close(snap(16.0), 16.0));
    }

    #[test]
    fn a_width_under_a_pixel_is_one() {
        assert!(close(snap(0.5), 1.0));
        assert!(close(snap(0.01), 1.0));
    }

    #[test]
    fn zero_and_what_is_not_a_width_are_not_drawn_up_to_a_pixel() {
        assert!(close(snap(0.0), 0.0));
        assert!(close(snap(-3.5), -3.5));
        assert!(snap(f32::NAN).is_nan());
        assert!(snap(f32::INFINITY).is_infinite());
    }

    #[test]
    fn at_a_scale_of_two_a_width_is_floored_to_half_pixels() {
        // ADR 0057, Evidence 5: each as Chrome 154 computes it under
        // `--force-device-scale-factor=2`.
        for (width, at_two) in [(4.8, 4.5), (0.3, 0.5), (0.7, 0.5), (1.6, 1.5), (2.74, 2.5)] {
            assert!(close(snapped(width, 2.0), at_two), "{width}");
        }
        assert!(close(snapped(1.3, 2.0), 1.0));
        assert!(close(snapped(1.3, 1.0), 1.0));
    }

    #[test]
    fn what_is_not_a_scale_is_one() {
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert!(close(snapped(4.8, scale), 4.0), "{scale}");
        }
    }
}
