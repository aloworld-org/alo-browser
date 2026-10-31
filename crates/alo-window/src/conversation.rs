/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the browser process holds of its own interface as a conversation
//! (ADR 0074, queue item 709), and what it tells the renderers that draw it.
//!
//! The interface is the chats panel on the left and, beside it, what the
//! chat on screen shows — for a new chat, the first screen. Each is a
//! document of ours drawn by a renderer of its own from state the browser
//! process sends as data (`alo-renderer`'s `panel` and `start`), never from
//! markup. This is that state, and the one place it is decided.
//!
//! # What it holds today
//!
//! One chat, new and on screen, to begin with, and nothing else: no kept
//! sites and no chat waiting, because nothing yet keeps either, and a first
//! screen that showed sites nobody added would be the browser inventing a
//! person's life.
//!
//! # What a press does
//!
//! Queue item 710. A person's press on either part is claimed by the
//! renderer that drew it and decided by the browser process
//! ([`crate::conversation_drawing`], `alo-renderer`'s
//! `conversation_decide`), and what was decided is [`Conversation::act`]ed
//! on here. Each chat has an identity of the browser process's own
//! ([`ChatId`]), which is what a decision names — never anything a renderer
//! said. Two decisions change what is held today:
//!
//! - **A chat** becomes the one on screen, and the panel is drawn again
//!   showing it so.
//! - **New chat** adds a chat with no name yet — the panel calls it *New
//!   chat* — first among the open ones, and on screen.
//!
//! Queue item 701 adds two more:
//!
//! - **The box** takes the keyboard ([`crate::conversation_box`]); what a
//!   person then types is shown in it, with what is offered for it, and
//!   the first screen is drawn again. A press on anything else, in either
//!   part, takes the keyboard from it.
//! - **Go** chooses what is offered first, as Return does
//!   ([`Conversation::edit`]): going to a site is decided and kept as
//!   [`Conversation::going`].
//!
//! Queue item 718 adds the card's page: a press on it gives **the page** the
//! keyboard ([`Conversation::press_the_page`]) and takes it from the box,
//! and a press anywhere else in either part — the card's bar too — takes it
//! from the page. While the page has it, a person's keys go to the page
//! ([`Conversation::keys_to_the_card`], [`crate::card_keys`]) and none to
//! the box. One thing has the keyboard at a time, as in any window.
//!
//! Queue item 708 adds searching: Return or *Go* on words, when searching
//! is set up ([`Conversation::set_search`]), shows the search's results
//! under what the person said — searching until the provider answers
//! ([`Conversation::searched`], newest only) — and empties the box; and a
//! press on a site's title among them chooses its address as the box
//! would ([`Conversation::choose_found`]), the card then taking the
//! results' place. One thing stands under what the person said: a card or
//! results.
//!
//! Every other — *Settings*, a kept site, *Add a site*, a waiting chat — is
//! decided and kept as [`Conversation::last_pressed`], and **changes
//! nothing yet**: there is no settings screen (705), and nothing keeps a
//! site or a waiting chat. What the first screen shows does not follow the
//! chat on screen either, since every chat today is new and shows the first
//! screen (703).
//!
//! # The greeting
//!
//! The design greets a person by the time of day — *Good morning.* — and the
//! browser process is the one that would know it. It does not yet: this
//! engine reads no time zone (ADR 0036 § 3 keeps a page's local time UTC),
//! and a greeting of the morning said in the evening is a wrong sentence on
//! the first screen a person sees. So it says [`GREETING`], which is true at
//! any hour, until the machine's zone is read.

use crate::conversation_box::{TheBox, Typed};
use crate::conversation_search::Searched;
use crate::scale::Scale;
use crate::typing::Edit;
use alo_layout::Size;
use alo_renderer::card::Card;
use alo_renderer::conversation_decide::{Decided, Shown};
use alo_renderer::panel::{Chat, Panel, drawn_order};
use alo_renderer::results::{self, Results, SetUp};
use alo_renderer::start::{KeptSite, Start};
use alo_renderer::typed::Offer;
use alo_url::Url;

