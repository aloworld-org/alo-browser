/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one box on the first screen, as the browser process holds it (ADR
//! 0074, queue item 701): whether it has the keyboard, what a person has
//! typed into it, and what choosing makes of that.
//!
//! It has the keyboard as the window opens and on every new chat (queue
//! item 875), as a Mac's search field in a new window has, so a person's
//! first keys reach it without a press; a press anywhere else takes it.
//!
//! What is typed is held **here and nowhere else**. The first screen's
//! renderer is sent it as data to show as the field's value
//! (`alo-renderer`'s `start`), with what is offered for it
//! (`alo-renderer`'s `typed`); the chats panel's renderer is never sent it,
//! and no page is. A key edits it only while the box has the keyboard,
//! which a press on the box gives it and a press anywhere else takes away
//! ([`crate::conversation::Conversation::act`]).
//!
//! What is offered is shown only while the box has the keyboard and
//! something is typed; what was typed stays in the field when it loses the
//! keyboard, as a field's value does.
//!
//! # What choosing comes to
//!
//! Return, or *Go*, chooses what is offered first. For an address that is
//! *Go to the site*, and choosing it decides to go to the parsed URL —
//! [`Conversation::going`](crate::conversation::Conversation::going) says
//! where, and the window has it opened in the machine's Chrome
//! ([`crate::conversation_chrome`], queue item 713), whose thread then
//! says what became of it in the first screen's line. Searching and
//! asking are offered first only for what is not an address. Choosing a
//! search that is set up has the window ask for it
//! ([`crate::conversation_search`], queue item 708); choosing one that is
//! not, or asking, says it is not set up yet and why ([`said`]).

use crate::field_edit::{self, Made};
use crate::text_field::TextField;
use crate::typing::Edit;
use alo_renderer::results::{PROVIDER, SetUp};
use alo_renderer::typed::{Offer, offers_for};

/// The one box.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TheBox {
    /// What a person has typed, at most
    /// [`alo_renderer::typed::LONGEST_TYPED`] characters, with its caret,
    /// its selection and what can be undone ([`crate::text_field`], queue
    /// item 861).
    field: TextField,
    /// Whether it has the keyboard.
    focused: bool,
}

/// What an edit did to the box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// Nothing: the box does not have the keyboard, or the edit left what
    /// is typed as it was.
    Unchanged,
    /// What is typed, where its caret stands, or whether the box has the
    /// keyboard, changed, and the first screen is to be drawn again.
    Changed,
    /// This was selected and is to go on the system's clipboard (⌘C):
    /// nothing changed.
    Copy(String),
    /// This was selected, is to go on the system's clipboard, and was
    /// taken out (⌘X): drawn again.
    Cut(String),
    /// This offer was chosen.
    Chose(Offer),
}

impl Typed {
    /// What `made` comes to, for a field that chooses nothing.
    pub fn of(made: Made) -> Self {
        match made {
            Made::Nothing | Made::Theirs(_) => Typed::Unchanged,
            Made::Changed => Typed::Changed,
            Made::Copy(text) => Typed::Copy(text),
            Made::Cut(text) => Typed::Cut(text),
        }
    }
}

impl TheBox {
    /// What is typed.
    pub fn typed(&self) -> &str {
        self.field.text()
    }

    /// The field itself: its caret, its selection, a composition.
    pub fn field(&self) -> &TextField {
        &self.field
    }

    /// The field, to place its caret or compose in, while the box has the
    /// keyboard.
    pub fn field_mut(&mut self) -> Option<&mut TextField> {
        self.focused.then_some(&mut self.field)
    }

    /// Whether the box has the keyboard.
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// What is offered under the box now, in order: nothing unless it has
    /// the keyboard and something is typed — asking alo before searching
    /// when `asks_first`, while a card is open and the agent can be asked
    /// (queue item 704).
    pub fn offers(&self, asks_first: bool) -> Vec<Offer> {
        if self.focused {
            offers_for(self.field.text(), asks_first)
        } else {
            Vec::new()
        }
    }

    /// An empty box with the keyboard: as the window opens, and on a new
    /// chat (queue item 875).
    pub fn with_the_keyboard() -> Self {
        Self {
            focused: true,
            ..Self::default()
        }
    }

    /// Give the box the keyboard: whether that changed anything.
    pub fn focus(&mut self) -> bool {
        !std::mem::replace(&mut self.focused, true)
    }

