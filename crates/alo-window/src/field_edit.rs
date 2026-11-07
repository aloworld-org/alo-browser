/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an edit makes of one of alo's fields (queue item 861): the one
//! road from a key's [`Edit`] ([`crate::typing`]) to the field it is made
//! to ([`crate::text_field`]), taken by the one box and by *Search chats*
//! alike, so the two can never come to edit differently.
//!
//! Choosing and an empty field's Escape are not edits of the words, and
//! are the caller's: the box chooses what it offers, *Search chats*
//! chooses nothing, and either lets the keyboard go.

use crate::text_field::TextField;
use crate::typing::Edit;

/// What an edit made of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Made {
    /// Nothing changed.
    Nothing,
    /// The words, the caret or the selection changed: drawn again.
    Changed,
    /// This, selected, is to go on the clipboard; nothing changed.
    Copy(String),
    /// This, selected, is to go on the clipboard, and was taken out.
    Cut(String),
    /// Not an edit of the words: the caller's — [`Edit::Choose`], or
    /// [`Edit::Clear`] of an empty field.
    Theirs(Edit),
}

/// Make `edit` to `field`.
pub fn make(field: &mut TextField, edit: Edit) -> Made {
    let changed = match edit {
        Edit::Put(text) => field.put(&text),
        Edit::Pasted(text) => field.paste(&text),
        Edit::Erase(unit, toward) => field.erase(unit, toward),
        Edit::Move(unit, toward, extend) => field.go(unit, toward, extend),
        Edit::SelectAll => field.select_all(),
        Edit::Kill => field.kill(),
        Edit::Yanked(text) => field.yank(&text),
        Edit::Transpose => field.transpose(),
        Edit::Undo => field.undo(),
        Edit::Redo => field.redo(),
        Edit::Copy => {
            let selected = field.selected();
            return if selected.is_empty() {
                Made::Nothing
            } else {
                Made::Copy(selected)
            };
        }
        Edit::Cut => return field.cut().map_or(Made::Nothing, Made::Cut),
        // The clipboard is read by the window, which makes it a paste of
        // what it read; and what was killed is held by the window, which
        // makes ⌃Y a yank of what it holds (queue item 865).
        Edit::Paste | Edit::Yank => false,
        Edit::Clear if field.text().is_empty() => return Made::Theirs(Edit::Clear),
        Edit::Clear => field.clear(),
        Edit::Choose => return Made::Theirs(Edit::Choose),
    };
    if changed {
        Made::Changed
    } else {
        Made::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_field::{Toward, Unit};

    #[test]
    fn each_edit_reaches_the_field() {
        let mut field = TextField::default();
        assert_eq!(
            make(&mut field, Edit::Put("kampala".to_owned())),
            Made::Changed
        );
        assert_eq!(
            make(&mut field, Edit::Copy),
            Made::Nothing,
            "nothing selected"
        );
        assert_eq!(
            make(&mut field, Edit::Move(Unit::Word, Toward::Start, true)),
            Made::Changed
        );
        assert_eq!(
            make(&mut field, Edit::Copy),
            Made::Copy("kampala".to_owned())
        );
        assert_eq!(make(&mut field, Edit::Cut), Made::Cut("kampala".to_owned()));
        assert_eq!(field.text(), "");
        assert_eq!(make(&mut field, Edit::Paste), Made::Nothing, "the window's");
        assert_eq!(
            make(&mut field, Edit::Pasted("x\ny".to_owned())),
            Made::Changed
        );
        assert_eq!(field.text(), "x y");
        assert_eq!(make(&mut field, Edit::Undo), Made::Changed);
        assert_eq!(field.text(), "");
        assert_eq!(make(&mut field, Edit::Redo), Made::Changed);
        assert_eq!(make(&mut field, Edit::SelectAll), Made::Changed);
        assert_eq!(
            make(&mut field, Edit::Erase(Unit::Character, Toward::Start)),
            Made::Changed
        );
        assert_eq!(make(&mut field, Edit::Clear), Made::Theirs(Edit::Clear));
        assert_eq!(make(&mut field, Edit::Choose), Made::Theirs(Edit::Choose));
        let _ = make(&mut field, Edit::Put("a".to_owned()));
        assert_eq!(make(&mut field, Edit::Clear), Made::Changed);
        // The Ctrl bindings (queue item 865).
        let _ = make(&mut field, Edit::Put("ab".to_owned()));
        assert_eq!(make(&mut field, Edit::Transpose), Made::Changed);
        assert_eq!(field.text(), "ba");
        assert_eq!(
            make(&mut field, Edit::Move(Unit::Edge, Toward::Start, false)),
            Made::Changed
        );
        assert_eq!(make(&mut field, Edit::Kill), Made::Changed);
        assert_eq!(field.text(), "");
        assert_eq!(make(&mut field, Edit::Yank), Made::Nothing, "the window's");
        assert_eq!(
            make(&mut field, Edit::Yanked("ba".to_owned())),
            Made::Changed
        );
        assert_eq!(field.text(), "ba");
    }
}
