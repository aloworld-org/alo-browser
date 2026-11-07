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
//! the box ([`crate::typing`], [`crate::field_edit`]): its caret moved, its
//! words selected, cut, copied, pasted and undone (queue item 861); Escape
//! empties it — and, empty, lets the keyboard go. Return chooses nothing:
//! the chats found are already what is shown.

use crate::field_edit::{self, Made};
use crate::text_field::TextField;
use crate::typing::Edit;

/// *Search chats*.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindChats {
    /// What a person has typed, at most
    /// [`alo_renderer::typed::LONGEST_TYPED`] characters, with its caret,
    /// its selection and what can be undone ([`crate::text_field`], queue
    /// item 861).
    field: TextField,
    /// Whether it has the keyboard.
    focused: bool,
}

impl FindChats {
    /// What is typed.
    pub fn typed(&self) -> &str {
        self.field.text()
    }

    /// The field itself: its caret, its selection, a composition.
    pub fn field(&self) -> &TextField {
        &self.field
    }

    /// The field, to place its caret or compose in, while it has the
    /// keyboard.
    pub fn field_mut(&mut self) -> Option<&mut TextField> {
        self.focused.then_some(&mut self.field)
    }

    /// Whether it has the keyboard.
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Give it the keyboard: whether that changed anything.
    pub fn focus(&mut self) -> bool {
        !std::mem::replace(&mut self.focused, true)
    }

    /// Take the keyboard from it: whether that changed anything. A
    /// composition is put away with it.
    pub fn blur(&mut self) -> bool {
        let _ = self.field.compose("", 0);
        std::mem::replace(&mut self.focused, false)
    }

    /// Make `edit`, if it has the keyboard ([`crate::field_edit`]): what it
    /// made. Return chooses nothing, and an empty field's Escape lets the
    /// keyboard go.
    pub fn edit(&mut self, edit: Edit) -> Made {
        if !self.focused {
            return Made::Nothing;
        }
        match field_edit::make(&mut self.field, edit) {
            Made::Theirs(Edit::Clear) if self.blur() => Made::Changed,
            Made::Theirs(_) => Made::Nothing,
            made => made,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_field::{Toward, Unit};
    use alo_renderer::typed::LONGEST_TYPED;

    fn changed(made: &Made) -> bool {
        *made != Made::Nothing
    }

    #[test]
    fn keys_edit_it_only_while_it_has_the_keyboard() {
        let mut find = FindChats::default();
        assert!(!changed(&find.edit(Edit::Put("a".to_owned()))));
        assert_eq!(find.typed(), "");
        assert!(find.focus());
        assert!(!find.focus(), "already had it");
        assert!(changed(&find.edit(Edit::Put("lis".to_owned()))));
        assert!(changed(&find.edit(Edit::Put("bon".to_owned()))));
        assert_eq!(find.typed(), "lisbon");
        assert!(changed(
            &find.edit(Edit::Erase(Unit::Character, Toward::Start))
        ));
        assert_eq!(find.typed(), "lisbo");
        assert!(!changed(&find.edit(Edit::Choose)), "Return chooses nothing");
        // Losing the keyboard keeps the words, as a field's value.
        assert!(find.blur());
        assert!(!changed(
            &find.edit(Edit::Erase(Unit::Character, Toward::Start))
        ));
        assert_eq!(find.typed(), "lisbo");
    }

    #[test]
    fn escape_empties_it_and_then_lets_the_keyboard_go() {
        let mut find = FindChats::default();
        let _ = find.focus();
        let _ = find.edit(Edit::Put("march".to_owned()));
        assert!(changed(&find.edit(Edit::Clear)));
        assert_eq!(find.typed(), "");
        assert!(find.focused());
        assert!(changed(&find.edit(Edit::Clear)));
        assert!(!find.focused());
        assert!(
            !changed(&find.edit(Edit::Erase(Unit::Character, Toward::Start))),
            "nothing to erase, no keyboard"
        );
    }

    #[test]
    fn it_holds_no_more_than_the_box_does() {
        let mut find = FindChats::default();
        let _ = find.focus();
        let _ = find.edit(Edit::Put("x".repeat(LONGEST_TYPED + 10)));
        assert_eq!(find.typed().chars().count(), LONGEST_TYPED);
        assert!(!changed(&find.edit(Edit::Put("y".to_owned()))));
    }
}