    /// Take the keyboard from the box: whether that changed anything. A
    /// composition is put away with it.
    pub fn blur(&mut self) -> bool {
        let _ = self.field.compose("", 0);
        std::mem::replace(&mut self.focused, false)
    }

    /// Empty the box, keeping the keyboard where it is, and forget what
    /// could be undone, as choosing does: whether anything was typed.
    pub fn empty(&mut self) -> bool {
        self.field.empty()
    }

    /// What the first offer is, if anything is offered: what Return and
    /// *Go* choose.
    pub fn choice(&self, asks_first: bool) -> Option<Offer> {
        self.offers(asks_first).into_iter().next()
    }

    /// Make `edit`, if the box has the keyboard, offering as `asks_first`
    /// says ([`crate::field_edit`]).
    pub fn edit(&mut self, edit: Edit, asks_first: bool) -> Typed {
        if !self.focused {
            return Typed::Unchanged;
        }
        match field_edit::make(&mut self.field, edit) {
            // Empty, Escape lets the keyboard go.
            Made::Theirs(Edit::Clear) => {
                if self.blur() {
                    Typed::Changed
                } else {
                    Typed::Unchanged
                }
            }
            Made::Theirs(Edit::Choose) => self
                .choice(asks_first)
                .map_or(Typed::Unchanged, Typed::Chose),
            made => Typed::of(made),
        }
    }
}

