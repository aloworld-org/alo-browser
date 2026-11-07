/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One of alo's own one-line fields — the one box and *Search chats* — as
//! the browser process holds it (queue items 861 and 848,
//! `docs/design/shell.md`, *The one box as a text field*): its words, its
//! caret and selection, an input method's composition, and what can be
//! undone.
//!
//! The words are held **here and nowhere else**, as they always were
//! ([`crate::conversation_box`], [`crate::conversation_find`]). A renderer
//! is sent them with where the caret stands ([`TextField::editing`]) and
//! draws them; no key and no press reaches it.
//!
//! # Places
//!
//! Every place is a count of characters from the words' start. The caret
//! is one end of the selection and the **anchor** the other; nothing is
//! selected while they are one place. An arrow moves by a letter and what
//! hangs on it, ⌥ by a word, ⌘ to the start or the end
//! ([`crate::text_stops`]); with ⇧ the anchor stays where it was, and the
//! selection grows or shrinks.
//!
//! # What goes in
//!
//! At most [`LONGEST_TYPED`] characters, as before: what would go past is
//! left out, typed or pasted. A control character never goes in. What is
//! pasted is one line: each line break in it — `\r\n`, `\r`, `\n`, U+2028,
//! U+2029 — and each tab is a space, and the white space at its two ends is
//! dropped, so a line copied with its newline pastes as the line
//! ([`pasted`]).

use crate::text_stops::{after, before, word_at, word_end, word_start};
use crate::text_undo::{History, Kind, Stood};
use alo_renderer::field::{Composing, Editing};
use alo_renderer::typed::LONGEST_TYPED;

/// How far a move or an erase goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A letter and what hangs on it: an arrow, Backspace.
    Character,
    /// A word: ⌥ with an arrow or Backspace.
    Word,
    /// The start or the end of the words: ⌘ with an arrow or Backspace,
    /// ↑ and ↓.
    Edge,
}

/// Which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    /// Toward the start.
    Start,
    /// Toward the end.
    End,
}

/// One field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextField {
    /// Its words.
    text: String,
    /// Where its caret is.
    caret: usize,
    /// Where its selection began.
    anchor: usize,
    /// What an input method is composing at the caret, if anything.
    composing: Option<Composing>,
    /// What can be undone and done again.
    history: History,
}

/// What is pasted, as one line: line breaks and tabs spaces, white space at
/// its ends dropped, control characters left out.
pub fn pasted(text: &str) -> String {
    let joined = text.replace("\r\n", " ");
    let line: String = joined
        .chars()
        .map(|c| match c {
            '\r' | '\n' | '\u{2028}' | '\u{2029}' | '\t' => ' ',
            other => other,
        })
        .filter(|c| !c.is_control())
        .collect();
    line.trim().to_owned()
}

impl TextField {
    /// Its words.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where its caret is, in characters.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The characters selected, start before end, if any are.
    pub fn selection(&self) -> Option<(usize, usize)> {
        (self.caret != self.anchor)
            .then(|| (self.caret.min(self.anchor), self.caret.max(self.anchor)))
    }

    /// The words selected, or nothing.
    pub fn selected(&self) -> String {
        self.selection()
            .map(|(start, end)| self.text.chars().skip(start).take(end - start).collect())
            .unwrap_or_default()
    }

    /// What an input method is composing, if anything.
    pub fn composing(&self) -> Option<&Composing> {
        self.composing.as_ref()
    }

    /// How its caret, selection and composition stand, for its renderer:
    /// the caret not yet shown, nor the words' scroll measured — the window
    /// says those.
    pub fn editing(&self) -> Editing {
        Editing {
            caret: self.caret,
            anchor: self.anchor,
            composing: self.composing.clone(),
            caret_shown: false,
            scrolled: 0.0,
            room: None,
        }
    }

    /// How many characters it holds.
    fn count(&self) -> usize {
        self.text.chars().count()
    }

