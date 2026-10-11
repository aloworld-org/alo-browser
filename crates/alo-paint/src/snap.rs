/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a box's picture lands on whole device pixels.
//!
//! Layout places a box at fractions of a pixel, and most of what this engine
//! draws is drawn there, its edges smoothed across the two pixels they fall
//! between. Chrome 154 draws some things on whole pixels instead, and a
//! checkbox laid out at y 22.77 drawn at 22.77 is a picture smeared across
//! two rows where Chrome's is crisp (queue item 457).
//!
//! # The rule, measured
//!
//! Measured from Chrome 154's screenshots at one device pixel to one, not
//! read from its source: a checkbox placed at every tenth of a pixel across
//! and down, and at sizes of 13.4, 13.5, 13.6 and 20.5.
//!
//! - **Each edge is rounded, not the size.** A checkbox 13.4 wide at x 10.3
//!   is drawn from 10 to 24, fourteen pixels: its right edge, at 23.7, is
//!   rounded on its own.
//! - **A half rounds up**, towards the right and the bottom, whatever the
//!   sign: 10.49 is drawn at 10, 10.5 at 11, and −5.5 at −5.
//!
//! Only the box is snapped. What a control draws inside it is placed in the
//! snapped box as it would be in any other: a checkbox 30 wide draws its
//! 13-pixel square from 8.5 pixels in, across a pixel's edge, in Chrome as
//! here.
//!
//! # One scale
//!
//! The canvas is drawn at one device pixel to each CSS pixel — a window that
//! has more replicates them (`alo-window`'s `place.rs`) — so a whole CSS
//! pixel is a whole device pixel here. When the canvas learns a scale, this
//! is where it rounds against it, as `alo-layout`'s `line_width.rs` already
//! says for a border's width.

use alo_layout::Rect;

/// The whole pixels `rect` is drawn on: each edge rounded to the nearest
/// whole pixel, a half towards the right and the bottom.
pub fn to_whole_pixels(rect: Rect) -> Rect {
    let left = round(rect.origin.x);
    let top = round(rect.origin.y);
    let right = round(rect.origin.x + rect.size.width);
    let bottom = round(rect.origin.y + rect.size.height);
    Rect::new(left, top, right - left, bottom - top)
}

/// The nearest whole number, a half rounded up whatever the sign — which
/// `f32::round`, rounding a half away from zero, does not do below zero.
fn round(value: f32) -> f32 {
    (value + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn edges(rect: Rect) -> (f32, f32, f32, f32) {
        (
            rect.origin.x,
            rect.origin.y,
            rect.origin.x + rect.size.width,
            rect.origin.y + rect.size.height,
        )
    }

    fn is(rect: Rect, wanted: (f32, f32, f32, f32)) -> bool {
        let got = edges(rect);
        close(got.0, wanted.0)
            && close(got.1, wanted.1)
            && close(got.2, wanted.2)
            && close(got.3, wanted.3)
    }

    #[test]
    fn a_box_moves_to_the_nearest_whole_pixel() {
        // Chrome: 10.49 is drawn at 10 and 10.5 at 11, across and down.
        let snapped = to_whole_pixels(Rect::new(10.49, 22.773_438, 13.0, 13.0));
        assert!(is(snapped, (10.0, 23.0, 23.0, 36.0)), "{snapped:?}");
        let snapped = to_whole_pixels(Rect::new(10.5, 10.4, 13.0, 13.0));
        assert!(is(snapped, (11.0, 10.0, 24.0, 23.0)), "{snapped:?}");
    }

    #[test]
    fn each_edge_is_rounded_on_its_own() {
        // Chrome draws 13.4 wide at 10.3 fourteen pixels wide, from 10 to
        // 24, and 20.5 at 130.3 twenty-one, from 130 to 151.
        let snapped = to_whole_pixels(Rect::new(10.3, 320.3, 13.4, 13.4));
        assert!(is(snapped, (10.0, 320.0, 24.0, 334.0)), "{snapped:?}");
        let snapped = to_whole_pixels(Rect::new(130.3, 320.3, 20.5, 20.5));
        assert!(is(snapped, (130.0, 320.0, 151.0, 341.0)), "{snapped:?}");
    }

    #[test]
    fn a_half_rounds_up_below_zero_too() {
        // Chrome draws a checkbox at −5.5 from −5, and at −6.5 from −6.
        let snapped = to_whole_pixels(Rect::new(-5.5, -6.5, 13.0, 13.0));
        assert!(is(snapped, (-5.0, -6.0, 8.0, 7.0)), "{snapped:?}");
    }

    #[test]
    fn a_box_already_on_whole_pixels_stays() {
        let snapped = to_whole_pixels(Rect::new(4.0, 3.0, 13.0, 13.0));
        assert!(is(snapped, (4.0, 3.0, 17.0, 16.0)), "{snapped:?}");
    }
}
