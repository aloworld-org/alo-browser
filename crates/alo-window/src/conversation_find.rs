/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Search chats*, as the browser process holds it (ADR 0074, queue item
//! 711): whether it has the keyboard, and what a person typed into it.
//!
//! What is typed is held **here and nowhere else**, as the one box's is
//! ([`crate::conversation_box`]). The panel's renderer is sent it as data,
//! to show as the field's value, with only the chats whose names hold it
//! (`alo-renderer`'s `panel::matching`) — so which chats a person sees, and
//! which chat a press on a row names, is decided by the browser process,
//! never by a renderer. The first screen's renderer is never sent it, and
//! no page is.
//!
//! A press on *Search chats* gives it the keyboard and takes it from the
//! box and the card's page; a press anywhere else takes it away
//! ([`crate::conversation::Conversation::act`]). Keys edit it as they edit
//! the box ([`crate::typing`]): text at the end, Backspace the last
//! character, Escape empties it — and, empty, lets the keyboard go. Return
//! chooses nothing: the chats found are already what is shown.

use crate::typing::Edit;
use alo_renderer::typed::LONGEST_TYPED;

/// *Search chats*.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindChats {
    /// What a person has typed, at most [`LONGEST_TYPED`] characters.
    typed: String,
    /// Whether it has the keyboard.
    focused: bool,
}

impl FindChats {
    /// What is typed.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// Whether it has the keyboard.
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Give it the keyboard: whether that changed anything.
    pub fn focus(&mut self) -> bool {
        !std::mem::replace(&mut self.focused, true)
    }

    /// Take the keyboard from it: whether that changed anything.
    pub fn blur(&mut self) -> bool {
        std::mem::replace(&mut self.focused, false)
    }

    /// Make `edit`, if it has the keyboard: whether what is typed, or
    /// whether it has the keyboard, changed.
    pub fn edit(&mut self, edit: Edit) -> bool {
        if !self.focused {
            return false;
        }
        match edit {
            Edit::Put(text) => {
                let room = LONGEST_TYPED.saturating_sub(self.typed.chars().count());
                let before = self.typed.len();
                self.typed.extend(text.chars().take(room));
                self.typed.len() != before
            }
            Edit::Erase => self.typed.pop().is_some(),
            // Empty, Escape lets the keyboard go.
            Edit::Clear if self.typed.is_empty() => self.blur(),
            Edit::Clear => {
                self.typed.clear();
                true
            }
            Edit::Choose => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_edit_it_only_while_it_has_the_keyboard() {
        let mut find = FindChats::default();
        assert!(!find.edit(Edit::Put("a".to_owned())));
        assert_eq!(find.typed(), "");
        assert!(find.focus());
        assert!(!find.focus(), "already had it");
        assert!(find.edit(Edit::Put("lis".to_owned())));
        assert!(find.edit(Edit::Put("bon".to_owned())));
        assert_eq!(find.typed(), "lisbon");
        assert!(find.edit(Edit::Erase));
        assert_eq!(find.typed(), "lisbo");
        assert!(!find.edit(Edit::Choose), "Return chooses nothing");
        // Losing the keyboard keeps the words, as a field's value.
        assert!(find.blur());
        assert!(!find.edit(Edit::Erase));
        assert_eq!(find.typed(), "lisbo");
    }

    #[test]
    fn escape_empties_it_and_then_lets_the_keyboard_go() {
        let mut find = FindChats::default();
        let _ = find.focus();
        let _ = find.edit(Edit::Put("march".to_owned()));
        assert!(find.edit(Edit::Clear));
        assert_eq!(find.typed(), "");
        assert!(find.focused());
        assert!(find.edit(Edit::Clear));
        assert!(!find.focused());
        assert!(!find.edit(Edit::Erase), "nothing to erase, no keyboard");
    }

    #[test]
    fn it_holds_no_more_than_the_box_does() {
        let mut find = FindChats::default();
        let _ = find.focus();
        let _ = find.edit(Edit::Put("x".repeat(LONGEST_TYPED + 10)));
        assert_eq!(find.typed().chars().count(), LONGEST_TYPED);
        assert!(!find.edit(Edit::Put("y".to_owned())));
    }
}
