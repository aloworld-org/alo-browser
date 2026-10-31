/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel's state: what the browser process tells the panel's
//! renderer, as data (ADR 0074, queue item 699).
//!
//! The panel is the left of the browser's own interface — the chats a person
//! has open and the ones before — and is drawn as the tab strip is (ADR 0024
//! § 4): a document of ours, built from this state by the panel's renderer
//! ([`crate::panel_document`]), never from markup. A chat's name is the
//! person's own words or a page's title, and its site is a page's to choose,
//! so both are strangers' strings and go in as text nodes.
//!
//! # Where the panel is
//!
//! At [`site`], a site no page can name, minted fresh as the strip's is: its
//! renderer holds nobody's page but the panel.

use crate::site::Site;
use crate::strip::longest;
use alo_layout::Size;
use alo_url::Url;

/// What the panel's document is said to be at: a scheme this engine
/// registers nothing for, so its origin is opaque and its [`site`] is a
/// process of its own.
pub const ADDRESS: &str = "alo:chats";

/// The panel's address, as a URL; [`None`] only if the URL parser stopped
/// accepting a scheme it accepts today, which a test pins.
pub fn address() -> Option<Url> {
    alo_url::parse(ADDRESS).ok()
}

/// A site for the panel's renderer that no page can name, minted on every
/// call as every opaque origin is.
pub fn site() -> Site {
    Site::of(&address().unwrap_or_else(Url::about_blank))
}

/// The panel, as the browser process sees it at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// Every chat, in the order the browser process keeps them: the open
    /// ones are drawn under *Open* and the rest under *Earlier*, each group
    /// in this order.
    pub chats: Vec<Chat>,
    /// How big the panel is drawn, in CSS pixels.
    pub viewport: Size,
    /// The scale of the screen the window is on (ADR 0057 § 3): any finite
    /// factor above zero, and anything else is one.
    pub scale: f32,
    /// What a person typed into *Search chats* (queue item 711): the
    /// field's value. The browser process sends only the chats whose names
    /// hold it ([`matching`]); the panel says so when none does.
    pub searched: String,
    /// Whether *Search chats* has the keyboard.
    pub searching: bool,
    /// Whether the panel is hidden, drawn as the design's rail 64 wide:
    /// the mark, *New chat* and *Show the panel* (queue item 711).
    pub hidden: bool,
}

/// One chat in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    /// What the chat is called: a person's words or a page's title. A
    /// stranger's string, drawn to [`crate::strip::LONGEST_TITLE`]
    /// characters and cut by the stylesheet where the row ends.
    pub name: String,
    /// The site open in it, by the name a person knows it by, or nothing
    /// when no page is open: its first letter is the row's mark.
    pub site: String,
    /// Whether it is open, rather than one from before.
    pub open: bool,
    /// Whether it is the chat on screen.
    pub current: bool,
    /// What the agent is doing in it.
    pub doing: Doing,
}

/// What the agent is doing in a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    /// Nothing: the chat is the person's.
    Nothing,
    /// At work, and what the chat shows may be stale.
    Working,
    /// It finished what it was asked, and has not been looked at since.
    Done,
}

impl Default for Panel {
    /// No chats, at no size, on a screen of one.
    fn default() -> Self {
        Self {
            chats: Vec::new(),
            viewport: Size::default(),
            scale: 1.0,
            searched: String::new(),
            searching: false,
            hidden: false,
        }
    }
}

impl Panel {
    /// The same panel, drawn for a screen of `scale` device pixels to the
    /// CSS pixel. Anything that is not a finite factor above zero is one.
    #[must_use]
    pub fn at_scale(mut self, scale: f32) -> Self {
        self.scale = alo_value::device_scale(scale);
        self
    }
}

