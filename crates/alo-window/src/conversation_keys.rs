/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which of a person's keys scroll the conversation, and how far (queue
//! item 754, ADR 0093 § 1).
//!
//! # The keys
//!
//! - **Page Down**, and **space**: on by a room less a line — the room the
//!   conversation is seen in (`alo-renderer`'s `conversation_scroll::room`)
//!   less [`crate::card_aim::LINE`], so the line at the room's foot is at its
//!   top after, and a person keeps their place.
//! - **Page Up**, and **Shift-space**: back by as much.
//! - **Home**: back to the first turn's top. **End**: on to the newest
//!   turn's foot.
//!
//! A key held down repeats, as it does over a page. ⌘, ⌃ or ⌥ with any of
//! them is not this: those are the browser's keys and the system's.
//!
//! # Only while nothing else wants them
//!
//! They scroll **while nothing holding the keyboard would take them**: the
//! card's page keeps every key while it has the keyboard (queue item 718);
//! *Search chats* keeps its own while it has it; and the one box keeps its
//! own while it has the keyboard and holds what a person typed, so a space
//! is a space between their words. An empty box gives them to the
//! conversation — none of them would edit it — as does a window where
//! nothing has the keyboard. Which of those holds is the conversation's to
//! say (`Conversation::keys_scroll`); this is what the key asks.
//!
//! A key's scroll is made at once, not glided: Chrome glides its keyboard
//! scrolls too, on a curve not measured here (queue item 792), and a wheel's
//! lines are glided as Chrome's are ([`crate::conversation_glide`]).

use crate::card_aim::LINE;
use crate::keys::{Key, Modifier, Press};

/// How a key asks the conversation to scroll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scroll {
    /// By this many CSS pixels down — on toward the newest turn's foot, or,
    /// below zero, back.
    By(f32),
    /// Back to the first turn's top.
    ToFirst,
    /// On to the newest turn's foot.
    ToNewest,
}

/// How far `press` scrolls a conversation seen in a room `room` CSS pixels
/// tall, if it is one of the keys that do.
pub fn scroll(press: &Press, room: f32) -> Option<Scroll> {
    let held = press.held;
    if held.holds(Modifier::Command)
        || held.holds(Modifier::Control)
        || held.holds(Modifier::Option)
    {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a line is 40 CSS pixels, exactly an f32"
    )]
    let line = LINE as f32;
    // A room less a line, and never less than a line, however short the
    // room.
    let page = (room - line).max(line);
    let shifted = held.holds(Modifier::Shift);
    match press.key {
        Key::PageUp => Some(Scroll::By(-page)),
        Key::Space if shifted => Some(Scroll::By(-page)),
        Key::PageDown | Key::Space => Some(Scroll::By(page)),
        Key::Home => Some(Scroll::ToFirst),
        Key::End => Some(Scroll::ToNewest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The room at the design's window, 820 tall: 40 down to 716.
    const ROOM: f32 = 676.0;

    fn by(scroll: Option<Scroll>) -> Option<u32> {
        match scroll {
            Some(Scroll::By(length)) => Some(length.to_bits()),
            _ => None,
        }
    }

    #[test]
    fn page_down_and_space_go_on_a_room_less_a_line_and_page_up_and_shift_space_back() {
        assert_eq!(
            by(scroll(&Press::of(Key::PageDown), ROOM)),
            Some(636.0_f32.to_bits())
        );
        assert_eq!(
            by(scroll(&Press::of(Key::Space), ROOM)),
            Some(636.0_f32.to_bits())
        );
        assert_eq!(
            by(scroll(&Press::of(Key::PageUp), ROOM)),
            Some((-636.0_f32).to_bits())
        );
        assert_eq!(
            by(scroll(&Press::of(Key::Space).with(Modifier::Shift), ROOM)),
            Some((-636.0_f32).to_bits())
        );
        // Held down, it goes on.
        assert_eq!(
            by(scroll(&Press::of(Key::PageDown).repeating(), ROOM)),
            Some(636.0_f32.to_bits())
        );
        // In the least window's room, 456, and a room shorter than a line.
        assert_eq!(
            by(scroll(&Press::of(Key::PageDown), 456.0)),
            Some(416.0_f32.to_bits())
        );
        assert_eq!(
            by(scroll(&Press::of(Key::PageDown), 10.0)),
            Some(40.0_f32.to_bits())
        );
    }

    #[test]
    fn home_and_end_go_to_the_ends() {
        assert_eq!(scroll(&Press::of(Key::Home), ROOM), Some(Scroll::ToFirst));
        assert_eq!(scroll(&Press::of(Key::End), ROOM), Some(Scroll::ToNewest));
        assert_eq!(
            scroll(&Press::of(Key::End).with(Modifier::Shift), ROOM),
            Some(Scroll::ToNewest)
        );
    }

    #[test]
    fn with_command_control_or_option_and_any_other_key_they_are_not_scrolls() {
        for modifier in [Modifier::Command, Modifier::Control, Modifier::Option] {
            for key in [Key::PageDown, Key::PageUp, Key::Space, Key::Home, Key::End] {
                assert_eq!(scroll(&Press::of(key).with(modifier), ROOM), None);
            }
        }
        for key in [
            Key::Return,
            Key::Tab,
            Key::Backspace,
            Key::Escape,
            Key::Other,
        ] {
            assert_eq!(scroll(&Press::of(key), ROOM), None);
        }
        assert_eq!(scroll(&Press::character("j"), ROOM), None);
    }
}
