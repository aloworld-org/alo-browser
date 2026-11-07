/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's key does to alo's own fields — the one box on the first
//! screen and *Search chats* (ADR 0074, queue items 701 and 861,
//! `docs/design/shell.md`, *The one box as a text field*).
//!
//! The conversation's window reads every key going down as a
//! [`crate::keys::Press`], with the text the window server says it writes,
//! and asks [`edit`] what it does to the field that has the keyboard. The
//! browser process — which alone holds what is typed
//! ([`crate::text_field`]) — makes the edit. No key reaches a renderer: the
//! field's renderer is sent what the field then holds, as data.
//!
//! # Which keys, on a Mac
//!
//! | | |
//! |---|---|
//! | Return | chooses what is offered first; held, not again |
//! | Escape | empties the field (and, empty, lets the keyboard go) |
//! | ← → | the caret a letter back or on; ⌥ a word; ⌘ to the start or end |
//! | ↑ ↓ | the caret to the start or the end, as a one-line field has them |
//! | ⇧ with any of those | the selection grown or shrunk instead |
//! | Backspace | the letter before the caret, or what is selected; ⌥ the word before; ⌘ all before |
//! | ⌦ (fn-Backspace) | the letter after; ⌥ the word after; ⌘ all after |
//! | ⌘A | select all |
//! | ⌘C ⌘X ⌘V | copy, cut, paste — the system's clipboard |
//! | ⌘Z, ⇧⌘Z | undo, and do again |
//! | anything else | the text it writes, less control characters |
//!
//! **macOS's own Ctrl bindings** (queue item 865), as its text system's
//! standard key bindings have them for a one-line field:
//!
//! | | |
//! |---|---|
//! | ⌃A ⌃E | the caret to the line's start, or its end |
//! | ⌃B ⌃F | a letter back, or on, as ← → |
//! | ⌃P ⌃N | the line above or below: a one-line field's start or end, as ↑ ↓ |
//! | ⌃H ⌃D | the letter before, or after, as Backspace and ⌦ |
//! | ⌃K | kill: from the caret — or the selection's start — to the end, kept for ⌃Y and never put on the clipboard |
//! | ⌃Y | yank: what was last killed, in either field, at the caret |
//! | ⌃T | the two letters about the caret swapped, and the caret on; at the end, the last two |
//!
//! **Every other key held with ⌘ is the browser's or the system's, and
//! types nothing** — ⌘T, ⌘W, ⌘Q, ⌘, among them — and **nothing else held
//! with Ctrl types**: ⌃ with ⇧, ⌥ or ⌘ too, and ⌃O, ⌃L and ⌃V, which mean
//! nothing in a one-line field. Tab does not type either: the design has
//! it ask the agent. Home, End and the paging keys type nothing; with the
//! field empty they scroll the conversation (queue item 754).
//!
//! The six ⌘ keys a field takes are read **only while a field has the
//! keyboard**: with none, they go nowhere, and with the card's page holding
//! it, they are the page's ([`crate::card_keys`]).
//!
//! Elsewhere than on a Mac the same keys are Ctrl's — Ctrl+A, C, X, V, Z
//! and Ctrl+Y or Ctrl+Shift+Z — Ctrl with an arrow moves by a word, and
//! Home and End to the edges.
//!
//! What an input method composes reaches the field as the window server's
//! `Ime` events, never as these (queue item 848).

use crate::keys::{Arrow, Held, Key, Modifier, Platform, Press};
use crate::text_field::{Toward, Unit};

