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
//! there, and a `Resize` carries a whole `Screen`, so a window moved to a
//! screen of another scale has its page laid out again at it (562).
//!
//! # What it is drawn on
//!
//! A page is drawn on white: a page with no background of its own is a
//! white page. One document of the browser's own is drawn on nothing
//! instead — the chats panel's rail, whose hovered chat's name stands past
//! the rail's edge over the first screen (queue item 760) — so the window
//! composes what lies beyond the rail's own background as it is
//! ([`Screen::on_nothing`]). Nobody's page is ever drawn so.

use alo_layout::Size;

/// A window's size and its screen's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// How big the window is, in CSS pixels.
    pub size: Size,
    /// Device pixels per CSS pixel: any finite factor above zero, and 1 for
    /// anything else.
    scale: f32,
    /// Whether it is drawn on nothing rather than on white.
    clear: bool,
}

impl Screen {
    /// A window of this size on a screen of one device pixel to the CSS
    /// pixel, which is every render's until a window says otherwise.
    pub fn new(size: Size) -> Self {
        Self {
            size,
            scale: 1.0,
            clear: false,
        }
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

    /// The same window, drawn on nothing rather than on white when `clear`:
    /// what no background covers is left transparent.
    #[must_use]
    pub fn on_nothing(self, clear: bool) -> Self {
        Self { clear, ..self }
    }

    /// What it is drawn on: white, or nothing.
    pub fn ground(self) -> alo_value::Rgba {
        if self.clear {
            alo_value::Rgba::TRANSPARENT
        } else {
            alo_value::Rgba::WHITE
        }
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
    fn a_screen_is_drawn_on_white_unless_it_is_drawn_on_nothing() {
        let screen = Screen::new(Size::new(64.0, 600.0));
        assert_eq!(screen.ground(), alo_value::Rgba::WHITE);
        assert_eq!(
            screen.at_scale(2.0).on_nothing(true).ground(),
            alo_value::Rgba::TRANSPARENT
        );
        assert_eq!(
            screen.on_nothing(true).at_scale(2.0).scale().to_bits(),
            2.0_f32.to_bits()
        );
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