    /// Its words as characters.
    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }

    /// How it stands, for its history.
    fn stood(&self) -> Stood {
        Stood {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    /// Stand as `stood` says.
    fn stand(&mut self, stood: Stood) {
        self.text = stood.text;
        let count = self.count();
        self.caret = stood.caret.min(count);
        self.anchor = stood.anchor.min(count);
        self.composing = None;
    }

    /// The selection, or the caret where there is none, start before end.
    fn span(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    /// Replace the characters `start` to `end` with `with`, as an edit of
    /// `kind`, the caret after what went in: whether anything changed.
    fn replace(&mut self, (start, end): (usize, usize), with: &str, kind: Kind) -> bool {
        let count = self.count();
        let (start, end) = (start.min(count), end.min(count));
        let room = LONGEST_TYPED.saturating_sub(count - (end - start));
        let with: String = with.chars().take(room).collect();
        if start == end && with.is_empty() {
            return false;
        }
        let selecting = self.caret != self.anchor;
        self.history.editing(kind, &self.stood(), selecting);
        let mut text: String = self.text.chars().take(start).collect();
        text.push_str(&with);
        text.extend(self.text.chars().skip(end));
        self.text = text;
        self.caret = start + with.chars().count();
        self.anchor = self.caret;
        self.composing = None;
        self.history.edited(kind, self.caret);
        true
    }

    /// Type `text` at the caret, over what is selected: whether it went in.
    /// A control character never does.
    pub fn put(&mut self, text: &str) -> bool {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.is_empty() {
            return false;
        }
        self.replace(self.span(), &text, Kind::Typing)
    }

    /// Paste `text` at the caret, over what is selected, as one line
    /// ([`pasted`]): a step of its own to undo. Whether anything changed.
    pub fn paste(&mut self, text: &str) -> bool {
        let line = pasted(text);
        if line.is_empty() && self.selection().is_none() {
            return false;
        }
        self.replace(self.span(), &line, Kind::Alone)
    }

    /// Erase by `unit` toward `toward` from the caret — what is selected,
    /// if anything is: whether anything was erased.
    pub fn erase(&mut self, unit: Unit, toward: Toward) -> bool {
        let kind = match toward {
            Toward::Start => Kind::ErasingBack,
            Toward::End => Kind::ErasingOn,
        };
        if let Some(selected) = self.selection() {
            return self.replace(selected, "", kind);
        }
        let chars = self.chars();
        let at = self.caret;
        let span = match (unit, toward) {
            (Unit::Character, Toward::Start) => (before(&chars, at), at),
            (Unit::Word, Toward::Start) => (word_start(&chars, at), at),
            (Unit::Edge, Toward::Start) => (0, at),
            (Unit::Character, Toward::End) => (at, after(&chars, at)),
            (Unit::Word, Toward::End) => (at, word_end(&chars, at)),
            (Unit::Edge, Toward::End) => (at, chars.len()),
        };
        self.replace(span, "", kind)
    }

    /// Cut what is selected: it, gone from the words, if anything was
    /// selected.
    pub fn cut(&mut self) -> Option<String> {
        let selected = self.selected();
        let span = self.selection()?;
        self.replace(span, "", Kind::Alone).then_some(selected)
    }

    /// Empty it, as Escape does, a step to undo: whether it held anything.
    pub fn clear(&mut self) -> bool {
        self.replace((0, self.count()), "", Kind::Alone)
    }

    /// Empty it and forget what could be undone, as choosing what it held
    /// does: whether it held anything.
    pub fn empty(&mut self) -> bool {
        let held = !self.text.is_empty();
        *self = Self::default();
        held
    }

    /// Move the caret by `unit` toward `toward` — with `extend`, the
    /// selection's moving end, as ⇧ does: whether the caret or the
    /// selection moved.
    ///
    /// Without ⇧, a selection is put away as macOS puts it away: ← and →
    /// leave the caret at its start or its end, and ⌥ and ⌘ move on from
    /// that end.
    pub fn go(&mut self, unit: Unit, toward: Toward, extend: bool) -> bool {
        self.history.close();
        let chars = self.chars();
        let (start, end) = self.span();
        let from = match (extend, toward) {
            (true, _) => self.caret,
            (false, Toward::Start) => start,
            (false, Toward::End) => end,
        };
        let collapsing = !extend && start != end && unit == Unit::Character;
        let to = match (collapsing, unit, toward) {
            (true, _, _) => from,
            (false, Unit::Character, Toward::Start) => before(&chars, from),
            (false, Unit::Character, Toward::End) => after(&chars, from),
            (false, Unit::Word, Toward::Start) => word_start(&chars, from),
            (false, Unit::Word, Toward::End) => word_end(&chars, from),
            (false, Unit::Edge, Toward::Start) => 0,
            (false, Unit::Edge, Toward::End) => chars.len(),
        };
        let before_move = (self.caret, self.anchor);
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
        (self.caret, self.anchor) != before_move
    }

    /// Select every character, as ⌘A does: whether that changed what was
    /// selected.
    pub fn select_all(&mut self) -> bool {
        self.history.close();
        let before_move = (self.caret, self.anchor);
        self.anchor = 0;
        self.caret = self.count();
        (self.caret, self.anchor) != before_move
    }

    /// Put the caret at `at`, as a press does, nothing selected: whether it
    /// moved. A composition is let go.
    pub fn place(&mut self, at: usize) -> bool {
        self.history.close();
        let at = at.min(self.count());
        let before_move = (self.caret, self.anchor);
        self.caret = at;
        self.anchor = at;
        self.composing = None;
        (self.caret, self.anchor) != before_move
    }

    /// Move the caret to `at` keeping the anchor, as a drag does: whether
    /// the selection changed.
    pub fn extend_to(&mut self, at: usize) -> bool {
        self.history.close();
        let at = at.min(self.count());
        let before_move = self.caret;
        self.caret = at;
        self.caret != before_move
    }

    /// Select the word at `at`, as a double press does
    /// ([`crate::text_stops::word_at`]): whether that changed the selection.
    pub fn select_word(&mut self, at: usize) -> bool {
        self.history.close();
        let (start, end) = word_at(&self.chars(), at);
        let before_move = (self.caret, self.anchor);
        self.anchor = start;
        self.caret = end;
        (self.caret, self.anchor) != before_move
    }

    /// An input method is composing `text`, its own caret `caret`
    /// characters into it — nothing, once a composition is put away:
    /// whether what is drawn changed.
    pub fn compose(&mut self, text: &str, caret: usize) -> bool {
        let composing = (!text.is_empty()).then(|| Composing {
            text: text.chars().filter(|c| !c.is_control()).collect(),
            caret: caret.min(text.chars().count()),
        });
        let changed = composing != self.composing;
        self.composing = composing;
        changed
    }

    /// An input method's composition came to `text`: put in at the caret,
    /// over what is selected, as typing is. Whether anything changed.
    pub fn commit(&mut self, text: &str) -> bool {
        let had = self.composing.take().is_some();
        self.put(text) || had
    }

    /// Undo the last step, as ⌘Z does: whether there was one.
    pub fn undo(&mut self) -> bool {
        let now = self.stood();
        match self.history.undo(&now) {
            Some(before_step) => {
                self.stand(before_step);
                true
            }
            None => false,
        }
    }

    /// Do again the step last undone, as ⇧⌘Z does: whether there was one.
    pub fn redo(&mut self) -> bool {
        let now = self.stood();
        match self.history.redo(&now) {
            Some(after_step) => {
                self.stand(after_step);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> TextField {
        let mut field = TextField::default();
        for letter in text.chars() {
            assert!(field.put(&letter.to_string()));
        }
        field
    }

    #[test]
    fn arrows_move_the_caret_back_and_letters_go_in_where_it_is() {
        let mut field = typed("kampla");
        assert_eq!(field.caret(), 6);
        assert!(field.go(Unit::Character, Toward::Start, false));
        assert!(field.go(Unit::Character, Toward::Start, false));
        assert!(field.put("a"));
        assert_eq!(field.text(), "kampala");
        assert_eq!(field.caret(), 5);
        assert!(field.go(Unit::Edge, Toward::Start, false));
        assert!(
            !field.go(Unit::Character, Toward::Start, false),
            "at the start"
        );
        assert!(field.go(Unit::Edge, Toward::End, false));
        assert_eq!(field.caret(), 7);
        assert!(!field.go(Unit::Character, Toward::End, false), "at the end");
    }

    #[test]
    fn option_moves_and_erases_by_words_and_command_to_the_edges() {
        let mut field = typed("go to en.wikipedia.org now");
        assert!(field.go(Unit::Word, Toward::Start, false));
        assert_eq!(field.caret(), 23);
        assert!(field.go(Unit::Word, Toward::Start, false));
        assert_eq!(field.caret(), 6);
        assert!(field.go(Unit::Word, Toward::End, false));
        assert_eq!(field.caret(), 22);
        assert!(field.erase(Unit::Word, Toward::Start));
        assert_eq!(field.text(), "go to  now");
        assert!(field.erase(Unit::Word, Toward::End));
        assert_eq!(field.text(), "go to ");
        assert!(field.erase(Unit::Edge, Toward::Start));
        assert_eq!(field.text(), "");
        assert!(!field.erase(Unit::Character, Toward::Start), "nothing left");
        let mut field = typed("abc");
        assert!(field.go(Unit::Edge, Toward::Start, false));
        assert!(field.erase(Unit::Character, Toward::End), "forward delete");
        assert_eq!(field.text(), "bc");
        assert!(field.erase(Unit::Edge, Toward::End));
        assert_eq!(field.text(), "");
    }

    #[test]
    fn shift_selects_and_typing_replaces_the_selection() {
        let mut field = typed("hello world");
        assert!(field.go(Unit::Word, Toward::Start, true));
        assert_eq!(field.selection(), Some((6, 11)));
        assert_eq!(field.selected(), "world");
        assert!(field.go(Unit::Character, Toward::End, true), "shrinks back");
        assert_eq!(field.selection(), Some((7, 11)));
        assert!(field.put("W"));
        assert_eq!(field.text(), "hello wW");
        assert_eq!(field.selection(), None);
        // Without ⇧, ← puts a selection away at its start.
        assert!(field.select_all());
        assert!(field.go(Unit::Character, Toward::Start, false));
        assert_eq!((field.caret(), field.selection()), (0, None));
    }

    #[test]
    fn select_all_then_typing_replaces_everything() {
        let mut field = typed("https://en.wikipedia.org");
        assert!(field.select_all());
        assert!(!field.select_all(), "already all");
        assert!(field.put("e"));
        assert_eq!(field.text(), "e");
    }

    #[test]
    fn cut_and_paste_move_words_and_a_paste_is_one_line() {
        let mut field = typed("one two three");
        assert!(field.go(Unit::Word, Toward::Start, true));
        assert_eq!(field.cut().as_deref(), Some("three"));
        assert_eq!(field.text(), "one two ");
        assert!(field.go(Unit::Edge, Toward::Start, false));
        assert!(field.paste("three "));
        assert_eq!(
            field.text(),
            "threeone two ",
            "its ends' white space dropped"
        );
        assert_eq!(field.cut(), None, "nothing selected");
        assert_eq!(pasted("  a\r\nb\nc\td\u{2028}e\u{0}\n"), "a b c d e");
        let mut long = TextField::default();
        assert!(long.paste(&"x".repeat(LONGEST_TYPED + 50)));
        assert_eq!(long.text().chars().count(), LONGEST_TYPED, "capped");
        assert!(!long.paste("more"), "no room");
    }

    #[test]
    fn undo_takes_back_a_run_of_typing_and_redo_puts_it_back() {
        let mut field = typed("kampala");
        assert!(field.go(Unit::Edge, Toward::Start, false));
        assert!(field.paste("www.\n"));
        assert!(field.go(Unit::Edge, Toward::End, false));
        assert!(field.put("!"));
        assert!(field.undo());
        assert_eq!(field.text(), "www.kampala");
        assert!(field.undo());
        assert_eq!(field.text(), "kampala");
        assert_eq!(field.caret(), 0, "where the paste went in");
        assert!(field.undo());
        assert_eq!(field.text(), "");
        assert!(!field.undo());
        assert!(field.redo());
        assert_eq!(field.text(), "kampala");
        assert!(field.redo());
        assert_eq!(field.text(), "www.kampala");
        // Escape's emptying is undone too; choosing forgets.
        assert!(field.clear());
        assert!(field.undo());
        assert_eq!(field.text(), "www.kampala");
        assert!(field.empty());
        assert!(!field.undo());
    }

    #[test]
    fn a_press_places_extends_and_selects_a_word() {
        let mut field = typed("open en.wikipedia.org now");
        assert!(field.place(7));
        assert_eq!((field.caret(), field.selection()), (7, None));
        assert!(field.extend_to(21));
        assert_eq!(field.selection(), Some((7, 21)));
        assert!(field.select_word(10));
        assert_eq!(field.selected(), "en.wikipedia.org");
        assert!(field.place(900));
        assert_eq!(field.caret(), 25, "past the end is the end");
    }

    #[test]
    fn an_input_method_composes_then_commits_over_the_selection() {
        let mut field = typed("Kampala ");
        assert!(field.compose("きょう", 3));
        assert_eq!(
            field.text(),
            "Kampala ",
            "a composition is not in the words"
        );
        assert_eq!(
            field.editing().composing.map(|c| c.text),
            Some("きょう".to_owned())
        );
        assert!(!field.compose("きょう", 3), "the same again");
        assert!(field.commit("今日"));
        assert_eq!(field.text(), "Kampala 今日");
        assert_eq!(field.caret(), 10);
        assert!(field.composing().is_none());
        // Cancelled, nothing goes in.
        assert!(field.compose("に", 1));
        assert!(field.compose("", 0));
        assert_eq!(field.text(), "Kampala 今日");
        assert!(field.undo());
        assert_eq!(field.text(), "", "typing and its commit, one run");
    }

    #[test]
    fn control_characters_never_go_in_and_the_field_holds_at_most_the_longest() {
        let mut field = TextField::default();
        assert!(!field.put("\u{0}\r"));
        assert!(field.put("a\u{7f}b"));
        assert_eq!(field.text(), "ab");
        let _ = field.put(&"é".repeat(LONGEST_TYPED));
        assert_eq!(field.text().chars().count(), LONGEST_TYPED);
        assert!(!field.put("c"));
    }
}
