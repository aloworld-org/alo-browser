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
//! # Every box's own area
//!
//! The same rule holds for what any box paints for itself (queue item 565),
//! measured from Chrome 154's screenshots of 1,536 boxes at tenths of a
//! pixel, each at one and at two (ADR 0057, *Measured since*): a background colour or image, a border on both
//! its edges, a rounded corner, an inset or spread or blurred shadow, a
//! piece of an inline box, a box inside a box, a faded box. A shadow moved
//! by an offset is the snapped box moved, so an offset of a fraction leaves
//! its edges a fraction into a pixel; a box under a transform is snapped
//! where it would be without one, and the transform moves the snapped box.
//! A fieldset's border is snapped from the line through its legend, not
//! from the top of its box (`crate::banded`). A box that clips its content
//! clips it to the same snapped border box, less its border, so the clip
//! is where its background is drawn (queue item 575). A picture is drawn
//! into its content box with each edge rounded here on its own — the
//! content box itself, not the snapped border box less a padding of
//! fractions (queue item 587). An `<svg>` is clipped to its content box
//! rounded the same way, and drawn from its border box's corner rounded,
//! plus its border and padding as laid out (queue item 589).
//!
//! # At a scale
//!
//! The pixels are **device** pixels (ADR 0057 § 2). On a screen of `s`
//! device pixels to the CSS pixel each edge is rounded as `floor(x·s + 0.5)
//! / s`, and the box stays in CSS pixels: measured from Chrome 154 started
//! with `--force-device-scale-factor=2`, a checkbox laid out at (10.3,
//! 22.77) is drawn from device pixel (21, 46), which is (10.5, 23), and
//! `cases/checkbox-beside-text`'s nine controls are each drawn from
//! `floor(x·2 + 0.5)` (queue item 561). At a scale of one this is the rule
//! above, unchanged.

use alo_layout::Rect;

/// The whole pixels `rect` is drawn on: each edge rounded to the nearest
/// whole pixel, a half towards the right and the bottom.
pub fn to_whole_pixels(rect: Rect) -> Rect {
    to_device_pixels(rect, 1.0)
}

/// The whole device pixels `rect` is drawn on, on a screen of `scale`
/// device pixels to the CSS pixel, in CSS pixels: each edge rounded as
/// `floor(x·s + 0.5) / s`. A scale that is not a scale is one.
pub fn to_device_pixels(rect: Rect, scale: f32) -> Rect {
    let scale = alo_value::device_scale(scale);
    let left = round(rect.origin.x, scale);
    let top = round(rect.origin.y, scale);
    let right = round(rect.origin.x + rect.size.width, scale);
    let bottom = round(rect.origin.y + rect.size.height, scale);
    Rect::new(left, top, right - left, bottom - top)
}

/// The nearest whole device pixel, a half rounded up whatever the sign —
/// which `f32::round`, rounding a half away from zero, does not do below
/// zero.
fn round(value: f32, scale: f32) -> f32 {
    (value * scale + 0.5).floor() / scale
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

    #[test]
    fn at_two_each_edge_is_rounded_to_a_device_pixel() {
        // Chrome 154 at a real scale of two (ADR 0057, Evidence 6): a
        // checkbox at (10.3, 22.77) is drawn from device pixel (21, 46),
        // and a box 13.4 square at 200.2 covers device pixels 400 to 427.
        let snapped = to_device_pixels(Rect::new(10.3, 22.77, 13.0, 13.0), 2.0);
        assert!(is(snapped, (10.5, 23.0, 23.5, 36.0)), "{snapped:?}");
        let snapped = to_device_pixels(Rect::new(200.2, 22.2, 13.4, 13.4), 2.0);
        assert!(is(snapped, (200.0, 22.0, 213.5, 35.5)), "{snapped:?}");
        // `checkbox-beside-text`'s first control, at 23.9609375 at two.
        let snapped = to_device_pixels(Rect::new(23.960_938, 63.5, 13.0, 13.0), 2.0);
        assert!(is(snapped, (24.0, 63.5, 37.0, 76.5)), "{snapped:?}");
    }

    #[test]
    fn at_one_it_is_the_whole_pixel_rule() {
        let rect = Rect::new(10.49, -5.5, 13.4, 20.5);
        let one = to_device_pixels(rect, 1.0);
        let whole = to_whole_pixels(rect);
        assert!(is(one, edges(whole)), "{one:?} {whole:?}");
    }

    #[test]
    fn what_is_not_a_scale_is_one() {
        let rect = Rect::new(10.3, 22.77, 13.4, 13.4);
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            let snapped = to_device_pixels(rect, scale);
            assert!(
                is(snapped, (10.0, 23.0, 24.0, 36.0)),
                "{scale}: {snapped:?}"
            );
        }
    }
}
