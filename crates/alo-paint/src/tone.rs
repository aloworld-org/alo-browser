/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The colours of the borders that are two tones of one: `inset`, `outset`,
//! `groove` and `ridge`.
//!
//! Each is one colour drawn as two tones, a darker and a lighter, and which
//! side gets which is the whole of what makes it look raised or sunk. The
//! light is taken to come from the top left, the convention bevelled
//! interfaces have long used:
//!
//! - **`inset`** is sunk into the page: its top and left are in shadow (the
//!   darker tone), its bottom and right catch the light (the lighter one).
//! - **`outset`** is raised: the other way round.
//! - **`groove`** is a channel cut into the page: its **outer half** is drawn
//!   as `inset` and its **inner half** as `outset`.
//! - **`ridge`** is a groove turned inside out: outer half `outset`, inner
//!   half `inset`.
//!
//! So no two of the four are the same picture, and a test can say so.
//!
//! # The two tones
//!
//! CSS leaves the exact colours to the browser. Ours are Chrome 154's,
//! measured from its screenshots (queue item 508) over every grey, 650
//! colours of every hue and colours chosen at the edges of its rule, with
//! not one channel of one side off. Its source was not read.
//!
//! - **The darker tone** takes 0.33 off the brightest channel and scales the
//!   others with it, so a blue border goes dark blue rather than grey; **the
//!   lighter** adds 0.33, up to 1. Each channel is then truncated to a byte
//!   by 255.99998, just under 256, so that 1 is 255 and a channel that lands
//!   exactly on a byte's edge is the byte below it.
//! - **A colour too dark to darken**, one no brighter to an eye than
//!   `rgb(32, 32, 32)` (its relative luminance, as WCAG measures it), is
//!   drawn lighter instead: its darker tone is the colour lightened, and its
//!   lighter tone that lightened again. So `outset` black is 168 on its top
//!   and left and 84 on its bottom and right, not black and a grey.
//! - **A colour too light to lighten**, brighter than `rgb(235, 235, 235)`,
//!   is its own lighter tone.
//!
//! Alpha is kept, and the tones are made of the other three channels alone.
//! The two tones are **never the same colour**, for any colour: a bevel
//! whose halves came out equal would be a solid border nobody asked for.

use crate::border::{DrawnSide, Line};
use crate::mitre::Side;
use alo_value::Rgba;

/// How far either tone moves the brightest channel, in a channel's zero to
/// one.
const SHIFT: f32 = 0.33;

/// What a channel is multiplied by to be truncated to a byte: the float
/// below 256, so that 1 is 255.
const TO_BYTE: f32 = 256.0 - 1.0 / 65_536.0;

/// The grey a colour must be brighter than to be darkened at all.
const DARKEST_DARKENED: u8 = 32;

/// The grey a colour must be no brighter than to be lightened at all.
const LIGHTEST_LIGHTENED: u8 = 235;

/// The darker and the lighter tone of a colour, in that order.
pub fn tones(color: Rgba) -> (Rgba, Rgba) {
    let luminance = relative_luminance(color);
    if luminance <= relative_luminance(grey(DARKEST_DARKENED)) {
        let lighter = lightened(color);
        return (lighter, lightened(lighter));
    }
    if luminance > relative_luminance(grey(LIGHTEST_LIGHTENED)) {
        return (darkened(color), color);
    }
    (darkened(color), lightened(color))
}

/// The colour with 0.33 added to its brightest channel, up to 1, and the
/// others scaled with it. Black, with no channel to scale, is Chrome's
/// grey of 84.
fn lightened(color: Rgba) -> Rgba {
    let brightest = brightest(color);
    if brightest <= 0.0 {
        let level = 84.0 / 255.0;
        return Rgba::new(level, level, level, color.alpha);
    }
    scaled(color, (brightest + SHIFT).min(1.0) / brightest)
}

