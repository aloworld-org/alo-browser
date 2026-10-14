/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How many device pixels one CSS pixel is in a window.
//!
//! ADR 0057 § 3: a page is laid out and painted for the scale of the screen
//! its window is on, and that scale is not rounded. § 2 says what the scale
//! changes — where each whole-pixel rule rounds — and that a rule is trusted
//! at a scale only once it has been measured there. So a window takes the
//! factor its screen reports when it is one this engine has measured, and
//! otherwise the largest measured one below it, as it once took the whole
//! part of every factor:
//!
//! - **every whole factor** from one to [`MOST_SCALE`], as the window has
//!   since queue item 562;
//! - **1.5**, measured by item 593 and taken by the window since item 599.
//!
//! A screen at 1.25 is shown at one and a screen at 1.75 at 1.5, with the
//! rest of the window as background, until somebody measures them.
//!
//! The window's own lengths — the strip's band, the lines it says things
//! in — are whole CSS pixels, and each is as many device pixels as a
//! renderer makes a canvas of that many CSS pixels: the length times the
//! scale, rounded half away from zero (`alo-renderer`'s `pipeline.rs`). So a
//! frame painted at a length fills the band laid out for it, at every scale.

/// The largest scale a window is laid out at.
///
/// No screen has a scale factor near it; the bound is there so that a window
/// server reporting nonsense costs a window of background rather than a
/// multiplication that overflows.
pub const MOST_SCALE: u32 = 8;

/// The most device pixels a window is counted as having on a side: 2^23,
/// so that it and one more are whole numbers an `f32` holds exactly.
pub const LARGEST_WINDOW: u32 = 1 << 23;

/// The scales between whole factors that have been measured (ADR 0057,
/// *Measured since*): a screen's factor is taken as one of these when it
/// reaches it.
const MEASURED_BETWEEN: [f32; 1] = [1.5];

/// A window's scale: one this engine has measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale(f32);

/// Every scale is a measured one, so none is a NaN, and equality is total.
impl Eq for Scale {}

impl Scale {
    /// One device pixel to the CSS pixel.
    pub const ONE: Self = Self(1.0);

    /// Three device pixels to two CSS pixels: a Windows or Linux screen at
    /// 150 per cent, or Chrome under `--force-device-scale-factor=1.5`.
    pub const ONE_AND_A_HALF: Self = Self(1.5);

    /// Two device pixels to the CSS pixel: a Mac's Retina screen.
    pub const TWO: Self = Self(2.0);

    /// `whole` device pixels to the CSS pixel, kept between one and
    /// [`MOST_SCALE`].
    pub fn whole(whole: u32) -> Self {
        let whole = whole.clamp(1, MOST_SCALE);
        // At most `MOST_SCALE`, so exact in an `f32` by way of a `u8`.
        Self(u8::try_from(whole).map_or(1.0, f32::from))
    }

    /// The scale a window on a screen whose window server reports
    /// `scale_factor` is laid out at: the largest measured scale the factor
    /// reaches.
    ///
    /// A factor that is not a finite number, or is below one, is one — the
    /// window is drawn at the size it says it is rather than not drawn.
    pub fn of_screen(scale_factor: f64) -> Self {
        if !scale_factor.is_finite() {
            return Self::ONE;
        }
        Self::measured()
            .filter(|scale| f64::from(scale.0) <= scale_factor)
            .last()
            .unwrap_or(Self::ONE)
    }

    /// The scale `factor` names exactly, if it is one this engine has
    /// measured: what a person may ask a window to be laid out at.
    pub fn named(factor: f64) -> Option<Self> {
        // Exactly: every measured scale is exact in either width.
        Self::measured().find(|scale| f64::from(scale.0).to_bits() == factor.to_bits())
    }

    /// Every measured scale, smallest first.
    fn measured() -> impl Iterator<Item = Self> {
        let mut scales: Vec<Self> = (1..=MOST_SCALE)
            .map(Self::whole)
            .chain(MEASURED_BETWEEN.into_iter().map(Self))
            .collect();
        scales.sort_by(|a, b| a.0.total_cmp(&b.0));
        scales.into_iter()
    }

    /// The factor, as a renderer is told it.
    pub fn factor(self) -> f32 {
        self.0
    }

    /// How many device pixels `css` whole CSS pixels are: as many as a
    /// renderer's canvas of that many is, the product rounded half away
    /// from zero.
    pub fn device(self, css: u32) -> u32 {
        let product = length(css) * self.0;
        // Rounded, and never past the largest window a `u32` names: the
        // cast is then exact.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rounded, non-negative product below 2^32 converts exactly"
        )]
        let rounded = product.round().clamp(0.0, 4_294_967_040.0) as u32;
        rounded
    }

    /// The most whole CSS pixels whose device pixels ([`Scale::device`])
    /// fit in `device` of them.
    ///
    /// At a whole scale that is `device` divided and rounded down. At 1.5
    /// an odd count of CSS pixels is a half device pixel more than it
    /// covers, rounded up: 1,333 CSS pixels fit 2,000 device pixels, and
    /// 1,334 would need 2,001.
    ///
    /// A window wider than [`LARGEST_WINDOW`] is counted as that wide: no
    /// screen is, and a length past it is not exact in an `f32`.
    pub fn fitting(self, device: u32) -> u32 {
        let device = device.min(LARGEST_WINDOW);
        let mut css = 0u32;
        // Counted up from a guess a little under the answer rather than
        // divided and cast, so the answer is the rule's exactly.
        let guess = f64::from(device) / f64::from(self.0) - 2.0;
        if guess > 0.0 {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a positive quotient of a u32 by at least one is within a u32"
            )]
            let floor = guess.floor() as u32;
            css = floor;
        }
        while self.device(css + 1) <= device {
            css += 1;
        }
        css
    }

    /// A position in device pixels, in CSS pixels.
    pub fn css(self, device: f64) -> f64 {
        device / f64::from(self.0)
    }
}

