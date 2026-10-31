/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The first screen's state: what the browser process tells the first
//! screen's renderer, as data (ADR 0074, queue item 700).
//!
//! The first screen is what a new chat shows: a greeting, the one box, the
//! sites a person keeps and the chats waiting on them. It is drawn as the
//! chats panel is ([`crate::panel`]): a document of ours built from this
//! state ([`crate::start_document`]), never from markup. A site's name is a
//! page's or a person's, and what a waiting chat says is the agent's words
//! about a page, so each is a stranger's string and goes in as a text node.
//!
//! # What is typed
//!
//! What a person types into the one box is held by the browser process
//! and sent here as [`Start::typed`] — the field's value, put into the
//! document as an attribute's value and never as markup — with whether
//! the box has the keyboard and what is offered for it
//! ([`crate::typed`], queue item 701). Nobody else is sent it: not the
//! chats panel's renderer, and not a page's.
//!
//! # A conversation
//!
//! Once a person has searched or opened a page, the screen is a
//! conversation (queue item 703, ADR 0085): every turn of the chat on
//! screen, oldest first ([`Start::turns`], [`crate::turn`]), scrolled as the
//! browser process says ([`Start::down`], [`crate::conversation_scroll`]),
//! with the box at the window's foot.
//!
//! # Where it is
//!
//! At [`site`], a site no page can name, minted fresh as the panel's is.

use crate::card::Card;
use crate::results::SetUp;
use crate::site::Site;
use crate::strip::longest;
use crate::turn::Turn;
use crate::typed::Offer;
use alo_layout::Size;
use alo_url::Url;

/// What the first screen's document is said to be at.
pub const ADDRESS: &str = "alo:new-chat";

/// The first screen's address, as a URL; [`None`] only if the URL parser
/// stopped accepting a scheme it accepts today, which a test pins.
pub fn address() -> Option<Url> {
    alo_url::parse(ADDRESS).ok()
}

/// A site for the first screen's renderer that no page can name, minted on
/// every call.
pub fn site() -> Site {
    Site::of(&address().unwrap_or_else(Url::about_blank))
}

/// The first screen, as the browser process sees it at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    /// What it says first — *Good morning.* — chosen by the browser
    /// process, which knows the time; the renderer is not told it.
    pub greeting: String,
    /// The sites a person keeps, in their order.
    pub sites: Vec<KeptSite>,
    /// The chats waiting on the person, most recent first.
    pub waiting: Vec<Waiting>,
    /// How big the screen is drawn, in CSS pixels.
    pub viewport: Size,
    /// The scale of the screen the window is on (ADR 0057 § 3).
    pub scale: f32,
    /// What a person has typed into the one box, shown as the field's
    /// value: a stranger's string, as a site's name is.
    pub typed: String,
    /// Whether the box has the keyboard: a person pressed it, and what they
    /// type goes into it.
    pub focused: bool,
    /// What is offered under the box for what is typed, in order, the
    /// first what Return chooses ([`crate::typed::offers`]); none while
    /// nothing is typed.
    pub offers: Vec<Offer>,
    /// The conversation's turns, oldest first (queue item 703): with any,
    /// they stand in place of the greeting and the groups. Only the newest
    /// may be a live card ([`Turn::Page`]).
    pub turns: Vec<Turn>,
    /// How far the conversation is scrolled, in CSS pixels: how far the
    /// newest turn's top is above the room's top — below `0` back up into
    /// the earlier turns ([`crate::conversation_scroll`]).
    pub down: f32,
    /// Whether searching is set up, which the box's offer to search says.
    pub search: SetUp,
}

/// One site a person keeps on the first screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptSite {
    /// What the tile is called.
    pub name: String,
    /// Where it goes.
    pub address: String,
}

/// One chat waiting on the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// The chat's name.
    pub name: String,
    /// What is waiting, in the agent's words: *"found 3 hotels · waiting
    /// for you to choose"*.
    pub said: String,
}

impl Default for Start {
    /// A greeting of nothing, no sites, nothing waiting, at no size, on a
    /// screen of one, an empty box without the keyboard, no turns, not
    /// scrolled, and searching not set up, for no reason given.
    fn default() -> Self {
        Self {
            greeting: String::new(),
            sites: Vec::new(),
            waiting: Vec::new(),
            viewport: Size::default(),
            scale: 1.0,
            typed: String::new(),
            focused: false,
            offers: Vec::new(),
            turns: Vec::new(),
            down: 0.0,
            search: SetUp::Not(String::new()),
        }
    }
}

impl Start {
    /// The same screen, drawn for a screen of `scale` device pixels to the
    /// CSS pixel. Anything that is not a finite factor above zero is one.
    #[must_use]
    pub fn at_scale(mut self, scale: f32) -> Self {
        self.scale = alo_value::device_scale(scale);
        self
    }

    /// Whether the screen is a conversation rather than a greeting: it
    /// holds a turn.
    pub fn conversing(&self) -> bool {
        !self.turns.is_empty()
    }

    /// The page shown live as a card, if the newest turn is one.
    pub fn card(&self) -> Option<&Card> {
        self.turns.last().and_then(Turn::card)
    }
}

impl KeptSite {
    /// A site called `name`, at `address`.
    pub fn new(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
        }
    }

    /// What the tile shows: its name, or its address when it has none, at
    /// most as many characters as a tab's title.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            longest(&self.address)
        } else {
            longest(&self.name)
        }
    }

    /// The tile's letter: the first of what it shows, as a capital.
    pub fn letter(&self) -> Option<String> {
        self.label()
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|first| first.to_uppercase().collect())
    }
}

impl Waiting {
    /// A chat called `name`, waiting because of what `said` says.
    pub fn new(name: impl Into<String>, said: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            said: said.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strip::LONGEST_TITLE;

    #[test]
    fn the_address_parses_and_its_site_is_a_process_of_its_own() {
        assert!(address().is_some());
        let first = site();
        assert!(first.is_alone());
        assert_eq!(first.scheme(), "alo");
        assert_ne!(first, site());
    }

    #[test]
    fn a_site_with_no_name_shows_its_address_and_its_letter_is_a_capital() {
        let named = KeptSite::new("Mail", "https://mail.example/");
        assert_eq!(named.label(), "Mail");
        assert_eq!(named.letter().as_deref(), Some("M"));
        let unnamed = KeptSite::new(" ", "evhomeguide.org");
        assert_eq!(unnamed.label(), "evhomeguide.org");
        assert_eq!(unnamed.letter().as_deref(), Some("E"));
        assert_eq!(KeptSite::new("", "").letter(), None);
        let long = KeptSite::new("é".repeat(LONGEST_TITLE + 1), "");
        assert_eq!(long.label().chars().count(), LONGEST_TITLE);
    }
}
