/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How far a page that scrolls as the conversation does is scrolled: the
//! chat on screen's turns ([`crate::conversation_thread`], queue item 703)
//! and *Settings* (queue item 880) alike. One number, [`Scrolled::down`],
//! kept inside how far the page reaches as its renderer last measured it
//! (`alo-renderer`'s `conversation_scroll`), moved by a wheel's pixels at
//! once and a wheel's lines glided as Chrome glides them (queue item 756),
//! by the paging keys to either end, and into view for what the keyboard
//! is on.
//!
//! Every length here is in a room so tall, the page's own — the
//! conversation's from 40 to the gap over the box, *Settings*' the whole
//! first screen — which the caller says each time; until a reach is known
//! nothing moves.

use crate::conversation_glide::Glide;
use alo_renderer::conversation_scroll::{self, Bar, Reach, Span};
use std::time::Instant;

/// How far a page is scrolled, how far it reaches, and a glide under way.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scrolled {
    /// How far the page's top is above its room's top, in CSS pixels: for
    /// the conversation, its newest turn's top.
    down: f32,
    /// How far it reaches, as its renderer last measured it.
    reach: Option<Reach>,
    /// A wheel's lines under way: what moves [`Scrolled::down`] at each
    /// frame until it ends.
    glide: Option<Glide>,
}

impl Scrolled {
    /// How far it is scrolled.
    pub fn down(&self) -> f32 {
        self.down
    }

    /// How far it reaches, once measured.
    pub fn reach(&self) -> Option<Reach> {
        self.reach
    }

    /// Whether a wheel's lines are being glided.
    pub fn gliding(&self) -> bool {
        self.glide.is_some()
    }

    /// Back to its top, its reach to be measured again: a new turn, or
    /// *Settings* opened.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Its reach to be measured again, where it is scrolled kept.
    pub fn forget_reach(&mut self) {
        self.reach = None;
    }

    /// Its renderer measured it to reach as `reach` says, in a room `room`
    /// tall: kept, and how far it is scrolled kept inside it. Whether that
    /// moved it.
    pub fn reached(&mut self, reach: Reach, room: f32) -> bool {
        self.reach = Some(reach.bounded());
        self.moved_to(conversation_scroll::clamped_in(self.down, reach, room))
    }

    /// Scrolled as far as it goes back — its top at the room's top — or,
    /// not `back`, on — its foot at the room's foot (queue item 754).
    /// Whether it moved; until a reach is known, it does not.
    pub fn to_end(&mut self, back: bool, room: f32) -> bool {
        let Some(reach) = self.reach else {
            return false;
        };
        let (least, most) = conversation_scroll::bounds_in(reach, room);
        self.glide = None;
        self.moved_to(if back { least } else { most })
    }

    /// The sign that it goes on above or below a room `room` tall on a
    /// screen of `scale` (queue item 755): none until its reach is
    /// measured, or while it fits.
    pub fn bar(&self, room: f32, scale: f32) -> Option<Bar> {
        self.reach
            .and_then(|reach| conversation_scroll::bar_in(self.down, reach, room, scale))
    }

    /// Scrolled `by` CSS pixels down — a wheel's pixels, a key's page, a
    /// drag of the bar — inside how far it reaches. Whether it moved.
    pub fn by(&mut self, by: f32, room: f32) -> bool {
        let Some(reach) = self.reach else {
            return false;
        };
        // Followed as it comes: a glide under way ends here.
        self.glide = None;
        self.moved_to(conversation_scroll::clamped_in(self.down + by, reach, room))
    }

    /// Scrolled as little as shows `span` whole with `margin` about it
    /// (queue item 880): the keyboard moved to what stands there. Whether
    /// it moved.
    pub fn into_view(&mut self, span: Span, margin: f32, room: f32) -> bool {
        let Some(reach) = self.reach else {
            return false;
        };
        self.glide = None;
        self.moved_to(conversation_scroll::into_view(
            self.down, span, margin, reach, room,
        ))
    }