/// What the window says, in the first screen's line, when `chosen` was
/// chosen, searching being as `search` says (queue item 708) and asking
/// alo as `asking` does (queue item 704).
pub fn said(chosen: &Offer, search: &SetUp, asking: &SetUp) -> String {
    if let Offer::Ask(words) = chosen {
        return match asking {
            SetUp::Ready => format!("alo is at work on \u{201c}{words}\u{201d}"),
            SetUp::Not(why) if why.trim().is_empty() => "asking alo is not set up yet".to_owned(),
            SetUp::Not(why) => format!("asking alo is not set up yet: {why}"),
        };
    }
    match (chosen, search) {
        (Offer::Go(url), _) => format!("going to {url}"),
        (Offer::Search(words), SetUp::Ready) => {
            format!("searching {PROVIDER} for \u{201c}{words}\u{201d}")
        }
        (Offer::Search(_), SetUp::Not(why)) if why.trim().is_empty() => {
            "searching is not set up yet".to_owned()
        }
        (Offer::Search(_), SetUp::Not(why)) => format!("searching is not set up yet: {why}"),
        (Offer::Ask(_), _) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::typed::LONGEST_TYPED;

    fn focused() -> TheBox {
        let mut the_box = TheBox::default();
        assert!(the_box.focus());
        the_box
    }

    fn erase() -> Edit {
        Edit::Erase(
            crate::text_field::Unit::Character,
            crate::text_field::Toward::Start,
        )
    }

    fn put(the_box: &mut TheBox, text: &str) -> Typed {
        the_box.edit(Edit::Put(text.to_owned()), false)
    }

    #[test]
    fn without_the_keyboard_nothing_is_typed_or_offered() {
        let mut the_box = TheBox::default();
        assert_eq!(put(&mut the_box, "example.com"), Typed::Unchanged);
        assert_eq!(the_box.edit(Edit::Choose, false), Typed::Unchanged);
        assert_eq!(the_box.typed(), "");
        assert!(the_box.offers(false).is_empty());
    }

    #[test]
    fn a_box_made_with_the_keyboard_takes_keys_at_once() {
        let mut the_box = TheBox::with_the_keyboard();
        assert!(the_box.focused());
        assert!(!the_box.focus(), "already had it");
        assert_eq!(put(&mut the_box, "h"), Typed::Changed);
        assert_eq!(the_box.typed(), "h");
    }

    #[test]
    fn typing_example_com_and_return_decides_to_go_to_it() {
        let mut the_box = focused();
        for letter in "example.com".chars() {
            assert_eq!(put(&mut the_box, &letter.to_string()), Typed::Changed);
        }
        assert_eq!(the_box.typed(), "example.com");
        let Typed::Chose(Offer::Go(url)) = the_box.edit(Edit::Choose, false) else {
            panic!("not gone to: {:?}", the_box.offers(false));
        };
        assert_eq!(url.to_string(), "https://example.com/");
        assert_eq!(the_box.offers(false).len(), 3);
    }

    #[test]
    fn return_on_a_question_chooses_search_which_is_not_set_up() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "what is x");
        let chosen = the_box.edit(Edit::Choose, false);
        assert_eq!(chosen, Typed::Chose(Offer::Search("what is x".to_owned())));
        let missing = SetUp::Not("no Brave Search key in this Mac's keychain".to_owned());
        if let Typed::Chose(offer) = chosen {
            assert_eq!(
                said(&offer, &missing, &SetUp::Ready),
                "searching is not set up yet: no Brave Search key in this Mac's keychain"
            );
            assert_eq!(
                said(&offer, &SetUp::Ready, &SetUp::Ready),
                "searching Brave Search for \u{201c}what is x\u{201d}"
            );
        }
        // Asking alo says what it does when it is set up, and why not when
        // it is not (queue item 704).
        let ask = Offer::Ask("send it".to_owned());
        assert_eq!(
            said(&ask, &SetUp::Ready, &SetUp::Ready),
            "alo is at work on \u{201c}send it\u{201d}"
        );
        assert_eq!(
            said(
                &ask,
                &SetUp::Ready,
                &SetUp::Not("no page is open".to_owned())
            ),
            "asking alo is not set up yet: no page is open"
        );
        // Nothing typed is nothing to choose.
        assert_eq!(focused().edit(Edit::Choose, false), Typed::Unchanged);
    }

    #[test]
    fn backspace_erases_and_escape_empties_then_lets_go() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "exampl€");
        assert_eq!(the_box.edit(erase(), false), Typed::Changed);
        assert_eq!(the_box.typed(), "exampl");
        assert_eq!(the_box.edit(Edit::Clear, false), Typed::Changed);
        assert_eq!(the_box.typed(), "");
        assert!(the_box.focused());
        assert_eq!(the_box.edit(erase(), false), Typed::Unchanged);
        assert_eq!(the_box.edit(Edit::Clear, false), Typed::Changed);
        assert!(!the_box.focused());
    }

    #[test]
    fn what_is_typed_stays_when_the_box_lets_go_and_its_offers_do_not() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "example.com");
        assert!(the_box.blur());
        assert!(!the_box.blur());
        assert_eq!(the_box.typed(), "example.com");
        assert!(the_box.offers(false).is_empty());
        assert_eq!(the_box.choice(false), None);
        assert!(the_box.focus());
        assert!(!the_box.focus());
        assert_eq!(the_box.offers(false).len(), 3);
    }

    #[test]
    fn the_box_holds_at_most_the_longest_typed() {
        let mut the_box = focused();
        let _ = put(&mut the_box, &"é".repeat(LONGEST_TYPED - 1));
        assert_eq!(put(&mut the_box, "ab"), Typed::Changed);
        assert_eq!(put(&mut the_box, "c"), Typed::Unchanged);
        assert_eq!(the_box.typed().chars().count(), LONGEST_TYPED);
        assert!(the_box.typed().ends_with("éa"));
    }

    #[test]
    fn markup_typed_is_held_as_typed_and_control_characters_left_out() {
        let mut the_box = focused();
        for hostile in ["<b>", "</script>", "\u{0}"] {
            let _ = put(&mut the_box, hostile);
        }
        assert_eq!(the_box.typed(), "<b></script>");
    }

    #[test]
    fn select_all_then_typing_replaces_what_was_typed_and_copy_cut_say_what() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "https://en.wikipedia.org");
        assert_eq!(the_box.edit(Edit::SelectAll, false), Typed::Changed);
        assert_eq!(
            the_box.edit(Edit::Copy, false),
            Typed::Copy("https://en.wikipedia.org".to_owned())
        );
        assert_eq!(put(&mut the_box, "example.com"), Typed::Changed);
        assert_eq!(the_box.typed(), "example.com", "not glued after it (859)");
        let _ = the_box.edit(Edit::SelectAll, false);
        assert_eq!(
            the_box.edit(Edit::Cut, false),
            Typed::Cut("example.com".to_owned())
        );
        assert_eq!(the_box.typed(), "");
        assert_eq!(the_box.edit(Edit::Undo, false), Typed::Changed);
        assert_eq!(the_box.typed(), "example.com");
        // Choosing empties it and forgets what could be undone.
        assert!(matches!(the_box.edit(Edit::Choose, false), Typed::Chose(_)));
        assert!(the_box.empty());
        assert_eq!(the_box.edit(Edit::Undo, false), Typed::Unchanged);
    }
}
