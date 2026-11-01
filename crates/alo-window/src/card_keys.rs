/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which of a person's keys reach the card's page while the card has the
//! keyboard (ADR 0076 B, ADR 0024 § 5, queue items 718 and 741), and as
//! what.
//!
//! The conversation's window gives the card the keyboard when a press lands
//! on its page, and takes it back with a press anywhere else
//! ([`crate::conversation::Conversation::press_the_page`]). While it has
//! it, every key going down is asked of [`key`] here instead of the one box
//! ([`crate::typing`]), and what it answers is sent to the page:
//!
//! - **A key that writes text** — with Shift a capital, with ⌥ what ⌥ makes —
//!   sends that text, a control character never.
//! - **Return, Backspace, Tab and Escape** are sent as themselves: a form
//!   sent, a character erased, the next field, a dialog closed — the page's
//!   to decide, as in any browser.
//! - **The arrows, Home, End, Page Up, Page Down, the forward delete and
//!   the function keys** are sent as themselves (queue item 741), with Shift
//!   and ⌥ held — the caret moved, by a word with ⌥, the selection grown
//!   with Shift, the page scrolled; on a Mac each carries the editing
//!   command macOS binds it to ([`crate::key_bindings`]).
//! - **The page's own editing shortcuts** — with ⌘ on a Mac, Ctrl
//!   elsewhere: select all (A), copy (C), cut (X), paste (V), undo (Z) and
//!   redo (⇧Z, and Ctrl+Y elsewhere) — are the page's, as they are in
//!   Chrome's own window, where its *Edit* menu sends them to the page. So,
//!   on a Mac, are ⌘ with an arrow (to the line's or the document's end)
//!   and ⌘ with Backspace (to the line's start), which macOS binds to
//!   editing.
//! - **Every other key held with ⌘ or Ctrl** stays the browser's and the
//!   system's: ⌘Q, ⌘W, ⌘T, ⌘H, ⌘Tab and the rest never reach the page,
//!   and nor does any key with Ctrl on a Mac — its text system's own
//!   Emacs-like bindings (Ctrl+A to a line's start) are not carried yet
//!   (queue item 803).
//! - **Any other key** writing nothing is not sent.
//!
//! A key held down repeats, as it does in a page in Chrome's own window.
//!
//! **Not yet: an input method's composition** (queue item 802) — Japanese,
//! Chinese, an accent held on a Mac — which the window server tells as
//! `Ime` events only once the window asks for them, and which would then
//! change how every key reaches the one box too.

use crate::card_input::{Editing, Keyed, PersonsKey};
use crate::keys::{Key, Modifier, Platform, Press};

/// What `press`, which the window server says writes `written`, sends to
/// the card's page, if anything, on this platform.
pub fn key(press: &Press, written: Option<&str>) -> Option<PersonsKey> {
    key_on(press, written, Platform::HERE)
}

/// What [`key`] answers on `platform`.
pub fn key_on(press: &Press, written: Option<&str>, platform: Platform) -> Option<PersonsKey> {
    // The platform's shortcut key, and the other of ⌘ and Ctrl, which is
    // the system's there.
    let (shortcut, systems) = match platform {
        Platform::Mac => (Modifier::Command, Modifier::Control),
        Platform::Elsewhere => (Modifier::Control, Modifier::Command),
    };
    if press.held.holds(systems) {
        return None;
    }
    let key = if press.held.holds(shortcut) {
        shortcut_of(press, platform)?
    } else {
        plain(press, written)?
    };
    Some(PersonsKey {
        key,
        held: press.held,
    })
}

/// Which of the page's own keys `press`, held with the platform's shortcut
/// key, is — if it is one of them.
fn shortcut_of(press: &Press, platform: Platform) -> Option<Keyed> {
    let shift = press.held.holds(Modifier::Shift);
    let letter = |wanted: &str| match &press.key {
        Key::Character(written) => written.eq_ignore_ascii_case(wanted),
        _ => false,
    };
    let editing = if letter("a") {
        Editing::SelectAll
    } else if letter("c") {
        Editing::Copy
    } else if letter("x") {
        Editing::Cut
    } else if letter("v") {
        Editing::Paste
    } else if letter("z") {
        if shift { Editing::Redo } else { Editing::Undo }
    } else if letter("y") && platform == Platform::Elsewhere {
        Editing::Redo
    } else {
        // ⌘ with an arrow or Backspace is editing on a Mac; elsewhere
        // Ctrl with them is Chrome's own to read.
        return match press.key {
            Key::Arrow(arrow) => Some(Keyed::Arrow(arrow)),
            Key::Backspace => Some(Keyed::Backspace),
            _ => None,
        };
    };
    Some(Keyed::Editing(editing))
}