/// What a key does to a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Type this text at the caret, over what is selected.
    Put(String),
    /// Erase this far this way, or what is selected.
    Erase(Unit, Toward),
    /// Move the caret this far this way — with `true`, the selection's
    /// moving end, as ⇧ does.
    Move(Unit, Toward, bool),
    /// Select every character.
    SelectAll,
    /// Put what is selected on the clipboard.
    Copy,
    /// Put what is selected on the clipboard, and take it out.
    Cut,
    /// Put the clipboard's text in, which the window reads
    /// ([`Edit::Pasted`]).
    Paste,
    /// This text, read from the clipboard, pasted at the caret.
    Pasted(String),
    /// Undo the last step.
    Undo,
    /// Do again the step last undone.
    Redo,
    /// Kill from the caret, or the selection's start, to the end, as ⌃K
    /// does: kept by the window for [`Edit::Yank`], never the clipboard.
    Kill,
    /// Put in what was last killed, which the window holds
    /// ([`Edit::Yanked`]).
    Yank,
    /// This text, last killed, put in at the caret, over what is selected.
    Yanked(String),
    /// Swap the letters about the caret, as ⌃T does.
    Transpose,
    /// Empty the field.
    Clear,
    /// Choose what is offered first.
    Choose,
}

/// What `press`, which the window server says writes `written`, does to a
/// field on this machine, if anything.
pub fn edit(press: &Press, written: Option<&str>) -> Option<Edit> {
    edit_on(press, written, Platform::HERE)
}

/// What `press` does to a field on `platform`.
pub fn edit_on(press: &Press, written: Option<&str>, platform: Platform) -> Option<Edit> {
    let held = press.held;
    let shift = held.holds(Modifier::Shift);
    let option = held.holds(Modifier::Option);
    let command = held.holds(Modifier::Command);
    let control = held.holds(Modifier::Control);
    // The key that stands for ⌘ here: ⌘ on a Mac, Ctrl elsewhere.
    let (shortcut, other) = match platform {
        Platform::Mac => (command, control),
        Platform::Elsewhere => (control, command),
    };
    if other {
        // A Mac's Ctrl bindings, with nothing else held (queue item 865).
        return match (platform, &press.key) {
            (Platform::Mac, Key::Character(letter))
                if held == Held::NOTHING.with(Modifier::Control) =>
            {
                control_binding(letter)
            }
            _ => None,
        };
    }
    // By a word: ⌥ on a Mac, Ctrl elsewhere (which is the shortcut key).
    let (word, edge) = match platform {
        Platform::Mac => (option, command),
        Platform::Elsewhere => (shortcut, false),
    };
    let unit = if edge {
        Unit::Edge
    } else if word {
        Unit::Word
    } else {
        Unit::Character
    };
    match &press.key {
        Key::Arrow(arrow) => {
            let (unit, toward) = match arrow {
                Arrow::Left => (unit, Toward::Start),
                Arrow::Right => (unit, Toward::End),
                Arrow::Up => (Unit::Edge, Toward::Start),
                Arrow::Down => (Unit::Edge, Toward::End),
            };
            Some(Edit::Move(unit, toward, shift))
        }
        Key::Backspace => Some(Edit::Erase(unit, Toward::Start)),
        Key::Delete => Some(Edit::Erase(unit, Toward::End)),
        Key::Home | Key::End if platform == Platform::Elsewhere && !shortcut => {
            let toward = if press.key == Key::Home {
                Toward::Start
            } else {
                Toward::End
            };
            Some(Edit::Move(Unit::Edge, toward, shift))
        }
        // Held down, neither chooses nor empties again.
        Key::Return | Key::Escape if press.repeat => None,
        Key::Return | Key::Escape if shortcut => None,
        Key::Return => Some(Edit::Choose),
        Key::Escape => Some(Edit::Clear),
        Key::Character(letter) if shortcut => shortcut_of(letter, shift, platform),
        _ if shortcut => None,
        Key::Tab | Key::PageUp | Key::PageDown | Key::Home | Key::End | Key::Function(_) => None,
        Key::Character(_) | Key::Space | Key::Other => {
            let text: String = written?.chars().filter(|each| !each.is_control()).collect();
            (!text.is_empty()).then_some(Edit::Put(text))
        }
    }
}

