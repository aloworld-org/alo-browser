/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's bar pressed and dragged (queue item 845,
//! `docs/design/shell.md`, *The conversation*): where on the first screen a
//! press is the bar's, and how far a drag of it scrolls.
//!
//! # Decided here
//!
//! - **A target wider than the bar**: the bar is 6 across, too thin to hit,
//!   so a press is its anywhere in a strip [`TARGET`] — 16 CSS pixels —
//!   wide centred on it, between the room's top and its foot, as macOS's
//!   overlay scroller widens under the pointer. The pointer there has
//!   already brought the bar back whole (queue item 793), so nothing faded
//!   is ever pressed.
//! - **On the thumb, a drag**: the conversation follows the pointer as far
//!   as the thumb moves — the pointer's travel down the room times the
//!   conversation's travel over the thumb's — from where it was pressed,
//!   kept inside how far the turns reach.
//! - **Above or below it, a page**: back or on by the room less a line, as
//!   Page Up and Page Down are (queue item 754) and as a Mac's scroll bar
//!   does with *Jump to the next page*, its default. Once a press; a press
//!   held is not repeated (queue item 876).
//! - A press on the bar is the bar's alone: never the renderer's to claim,
//!   and the keyboard stays where it was, as a Mac's scroll bar leaves it.

use alo_layout::Size;
use alo_renderer::card_place::TOP;
use alo_renderer::conversation_scroll::{self, BAR_WIDTH, Bar, Reach};

/// How wide the strip that is the bar's to press is, centred on it, in CSS
/// pixels.
pub const TARGET: f32 = 16.0;

/// What part of the bar a press landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnTheBar {
    /// The thumb itself: a drag.
    Thumb,
    /// The track above it: a page back.
    Above,
    /// The track below it: a page on.
    Below,
}

/// What part of `bar` a press at `x`, `y` on a first screen laid out at
/// `viewport`, in its CSS pixels, landed on, if any: inside the strip
/// [`TARGET`] wide about the bar, and between the room's top and its foot.
pub fn pressed(x: f32, y: f32, viewport: Size, bar: &Bar) -> Option<OnTheBar> {
    let middle = conversation_scroll::bar_left(viewport) + BAR_WIDTH / 2.0;
    let room = conversation_scroll::room(viewport);
    let half = TARGET / 2.0;
    let across = (middle - half..=middle + half).contains(&x);
    let down = (TOP..=TOP + room).contains(&y);
    if !(across && down) {
        return None;
    }
    let thumb = TOP + bar.top;
    Some(if y < thumb {
        OnTheBar::Above
    } else if y > thumb + bar.length {
        OnTheBar::Below
    } else {
        OnTheBar::Thumb
    })
}

/// How far the conversation is scrolled when its thumb, pressed with the
/// conversation scrolled `from`, has been dragged `moved` CSS pixels down
/// (up when less than nothing), on a first screen laid out at `viewport`
/// on a screen of `scale` whose turns reach as `reach` says: the thumb
/// following the pointer, kept inside how far the turns reach.
pub fn dragged(from: f32, moved: f32, reach: Reach, viewport: Size, scale: f32) -> f32 {
    let Some(bar) = conversation_scroll::bar(from, reach, viewport, scale) else {
        return conversation_scroll::clamped(from, reach, viewport);
    };
    let (least, most) = conversation_scroll::bounds(reach, viewport);
    let track = conversation_scroll::room(viewport) - bar.length;
    let by = if track > 0.0 && moved.is_finite() {
        moved * (most - least) / track
    } else {
        0.0
    };
    conversation_scroll::clamped(from + by, reach, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The design's first screen at one, 1040 by 820: its room 40 to 716,
    /// 676 tall; the bar from 973 to 979.
    fn design() -> Size {
        Size::new(1040.0, 820.0)
    }

    /// Three rooms of turns, scrolled to the first turn's top.
    fn reach() -> Reach {
        Reach {
            earlier: 1352.0,
            newest: 600.0,
        }
    }

    #[test]
    fn a_press_is_the_bars_within_its_strip_of_16_and_the_room() {
        let bar = conversation_scroll::bar(-1352.0, reach(), design(), 1.0).unwrap();
        // 676 of 2028: the thumb a third of the room, at its top.
        assert_eq!(bar.top.to_bits(), 0.0_f32.to_bits());
        assert_eq!(bar.length.to_bits(), 225.0_f32.to_bits());
        // The bar's middle at 976: 968 to 984 is its, the room's top 40 and
        // foot 716 with it.
        for x in [968.0, 976.0, 984.0] {
            assert_eq!(pressed(x, 100.0, design(), &bar), Some(OnTheBar::Thumb));
        }
        for (x, y) in [
            (967.9, 100.0),
            (984.1, 100.0),
            (976.0, 39.0),
            (976.0, 717.0),
        ] {
            assert_eq!(pressed(x, y, design(), &bar), None, "({x}, {y})");
        }
        assert_eq!(pressed(976.0, 40.0, design(), &bar), Some(OnTheBar::Thumb));
        assert_eq!(pressed(976.0, 265.0, design(), &bar), Some(OnTheBar::Thumb));
        assert_eq!(pressed(976.0, 266.0, design(), &bar), Some(OnTheBar::Below));
        assert_eq!(pressed(976.0, 716.0, design(), &bar), Some(OnTheBar::Below));
        // Scrolled to the newest turn's foot, the thumb at the room's foot.
        let foot = conversation_scroll::bar(0.0, reach(), design(), 1.0).unwrap();
        assert_eq!(
            pressed(976.0, 490.0, design(), &foot),
            Some(OnTheBar::Above)
        );
        assert_eq!(
            pressed(976.0, 491.0, design(), &foot),
            Some(OnTheBar::Thumb)
        );
    }

    #[test]
    fn a_drag_scrolls_as_far_as_the_thumb_moves() {
        // 1352 to travel over a track of 451: three of the conversation's
        // pixels for each of the thumb's, near enough.
        let travel = 1352.0_f32;
        let track = 676.0_f32 - 225.0;
        let to = dragged(-1352.0, 100.0, reach(), design(), 1.0);
        let expected = -1352.0 + 100.0 * travel / track;
        assert!((to - expected).abs() < 0.01, "{to} for {expected}");
        // Dragged the whole track, at the newest turn's foot; past it,
        // kept there; back, the first turn's top.
        assert_eq!(
            dragged(-1352.0, track, reach(), design(), 1.0).to_bits(),
            0.0_f32.to_bits()
        );
        assert_eq!(
            dragged(-1352.0, 5_000.0, reach(), design(), 1.0).to_bits(),
            0.0_f32.to_bits()
        );
        assert_eq!(
            dragged(0.0, -5_000.0, reach(), design(), 1.0).to_bits(),
            (-1352.0_f32).to_bits()
        );
        // Not moved, not scrolled; a moved that is no number, nothing.
        assert_eq!(
            dragged(-600.0, 0.0, reach(), design(), 1.0).to_bits(),
            (-600.0_f32).to_bits()
        );
        assert_eq!(
            dragged(-600.0, f32::NAN, reach(), design(), 1.0).to_bits(),
            (-600.0_f32).to_bits()
        );
    }

    #[test]
    fn turns_that_fit_have_no_bar_to_drag() {
        let fits = Reach {
            earlier: 0.0,
            newest: 300.0,
        };
        assert!(conversation_scroll::bar(0.0, fits, design(), 1.0).is_none());
        assert_eq!(
            dragged(0.0, 200.0, fits, design(), 1.0).to_bits(),
            0.0_f32.to_bits()
        );
    }
}