/// Which key `press`, writing `written`, held with no ⌘ or Ctrl, is sent
/// as — if it is sent at all.
fn plain(press: &Press, written: Option<&str>) -> Option<Keyed> {
    Some(match press.key {
        Key::Return => Keyed::Return,
        Key::Backspace => Keyed::Backspace,
        Key::Tab => Keyed::Tab,
        Key::Escape => Keyed::Escape,
        Key::PageUp => Keyed::PageUp,
        Key::PageDown => Keyed::PageDown,
        Key::Home => Keyed::Home,
        Key::End => Keyed::End,
        Key::Arrow(arrow) => Keyed::Arrow(arrow),
        Key::Delete => Keyed::Delete,
        Key::Function(number) => Keyed::Function(number),
        Key::Character(_) | Key::Space | Key::Other => {
            let text: String = written?.chars().filter(|each| !each.is_control()).collect();
            if text.is_empty() {
                return None;
            }
            Keyed::Text(text)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Held;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "compared with what `key` answers, which is an `Option`"
    )]
    fn plain(key: Keyed) -> Option<PersonsKey> {
        Some(PersonsKey {
            key,
            held: Held::NOTHING,
        })
    }

    #[test]
    fn a_letter_sends_what_it_writes() {
        assert_eq!(
            key(&Press::character("a"), Some("a")),
            plain(Keyed::Text("a".to_owned()))
        );
        assert_eq!(
            key(&Press::character(" "), Some(" ")),
            plain(Keyed::Text(" ".to_owned()))
        );
        assert_eq!(
            key(&Press::character("a").with(Modifier::Shift), Some("A")),
            Some(PersonsKey {
                key: Keyed::Text("A".to_owned()),
                held: Held::NOTHING.with(Modifier::Shift),
            })
        );
        assert_eq!(
            key(&Press::character("p").with(Modifier::Option), Some("π")),
            Some(PersonsKey {
                key: Keyed::Text("π".to_owned()),
                held: Held::NOTHING.with(Modifier::Option),
            })
        );
        assert_eq!(
            key(&Press::character("a").repeating(), Some("a")),
            plain(Keyed::Text("a".to_owned())),
            "held, it repeats"
        );
    }

    #[test]
    fn return_backspace_tab_and_escape_are_sent_as_themselves() {
        assert_eq!(
            key(&Press::of(Key::Return), Some("\r")),
            plain(Keyed::Return)
        );
        assert_eq!(
            key(&Press::of(Key::Return).repeating(), None),
            plain(Keyed::Return)
        );
        assert_eq!(
            key(&Press::of(Key::Backspace), Some("\u{8}")),
            plain(Keyed::Backspace)
        );
        assert_eq!(key(&Press::of(Key::Tab), Some("\t")), plain(Keyed::Tab));
        assert_eq!(
            key(&Press::of(Key::Tab).with(Modifier::Shift), None),
            Some(PersonsKey {
                key: Keyed::Tab,
                held: Held::NOTHING.with(Modifier::Shift),
            })
        );
        assert_eq!(
            key(&Press::of(Key::Escape), Some("\u{1b}")),
            plain(Keyed::Escape)
        );
    }

    fn mac(press: &Press, written: Option<&str>) -> Option<Keyed> {
        key_on(press, written, Platform::Mac).map(|sent| sent.key)
    }

    fn elsewhere(press: &Press, written: Option<&str>) -> Option<Keyed> {
        key_on(press, written, Platform::Elsewhere).map(|sent| sent.key)
    }

    #[test]
    fn the_keys_that_write_nothing_are_sent_as_themselves_with_what_is_held() {
        use crate::keys::Arrow;
        for (pressed, sent) in [
            (Key::Arrow(Arrow::Left), Keyed::Arrow(Arrow::Left)),
            (Key::Arrow(Arrow::Right), Keyed::Arrow(Arrow::Right)),
            (Key::Arrow(Arrow::Up), Keyed::Arrow(Arrow::Up)),
            (Key::Arrow(Arrow::Down), Keyed::Arrow(Arrow::Down)),
            (Key::Home, Keyed::Home),
            (Key::End, Keyed::End),
            (Key::PageUp, Keyed::PageUp),
            (Key::PageDown, Keyed::PageDown),
            (Key::Delete, Keyed::Delete),
            (Key::Function(5), Keyed::Function(5)),
        ] {
            // Whatever private character the window server says it writes.
            assert_eq!(
                key(&Press::of(pressed.clone()), Some("\u{f702}")),
                plain(sent.clone()),
                "{pressed:?}"
            );
            let selecting = Press::of(pressed.clone())
                .with(Modifier::Shift)
                .with(Modifier::Option);
            assert_eq!(
                key(&selecting, None),
                Some(PersonsKey {
                    key: sent,
                    held: selecting.held,
                }),
                "{pressed:?} with Shift and ⌥, held"
            );
        }
    }

    #[test]
    fn the_pages_editing_shortcuts_are_its_own_on_a_mac_with_command() {
        let command = |letter: &str| Press::character(letter).with(Modifier::Command);
        assert_eq!(
            mac(&command("a"), Some("a")),
            Some(Keyed::Editing(Editing::SelectAll))
        );
        assert_eq!(
            mac(&command("c"), Some("c")),
            Some(Keyed::Editing(Editing::Copy))
        );
        assert_eq!(
            mac(&command("x"), Some("x")),
            Some(Keyed::Editing(Editing::Cut))
        );
        assert_eq!(
            mac(&command("v"), Some("v")),
            Some(Keyed::Editing(Editing::Paste))
        );
        assert_eq!(
            mac(&command("z"), Some("z")),
            Some(Keyed::Editing(Editing::Undo))
        );
        assert_eq!(
            mac(&command("z").with(Modifier::Shift), Some("Z")),
            Some(Keyed::Editing(Editing::Redo))
        );
        assert_eq!(
            mac(&command("A"), Some("A")),
            Some(Keyed::Editing(Editing::SelectAll))
        );
        // With ⌘, an arrow and Backspace edit, carrying ⌘.
        let to_the_end = Press::of(Key::Arrow(crate::keys::Arrow::Right)).with(Modifier::Command);
        assert_eq!(
            key_on(&to_the_end, None, Platform::Mac),
            Some(PersonsKey {
                key: Keyed::Arrow(crate::keys::Arrow::Right),
                held: to_the_end.held,
            })
        );
        assert_eq!(
            mac(&Press::of(Key::Backspace).with(Modifier::Command), None),
            Some(Keyed::Backspace)
        );
    }

    #[test]
    fn every_other_key_with_command_or_control_stays_the_browsers_and_the_systems() {
        let command = |letter: &str| Press::character(letter).with(Modifier::Command);
        for letter in ["q", "w", "t", "h", "y", "l", "r", "m", "n", ","] {
            assert_eq!(mac(&command(letter), Some(letter)), None, "⌘{letter}");
        }
        for key in [
            Key::Return,
            Key::Tab,
            Key::Escape,
            Key::Home,
            Key::Function(1),
        ] {
            assert_eq!(
                mac(&Press::of(key.clone()).with(Modifier::Command), None),
                None
            );
        }
        // Ctrl on a Mac is the system's, editing letters included.
        for letter in ["a", "c", "v", "e", "k", "q"] {
            assert_eq!(
                mac(
                    &Press::character(letter).with(Modifier::Control),
                    Some(letter)
                ),
                None,
                "Ctrl+{letter}"
            );
        }
        assert_eq!(
            mac(
                &Press::of(Key::Arrow(crate::keys::Arrow::Left)).with(Modifier::Control),
                None
            ),
            None
        );
        assert_eq!(
            mac(&command("a").with(Modifier::Control), Some("a")),
            None,
            "⌘ and Ctrl together"
        );
    }

    #[test]
    fn elsewhere_the_shortcuts_are_ctrls_and_the_windows_key_the_systems() {
        let control = |letter: &str| Press::character(letter).with(Modifier::Control);
        assert_eq!(
            elsewhere(&control("a"), Some("a")),
            Some(Keyed::Editing(Editing::SelectAll))
        );
        assert_eq!(
            elsewhere(&control("y"), Some("y")),
            Some(Keyed::Editing(Editing::Redo))
        );
        assert_eq!(
            elsewhere(&control("z").with(Modifier::Shift), Some("Z")),
            Some(Keyed::Editing(Editing::Redo))
        );
        assert_eq!(elsewhere(&control("w"), Some("w")), None);
        assert_eq!(
            elsewhere(&Press::character("a").with(Modifier::Command), Some("a")),
            None
        );
        assert_eq!(mac(&command_y(), Some("y")), None, "⌘Y is no redo on a Mac");
    }

    fn command_y() -> Press {
        Press::character("y").with(Modifier::Command)
    }

    #[test]
    fn a_key_that_writes_nothing_or_a_control_character_is_not_sent() {
        assert_eq!(key(&Press::of(Key::Other), None), None);
        assert_eq!(key(&Press::character("\u{0}"), Some("\u{0}")), None);
        assert_eq!(key(&Press::character("\u{7f}"), Some("\u{7f}")), None);
        assert_eq!(
            key(&Press::of(Key::Space), Some(" ")).map(|sent| sent.key),
            Some(Keyed::Text(" ".to_owned()))
        );
    }
}
