/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which editing command a person's key over a card's page carries (queue
//! items 718 and 741): what macOS's own key bindings make of the key in
//! Chrome's own window.
//!
//! On macOS a page's field is not edited by the key itself. Chrome's window
//! asks the system what the key means — the text system's standard key
//! bindings, `StandardKeyBinding.dict`: ← is `moveLeft:`, ⌥← `moveWordLeft:`,
//! ⌘← `moveToLeftEndOfLine:`, ⌦ `deleteForward:` — and hands the page that
//! command beside the key. A key sent over the DevTools pipe is asked of no
//! one, so `Input.dispatchKeyEvent` carries the command itself
//! (`commands`), by the name Chrome's editing knows it, the selector without
//! its colon: as an agent's ⌘A carries `selectAll` and a person's Backspace
//! `deleteBackward` (`Chord`, queue item 718). The page hears `keydown`
//! either way, and a page that cancels it cancels the command.
//!
//! **Measured** in the machine's Chrome 156 (queue item 741): ⌘A sent with
//! no command selected nothing — the letters typed after it went in at the
//! caret — so the shortcuts' commands are what make them work. The arrows
//! and the forward delete moved the caret and erased with no command too,
//! Chrome's own handling of the key doing it; theirs are sent anyway, so
//! that what a key does is what macOS binds it to in Chrome's own window,
//! not the engine's own reading of it.
//!
//! The command is the editing's; where it does nothing — an arrow outside a
//! field — Chrome's own handling of the key goes on, and the page scrolls.
//! Elsewhere than macOS, Chrome reads the keys itself, and none is sent.

use crate::card_input::{Editing, Keyed};
use crate::keys::{Arrow, Held, Modifier};

/// The editing command macOS's own bindings give `key` with `held`, if it
/// has one.
pub fn command(key: &Keyed, held: Held) -> Option<&'static str> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    on_a_mac(key, held)
}

