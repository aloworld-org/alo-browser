/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which of a person's keys reach the card's page while the card has the
//! keyboard (ADR 0076 B, ADR 0024 § 5, queue item 718), and as what.
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
//! - **Nothing held with ⌘ or Ctrl** reaches the page: those are the
//!   browser's keys and the system's (⌘Q, ⌘W), and the page's own
//!   shortcuts — copying, pasting, selecting all — are not carried yet
//!   (queue item 741).
//! - **Any other key** writing nothing — the arrows, Home, the function
//!   keys — is not sent yet, since the window reads none of them apart
//!   (queue item 741).
//!
//! A key held down repeats, as it does in a page in Chrome's own window.

use crate::card_input::{Keyed, PersonsKey};
use crate::keys::{Key, Modifier, Press};

/// What `press`, which the window server says writes `written`, sends to
/// the card's page, if anything.
pub fn key(press: &Press, written: Option<&str>) -> Option<PersonsKey> {
    if press.held.holds(Modifier::Command) || press.held.holds(Modifier::Control) {
        return None;
    }
    let key = match press.key {
        Key::Return => Keyed::Return,
        Key::Backspace => Keyed::Backspace,
        Key::Tab => Keyed::Tab,
        Key::Escape => Keyed::Escape,
        Key::Character(_) | Key::Other => {
            let text: String = written?.chars().filter(|each| !each.is_control()).collect();
            if text.is_empty() {
                return None;
            }
            Keyed::Text(text)
        }
    };
    Some(PersonsKey {
        key,
        shift: press.held.holds(Modifier::Shift),
        option: press.held.holds(Modifier::Option),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "compared with what `key` answers, which is an `Option`"
    )]
    fn plain(key: Keyed) -> Option<PersonsKey> {
        Some(PersonsKey {
            key,
            shift: false,
            option: false,
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
                shift: true,
                option: false,
            })
        );
        assert_eq!(
            key(&Press::character("p").with(Modifier::Option), Some("π")),
            Some(PersonsKey {
                key: Keyed::Text("π".to_owned()),
                shift: false,
                option: true,
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
                shift: true,
                option: false,
            })
        );
        assert_eq!(
            key(&Press::of(Key::Escape), Some("\u{1b}")),
            plain(Keyed::Escape)
        );
    }

    #[test]
    fn nothing_held_with_command_or_control_reaches_the_page() {
        for modifier in [Modifier::Command, Modifier::Control] {
            assert_eq!(key(&Press::character("q").with(modifier), Some("q")), None);
            assert_eq!(key(&Press::character("v").with(modifier), Some("v")), None);
            assert_eq!(key(&Press::of(Key::Return).with(modifier), None), None);
            assert_eq!(key(&Press::of(Key::Tab).with(modifier), None), None);
        }
    }

    #[test]
    fn a_key_that_writes_nothing_or_a_control_character_is_not_sent() {
        assert_eq!(key(&Press::of(Key::Other), None), None);
        assert_eq!(key(&Press::character("\u{0}"), Some("\u{0}")), None);
        assert_eq!(key(&Press::character("\u{7f}"), Some("\u{7f}")), None);
    }
}
