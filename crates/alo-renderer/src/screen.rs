/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page is laid out for: a window's size in CSS pixels, and the scale
//! of the screen it is shown on (ADR 0057 § 3).
//!
//! The scale is one number and travels with the size, because the two reach
//! a page together: the cascade is told both ([`alo_css::MediaContext`]),
//! and every whole-pixel rule layout holds rounds against the scale (§ 2).
//! A page laid out at two is reported in CSS pixels like any other; only
//! where its numbers were rounded differs.
//!
//! Paint draws at the scale too, onto a canvas of device pixels (queue item
//! 561). A renderer is told its page's scale with the page
//! ([`crate::Page::scale`], queue item 563) and lays it out and draws it
//! there; the window tells it one until it learns its screen's (562).

use alo_layout::Size;

/// A window's size and its screen's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// How big the window is, in CSS pixels.
    pub size: Size,
    /// Device pixels per CSS pixel: any finite factor above zero, and 1 for
    /// anything else.
    scale: f32,
}

impl Screen {
    /// A window of this size on a screen of one device pixel to the CSS
    /// pixel, which is every render's until a window says otherwise.
    pub fn new(size: Size) -> Self {
        Self { size, scale: 1.0 }
    }

    /// The same window on a screen of `scale` device pixels to the CSS
    /// pixel. Not rounded: a screen at 1.5 is laid out at 1.5.
    #[must_use]
    pub fn at_scale(self, scale: f32) -> Self {
        Self {
            scale: alo_value::device_scale(scale),
            ..self
        }
    }

    /// Device pixels per CSS pixel.
    pub fn scale(self) -> f32 {
        self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_is_at_one_until_somebody_says_otherwise() {
        let screen = Screen::new(Size::new(800.0, 600.0));
        assert_eq!(screen.scale().to_bits(), 1.0_f32.to_bits());
        assert_eq!(screen.at_scale(2.0).scale().to_bits(), 2.0_f32.to_bits());
        assert_eq!(screen.at_scale(1.5).scale().to_bits(), 1.5_f32.to_bits());
    }

    #[test]
    fn what_is_not_a_scale_is_one() {
        let screen = Screen::new(Size::new(800.0, 600.0));
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                screen.at_scale(scale).scale().to_bits(),
                1.0_f32.to_bits(),
                "{scale}"
            );
        }
    }
}
