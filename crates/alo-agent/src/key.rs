/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The keys an agent may press on a thing, by name (ADR 0109 § 1, queue
//! item 938).
//!
//! A key is a verb's, aimed at a node as every verb is — never at a
//! position (ADR 0002). The list is closed, and short: the keys an app's
//! own panels move by, which no other verb says. Google Sheets' Name box
//! goes to a cell only on Return, and a menu or a dialog closes on Escape.
//!
//! **What is not here, and why.** No letter, digit or character: text goes
//! in through `PutText`, which a page hears as typing. No Space: it presses
//! a button, which `Activate` already does, and which the person's gate
//! judges. No Backspace or Delete: on a board they delete what is selected,
//! and nothing in a key's name says what. No modifier: a shortcut with ⌘
//! or Ctrl is the app's command, and every one would need saying what it
//! does before it could be judged.

use core::fmt;

/// A key, as the DOM's `KeyboardEvent.key` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Return, or Enter.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
}

impl Key {
    /// Every key, in a fixed order.
    pub const ALL: [Key; 11] = [
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
    ];

    /// Its name, as the DOM's `KeyboardEvent.key` spells it.
    pub fn name(self) -> &'static str {
        match self {
            Key::Enter => "Enter",
            Key::Escape => "Escape",
            Key::Tab => "Tab",
            Key::ArrowUp => "ArrowUp",
            Key::ArrowDown => "ArrowDown",
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PageUp",
            Key::PageDown => "PageDown",
        }
    }

    /// The key named `name`, spelled exactly as [`Key::name`] spells it —
    /// or [`None`]: a key not in the list is not guessed at.
    pub fn named(name: &str) -> Option<Key> {
        Key::ALL.into_iter().find(|key| key.name() == name)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_named_back_as_it_is_spelled_and_nothing_else_is_a_key() {
        for key in Key::ALL {
            assert_eq!(Key::named(key.name()), Some(key));
        }
        for not_one in [
            "enter",
            "Return",
            "Space",
            " ",
            "Delete",
            "Backspace",
            "a",
            "Meta",
            "",
        ] {
            assert_eq!(Key::named(not_one), None, "{not_one:?}");
        }
    }
}