/// A count of pixels as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "a whole number below 2^24 is exact in an f32, and no window is that wide"
)]
fn length(pixels: u32) -> f32 {
    pixels as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factor(scale: Scale) -> u32 {
        scale.factor().to_bits()
    }

    #[test]
    fn a_whole_scale_factor_is_kept_whole() {
        assert_eq!(factor(Scale::of_screen(1.0)), 1.0_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(2.0)), 2.0_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(3.0)), 3.0_f32.to_bits());
    }

    #[test]
    fn a_screen_at_one_and_a_half_is_laid_out_at_its_own_scale() {
        assert_eq!(factor(Scale::of_screen(1.5)), 1.5_f32.to_bits());
    }

    #[test]
    fn a_factor_not_measured_is_the_largest_measured_below_it() {
        assert_eq!(factor(Scale::of_screen(1.25)), 1.0_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(1.499_999)), 1.0_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(1.75)), 1.5_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(1.999)), 1.5_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(2.5)), 2.0_f32.to_bits());
        assert_eq!(factor(Scale::of_screen(2.999)), 2.0_f32.to_bits());
    }

    #[test]
    fn a_scale_factor_that_is_nonsense_is_one_or_the_most() {
        for nonsense in [0.0, 0.5, -2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(factor(Scale::of_screen(nonsense)), 1.0_f32.to_bits());
        }
        assert_eq!(factor(Scale::of_screen(1e300)), 8.0_f32.to_bits());
        assert_eq!(factor(Scale::whole(0)), 1.0_f32.to_bits());
        assert_eq!(factor(Scale::whole(u32::MAX)), 8.0_f32.to_bits());
    }

    #[test]
    fn a_person_names_only_a_measured_scale() {
        assert_eq!(Scale::named(1.5), Some(Scale::ONE_AND_A_HALF));
        assert_eq!(Scale::named(2.0), Some(Scale::TWO));
        assert_eq!(Scale::named(8.0), Some(Scale::whole(8)));
        for unmeasured in [1.25, 1.75, 0.5, 0.0, 9.0, f64::NAN, f64::INFINITY] {
            assert_eq!(Scale::named(unmeasured), None, "{unmeasured}");
        }
    }

    #[test]
    fn a_length_is_as_many_device_pixels_as_a_renderers_canvas() {
        assert_eq!(Scale::ONE.device(36), 36);
        assert_eq!(Scale::TWO.device(36), 72);
        assert_eq!(Scale::ONE_AND_A_HALF.device(36), 54);
        assert_eq!(Scale::ONE_AND_A_HALF.device(32), 48);
        // A half device pixel is rounded away from zero, as `pipeline.rs`'
        // `f32::round` rounds a canvas.
        assert_eq!(Scale::ONE_AND_A_HALF.device(1), 2);
        assert_eq!(Scale::ONE_AND_A_HALF.device(1333), 2000);
        assert_eq!(Scale::ONE_AND_A_HALF.device(0), 0);
        assert_eq!(Scale::whole(8).device(u32::MAX), 4_294_967_040);
    }

    #[test]
    fn the_most_css_pixels_that_fit_are_never_more_than_the_device_pixels() {
        assert_eq!(Scale::ONE.fitting(480), 480);
        assert_eq!(Scale::TWO.fitting(960), 480);
        assert_eq!(Scale::TWO.fitting(961), 480);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(2000), 1333);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(1999), 1332);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(720), 480);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(1), 0);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(2), 1);
        assert_eq!(Scale::ONE_AND_A_HALF.fitting(0), 0);
        for scale in [
            Scale::ONE,
            Scale::ONE_AND_A_HALF,
            Scale::TWO,
            Scale::whole(3),
        ] {
            for device in 0..400 {
                let css = scale.fitting(device);
                assert!(scale.device(css) <= device, "{scale:?} {device}");
                assert!(scale.device(css + 1) > device, "{scale:?} {device}");
            }
        }
        // A window no screen has is counted as the largest there could be,
        // and answered at once.
        assert_eq!(Scale::ONE.fitting(u32::MAX), LARGEST_WINDOW);
        assert_eq!(Scale::TWO.fitting(u32::MAX), LARGEST_WINDOW / 2);
        assert!(Scale::ONE_AND_A_HALF.fitting(u32::MAX) <= LARGEST_WINDOW / 3 * 2 + 1);
    }

    #[test]
    fn a_point_in_device_pixels_is_divided_by_the_scale() {
        assert!((Scale::ONE_AND_A_HALF.css(54.0) - 36.0).abs() < 1e-12);
        assert!((Scale::TWO.css(141.2) - 70.6).abs() < 1e-12);
    }
}