/// What Ctrl with `letter` does to a field on a Mac — the key's letter
/// as the person's layout names it — as macOS's standard key bindings have
/// it (queue item 865).
fn control_binding(letter: &str) -> Option<Edit> {
    match letter.to_lowercase().as_str() {
        // The line's start or end; the line above or below, which in one
        // line are the same.
        "a" | "p" => Some(Edit::Move(Unit::Edge, Toward::Start, false)),
        "e" | "n" => Some(Edit::Move(Unit::Edge, Toward::End, false)),
        "b" => Some(Edit::Move(Unit::Character, Toward::Start, false)),
        "f" => Some(Edit::Move(Unit::Character, Toward::End, false)),
        "h" => Some(Edit::Erase(Unit::Character, Toward::Start)),
        "d" => Some(Edit::Erase(Unit::Character, Toward::End)),
        "k" => Some(Edit::Kill),
        "y" => Some(Edit::Yank),
        "t" => Some(Edit::Transpose),
        _ => None,
    }
}

/// What the shortcut key with `letter` — the key's letter without its
/// modifiers, as the person's layout names it — does to a field.
fn shortcut_of(letter: &str, shift: bool, platform: Platform) -> Option<Edit> {
    match (letter.to_lowercase().as_str(), shift) {
        ("a", false) => Some(Edit::SelectAll),
        ("c", false) => Some(Edit::Copy),
        ("x", false) => Some(Edit::Cut),
        ("v", false) => Some(Edit::Paste),
        ("z", false) => Some(Edit::Undo),
        ("z", true) => Some(Edit::Redo),
        ("y", false) if platform == Platform::Elsewhere => Some(Edit::Redo),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(character: &str) -> Option<Edit> {
        edit_on(&Press::character(character), Some(character), Platform::Mac)
    }

    fn on_a_mac(press: &Press) -> Option<Edit> {
        edit_on(press, None, Platform::Mac)
    }

    #[test]
    fn a_letter_puts_what_it_writes() {
        assert_eq!(typed("e"), Some(Edit::Put("e".to_owned())));
        assert_eq!(typed("."), Some(Edit::Put(".".to_owned())));
        assert_eq!(typed(" "), Some(Edit::Put(" ".to_owned())));
        // With Shift, the capital the window server says it writes.
        assert_eq!(
            edit_on(
                &Press::character("e").with(Modifier::Shift),
                Some("E"),
                Platform::Mac
            ),
            Some(Edit::Put("E".to_owned()))
        );
        // With ⌥ on a Mac, what ⌥ makes of it.
        assert_eq!(
            edit_on(
                &Press::character("p").with(Modifier::Option),
                Some("π"),
                Platform::Mac
            ),
            Some(Edit::Put("π".to_owned()))
        );
        // Held down, it goes on typing.
        assert_eq!(
            edit_on(&Press::character("e").repeating(), Some("e"), Platform::Mac),
            Some(Edit::Put("e".to_owned()))
        );
    }

    #[test]
    fn the_fields_six_command_keys_and_no_others() {
        let command = |letter: &str| Press::character(letter).with(Modifier::Command);
        assert_eq!(on_a_mac(&command("a")), Some(Edit::SelectAll));
        assert_eq!(on_a_mac(&command("c")), Some(Edit::Copy));
        assert_eq!(on_a_mac(&command("x")), Some(Edit::Cut));
        assert_eq!(on_a_mac(&command("v")), Some(Edit::Paste));
        assert_eq!(on_a_mac(&command("z")), Some(Edit::Undo));
        assert_eq!(
            on_a_mac(&command("z").with(Modifier::Shift)),
            Some(Edit::Redo)
        );
        // A layout's capital names the same key.
        assert_eq!(on_a_mac(&command("A")), Some(Edit::SelectAll));
        // The browser's and the system's: nothing typed, nothing edited.
        for letter in ["t", "w", "q", ",", "n", "y", "h", "l"] {
            assert_eq!(
                edit_on(&command(letter), Some(letter), Platform::Mac),
                None,
                "⌘{letter}"
            );
        }
        assert_eq!(on_a_mac(&command("a").with(Modifier::Shift)), None);
        assert_eq!(
            on_a_mac(&Press::of(Key::Return).with(Modifier::Command)),
            None
        );
        assert_eq!(on_a_mac(&Press::of(Key::Tab).with(Modifier::Command)), None);
    }

    #[test]
    fn macs_control_bindings_move_erase_kill_yank_and_transpose() {
        // Queue item 865: the text system's standard key bindings, with
        // the letter a layout names the key (the window server's written
        // text for ⌃A is a control character, never typed).
        let control = |letter: &str| {
            edit_on(
                &Press::character(letter).with(Modifier::Control),
                Some("\u{1}"),
                Platform::Mac,
            )
        };
        let moved = |unit, toward| Some(Edit::Move(unit, toward, false));
        assert_eq!(control("a"), moved(Unit::Edge, Toward::Start));
        assert_eq!(control("e"), moved(Unit::Edge, Toward::End));
        assert_eq!(control("b"), moved(Unit::Character, Toward::Start));
        assert_eq!(control("f"), moved(Unit::Character, Toward::End));
        assert_eq!(control("p"), moved(Unit::Edge, Toward::Start));
        assert_eq!(control("n"), moved(Unit::Edge, Toward::End));
        assert_eq!(
            control("h"),
            Some(Edit::Erase(Unit::Character, Toward::Start))
        );
        assert_eq!(
            control("d"),
            Some(Edit::Erase(Unit::Character, Toward::End))
        );
        assert_eq!(control("k"), Some(Edit::Kill));
        assert_eq!(control("y"), Some(Edit::Yank));
        assert_eq!(control("t"), Some(Edit::Transpose));
        // A layout's capital names the same key; held, it goes on.
        assert_eq!(control("E"), moved(Unit::Edge, Toward::End));
        assert_eq!(
            on_a_mac(&Press::character("f").with(Modifier::Control).repeating()),
            moved(Unit::Character, Toward::End)
        );
    }

    #[test]
    fn nothing_else_held_with_control_types_on_a_mac() {
        for letter in ["o", "l", "v", "w", "q", "1", " "] {
            let press = Press::character(letter).with(Modifier::Control);
            assert_eq!(
                edit_on(&press, Some(letter), Platform::Mac),
                None,
                "⌃{letter}"
            );
        }
        // With anything else held too, nothing.
        for also in [Modifier::Shift, Modifier::Option, Modifier::Command] {
            let press = Press::character("a").with(Modifier::Control).with(also);
            assert_eq!(on_a_mac(&press), None, "{also:?}");
        }
        assert_eq!(
            on_a_mac(&Press::of(Key::Arrow(Arrow::Left)).with(Modifier::Control)),
            None
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Backspace).with(Modifier::Control)),
            None
        );
        // Elsewhere Ctrl is the shortcut key: ⌃K and ⌃E are nothing there.
        let elsewhere = |letter: &str| {
            edit_on(
                &Press::character(letter).with(Modifier::Control),
                None,
                Platform::Elsewhere,
            )
        };
        assert_eq!(elsewhere("k"), None);
        assert_eq!(elsewhere("e"), None);
    }

    #[test]
    fn arrows_move_by_a_letter_a_word_or_to_an_edge_and_shift_selects() {
        let arrow = |arrow: Arrow| Press::of(Key::Arrow(arrow));
        assert_eq!(
            on_a_mac(&arrow(Arrow::Left)),
            Some(Edit::Move(Unit::Character, Toward::Start, false))
        );
        assert_eq!(
            on_a_mac(&arrow(Arrow::Right).with(Modifier::Option)),
            Some(Edit::Move(Unit::Word, Toward::End, false))
        );
        assert_eq!(
            on_a_mac(&arrow(Arrow::Left).with(Modifier::Command)),
            Some(Edit::Move(Unit::Edge, Toward::Start, false))
        );
        assert_eq!(
            on_a_mac(
                &arrow(Arrow::Left)
                    .with(Modifier::Option)
                    .with(Modifier::Shift)
            ),
            Some(Edit::Move(Unit::Word, Toward::Start, true))
        );
        assert_eq!(
            on_a_mac(&arrow(Arrow::Up)),
            Some(Edit::Move(Unit::Edge, Toward::Start, false))
        );
        assert_eq!(
            on_a_mac(&arrow(Arrow::Down).with(Modifier::Shift)),
            Some(Edit::Move(Unit::Edge, Toward::End, true))
        );
        // Held, an arrow goes on.
        assert_eq!(
            on_a_mac(&arrow(Arrow::Right).repeating()),
            Some(Edit::Move(Unit::Character, Toward::End, false))
        );
    }

    #[test]
    fn backspace_and_delete_erase_by_a_letter_a_word_or_to_an_edge() {
        assert_eq!(
            on_a_mac(&Press::of(Key::Backspace)),
            Some(Edit::Erase(Unit::Character, Toward::Start))
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Backspace).with(Modifier::Option)),
            Some(Edit::Erase(Unit::Word, Toward::Start))
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Backspace).with(Modifier::Command)),
            Some(Edit::Erase(Unit::Edge, Toward::Start))
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Delete)),
            Some(Edit::Erase(Unit::Character, Toward::End))
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Delete).with(Modifier::Option)),
            Some(Edit::Erase(Unit::Word, Toward::End))
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Backspace).repeating()),
            Some(Edit::Erase(Unit::Character, Toward::Start))
        );
    }

    #[test]
    fn return_chooses_and_escape_empties_once_each() {
        assert_eq!(
            edit_on(&Press::of(Key::Return), Some("\r"), Platform::Mac),
            Some(Edit::Choose)
        );
        assert_eq!(
            edit_on(&Press::of(Key::Escape), Some("\u{1b}"), Platform::Mac),
            Some(Edit::Clear)
        );
        assert_eq!(on_a_mac(&Press::of(Key::Return).repeating()), None);
        assert_eq!(on_a_mac(&Press::of(Key::Escape).repeating()), None);
    }

    #[test]
    fn control_characters_tab_and_keys_that_write_nothing_do_not_type() {
        assert_eq!(typed("\u{0}"), None);
        assert_eq!(typed("\u{7f}"), None);
        assert_eq!(typed("\n"), None);
        assert_eq!(
            edit_on(&Press::of(Key::Tab), Some("\t"), Platform::Mac),
            None
        );
        assert_eq!(on_a_mac(&Press::of(Key::Other)), None);
        assert_eq!(
            edit_on(&Press::of(Key::Other), Some("a\u{0}b"), Platform::Mac),
            Some(Edit::Put("ab".to_owned()))
        );
        assert_eq!(typed("<"), Some(Edit::Put("<".to_owned())));
        assert_eq!(
            edit_on(&Press::of(Key::Space), Some(" "), Platform::Mac),
            Some(Edit::Put(" ".to_owned()))
        );
        for key in [
            Key::PageUp,
            Key::PageDown,
            Key::Home,
            Key::End,
            Key::Function(5),
        ] {
            assert_eq!(
                edit_on(&Press::of(key), Some("\u{f72c}"), Platform::Mac),
                None
            );
        }
    }

    #[test]
    fn elsewhere_the_same_keys_are_controls() {
        let control = |letter: &str| Press::character(letter).with(Modifier::Control);
        let elsewhere = |press: &Press| edit_on(press, None, Platform::Elsewhere);
        assert_eq!(elsewhere(&control("a")), Some(Edit::SelectAll));
        assert_eq!(elsewhere(&control("v")), Some(Edit::Paste));
        assert_eq!(elsewhere(&control("y")), Some(Edit::Redo));
        assert_eq!(
            elsewhere(&Press::of(Key::Arrow(Arrow::Left)).with(Modifier::Control)),
            Some(Edit::Move(Unit::Word, Toward::Start, false))
        );
        assert_eq!(
            elsewhere(&Press::of(Key::End).with(Modifier::Shift)),
            Some(Edit::Move(Unit::Edge, Toward::End, true))
        );
        assert_eq!(
            elsewhere(&Press::character("a").with(Modifier::Command)),
            None
        );
    }
}