/// The colour with 0.33 taken off its brightest channel, down to 0, and the
/// others scaled with it.
fn darkened(color: Rgba) -> Rgba {
    let brightest = brightest(color);
    if brightest <= 0.0 {
        return color;
    }
    scaled(color, (brightest - SHIFT).max(0.0) / brightest)
}

fn brightest(color: Rgba) -> f32 {
    color.red.max(color.green).max(color.blue)
}

/// Each channel multiplied and truncated to a byte; alpha as it was.
fn scaled(color: Rgba, by: f32) -> Rgba {
    Rgba::new(
        truncated(color.red, by),
        truncated(color.green, by),
        truncated(color.blue, by),
        color.alpha,
    )
}

fn truncated(channel: f32, by: f32) -> f32 {
    let byte = (channel * by * TO_BYTE).clamp(0.0, 255.0).trunc();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 and truncated, so it is a whole byte"
    )]
    let byte = byte as u8;
    f32::from(byte) / 255.0
}

/// How bright a colour is to an eye: WCAG's relative luminance, from the
/// channels made linear again.
fn relative_luminance(color: Rgba) -> f64 {
    0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/// An sRGB channel made linear.
fn linear(channel: f32) -> f64 {
    let channel = f64::from(channel);
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn grey(level: u8) -> Rgba {
    Rgba::from_rgba8(level, level, level, 255)
}

/// The colours one side is drawn in: its outer half, then its inner half.
///
/// The same colour twice for every line but `groove` and `ridge`, and the
/// border's own colour for the lines that are not toned at all.
pub fn colors_of(drawn: DrawnSide) -> (Rgba, Rgba) {
    let (darker, lighter) = tones(drawn.color);
    let (sunk, raised) = if faces_the_light(drawn.side) {
        (darker, lighter)
    } else {
        (lighter, darker)
    };
    match drawn.line {
        Line::Solid | Line::Dashed | Line::Dotted | Line::Double => (drawn.color, drawn.color),
        Line::Inset => (sunk, sunk),
        Line::Outset => (raised, raised),
        Line::Groove => (sunk, raised),
        Line::Ridge => (raised, sunk),
    }
}

/// Whether a side faces the light, which comes from the top left.
fn faces_the_light(side: Side) -> bool {
    matches!(side, Side::Top | Side::Left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mitre::SIDES;

    const LINES: [Line; 4] = [Line::Inset, Line::Outset, Line::Groove, Line::Ridge];

    /// A colour's two tones, as bytes.
    fn bytes(red: u8, green: u8, blue: u8) -> ((u8, u8, u8), (u8, u8, u8)) {
        let (darker, lighter) = tones(Rgba::from_rgba8(red, green, blue, 255));
        let (dr, dg, db, _) = darker.to_rgba8();
        let (lr, lg, lb, _) = lighter.to_rgba8();
        ((dr, dg, db), (lr, lg, lb))
    }

    #[test]
    fn the_two_tones_are_never_the_same_colour() {
        for level in 0..=255_u8 {
            let (darker, lighter) = tones(grey(level));
            assert_ne!(darker, lighter, "grey {level}");
        }
        for color in [
            Rgba::from_rgba8(255, 0, 0, 255),
            Rgba::from_rgba8(0, 0, 255, 255),
            Rgba::from_rgba8(1, 0, 0, 255),
            Rgba::from_rgba8(0, 128, 255, 128),
        ] {
            let (darker, lighter) = tones(color);
            assert_ne!(darker, lighter, "{color:?}");
        }
    }

    #[test]
    fn the_tones_are_chromes_and_keep_the_hue() {
        // Each measured on Chrome 154's screenshot of an `outset` border:
        // the bottom side, then the top.
        assert_eq!(bytes(128, 128, 128), ((44, 44, 44), (212, 212, 212)));
        assert_eq!(bytes(153, 153, 153), ((69, 69, 69), (238, 238, 238)));
        assert_eq!(bytes(118, 118, 118), ((33, 33, 33), (202, 202, 202)));
        assert_eq!(bytes(192, 192, 192), ((108, 108, 108), (255, 255, 255)));
        // A dark grey that is still darkened goes all the way to black.
        assert_eq!(bytes(33, 33, 33), ((0, 0, 0), (117, 117, 117)));
        // The hue is kept: the channels keep their proportions.
        assert_eq!(bytes(128, 0, 0), ((44, 0, 0), (212, 0, 0)));
        assert_eq!(bytes(51, 133, 187), ((28, 73, 103), (69, 182, 255)));
        assert_eq!(bytes(91, 85, 54), ((6, 6, 4), (175, 164, 104)));
        assert_eq!(bytes(0, 64, 0), ((0, 0, 0), (0, 148, 0)));
    }

    #[test]
    fn a_channel_on_a_bytes_edge_is_the_byte_below() {
        // 135 of 255 lightened by 255 / 180 is exactly three quarters, 192
        // of 256, and Chrome's is 191.
        assert_eq!(bytes(24, 180, 135), ((12, 96, 72), (34, 255, 191)));
        assert_eq!(bytes(168, 224, 216), ((105, 140, 135), (191, 255, 246)));
    }

    #[test]
    fn a_colour_too_dark_to_darken_is_drawn_lighter() {
        // Black's `outset` is 168 on its top and left and 84 below and
        // right, not a grey and black.
        assert_eq!(bytes(0, 0, 0), ((84, 84, 84), (168, 168, 168)));
        assert_eq!(bytes(32, 32, 32), ((116, 116, 116), (200, 200, 200)));
        // Brightness is luminance, not the brightest channel: a red of 40
        // is darker to an eye than a grey of 32.
        assert_eq!(bytes(40, 0, 0), ((124, 0, 0), (208, 0, 0)));
        assert_eq!(bytes(16, 16, 32), ((58, 58, 116), (100, 100, 200)));
    }

    #[test]
    fn a_colour_too_light_to_lighten_is_its_own_lighter_tone() {
        assert_eq!(bytes(235, 235, 235), ((151, 151, 151), (255, 255, 255)));
        assert_eq!(bytes(236, 236, 236), ((152, 152, 152), (236, 236, 236)));
        assert_eq!(bytes(255, 255, 255), ((171, 171, 171), (255, 255, 255)));
        assert_eq!(bytes(214, 253, 35), ((143, 169, 23), (214, 253, 35)));
    }

    #[test]
    fn a_translucent_border_keeps_its_alpha() {
        let (darker, lighter) = tones(Rgba::from_rgba8(100, 150, 200, 64));
        assert_eq!(darker.to_rgba8().3, 64);
        assert_eq!(lighter.to_rgba8().3, 64);
    }

    #[test]
    fn inset_is_dark_at_the_top_left_and_outset_the_other_way() {
        let color = grey(128);
        let (darker, lighter) = tones(color);
        let of = |line, side| colors_of(DrawnSide { side, line, color });
        for side in [Side::Top, Side::Left] {
            assert_eq!(of(Line::Inset, side), (darker, darker));
            assert_eq!(of(Line::Outset, side), (lighter, lighter));
            assert_eq!(of(Line::Groove, side), (darker, lighter));
            assert_eq!(of(Line::Ridge, side), (lighter, darker));
        }
        for side in [Side::Bottom, Side::Right] {
            assert_eq!(of(Line::Inset, side), (lighter, lighter));
            assert_eq!(of(Line::Outset, side), (darker, darker));
            assert_eq!(of(Line::Groove, side), (lighter, darker));
            assert_eq!(of(Line::Ridge, side), (darker, lighter));
        }
        assert_eq!(of(Line::Solid, Side::Top), (color, color));
    }

    #[test]
    fn no_two_lines_are_drawn_alike() {
        let color = grey(128);
        let picture = |line| SIDES.map(|side| colors_of(DrawnSide { side, line, color }));
        for (index, first) in LINES.iter().enumerate() {
            for second in LINES.iter().skip(index + 1) {
                assert_ne!(picture(*first), picture(*second), "{first:?} {second:?}");
            }
        }
    }
}
