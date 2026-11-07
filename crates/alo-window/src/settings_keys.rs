/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's key does on *Settings* (queue item 856,
//! `docs/design/shell.md`, *Settings*): the keys the page takes while it is
//! on screen and neither *Search chats* nor anything else holding the
//! keyboard would take them, as the ARIA radio group pattern and a Mac's
//! radio buttons have them.
//!
//! | | |
//! |---|---|
//! | Tab, ⇧Tab | the keyboard on *Back to the chat*, then *Font*, round |
//! | ← ↑, → ↓ | on *Font*: the face before, or after, chosen, round |
//! | Space, Return | on *Back to the chat*: pressed |
//!
//! Escape closes *Settings* wherever the keyboard is, as before (854). A key
//! held with ⌘, ⌃ or ⌥ is none of these; held down, Tab and the arrows go
//! on, and Space and Return press once.

use crate::keys::{Arrow, Key, Modifier, Press};

/// What a key asks of *Settings*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsKey {
    /// Tab, or ⇧Tab when `back`.
    Tab {
        /// Whether back, as ⇧Tab is.
        back: bool,
    },
    /// An arrow: on, as → and ↓, or back, as ← and ↑.
    Step {
        /// Whether on.
        on: bool,
    },
    /// Space or Return: what the keyboard is on, pressed.
    Press,
}

/// What `press` asks of *Settings*, if anything.
pub fn key(press: &Press) -> Option<SettingsKey> {
    let held = press.held;
    if [Modifier::Command, Modifier::Control, Modifier::Option]
        .into_iter()
        .any(|modifier| held.holds(modifier))
    {
        return None;
    }
    let shift = held.holds(Modifier::Shift);
    match &press.key {
        Key::Tab => Some(SettingsKey::Tab { back: shift }),
        Key::Arrow(arrow) if !shift => Some(SettingsKey::Step {
            on: matches!(arrow, Arrow::Right | Arrow::Down),
        }),
        Key::Space | Key::Return if !shift && !press.repeat => Some(SettingsKey::Press),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_the_arrows_space_and_return_and_nothing_else() {
        assert_eq!(
            key(&Press::of(Key::Tab)),
            Some(SettingsKey::Tab { back: false })
        );
        assert_eq!(
            key(&Press::of(Key::Tab).with(Modifier::Shift)),
            Some(SettingsKey::Tab { back: true })
        );
        for (arrow, on) in [
            (Arrow::Left, false),
            (Arrow::Up, false),
            (Arrow::Right, true),
            (Arrow::Down, true),
        ] {
            assert_eq!(
                key(&Press::of(Key::Arrow(arrow))),
                Some(SettingsKey::Step { on }),
                "{arrow:?}"
            );
        }
        assert_eq!(key(&Press::of(Key::Space)), Some(SettingsKey::Press));
        assert_eq!(key(&Press::of(Key::Return)), Some(SettingsKey::Press));
        // Held down: Tab and the arrows go on; Space and Return press once.
        assert_eq!(
            key(&Press::of(Key::Tab).repeating()),
            Some(SettingsKey::Tab { back: false })
        );
        assert_eq!(key(&Press::of(Key::Space).repeating()), None);
        // With ⌘, ⌃ or ⌥, nothing; ⇧ with an arrow or Space, nothing.
        for modifier in [Modifier::Command, Modifier::Control, Modifier::Option] {
            assert_eq!(key(&Press::of(Key::Tab).with(modifier)), None);
            assert_eq!(
                key(&Press::of(Key::Arrow(Arrow::Right)).with(modifier)),
                None
            );
        }
        assert_eq!(
            key(&Press::of(Key::Arrow(Arrow::Right)).with(Modifier::Shift)),
            None
        );
        assert_eq!(key(&Press::of(Key::Space).with(Modifier::Shift)), None);
        for other in [Key::Escape, Key::Backspace, Key::PageDown, Key::Home] {
            assert_eq!(key(&Press::of(other.clone())), None, "{other:?}");
        }
        assert_eq!(key(&Press::character("a")), None);
    }
}
