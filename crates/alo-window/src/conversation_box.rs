/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one box on the first screen, as the browser process holds it (ADR
//! 0074, queue item 701): whether it has the keyboard, what a person has
//! typed into it, and what choosing makes of that.
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
//! where. Opening that address in the machine's Chrome from this window, as
//! `alo --chrome` opens one, is queue item 713: `alo --chrome` runs the
//! frame around Chrome's window on an event loop of its own, and a process
//! has one. Searching and asking are offered first only for what is not an
//! address, and choosing either says it is not set up yet ([`said`]).

use crate::typing::Edit;
use alo_renderer::typed::{LONGEST_TYPED, Offer, offers};

/// The one box.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TheBox {
    /// What a person has typed, at most [`LONGEST_TYPED`] characters.
    typed: String,
    /// Whether it has the keyboard.
    focused: bool,
}

/// What an edit did to the box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// Nothing: the box does not have the keyboard, or the edit left what
    /// is typed as it was.
    Unchanged,
    /// What is typed, or whether the box has the keyboard, changed, and the
    /// first screen is to be drawn again.
    Changed,
    /// This offer was chosen.
    Chose(Offer),
}

impl TheBox {
    /// What is typed.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// Whether the box has the keyboard.
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// What is offered under the box now, in order: nothing unless it has
    /// the keyboard and something is typed.
    pub fn offers(&self) -> Vec<Offer> {
        if self.focused {
            offers(&self.typed)
        } else {
            Vec::new()
        }
    }

    /// Give the box the keyboard: whether that changed anything.
    pub fn focus(&mut self) -> bool {
        !std::mem::replace(&mut self.focused, true)
    }

    /// Take the keyboard from the box: whether that changed anything.
    pub fn blur(&mut self) -> bool {
        std::mem::replace(&mut self.focused, false)
    }

    /// What the first offer is, if anything is offered: what Return and
    /// *Go* choose.
    pub fn choice(&self) -> Option<Offer> {
        self.offers().into_iter().next()
    }

    /// Make `edit`, if the box has the keyboard.
    pub fn edit(&mut self, edit: Edit) -> Typed {
        if !self.focused {
            return Typed::Unchanged;
        }
        let changed = match edit {
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
            Edit::Choose => {
                return self.choice().map_or(Typed::Unchanged, Typed::Chose);
            }
        };
        if changed {
            Typed::Changed
        } else {
            Typed::Unchanged
        }
    }
}

/// What the window says, in the first screen's line, when `chosen` was
/// chosen.
pub fn said(chosen: &Offer) -> String {
    match chosen {
        Offer::Go(url) => format!(
            "going to {url} is decided; opening it in Chrome from this window is queue item 713"
        ),
        Offer::Search(_) => {
            "searching is not set up yet: no search provider has been chosen (queue item 708)"
                .to_owned()
        }
        Offer::Ask(_) => {
            "asking alo is not set up yet: no model has been chosen (queue item 707)".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focused() -> TheBox {
        let mut the_box = TheBox::default();
        assert!(the_box.focus());
        the_box
    }

    fn put(the_box: &mut TheBox, text: &str) -> Typed {
        the_box.edit(Edit::Put(text.to_owned()))
    }

    #[test]
    fn without_the_keyboard_nothing_is_typed_or_offered() {
        let mut the_box = TheBox::default();
        assert_eq!(put(&mut the_box, "example.com"), Typed::Unchanged);
        assert_eq!(the_box.edit(Edit::Choose), Typed::Unchanged);
        assert_eq!(the_box.typed(), "");
        assert!(the_box.offers().is_empty());
    }

    #[test]
    fn typing_example_com_and_return_decides_to_go_to_it() {
        let mut the_box = focused();
        for letter in "example.com".chars() {
            assert_eq!(put(&mut the_box, &letter.to_string()), Typed::Changed);
        }
        assert_eq!(the_box.typed(), "example.com");
        let Typed::Chose(Offer::Go(url)) = the_box.edit(Edit::Choose) else {
            panic!("not gone to: {:?}", the_box.offers());
        };
        assert_eq!(url.to_string(), "https://example.com/");
        assert_eq!(the_box.offers().len(), 3);
    }

    #[test]
    fn return_on_a_question_chooses_search_which_is_not_set_up() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "what is x");
        let chosen = the_box.edit(Edit::Choose);
        assert_eq!(chosen, Typed::Chose(Offer::Search("what is x".to_owned())));
        if let Typed::Chose(offer) = chosen {
            assert!(said(&offer).contains("not set up yet"));
        }
        assert!(said(&Offer::Ask(String::new())).contains("not set up yet"));
        // Nothing typed is nothing to choose.
        assert_eq!(focused().edit(Edit::Choose), Typed::Unchanged);
    }

    #[test]
    fn backspace_erases_and_escape_empties_then_lets_go() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "exampl€");
        assert_eq!(the_box.edit(Edit::Erase), Typed::Changed);
        assert_eq!(the_box.typed(), "exampl");
        assert_eq!(the_box.edit(Edit::Clear), Typed::Changed);
        assert_eq!(the_box.typed(), "");
        assert!(the_box.focused());
        assert_eq!(the_box.edit(Edit::Erase), Typed::Unchanged);
        assert_eq!(the_box.edit(Edit::Clear), Typed::Changed);
        assert!(!the_box.focused());
    }

    #[test]
    fn what_is_typed_stays_when_the_box_lets_go_and_its_offers_do_not() {
        let mut the_box = focused();
        let _ = put(&mut the_box, "example.com");
        assert!(the_box.blur());
        assert!(!the_box.blur());
        assert_eq!(the_box.typed(), "example.com");
        assert!(the_box.offers().is_empty());
        assert_eq!(the_box.choice(), None);
        assert!(the_box.focus());
        assert!(!the_box.focus());
        assert_eq!(the_box.offers().len(), 3);
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
    fn markup_typed_is_held_as_typed() {
        let mut the_box = focused();
        for hostile in ["<b>", "</script>", "\u{0}"] {
            let _ = put(&mut the_box, hostile);
        }
        assert_eq!(the_box.typed(), "<b></script>\u{0}");
    }
}
