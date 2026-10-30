/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's key does to the one box on the first screen (ADR 0074,
//! queue item 701).
//!
//! The conversation's window reads every key going down as a
//! [`crate::keys::Press`], with the text the window server says it writes,
//! and asks [`edit`] what it does to the box. The answer is one of four
//! edits, and the browser process — which alone holds what is typed
//! ([`crate::conversation_box`]) — makes it. No key reaches a renderer: the
//! first screen's renderer is sent what the box then holds, as data.
//!
//! # Which keys type
//!
//! - **Return** chooses what is offered first: going to the site, when what
//!   is typed is an address. Held down, it does not choose again.
//! - **Backspace** erases the last character, and repeats while held.
//! - **Escape** empties the box.
//! - **Any other key** puts in the text the window server says it writes —
//!   with Shift a capital, with ⌥ on a Mac the character ⌥ makes, `π` for
//!   ⌥P — and repeats while held. A control character in that text is
//!   never put in: Return's `\r`, Backspace's, Escape's, a NUL.
//!
//! **Nothing held with ⌘ or Ctrl types.** Those are the browser's keys and
//! the system's — ⌘T, ⌘Q, ⌘W — and a person pressing one is asking for
//! something other than a letter in the box. Tab does not type either: the
//! design has it ask the agent, which is not built (queue item 707).
//!
//! Not read yet: an input method's composition — the window server's
//! `Ime` events, by which a person types Japanese or Chinese — and moving
//! within what is typed. What is typed is edited at its end.

use crate::keys::{Key, Modifier, Press};

/// What a key does to the one box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Put this text at the end of what is typed.
    Put(String),
    /// Erase the last character typed.
    Erase,
    /// Empty the box.
    Clear,
    /// Choose what is offered first.
    Choose,
}

/// What `press`, which the window server says writes `written`, does to the
/// one box, if anything.
pub fn edit(press: &Press, written: Option<&str>) -> Option<Edit> {
    if press.held.holds(Modifier::Command) || press.held.holds(Modifier::Control) {
        return None;
    }
    match press.key {
        // Held down, neither chooses nor empties again.
        Key::Return | Key::Escape if press.repeat => None,
        Key::Return => Some(Edit::Choose),
        Key::Backspace => Some(Edit::Erase),
        Key::Escape => Some(Edit::Clear),
        Key::Tab => None,
        Key::Character(_) | Key::Other => {
            let text: String = written?.chars().filter(|each| !each.is_control()).collect();
            (!text.is_empty()).then_some(Edit::Put(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(character: &str) -> Option<Edit> {
        edit(&Press::character(character), Some(character))
    }

    #[test]
    fn a_letter_puts_what_it_writes() {
        assert_eq!(typed("e"), Some(Edit::Put("e".to_owned())));
        assert_eq!(typed("."), Some(Edit::Put(".".to_owned())));
        assert_eq!(typed(" "), Some(Edit::Put(" ".to_owned())));
        // With Shift, the capital the window server says it writes.
        assert_eq!(
            edit(&Press::character("e").with(Modifier::Shift), Some("E")),
            Some(Edit::Put("E".to_owned()))
        );
        // With ⌥ on a Mac, what ⌥ makes of it.
        assert_eq!(
            edit(&Press::character("p").with(Modifier::Option), Some("π")),
            Some(Edit::Put("π".to_owned()))
        );
        // Held down, it goes on typing.
        assert_eq!(
            edit(&Press::character("e").repeating(), Some("e")),
            Some(Edit::Put("e".to_owned()))
        );
    }

    #[test]
    fn nothing_held_with_command_or_control_types() {
        for modifier in [Modifier::Command, Modifier::Control] {
            assert_eq!(
                edit(&Press::character("t").with(modifier), Some("t")),
                None,
                "{modifier:?}"
            );
            assert_eq!(edit(&Press::of(Key::Return).with(modifier), None), None);
            assert_eq!(edit(&Press::of(Key::Backspace).with(modifier), None), None);
            assert_eq!(
                edit(
                    &Press::character("v").with(modifier).with(Modifier::Shift),
                    Some("V")
                ),
                None
            );
        }
    }

    #[test]
    fn return_chooses_backspace_erases_and_escape_empties() {
        assert_eq!(
            edit(&Press::of(Key::Return), Some("\r")),
            Some(Edit::Choose)
        );
        assert_eq!(
            edit(&Press::of(Key::Backspace), Some("\u{8}")),
            Some(Edit::Erase)
        );
        assert_eq!(
            edit(&Press::of(Key::Escape), Some("\u{1b}")),
            Some(Edit::Clear)
        );
        // Held: Backspace goes on erasing, and Return does not choose again.
        assert_eq!(
            edit(&Press::of(Key::Backspace).repeating(), None),
            Some(Edit::Erase)
        );
        assert_eq!(edit(&Press::of(Key::Return).repeating(), None), None);
        assert_eq!(edit(&Press::of(Key::Escape).repeating(), None), None);
    }

    #[test]
    fn control_characters_tab_and_keys_that_write_nothing_do_not_type() {
        assert_eq!(typed("\u{0}"), None);
        assert_eq!(typed("\u{7f}"), None);
        assert_eq!(typed("\n"), None);
        assert_eq!(edit(&Press::of(Key::Tab), Some("\t")), None);
        assert_eq!(edit(&Press::of(Key::Other), None), None);
        assert_eq!(
            edit(&Press::of(Key::Other), Some("a\u{0}b")),
            Some(Edit::Put("ab".to_owned()))
        );
        // Markup is characters like any others.
        assert_eq!(typed("<"), Some(Edit::Put("<".to_owned())));
    }
}