/// What the first screen says first, at any hour.
pub const GREETING: &str = "Hello.";

/// A chat, as the browser process knows it: a number it gave the chat
/// when it was made, never reused, and never sent to a renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatId(u64);

/// What the parts showed, as the browser process names each thing in them.
pub type Showing = Shown<ChatId, KeptSite>;

/// What a press on either part came to, in the browser process's names.
pub type Pressed = Decided<ChatId, KeptSite>;

/// The conversation interface, as the browser process holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// Every chat, in the order the browser process keeps them, each with
    /// its identity.
    chats: Vec<(ChatId, Chat)>,
    /// The identity the next chat made is given.
    next: u64,
    /// What the first screen says first.
    greeting: String,
    /// What the last press on either part came to, if there was one.
    last_pressed: Option<Pressed>,
    /// The one box: what is typed into it, and whether it has the
    /// keyboard.
    the_box: TheBox,
    /// What was last chosen from what the box offered, if anything was.
    chosen: Option<Offer>,
    /// What the box held when it was last chosen from: what the person
    /// said, which a card answers.
    said: String,
    /// The page opened from the conversation, shown as a card (ADR 0076 B,
    /// queue item 717), if one is.
    card: Option<Card>,
    /// Whether the card's page has the keyboard (queue item 718): given it
    /// by a press on the page, taken by a press anywhere else.
    card_keys: bool,
    /// Whether searching can be done (queue item 708).
    search: SetUp,
    /// The newest search's results, shown as the All tab, if there are any.
    results: Option<Results>,
    /// How many searches were chosen: the newest one's number.
    searches: u64,
}

impl Default for Conversation {
    /// One new chat, on screen, and the greeting.
    fn default() -> Self {
        Self {
            chats: vec![(ChatId(0), Chat::new("", "").current())],
            next: 1,
            greeting: GREETING.to_owned(),
            last_pressed: None,
            the_box: TheBox::default(),
            chosen: None,
            said: String::new(),
            card: None,
            card_keys: false,
            search: SetUp::Not(String::new()),
            results: None,
            searches: 0,
        }
    }
}