    /// A wheel's lines turned `by` CSS pixels down `now`, in a room `room`
    /// tall on a screen of `scale` (queue item 756): glided there, inside
    /// how far it reaches — on from the end of a glide under way, as fast
    /// as it was going. Whether a glide is under way for it.
    pub fn glide_lines(&mut self, by: f32, room: f32, now: Instant, scale: f32) -> bool {
        let Some(reach) = self.reach else {
            return false;
        };
        let from = self.glide.map_or(self.down, |glide| glide.end());
        let to = conversation_scroll::clamped_in(from + by, reach, room);
        self.glide = match self.glide {
            Some(glide) => Some(glide.to(to, now, scale)),
            None if to.to_bits() != self.down.to_bits() => {
                Some(Glide::new(self.down, to, now, scale))
            }
            None => None,
        };
        self.glide.is_some()
    }

    /// The glide under way moved on to where it stands `now`, inside how
    /// far it reaches, and ended once its time is up. Whether that moved
    /// it.
    pub fn glide(&mut self, now: Instant, room: f32, scale: f32) -> bool {
        let (Some(glide), Some(reach)) = (self.glide, self.reach) else {
            self.glide = None;
            return false;
        };
        let kept = conversation_scroll::clamped_in(glide.at(now, scale), reach, room);
        if glide.done(now) {
            self.glide = None;
        }
        self.moved_to(kept)
    }

    /// Scrolled to `kept`: whether that moved it.
    fn moved_to(&mut self, kept: f32) -> bool {
        let moved = kept.to_bits() != self.down.to_bits();
        self.down = kept;
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// *Settings*' room at the least window, 600 tall.
    const ROOM: f32 = 600.0;

    /// The page *Settings* is, 816 tall with what is above and below it.
    fn settings() -> Scrolled {
        let mut scrolled = Scrolled::default();
        let _ = scrolled.reached(
            Reach {
                earlier: 0.0,
                newest: 816.0,
            },
            ROOM,
        );
        scrolled
    }

    #[test]
    fn nothing_moves_until_its_reach_is_measured_and_then_only_inside_it() {
        let mut scrolled = Scrolled::default();
        assert!(!scrolled.by(100.0, ROOM));
        assert!(!scrolled.to_end(false, ROOM));
        assert_eq!(scrolled.bar(ROOM, 1.0), None);
        let mut scrolled = settings();
        assert!(scrolled.by(100.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 100.0_f32.to_bits());
        assert!(scrolled.by(1000.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 216.0_f32.to_bits(), "816 - 600");
        assert!(!scrolled.by(1.0, ROOM), "no further");
        assert!(scrolled.to_end(true, ROOM));
        assert_eq!(scrolled.down().abs().to_bits(), 0.0_f32.to_bits());
        // A taller window: all of it fits, and it scrolls nowhere.
        assert!(scrolled.to_end(false, ROOM));
        assert!(scrolled.reached(
            Reach {
                earlier: 0.0,
                newest: 816.0,
            },
            820.0
        ));
        assert_eq!(scrolled.down().abs().to_bits(), 0.0_f32.to_bits());
        assert_eq!(scrolled.bar(820.0, 1.0), None);
    }

    #[test]
    fn what_the_keyboard_is_on_is_scrolled_into_view_and_no_further() {
        let mut scrolled = settings();
        // Seen already: nothing moves.
        let back = Span {
            top: 32.0,
            foot: 68.0,
        };
        assert!(!scrolled.into_view(back, 16.0, ROOM));
        // Below the room's foot: just far enough that its foot, and 16
        // under it, is at the room's foot.
        let asks = Span {
            top: 700.0,
            foot: 745.0,
        };
        assert!(scrolled.into_view(asks, 16.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 161.0_f32.to_bits(), "761 - 600");
        // Above the room's top: its top, 16 above it, at the room's top.
        let font = Span {
            top: 120.0,
            foot: 400.0,
        };
        assert!(scrolled.into_view(font, 16.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 104.0_f32.to_bits());
        // Taller than the room: its top at the room's top, inside the reach.
        let tall = Span {
            top: 200.0,
            foot: 900.0,
        };
        assert!(scrolled.into_view(tall, 16.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 184.0_f32.to_bits());
        assert!(scrolled.into_view(back, 16.0, ROOM));
        assert_eq!(scrolled.down().to_bits(), 16.0_f32.to_bits());
    }
}
