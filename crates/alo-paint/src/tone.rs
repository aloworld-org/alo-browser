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
//! CSS leaves the exact colours to the browser. Ours, in [`tones`]: the
//! darker tone takes a third off the brightest channel and scales the others
//! with it, so a blue border goes dark blue rather than grey; the lighter tone
//! adds a third, up to white. Black has no hue to keep, and lightens to a grey
//! a third of the way to white. The two tones are **never the same colour**,
//! for any colour — a bevel whose halves came out equal would be a solid
//! border nobody asked for.

use crate::border::{DrawnSide, Line};
use crate::mitre::Side;
use alo_value::Rgba;

/// How far either tone moves from the colour it is made of, as a share of the
/// whole range of a channel.
const SHIFT: f32 = 1.0 / 3.0;

/// The darker and the lighter tone of a colour, in that order.
///
/// Alpha is kept: a translucent border's bevel is as translucent as it is.
pub fn tones(color: Rgba) -> (Rgba, Rgba) {
    let brightest = color.red.max(color.green).max(color.blue);
    if brightest <= 0.0 {
        return (color, Rgba::new(SHIFT, SHIFT, SHIFT, color.alpha));
    }
    let darker = (brightest - SHIFT).max(0.0) / brightest;
    let lighter = (brightest + SHIFT).min(1.0) / brightest;
    (scaled(color, darker), scaled(color, lighter))
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

fn scaled(color: Rgba, by: f32) -> Rgba {
    Rgba::new(
        color.red * by,
        color.green * by,
        color.blue * by,
        color.alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mitre::SIDES;

    const LINES: [Line; 4] = [Line::Inset, Line::Outset, Line::Groove, Line::Ridge];
    fn grey(level: u8) -> Rgba {
        Rgba::from_rgba8(level, level, level, 255)
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
    fn the_tones_are_a_third_either_way_and_keep_the_hue() {
        let (darker, lighter) = tones(grey(153)); // 0.6
        assert_eq!(darker.to_rgba8(), (68, 68, 68, 255));
        assert_eq!(lighter.to_rgba8(), (238, 238, 238, 255));

        // Blue stays blue: the channels keep their proportions.
        let (darker, lighter) = tones(Rgba::from_rgba8(0, 51, 204, 255));
        assert_eq!(darker.to_rgba8(), (0, 30, 119, 255));
        assert_eq!(lighter.to_rgba8(), (0, 64, 255, 255));
    }

    #[test]
    fn black_lightens_to_a_grey_and_white_darkens_to_one() {
        assert_eq!(
            tones(Rgba::BLACK),
            (Rgba::BLACK, Rgba::new(SHIFT, SHIFT, SHIFT, 1.0))
        );
        let (darker, lighter) = tones(Rgba::WHITE);
        assert_eq!(darker.to_rgba8(), (170, 170, 170, 255));
        assert_eq!(lighter, Rgba::WHITE);
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