impl Conversation {
    /// The chats panel as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn panel(&self, viewport: Size, scale: Scale) -> Panel {
        Panel {
            chats: self.chats.iter().map(|(_, chat)| chat.clone()).collect(),
            viewport,
            scale: 1.0,
        }
        .at_scale(scale.factor())
    }

    /// What the panel and the first screen show as they are now, each list
    /// in the order its part draws it: what a claim on either is decided
    /// against once they are painted.
    pub fn showing(&self) -> Showing {
        let chats: Vec<Chat> = self.chats.iter().map(|(_, chat)| chat.clone()).collect();
        Shown {
            chats: drawn_order(&chats)
                .into_iter()
                .filter_map(|place| self.chats.get(place).map(|(id, _)| *id))
                .collect(),
            sites: Vec::new(),
            waiting: Vec::new(),
            card: self.card.is_some(),
            found: self
                .results
                .iter()
                .flat_map(Results::on_all)
                .map(|found| found.url.clone())
                .collect(),
        }
    }

    /// Act on what a press came to, and keep it as the last pressed:
    /// whether the panel or the first screen now shows something else, and
    /// so is to be drawn again.
    ///
    /// A chat becomes the one on screen; *New chat* adds one, on screen;
    /// the box takes the keyboard, and *Go* chooses what it offers first,
    /// kept as [`Conversation::chosen`]. A press on anything but the box
    /// and *Go* takes the keyboard from the box. Everything else changes
    /// nothing yet (see the module's words), and neither does a chat this
    /// does not hold, which the decision refused unless it went between
    /// being decided and being acted on, nor a press refused or unanswered.
    pub fn act(&mut self, pressed: Pressed) -> bool {
        let changed = match &pressed {
            Pressed::TheBox => self.the_box.focus(),
            Pressed::Go => {
                let choice = self.the_box.choice();
                self.chose(choice)
            }
            Pressed::Refused(_) | Pressed::Failed(_) => false,
            _ => self.the_box.blur(),
        };
        if !matches!(pressed, Pressed::Refused(_) | Pressed::Failed(_)) {
            // A press on either part, the card's bar among them, is not on
            // its page.
            self.card_keys = false;
        }
        let changed = match &pressed {
            Pressed::Chat(id) => self.put_on_screen(*id) || changed,
            Pressed::Found(url) => {
                self.choose_found(url.clone());
                changed
            }
            Pressed::NewChat => {
                self.new_chat();
                true
            }
            _ => changed,
        };
        self.last_pressed = Some(pressed);
        changed
    }

    /// Make a person's `edit` to the box, which changes nothing unless the
    /// box has the keyboard: what it came to. What is chosen is kept as
    /// [`Conversation::chosen`].
    pub fn edit(&mut self, edit: Edit) -> Typed {
        let typed = self.the_box.edit(edit);
        if let Typed::Chose(offer) = &typed {
            let _ = self.chose(Some(offer.clone()));
        }
        typed
    }

    /// `choice` was chosen from the box: kept as [`Conversation::chosen`],
    /// with what the box held as what the person said. Whether the first
    /// screen shows something else for it.
    ///
    /// A site opens as a card under what the person said, and a search
    /// that is set up (queue item 708) is shown under it as its results,
    /// searching until they come: either way the box is emptied, so it
    /// offers nothing more, since the person's line holds what they typed
    /// (queue item 742). A search puts away a card that was shown, whose
    /// page the window then closes. Searching not set up, and asking, leave
    /// what was typed where it was.
    fn chose(&mut self, choice: Option<Offer>) -> bool {
        self.chosen = choice;
        self.said = self.the_box.typed().to_owned();
        match &self.chosen {
            Some(Offer::Go(_)) => self.the_box.empty(),
            Some(Offer::Search(words)) if self.search == SetUp::Ready => {
                self.searches += 1;
                self.results = Some(Results::searching(words.clone()));
                self.card = None;
                self.card_keys = false;
                let _ = self.the_box.empty();
                true
            }
            _ => false,
        }
    }

    /// Whether searching can be done is now `search`, as the searcher says
    /// (queue item 708): the box's offer to search says so.
    pub fn set_search(&mut self, search: SetUp) {
        self.search = search;
    }

    /// Whether searching can be done, and why not.
    pub fn search_set_up(&self) -> &SetUp {
        &self.search
    }

    /// The search under way, if the newest is: its number and its words,
    /// which the window hands the searcher.
    pub fn searching(&self) -> Option<(u64, &str)> {
        match (&self.chosen, &self.results) {
            (
                Some(Offer::Search(_)),
                Some(Results {
                    said,
                    showing: results::Showing::Searching,
                }),
            ) => Some((self.searches, said.as_str())),
            _ => None,
        }
    }

    /// What came of a search: shown, if it is the newest and its results
    /// are still shown. Whether the first screen shows something else.
    pub fn searched(&mut self, searched: Searched) -> bool {
        if searched.number != self.searches {
            return false;
        }
        match self.results.as_mut() {
            Some(results) if results.said == searched.words => {
                results.showing = searched.showing;
                true
            }
            _ => false,
        }
    }

    /// A search's results, if they are shown.
    pub fn results(&self) -> Option<&Results> {
        self.results.as_ref()
    }

    /// A person pressed the title of the site found at `url` (queue item
    /// 708): it is chosen as an address from the box is, and opens as a
    /// card under what the person searched for.
    pub fn choose_found(&mut self, url: alo_url::Url) {
        if let Some(results) = &self.results {
            self.said.clone_from(&results.said);
        }
        self.chosen = Some(Offer::Go(url));
    }

    /// What was last chosen from what the box offered, by Return or *Go*.
    pub fn chosen(&self) -> Option<&Offer> {
        self.chosen.as_ref()
    }

    /// Where it was decided to go: the address of the site last chosen, if
    /// what was last chosen was a site, which the window opens in Chrome
    /// ([`crate::conversation_chrome`], queue item 713).
    pub fn going(&self) -> Option<&Url> {
        match &self.chosen {
            Some(Offer::Go(url)) => Some(url),
            _ => None,
        }
    }

    /// What the last press on either part came to, if there was one.
    pub fn last_pressed(&self) -> Option<&Pressed> {
        self.last_pressed.as_ref()
    }

    /// The chat on screen, if one is.
    pub fn on_screen(&self) -> Option<ChatId> {
        self.chats
            .iter()
            .find(|(_, chat)| chat.current)
            .map(|(id, _)| *id)
    }

    /// Make the chat `id` the one on screen: whether that changed anything,
    /// which it does not when it already was or when no chat is `id`.
    fn put_on_screen(&mut self, id: ChatId) -> bool {
        if !self.chats.iter().any(|(each, _)| *each == id) || self.on_screen() == Some(id) {
            return false;
        }
        for (each, chat) in &mut self.chats {
            chat.current = *each == id;
        }
        true
    }

    /// Add a chat with no name yet, first, on screen.
    fn new_chat(&mut self) {
        for (_, chat) in &mut self.chats {
            chat.current = false;
        }
        let id = ChatId(self.next);
        self.next += 1;
        self.chats.insert(0, (id, Chat::new("", "").current()));
    }

    /// The first screen as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn start(&self, viewport: Size, scale: Scale) -> Start {
        Start {
            greeting: self.greeting.clone(),
            sites: Vec::new(),
            waiting: Vec::new(),
            viewport,
            scale: 1.0,
            typed: self.the_box.typed().to_owned(),
            focused: self.the_box.focused(),
            offers: self.the_box.offers(),
            card: self.card.clone(),
            results: self.results.clone(),
            search: self.search.clone(),
        }
        .at_scale(scale.factor())
    }

    /// Show the page at `url` as a card, answering what the person last
    /// chose from the box, with no frame yet (queue item 717): in place of
    /// any card shown, since one page is open at a time (ADR 0075 § 4).
    pub fn open_card(&mut self, url: Url) {
        self.card = Some(Card::new(self.said.clone(), url));
        // One thing under what the person said: the page, in place of the
        // results it was chosen from (queue item 708).
        self.results = None;
        self.card_keys = false;
    }

    /// A person pressed the card's page (queue item 718): the page has the
    /// keyboard, and the box no longer — whether the first screen is drawn
    /// again for it, which it is when the box had it. Nothing changes with
    /// no card.
    pub fn press_the_page(&mut self) -> bool {
        if self.card.is_none() {
            return false;
        }
        self.card_keys = true;
        self.the_box.blur()
    }

    /// Whether a person's keys go to the card's page rather than the box:
    /// since a press on the page, until a press anywhere else or the card
    /// goes.
    pub fn keys_to_the_card(&self) -> bool {
        self.card_keys && self.card.is_some()
    }

    /// The card's page is now at `url` — it followed a link, was sent
    /// somewhere, or moved within its document (queue item 742): whether
    /// the card shows another address for it. An address that is not a
    /// site's is not one a card shows, and the card keeps the last it
    /// showed.
    pub fn moved_card(&mut self, url: Url) -> bool {
        if !matches!(url.scheme.as_str(), "http" | "https") {
            return false;
        }
        match self.card.as_mut() {
            Some(card) if card.url != url => {
                card.url = url;
                true
            }
            _ => false,
        }
    }

    /// The card holds `frame`, Chrome's newest of its page, a PNG's bytes:
    /// whether there is a card to hold it.
    pub fn framed(&mut self, frame: Vec<u8>) -> bool {
        match self.card.as_mut() {
            Some(card) => {
                card.frame = Some(frame);
                true
            }
            None => false,
        }
    }

    /// No card: its page was closed, or never opened. Whether one was
    /// shown.
    pub fn close_card(&mut self) -> bool {
        self.card_keys = false;
        self.card.take().is_some()
    }

    /// No card, because its page could not be opened or went on its own
    /// (queue item 749): the box takes the keyboard, so the next address
    /// typed reaches it. Pressing a site's title had taken the keyboard from
    /// the box, a card with no picture yet cannot be pressed for it, and the
    /// card's failing gave it to nothing: typed by hand, the next address
    /// went nowhere. Whether the first screen shows something else.
    pub fn card_failed(&mut self) -> bool {
        let closed = self.close_card();
        self.the_box.focus() || closed
    }

    /// The page shown as a card, if one is.
    pub fn card(&self) -> Option<&Card> {
        self.card.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::conversation_decide::Refusal;
    use alo_renderer::message::Failure;
    use alo_renderer::panel::{Doing, NEW_CHAT};

    /// The labels of the panel's chats, in order, and which is on screen.
    fn rows(conversation: &Conversation) -> (Vec<String>, Vec<bool>) {
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        (
            panel
                .chats
                .iter()
                .map(|chat| chat.label().to_owned())
                .collect(),
            panel.chats.iter().map(|chat| chat.current).collect(),
        )
    }

    #[test]
    fn a_new_conversation_is_one_new_chat_on_screen() {
        let panel = Conversation::default().panel(Size::new(240.0, 820.0), Scale::ONE);
        let [chat] = panel.chats.as_slice() else {
            panic!("one chat: {panel:?}");
        };
        assert_eq!(chat.label(), NEW_CHAT);
        assert!(chat.open && chat.current);
        assert_eq!(chat.doing, Doing::Nothing);
        assert_eq!(chat.mark(), None, "no site is open in it");
        assert_eq!(panel.viewport, Size::new(240.0, 820.0));
    }

    #[test]
    fn the_first_screen_greets_and_shows_nothing_nobody_kept() {
        let start = Conversation::default().start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.greeting, GREETING);
        assert!(start.sites.is_empty());
        assert!(start.waiting.is_empty());
        assert_eq!(start.viewport, Size::new(1040.0, 820.0));
    }

    #[test]
    fn a_page_opened_is_a_card_answering_what_was_said_until_it_closes() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        let _ = conversation.edit(Edit::Put("example.com".to_owned()));
        assert!(matches!(conversation.edit(Edit::Choose), Typed::Chose(_)));
        let url = conversation.going().cloned().unwrap();
        assert!(!conversation.framed(vec![1]), "no card to hold a frame");
        conversation.open_card(url.clone());
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        let card = start.card.unwrap();
        assert_eq!(card.said, "example.com");
        assert_eq!(card.url, url);
        assert_eq!(card.frame, None);
        assert!(
            conversation.showing().card,
            "a press on its buttons is decided"
        );
        assert!(conversation.framed(vec![1, 2]));
        assert_eq!(
            conversation.card().and_then(|card| card.frame.clone()),
            Some(vec![1, 2])
        );
        assert!(conversation.close_card());
        assert!(!conversation.close_card());
        assert!(!conversation.showing().card);
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .card,
            None
        );
    }

    #[test]
    fn a_press_on_the_page_gives_it_the_keyboard_until_a_press_elsewhere() {
        let mut conversation = Conversation::default();
        assert!(!conversation.press_the_page(), "no card, no page to press");
        assert!(!conversation.keys_to_the_card());
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        assert!(matches!(conversation.edit(Edit::Choose), Typed::Chose(_)));
        let url = conversation.going().cloned().unwrap();
        conversation.open_card(url.clone());
        assert!(!conversation.keys_to_the_card(), "the box keeps it");
        assert!(conversation.press_the_page(), "the box drawn again");
        assert!(conversation.keys_to_the_card());
        assert!(
            !conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .focused
        );
        // Typing does not reach the box, emptied as the site was gone to.
        type_in(&mut conversation, "x");
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            ""
        );
        assert!(!conversation.press_the_page(), "already the page's");
        // A refused press leaves it where it was; the card's bar takes it.
        let _ = conversation.act(Pressed::Refused(Refusal::Unasked));
        assert!(conversation.keys_to_the_card());
        let _ = conversation.act(Pressed::Nothing);
        assert!(!conversation.keys_to_the_card());
        // The box takes it back from the page.
        let _ = conversation.press_the_page();
        let _ = conversation.act(Pressed::TheBox);
        assert!(!conversation.keys_to_the_card());
        // The card closed takes it.
        let _ = conversation.press_the_page();
        assert!(conversation.close_card());
        assert!(!conversation.keys_to_the_card());
        conversation.open_card(url);
        assert!(!conversation.keys_to_the_card(), "a new card has not it");
    }

    #[test]
    fn new_chat_adds_one_first_and_on_screen() {
        let mut conversation = Conversation::default();
        let first = conversation.on_screen();
        assert!(
            conversation.act(Pressed::NewChat),
            "the panel is drawn again"
        );
        assert_eq!(
            rows(&conversation),
            (
                vec![NEW_CHAT.to_owned(), NEW_CHAT.to_owned()],
                vec![true, false]
            )
        );
        let showing = conversation.showing();
        assert_eq!(showing.chats.len(), 2);
        assert_eq!(showing.chats.last().copied(), first);
        assert_ne!(conversation.on_screen(), first, "a chat of its own");
        assert_eq!(conversation.last_pressed(), Some(&Pressed::NewChat));
    }

    #[test]
    fn a_chat_pressed_becomes_the_one_on_screen() {
        let mut conversation = Conversation::default();
        let first = conversation.on_screen();
        let _ = conversation.act(Pressed::NewChat);
        let Some(first) = first else {
            panic!("a new conversation has a chat on screen");
        };
        assert!(conversation.act(Pressed::Chat(first)));
        assert_eq!(conversation.on_screen(), Some(first));
        assert_eq!(rows(&conversation).1, vec![false, true]);
        // Pressing it again changes nothing.
        assert!(!conversation.act(Pressed::Chat(first)));
        assert_eq!(conversation.last_pressed(), Some(&Pressed::Chat(first)));
    }

    #[test]
    fn a_chat_it_does_not_hold_changes_nothing() {
        let mut conversation = Conversation::default();
        let before = conversation.clone();
        assert!(!conversation.act(Pressed::Chat(ChatId(99))));
        assert_eq!(rows(&conversation), rows(&before));
        assert_eq!(conversation.on_screen(), before.on_screen());
    }

    #[test]
    fn every_other_press_is_kept_and_changes_nothing_yet() {
        for pressed in [
            Pressed::Settings,
            Pressed::Site(KeptSite::new("Mail", "https://mail.example/")),
            Pressed::AddASite,
            Pressed::Waiting(ChatId(0)),
            Pressed::Go,
            Pressed::Nothing,
            Pressed::Refused(Refusal::Unasked),
            Pressed::Failed(Failure::NotTheConversation),
        ] {
            let mut conversation = Conversation::default();
            let before = conversation.clone();
            assert!(!conversation.act(pressed.clone()), "{pressed:?}");
            assert_eq!(
                conversation.panel(Size::new(240.0, 820.0), Scale::ONE),
                before.panel(Size::new(240.0, 820.0), Scale::ONE),
                "{pressed:?}"
            );
            assert_eq!(
                conversation.start(Size::new(1040.0, 820.0), Scale::ONE),
                before.start(Size::new(1040.0, 820.0), Scale::ONE),
                "{pressed:?}"
            );
            assert_eq!(conversation.last_pressed(), Some(&pressed));
        }
    }

    #[test]
    fn what_is_showing_is_in_the_order_the_panel_draws() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::NewChat);
        let ids: Vec<ChatId> = conversation.chats.iter().map(|(id, _)| *id).collect();
        assert_eq!(conversation.showing().chats, ids);
        assert_eq!(ids, vec![ChatId(2), ChatId(1), ChatId(0)]);
        assert!(conversation.showing().sites.is_empty());
        assert!(conversation.showing().waiting.is_empty());
    }

    #[test]
    fn both_are_drawn_at_the_windows_scale() {
        let conversation = Conversation::default();
        for scale in [Scale::ONE, Scale::ONE_AND_A_HALF, Scale::TWO] {
            let panel = conversation.panel(Size::new(240.0, 410.0), scale);
            let start = conversation.start(Size::new(520.0, 410.0), scale);
            assert_eq!(panel.scale.to_bits(), scale.factor().to_bits());
            assert_eq!(start.scale.to_bits(), scale.factor().to_bits());
        }
    }

    fn found(address: &str) -> alo_renderer::results::Found {
        alo_renderer::results::Found::new("A site", alo_url::parse(address).unwrap(), "Of it.")
    }

    #[test]
    fn a_search_set_up_shows_its_results_under_what_was_said_and_empties_the_box() {
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "ev charger");
        assert_eq!(
            conversation.edit(Edit::Choose),
            Typed::Chose(Offer::Search("ev charger".to_owned()))
        );
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.typed, "", "the box is emptied");
        assert_eq!(start.search, SetUp::Ready);
        assert_eq!(start.results, Some(Results::searching("ev charger")));
        assert_eq!(conversation.searching(), Some((1, "ev charger")));
        // An older search's answer is not shown; the newest's is.
        let answer = |number| Searched {
            number,
            words: "ev charger".to_owned(),
            showing: results::Showing::Found(vec![
                found("https://one.example/"),
                found("https://two.example/"),
                found("https://three.example/"),
                found("https://four.example/"),
            ]),
        };
        assert!(!conversation.searched(answer(0)));
        assert!(conversation.searched(answer(1)));
        assert_eq!(conversation.searching(), None, "answered");
        let shown: Vec<String> = conversation
            .showing()
            .found
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            shown,
            vec![
                "https://one.example/",
                "https://two.example/",
                "https://three.example/"
            ],
            "what the All tab shows, which a press is decided against"
        );
        // A title pressed is chosen as an address is, under the words.
        let two = alo_url::parse("https://two.example/").unwrap();
        let _ = conversation.act(Pressed::Found(two.clone()));
        assert_eq!(conversation.going(), Some(&two));
        conversation.open_card(two);
        assert_eq!(
            conversation.card().map(|card| card.said.as_str()),
            Some("ev charger")
        );
        assert_eq!(conversation.results(), None, "the card in its place");
        assert!(conversation.showing().found.is_empty());
    }

    #[test]
    fn a_search_puts_a_card_away_and_one_not_set_up_keeps_what_was_typed() {
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "what is x");
        let _ = conversation.edit(Edit::Choose);
        assert_eq!(conversation.card(), None);
        assert!(conversation.results().is_some());

        let mut unset = Conversation::default();
        unset.set_search(SetUp::Not("no key".to_owned()));
        let _ = unset.act(Pressed::TheBox);
        type_in(&mut unset, "what is x");
        let _ = unset.edit(Edit::Choose);
        let start = unset.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.typed, "what is x");
        assert_eq!(start.results, None);
        assert_eq!(unset.searching(), None);
    }

    /// Type `text` into `conversation`'s box, a key at a time.
    fn type_in(conversation: &mut Conversation, text: &str) {
        for letter in text.chars() {
            let _ = conversation.edit(Edit::Put(letter.to_string()));
        }
    }

    #[test]
    fn a_press_on_the_box_gives_it_the_keyboard_and_typing_reaches_only_the_first_screen() {
        let mut conversation = Conversation::default();
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        type_in(&mut conversation, "ignored");
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            ""
        );
        assert!(conversation.act(Pressed::TheBox), "drawn again, focused");
        assert!(!conversation.act(Pressed::TheBox), "already focused");
        type_in(&mut conversation, "example.com");
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.typed, "example.com");
        assert!(start.focused);
        assert_eq!(start.offers, alo_renderer::typed::offers("example.com"));
        assert_eq!(
            conversation.panel(Size::new(240.0, 820.0), Scale::ONE),
            panel,
            "the panel is sent nothing of it"
        );
    }

    #[test]
    fn typing_example_com_and_return_decides_to_go_to_https_example_com() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        assert_eq!(conversation.going(), None);
        let typed = conversation.edit(Edit::Choose);
        assert!(matches!(typed, Typed::Chose(Offer::Go(_))), "{typed:?}");
        assert_eq!(
            conversation.going().map(ToString::to_string).as_deref(),
            Some("https://example.com/")
        );
        // Gone to, the box is emptied and offers nothing, keeping the
        // keyboard (queue item 742): the person's line above the card holds
        // the address.
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.typed, "");
        assert!(start.offers.is_empty());
        assert!(start.focused);
    }

    #[test]
    fn go_chooses_as_return_does() {
        let mut conversation = Conversation::default();
        assert!(!conversation.act(Pressed::Go));
        assert_eq!(conversation.chosen(), None, "nothing typed, nothing chosen");
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "localhost:8080");
        assert!(
            conversation.act(Pressed::Go),
            "the box emptied is drawn again"
        );
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            ""
        );
        assert_eq!(
            conversation.going().map(ToString::to_string).as_deref(),
            Some("https://localhost:8080/")
        );
    }

    #[test]
    fn return_on_a_question_decides_to_go_nowhere() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "what is x");
        let typed = conversation.edit(Edit::Choose);
        assert_eq!(typed, Typed::Chose(Offer::Search("what is x".to_owned())));
        assert_eq!(conversation.going(), None);
        // Not gone anywhere, what was typed stays.
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            "what is x"
        );
    }

    #[test]
    fn the_card_shows_where_its_page_is_now() {
        let mut conversation = Conversation::default();
        let url = |text: &str| alo_url::parse(text).unwrap();
        assert!(
            !conversation.moved_card(url("https://example.com/b")),
            "no card"
        );
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let _ = conversation.edit(Edit::Choose);
        conversation.open_card(url("https://example.com/"));
        assert!(conversation.moved_card(url("https://example.com/wiki/Talk:Main_Page")));
        assert!(!conversation.moved_card(url("https://example.com/wiki/Talk:Main_Page")));
        // Not a site's address: the card keeps the last it showed.
        assert!(!conversation.moved_card(url("about:blank")));
        let card = conversation
            .start(Size::new(1040.0, 820.0), Scale::ONE)
            .card
            .unwrap();
        assert_eq!(
            card.url.to_string(),
            "https://example.com/wiki/Talk:Main_Page"
        );
        assert_eq!(card.said, "example.com", "what the person said stays");
    }

    #[test]
    fn a_press_elsewhere_takes_the_keyboard_and_hides_the_offers() {
        for elsewhere in [Pressed::Nothing, Pressed::Settings, Pressed::AddASite] {
            let mut conversation = Conversation::default();
            let _ = conversation.act(Pressed::TheBox);
            type_in(&mut conversation, "example.com");
            assert!(conversation.act(elsewhere.clone()), "{elsewhere:?}");
            let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
            assert!(!start.focused);
            assert_eq!(start.typed, "example.com", "kept as the field's value");
            assert!(start.offers.is_empty());
            type_in(&mut conversation, "x");
            assert_eq!(
                conversation
                    .start(Size::new(1040.0, 820.0), Scale::ONE)
                    .typed,
                "example.com"
            );
        }
        // A refused claim leaves the keyboard where it was.
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        assert!(!conversation.act(Pressed::Refused(Refusal::Unasked)));
        assert!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .focused
        );
    }
}
