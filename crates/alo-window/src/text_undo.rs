/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What ⌘Z undoes and ⇧⌘Z does again in one of alo's fields (queue item
//! 861, `docs/design/shell.md`, *The one box as a text field*).
//!
//! # A step is what macOS's fields make one
//!
//! An edit is a step of its own unless it goes on the step before it, as
//! macOS's text system groups them:
//!
//! - **Typing** goes on while each letter goes in where the one before left
//!   the caret: a word, a sentence, an address typed straight through is one
//!   step. A letter typed over a selection begins a new one.
//! - **Erasing** with Backspace goes on with Backspace, and the forward
//!   delete with the forward delete, each from where the one before left
//!   the caret; ⌥ and ⌘ with them count as erasing too.
//! - **Anything else** — a paste, a cut, the box emptied with Escape, an
//!   input method's commit over a selection — is a step alone, and so is the
//!   first edit after the caret was moved, the selection changed or a press
//!   placed it: moving **closes** the step being grouped.
//!
//! What a step holds is the field as it was before it — its words, its caret
//! and its anchor — so undoing puts the words back and the caret where it
//! was; doing it again puts back the field as it was after. A new edit after
//! an undo forgets what could have been done again.
//!
//! At most [`MOST_STEPS`] are kept, the oldest forgotten first: a field
//! holds at most 2,048 characters, so the history is bounded at a few
//! hundred kilobytes however long a person types.

/// The most steps kept to undo.
pub const MOST_STEPS: usize = 100;

/// A field as it stood: its words, its caret and its anchor, in characters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stood {
    /// Its words.
    pub text: String,
    /// Where its caret was.
    pub caret: usize,
    /// Where its selection began.
    pub anchor: usize,
}

/// What kind of edit was made, as steps are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Letters typed, or an input method's words put in.
    Typing,
    /// Backspace, with or without ⌥ or ⌘.
    ErasingBack,
    /// The forward delete, with or without ⌥ or ⌘.
    ErasingOn,
    /// A step alone: a paste, a cut, the field emptied.
    Alone,
}

/// The history of one field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    /// What can be undone, oldest first: the field before each step.
    done: Vec<Stood>,
    /// What can be done again, most recently undone last: the field after
    /// each step undone.
    undone: Vec<Stood>,
    /// The step being grouped, and where it left the caret, if one is
    /// open.
    open: Option<(Kind, usize)>,
}

impl History {
    /// An edit of `kind` is about to be made to the field standing as
    /// `before`, its caret at `before.caret`: kept as a step of its own
    /// unless it goes on the step open. `selecting` says whether it
    /// replaces a selection, which always begins a step.
    pub fn editing(&mut self, kind: Kind, before: &Stood, selecting: bool) {
        let goes_on = !selecting
            && kind != Kind::Alone
            && self
                .open
                .is_some_and(|(open, left)| open == kind && left == before.caret);
        if !goes_on {
            self.done.push(before.clone());
            if self.done.len() > MOST_STEPS {
                let _ = self.done.remove(0);
            }
        }
        self.undone.clear();
        self.open = None;
    }

    /// The edit made left the caret at `caret`: a step of `kind` stays open
    /// for the next to go on, unless it is a step alone.
    pub fn edited(&mut self, kind: Kind, caret: usize) {
        self.open = (kind != Kind::Alone).then_some((kind, caret));
    }

    /// The caret was moved, the selection changed or a press placed it:
    /// the next edit begins a step.
    pub fn close(&mut self) {
        self.open = None;
    }

    /// Undo the last step of the field standing as `now`: how it stood
    /// before that step, if there was one.
    pub fn undo(&mut self, now: &Stood) -> Option<Stood> {
        let before = self.done.pop()?;
        self.undone.push(now.clone());
        self.open = None;
        Some(before)
    }

    /// Do again the step last undone of the field standing as `now`: how it
    /// stood after that step, if one was undone.
    pub fn redo(&mut self, now: &Stood) -> Option<Stood> {
        let after = self.undone.pop()?;
        self.done.push(now.clone());
        self.open = None;
        Some(after)
    }

    /// Forget every step: the field begins again, as it does once what was
    /// in it was chosen.
    pub fn forget(&mut self) {
        *self = Self::default();
    }

    /// How many steps can be undone.
    pub fn steps(&self) -> usize {
        self.done.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stood(text: &str, caret: usize) -> Stood {
        Stood {
            text: text.to_owned(),
            caret,
            anchor: caret,
        }
    }

    /// Type `text` letter by letter into `field`, recording each.
    fn type_in(history: &mut History, field: &mut Stood, text: &str) {
        for letter in text.chars() {
            history.editing(Kind::Typing, field, false);
            field.text.push(letter);
            field.caret += 1;
            field.anchor = field.caret;
            history.edited(Kind::Typing, field.caret);
        }
    }

    #[test]
    fn typing_straight_through_is_one_step() {
        let mut history = History::default();
        let mut field = stood("", 0);
        type_in(&mut history, &mut field, "kampala city");
        assert_eq!(history.steps(), 1);
        assert_eq!(history.undo(&field), Some(stood("", 0)));
        assert_eq!(history.redo(&stood("", 0)), Some(stood("kampala city", 12)));
    }

    #[test]
    fn moving_the_caret_closes_a_step_and_erasing_is_another() {
        let mut history = History::default();
        let mut field = stood("", 0);
        type_in(&mut history, &mut field, "abc");
        history.close();
        type_in(&mut history, &mut field, "d");
        assert_eq!(history.steps(), 2, "moved, then typed: two steps");
        for _ in 0..2 {
            history.editing(Kind::ErasingBack, &field, false);
            let _ = field.text.pop();
            field.caret -= 1;
            field.anchor = field.caret;
            history.edited(Kind::ErasingBack, field.caret);
        }
        assert_eq!(history.steps(), 3, "two Backspaces are one step");
        assert_eq!(history.undo(&field), Some(stood("abcd", 4)));
    }

    #[test]
    fn a_paste_is_alone_and_a_new_edit_forgets_what_was_undone() {
        let mut history = History::default();
        let mut field = stood("", 0);
        history.editing(Kind::Alone, &field, false);
        field = stood("pasted", 6);
        history.edited(Kind::Alone, 6);
        type_in(&mut history, &mut field, "!");
        assert_eq!(history.steps(), 2, "a paste, then typing");
        let back = history.undo(&field);
        assert_eq!(back, Some(stood("pasted", 6)));
        field = stood("pasted", 6);
        type_in(&mut history, &mut field, "?");
        assert_eq!(history.redo(&field), None, "forgotten");
    }

    #[test]
    fn typing_over_a_selection_begins_a_step_and_at_most_a_hundred_are_kept() {
        let mut history = History::default();
        let mut field = stood("", 0);
        type_in(&mut history, &mut field, "ab");
        history.editing(Kind::Typing, &field, true);
        history.edited(Kind::Typing, 3);
        assert_eq!(history.steps(), 2);
        for each in 0..150 {
            history.close();
            history.editing(Kind::Typing, &stood(&each.to_string(), 0), false);
        }
        assert_eq!(history.steps(), MOST_STEPS);
        history.forget();
        assert_eq!(history.steps(), 0);
        assert_eq!(history.undo(&field), None);
    }
}