impl Chat {
    /// An open chat called `name` with `site` open in it, not on screen,
    /// the agent doing nothing.
    pub fn new(name: impl Into<String>, site: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            site: site.into(),
            open: true,
            current: false,
            doing: Doing::Nothing,
        }
    }

    /// The same chat, one from before rather than open.
    #[must_use]
    pub fn earlier(mut self) -> Self {
        self.open = false;
        self
    }

    /// The same chat, on screen.
    #[must_use]
    pub fn current(mut self) -> Self {
        self.current = true;
        self
    }

    /// The same chat, with the agent `doing` this in it.
    #[must_use]
    pub fn doing(mut self, doing: Doing) -> Self {
        self.doing = doing;
        self
    }

    /// What the row shows as the chat's name: its name, or *New chat* when
    /// it has none yet, at most as many characters as a tab's title.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            NEW_CHAT
        } else {
            longest(&self.name)
        }
    }

    /// The row's mark: the first letter of its site, as a capital, or
    /// nothing when no site is open.
    pub fn mark(&self) -> Option<String> {
        self.site
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|first| first.to_uppercase().collect())
    }
}

/// The places in `chats` of each chat, in the order the panel draws them:
/// the open ones, then the earlier ones, each group in the order sent
/// ([`crate::panel_document`]). A press on the panel names a chat by its
/// place in this order ([`crate::conversation_claim`]), and the browser
/// process reads it back through the same order.
pub fn drawn_order(chats: &[Chat]) -> Vec<usize> {
    let places = || chats.iter().enumerate();
    places()
        .filter(|(_, chat)| chat.open)
        .chain(places().filter(|(_, chat)| !chat.open))
        .map(|(place, _)| place)
        .collect()
}

/// What a chat with no name yet is called.
pub const NEW_CHAT: &str = "New chat";

/// Whether `chat` is one *Search chats* finds for `searched` (queue item
/// 711): every chat when nothing is searched, and otherwise one whose name
/// as the row shows it holds what was searched, letters compared without
/// their case and the words' spaces at either end ignored.
pub fn matching(chat: &Chat, searched: &str) -> bool {
    let searched = searched.trim();
    searched.is_empty()
        || chat
            .label()
            .to_lowercase()
            .contains(&searched.to_lowercase())
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
    fn a_chat_is_open_off_screen_and_idle_until_it_is_said_otherwise() {
        let chat = Chat::new("Charging my EV", "evhomeguide.org");
        assert!(chat.open);
        assert!(!chat.current);
        assert_eq!(chat.doing, Doing::Nothing);
        let chat = chat.earlier().current().doing(Doing::Done);
        assert!(!chat.open);
        assert!(chat.current);
        assert_eq!(chat.doing, Doing::Done);
    }

    #[test]
    fn a_chat_with_no_name_is_a_new_chat_and_a_long_one_is_kept_to_the_longest() {
        assert_eq!(Chat::new(" \n", "").label(), NEW_CHAT);
        let long = Chat::new("é".repeat(LONGEST_TITLE + 3), "");
        assert_eq!(long.label().chars().count(), LONGEST_TITLE);
    }

    #[test]
    fn the_panel_draws_the_open_chats_and_then_the_earlier_ones() {
        let chats = [
            Chat::new("a", "").earlier(),
            Chat::new("b", ""),
            Chat::new("c", "").earlier(),
            Chat::new("d", ""),
        ];
        assert_eq!(drawn_order(&chats), vec![1, 3, 0, 2]);
        assert_eq!(drawn_order(&[]), Vec::<usize>::new());
    }

    #[test]
    fn search_chats_finds_a_name_holding_what_was_searched_whatever_its_case() {
        let lisbon = Chat::new("Weekend in Lisbon", "lisbon.example");
        assert!(matching(&lisbon, ""));
        assert!(matching(&lisbon, "  "));
        assert!(matching(&lisbon, "lisbon"));
        assert!(matching(&lisbon, " IN LIS "));
        assert!(!matching(&lisbon, "lisbon.example"), "not by its site");
        assert!(!matching(&lisbon, "Lisboa"));
        // A chat with no name is found by what the row calls it.
        assert!(matching(&Chat::new("", ""), "new"));
        assert!(matching(&Chat::new("Été", ""), "été"));
        assert!(matching(&Chat::new("ÉTÉ", ""), "été"));
    }

    #[test]
    fn the_mark_is_the_sites_first_letter_as_a_capital() {
        assert_eq!(
            Chat::new("a", "evhomeguide.org").mark().as_deref(),
            Some("E")
        );
        assert_eq!(Chat::new("a", "  ßtraße").mark().as_deref(), Some("SS"));
        assert_eq!(Chat::new("a", "").mark(), None);
        assert_eq!(Chat::new("a", "…!").mark(), None);
    }
}