/// What [`command`] answers on macOS, whatever this is built for.
fn on_a_mac(key: &Keyed, held: Held) -> Option<&'static str> {
    let shift = held.holds(Modifier::Shift);
    let option = held.holds(Modifier::Option);
    let command = held.holds(Modifier::Command);
    // Six ways for each arrow: plain, by word or paragraph with ⌥, to the
    // line's end or the document's with ⌘ — each with Shift growing the
    // selection.
    let six = |plain: [&'static str; 2], by: [&'static str; 2], to: [&'static str; 2]| {
        let way = if command {
            to
        } else if option {
            by
        } else {
            plain
        };
        Some(if shift { way[1] } else { way[0] })
    };
    match key {
        Keyed::Arrow(Arrow::Left) => six(
            ["moveLeft", "moveLeftAndModifySelection"],
            ["moveWordLeft", "moveWordLeftAndModifySelection"],
            [
                "moveToLeftEndOfLine",
                "moveToLeftEndOfLineAndModifySelection",
            ],
        ),
        Keyed::Arrow(Arrow::Right) => six(
            ["moveRight", "moveRightAndModifySelection"],
            ["moveWordRight", "moveWordRightAndModifySelection"],
            [
                "moveToRightEndOfLine",
                "moveToRightEndOfLineAndModifySelection",
            ],
        ),
        Keyed::Arrow(Arrow::Up) => six(
            ["moveUp", "moveUpAndModifySelection"],
            [
                "moveToBeginningOfParagraph",
                "moveParagraphBackwardAndModifySelection",
            ],
            [
                "moveToBeginningOfDocument",
                "moveToBeginningOfDocumentAndModifySelection",
            ],
        ),
        Keyed::Arrow(Arrow::Down) => six(
            ["moveDown", "moveDownAndModifySelection"],
            [
                "moveToEndOfParagraph",
                "moveParagraphForwardAndModifySelection",
            ],
            [
                "moveToEndOfDocument",
                "moveToEndOfDocumentAndModifySelection",
            ],
        ),
        // On a Mac Home and End scroll the page, in a field too, and with
        // Shift grow the selection to the document's ends; the page keys
        // scroll a page at a time.
        Keyed::Home if shift => Some("moveToBeginningOfDocumentAndModifySelection"),
        Keyed::Home => Some("scrollToBeginningOfDocument"),
        Keyed::End if shift => Some("moveToEndOfDocumentAndModifySelection"),
        Keyed::End => Some("scrollToEndOfDocument"),
        Keyed::PageUp => Some("scrollPageUp"),
        Keyed::PageDown => Some("scrollPageDown"),
        Keyed::Backspace if command => Some("deleteToBeginningOfLine"),
        Keyed::Backspace if option => Some("deleteWordBackward"),
        Keyed::Backspace => Some("deleteBackward"),
        Keyed::Delete if option => Some("deleteWordForward"),
        Keyed::Delete => Some("deleteForward"),
        Keyed::Editing(editing) => Some(match editing {
            Editing::SelectAll => "selectAll",
            Editing::Copy => "copy",
            Editing::Cut => "cut",
            Editing::Paste => "paste",
            Editing::Undo => "undo",
            Editing::Redo => "redo",
        }),
        Keyed::Text(_) | Keyed::Return | Keyed::Tab | Keyed::Escape | Keyed::Function(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(modifiers: &[Modifier]) -> Held {
        modifiers
            .iter()
            .fold(Held::NOTHING, |held, modifier| held.with(*modifier))
    }

    #[test]
    fn an_arrow_moves_by_a_character_a_word_or_to_the_end_and_shift_selects() {
        let left = Keyed::Arrow(Arrow::Left);
        assert_eq!(on_a_mac(&left, Held::NOTHING), Some("moveLeft"));
        assert_eq!(
            on_a_mac(&left, with(&[Modifier::Shift])),
            Some("moveLeftAndModifySelection")
        );
        assert_eq!(
            on_a_mac(&left, with(&[Modifier::Option])),
            Some("moveWordLeft")
        );
        assert_eq!(
            on_a_mac(&left, with(&[Modifier::Option, Modifier::Shift])),
            Some("moveWordLeftAndModifySelection")
        );
        assert_eq!(
            on_a_mac(&left, with(&[Modifier::Command])),
            Some("moveToLeftEndOfLine")
        );
        assert_eq!(
            on_a_mac(&left, with(&[Modifier::Command, Modifier::Shift])),
            Some("moveToLeftEndOfLineAndModifySelection")
        );
        assert_eq!(
            on_a_mac(&Keyed::Arrow(Arrow::Right), Held::NOTHING),
            Some("moveRight")
        );
        assert_eq!(
            on_a_mac(&Keyed::Arrow(Arrow::Up), with(&[Modifier::Command])),
            Some("moveToBeginningOfDocument")
        );
        assert_eq!(
            on_a_mac(&Keyed::Arrow(Arrow::Down), with(&[Modifier::Option])),
            Some("moveToEndOfParagraph")
        );
    }

    #[test]
    fn home_end_and_the_page_keys_scroll_and_the_deletes_erase() {
        assert_eq!(
            on_a_mac(&Keyed::Home, Held::NOTHING),
            Some("scrollToBeginningOfDocument")
        );
        assert_eq!(
            on_a_mac(&Keyed::End, with(&[Modifier::Shift])),
            Some("moveToEndOfDocumentAndModifySelection")
        );
        assert_eq!(
            on_a_mac(&Keyed::PageDown, Held::NOTHING),
            Some("scrollPageDown")
        );
        assert_eq!(
            on_a_mac(&Keyed::PageUp, Held::NOTHING),
            Some("scrollPageUp")
        );
        assert_eq!(
            on_a_mac(&Keyed::Delete, Held::NOTHING),
            Some("deleteForward")
        );
        assert_eq!(
            on_a_mac(&Keyed::Delete, with(&[Modifier::Option])),
            Some("deleteWordForward")
        );
        assert_eq!(
            on_a_mac(&Keyed::Backspace, Held::NOTHING),
            Some("deleteBackward")
        );
        assert_eq!(
            on_a_mac(&Keyed::Backspace, with(&[Modifier::Option])),
            Some("deleteWordBackward")
        );
        assert_eq!(
            on_a_mac(&Keyed::Backspace, with(&[Modifier::Command])),
            Some("deleteToBeginningOfLine")
        );
    }

    #[test]
    fn the_pages_shortcuts_are_its_editing_and_the_rest_carry_none() {
        assert_eq!(
            on_a_mac(&Keyed::Editing(Editing::SelectAll), Held::NOTHING),
            Some("selectAll")
        );
        assert_eq!(
            on_a_mac(&Keyed::Editing(Editing::Paste), Held::NOTHING),
            Some("paste")
        );
        assert_eq!(
            on_a_mac(&Keyed::Editing(Editing::Redo), Held::NOTHING),
            Some("redo")
        );
        for none in [
            Keyed::Text("a".to_owned()),
            Keyed::Return,
            Keyed::Tab,
            Keyed::Escape,
            Keyed::Function(5),
        ] {
            assert_eq!(on_a_mac(&none, with(&[Modifier::Shift])), None, "{none:?}");
        }
    }

    #[test]
    fn here_it_is_macs_and_only_on_a_mac() {
        let carried = command(&Keyed::Arrow(Arrow::Left), Held::NOTHING);
        if cfg!(target_os = "macos") {
            assert_eq!(carried, Some("moveLeft"));
        } else {
            assert_eq!(carried, None);
        }
    }
}
