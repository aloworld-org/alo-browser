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
//! Queue item 703 makes the conversation keep every turn: each chat holds
//! its own ([`crate::conversation_thread`]) — a search and a page opened
//! each a turn under the last, a card put away as its address when
//! anything newer comes — and the first screen shows the turns of the chat
//! on screen, scrolled as a person's wheel scrolled them
//! ([`Conversation::scroll`]). *New chat* starts with none; a chat pressed
//! in the panel shows its own. A press on a site's title or a page shown
//! earlier, in any turn, opens it as a new turn, its bubble the words it
//! was drawn with ([`Conversation::choose_found`]).
//!
//! Queue item 714 adds a press on an offer for what was typed: chosen as
//! Return chooses the first ([`Conversation::edit`]), whichever it is.
//!
//! Queue item 711 adds the panel's *Search chats* and the button that hides
//! it. *Search chats* takes the keyboard as the box does
//! ([`crate::conversation_find`]); while it has it a person's keys edit it,
//! and the panel shows only the chats whose names hold what is typed —
//! found here, so which chat a press on a row names is still the browser
//! process's to say. Hiding the panel draws it as the design's rail, and
//! the window lays the first screen out beside the rail
//! ([`Conversation::side`]); the rail's *Show the panel* brings it back,
//! and its *New chat* is the panel's.
//!
//! Every other — *Settings*, a kept site, *Add a site*, a waiting chat — is
//! decided and kept as [`Conversation::last_pressed`], and **changes
//! nothing yet**: there is no settings screen (705), and nothing keeps a
//! site or a waiting chat.
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
use crate::conversation_find::FindChats;
use crate::conversation_place::Side;
use crate::conversation_search::{Came, Searched};
use crate::conversation_thread::Thread;
use crate::scale::Scale;
use crate::typing::Edit;
use alo_layout::Size;
use alo_renderer::card::Card;
use alo_renderer::conversation_decide::{Decided, Shown};
use alo_renderer::conversation_scroll::Reach;
use alo_renderer::panel::{Chat, Panel, drawn_order, matching};
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
#[derive(Debug, Clone, PartialEq)]
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
    /// Each chat's turns and how far they are scrolled (queue item 703),
    /// by the chat's identity: the live card, if any, is the newest turn of
    /// the chat on screen (ADR 0076 B, queue item 717).
    threads: Vec<(ChatId, Thread)>,
    /// Whether the card's page has the keyboard (queue item 718): given it
    /// by a press on the page, taken by a press anywhere else.
    card_keys: bool,
    /// Whether searching can be done (queue item 708).
    search: SetUp,
    /// How many searches were chosen: the newest one's number.
    searches: u64,
    /// The panel's *Search chats* (queue item 711).
    find: FindChats,
    /// Whether a person hid the panel, which is then the rail.
    hidden: bool,
    /// How many tabs were asked for (queue items 745, 746): the newest
    /// one's number.
    further: u64,
    /// The tabs opened whose results are to be asked for, not yet handed to
    /// the searcher.
    to_ask: Vec<Further>,
    /// The tabs asked for and not yet answered: each one's number, and the
    /// chat and turn it was opened in.
    asked: Vec<(u64, ChatId, usize)>,
}

/// A tab opened whose results are to be asked of the provider — Videos or
/// News, once for a turn (queue items 745, 746, ADR 0087).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Further {
    /// The number its answer comes back under.
    pub number: u64,
    /// The turn's words, asked alone.
    pub words: String,
    /// Which tab: [`results::Tab::Videos`] or [`results::Tab::News`].
    pub tab: results::Tab,
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
            threads: vec![(ChatId(0), Thread::default())],
            card_keys: false,
            search: SetUp::Not(String::new()),
            searches: 0,
            find: FindChats::default(),
            hidden: false,
            further: 0,
            to_ask: Vec::new(),
            asked: Vec::new(),
        }
    }
}

impl Conversation {
    /// The chats panel as it is now, laid out at `viewport` on a screen of
    /// `scale`: the chats *Search chats* finds, and what is typed into it,
    /// or — hidden — the rail, which shows no chat (queue item 711).
    pub fn panel(&self, viewport: Size, scale: Scale) -> Panel {
        Panel {
            chats: self.listed().map(|(_, chat)| chat.clone()).collect(),
            viewport,
            scale: 1.0,
            searched: self.find.typed().to_owned(),
            searching: self.find.focused(),
            hidden: self.hidden,
        }
        .at_scale(scale.factor())
    }

    /// The chats the panel shows, with their identities, in the order the
    /// browser process keeps them: those whose names hold what *Search
    /// chats* holds, and none while the panel is hidden.
    fn listed(&self) -> impl Iterator<Item = &(ChatId, Chat)> {
        let searched = self.find.typed();
        self.chats
            .iter()
            .filter(move |(_, chat)| !self.hidden && matching(chat, searched))
    }

    /// What stands down the window's left: the panel, or its rail while a
    /// person has hidden it (queue item 711).
    pub fn side(&self) -> Side {
        if self.hidden { Side::Rail } else { Side::Panel }
    }

    /// What is typed into *Search chats*, and whether it has the keyboard.
    pub fn find(&self) -> &FindChats {
        &self.find
    }

    /// What the panel and the first screen show as they are now, each list
    /// in the order its part draws it: what a claim on either is decided
    /// against once they are painted.
    pub fn showing(&self) -> Showing {
        let listed: Vec<&(ChatId, Chat)> = self.listed().collect();
        let chats: Vec<Chat> = listed.iter().map(|(_, chat)| chat.clone()).collect();
        Shown {
            chats: drawn_order(&chats)
                .into_iter()
                .filter_map(|place| listed.get(place).map(|(id, _)| *id))
                .collect(),
            sites: Vec::new(),
            waiting: Vec::new(),
            card: self.card().is_some(),
            found: self.thread().map(Thread::links).unwrap_or_default(),
            offers: self.the_box.offers(),
            hidden: self.hidden,
            tabs: self
                .on_this_chat(Thread::tabs)
                .into_iter()
                .map(|(id, (turn, tab))| (id, turn, tab))
                .collect(),
            more: self.on_this_chat(Thread::more_sites),
            see_all: self.on_this_chat(Thread::see_all),
            pictured: self
                .thread()
                .map(Thread::pictured_pages)
                .unwrap_or_default(),
        }
    }

    /// Act on what a press came to, and keep it as the last pressed:
    /// whether the panel or the first screen now shows something else, and
    /// so is to be drawn again.
    ///
    /// A chat becomes the one on screen; *New chat* adds one, on screen;
    /// the box takes the keyboard, and *Go* chooses what it offers first,
    /// and an offer itself (queue item 714), kept as
    /// [`Conversation::chosen`]. *Search chats* takes the keyboard, and the
    /// button that hides the panel, or the rail's that shows it, does so
    /// (queue item 711). A press on anything but the box, *Go* and an offer
    /// takes the keyboard from the box, and on anything but *Search chats*
    /// from it. Everything else changes nothing yet (see the module's
    /// words), and neither does a chat this does not hold, which the
    /// decision refused unless it went between being decided and being
    /// acted on, nor a press refused or unanswered.
    pub fn act(&mut self, pressed: Pressed) -> bool {
        let changed = match &pressed {
            Pressed::TheBox => self.the_box.focus(),
            Pressed::Go => {
                let choice = self.the_box.choice();
                self.chose(choice)
            }
            // Chosen as Return chooses the first: the box keeps the
            // keyboard, emptied when a site or a search was chosen.
            Pressed::Offer(offer) => {
                let offer = offer.clone();
                self.chose(Some(offer))
            }
            Pressed::Refused(_) | Pressed::Failed(_) => false,
            _ => self.the_box.blur(),
        };
        let changed = match &pressed {
            Pressed::Refused(_) | Pressed::Failed(_) => changed,
            Pressed::SearchChats => self.find.focus() || changed,
            _ => self.find.blur() || changed,
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
            // An image opens its page as a site's title does (queue item
            // 743); a tab, or *More sites ›* or *See all images*, shows
            // that tab of the search pressed (743, 744).
            Pressed::Image(url) => {
                self.choose_image(url.clone());
                changed
            }
            // Videos and News are asked for the first time either is shown
            // in a turn, and never again (745, 746, ADR 0087).
            Pressed::Tab { chat, turn, tab } => {
                let Some((_, thread)) = self.threads.iter_mut().find(|(each, _)| each == chat)
                else {
                    return self.pressed(pressed, changed);
                };
                let shown = thread.show_tab(*turn, *tab);
                if shown && let Some(words) = thread.ask_for(*turn, *tab) {
                    self.further += 1;
                    self.asked.push((self.further, *chat, *turn));
                    self.to_ask.push(Further {
                        number: self.further,
                        words,
                        tab: *tab,
                    });
                }
                shown || changed
            }
            Pressed::NewChat => {
                self.new_chat();
                true
            }
            Pressed::HideThePanel => !std::mem::replace(&mut self.hidden, true) || changed,
            Pressed::ShowThePanel => std::mem::replace(&mut self.hidden, false) || changed,
            _ => changed,
        };
        self.pressed(pressed, changed)
    }

    /// Keep `pressed` as the last pressed, and say whether it `changed`
    /// what is shown.
    fn pressed(&mut self, pressed: Pressed, changed: bool) -> bool {
        self.last_pressed = Some(pressed);
        changed
    }

    /// The tabs opened whose results are now to be asked for, each once:
    /// the window hands them to the searcher.
    pub fn take_asks(&mut self) -> Vec<Further> {
        std::mem::take(&mut self.to_ask)
    }

    /// Make a person's `edit` to the box — or to *Search chats*, while it
    /// has the keyboard — which changes nothing unless one of them has it:
    /// what it came to. What is chosen is kept as
    /// [`Conversation::chosen`].
    pub fn edit(&mut self, edit: Edit) -> Typed {
        // *Search chats* has the keyboard (queue item 711): its words, not
        // the box's.
        if self.find.focused() {
            return if self.find.edit(edit) {
                Typed::Changed
            } else {
                Typed::Unchanged
            };
        }
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
                let words = words.clone();
                if let Some(thread) = self.thread_mut() {
                    thread.search(words);
                }
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
        match (&self.chosen, self.results()) {
            (
                Some(Offer::Search(_)),
                Some(Results {
                    said,
                    showing: results::Showing::Searching,
                    ..
                }),
            ) => Some((self.searches, said.as_str())),
            _ => None,
        }
    }

    /// What came of a search: its sites shown, if it is the newest and its
    /// results are still shown; its images (queue item 743) shown in the
    /// turn still asking for them, of any chat, whichever search is newest,
    /// since a turn asking would otherwise ask for ever. Whether the first
    /// screen shows something else.
    pub fn searched(&mut self, searched: Searched) -> bool {
        match searched.came {
            Came::Sites { showing, images } => {
                if searched.number != self.searches {
                    return false;
                }
                self.thread_mut()
                    .is_some_and(|thread| thread.searched(&searched.words, showing, images))
            }
            Came::Images(images) => {
                let on_screen = self.on_screen();
                let mut shown = false;
                for (id, thread) in &mut self.threads {
                    if thread.pictured(&searched.words, images.clone()) {
                        shown = shown || Some(*id) == on_screen;
                        break;
                    }
                }
                shown
            }
            Came::Marks(marks) => {
                let on_screen = self.on_screen();
                let mut shown = false;
                for (id, thread) in &mut self.threads {
                    if thread.marked(&searched.words, &marks) {
                        shown = shown || Some(*id) == on_screen;
                        break;
                    }
                }
                shown
            }
            Came::Videos(videos) => self.further_came(searched.number, |thread, turn| {
                thread.videos_came(turn, videos)
            }),
            Came::News(news) => {
                self.further_came(searched.number, |thread, turn| thread.news_came(turn, news))
            }
        }
    }

    /// What came of the tab asked for under `number`, put in the turn it was
    /// opened in by `put`, in whichever chat: whether the chat on screen now
    /// shows something else.
    fn further_came(&mut self, number: u64, put: impl FnOnce(&mut Thread, usize) -> bool) -> bool {
        let Some(at) = self.asked.iter().position(|(each, _, _)| *each == number) else {
            return false;
        };
        let (_, chat, turn) = self.asked.remove(at);
        let on_screen = self.on_screen() == Some(chat);
        self.threads
            .iter_mut()
            .find(|(each, _)| *each == chat)
            .is_some_and(|(_, thread)| put(thread, turn))
            && on_screen
    }

    /// The newest search's results in the chat on screen, if a turn holds
    /// any.
    pub fn results(&self) -> Option<&Results> {
        self.thread().and_then(Thread::results)
    }

    /// A person pressed the title of the site found at `url` (queue item
    /// 708), or a page shown earlier at it (703), in any turn: it is chosen
    /// as an address from the box is, and opens as a new turn whose bubble
    /// is the words it was drawn with — the site's title, or what was said
    /// for the page.
    pub fn choose_found(&mut self, url: alo_url::Url) {
        if let Some(said) = self.thread().and_then(|thread| thread.said_for(&url)) {
            self.said = said;
        }
        self.chosen = Some(Offer::Go(url));
    }

    /// A person pressed an image found on the page at `url` (queue item
    /// 743): its page is chosen as a site's title's is, and opens as a new
    /// turn whose bubble is the image's title.
    pub fn choose_image(&mut self, url: alo_url::Url) {
        if let Some(said) = self.thread().and_then(|thread| thread.said_for_image(&url)) {
            self.said = said;
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
        // One page is open at a time, and it was the chat before's.
        let _ = self.close_card();
        for (each, chat) in &mut self.chats {
            chat.current = *each == id;
        }
        true
    }

    /// The turns of the chat on screen, if one is.
    pub fn thread(&self) -> Option<&Thread> {
        let id = self.on_screen()?;
        self.threads
            .iter()
            .find(|(each, _)| *each == id)
            .map(|(_, thread)| thread)
    }

    /// What `of` lists of the chat on screen's turns, each named with the
    /// chat: what a press on a tab is decided against (queue item 744).
    fn on_this_chat<T>(&self, of: impl Fn(&Thread) -> Vec<T>) -> Vec<(ChatId, T)> {
        match (self.on_screen(), self.thread()) {
            (Some(id), Some(thread)) => of(thread).into_iter().map(|each| (id, each)).collect(),
            _ => Vec::new(),
        }
    }

    /// The turns of the chat on screen, to change.
    fn thread_mut(&mut self) -> Option<&mut Thread> {
        let id = self.on_screen()?;
        self.threads
            .iter_mut()
            .find(|(each, _)| *each == id)
            .map(|(_, thread)| thread)
    }

    /// Add a chat with no name yet, first, on screen, with no turns: the
    /// live card, which was the chat before's, put away.
    fn new_chat(&mut self) {
        let _ = self.close_card();
        for (_, chat) in &mut self.chats {
            chat.current = false;
        }
        let id = ChatId(self.next);
        self.next += 1;
        self.chats.insert(0, (id, Chat::new("", "").current()));
        self.threads.push((id, Thread::default()));
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
            // Only the pictures each tab draws, the newest turns' first,
            // within the first screen's budget (queue item 743).
            turns: self
                .thread()
                .map(|thread| results::within_budget(thread.turns()))
                .unwrap_or_default(),
            down: self.thread().map_or(0.0, Thread::down),
            search: self.search.clone(),
        }
        .at_scale(scale.factor())
    }

    /// Show the page at `url` as a card, answering what the person last
    /// chose, with no frame yet (queue item 717), as a new turn (703): any
    /// card shown put away as its address first, since one page is open at
    /// a time (ADR 0075 § 4).
    pub fn open_card(&mut self, url: Url) {
        let said = self.said.clone();
        if let Some(thread) = self.thread_mut() {
            thread.open_card(said, url);
        }
        self.card_keys = false;
    }

    /// A person pressed the card's page (queue item 718): the page has the
    /// keyboard, and the box no longer — whether the first screen is drawn
    /// again for it, which it is when the box had it. Nothing changes with
    /// no card.
    pub fn press_the_page(&mut self) -> bool {
        if self.card().is_none() {
            return false;
        }
        self.card_keys = true;
        let searching = self.find.blur();
        self.the_box.blur() || searching
    }

    /// Whether a person's keys go to the card's page rather than the box:
    /// since a press on the page, until a press anywhere else or the card
    /// goes.
    pub fn keys_to_the_card(&self) -> bool {
        self.card_keys && self.card().is_some()
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
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) if card.url != url => {
                card.url = url;
                true
            }
            _ => false,
        }
    }

    /// The card's page is `loading`, or has loaded (queue item 751): the
    /// card says *loading* until it has. Whether the card shows something
    /// else for it.
    pub fn card_loading(&mut self, loading: bool) -> bool {
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) if card.loading != loading => {
                card.loading = loading;
                true
            }
            _ => false,
        }
    }

    /// The card holds `frame`, Chrome's newest of its page, a PNG's bytes:
    /// whether there is a card to hold it.
    pub fn framed(&mut self, frame: Vec<u8>) -> bool {
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) => {
                card.frame = Some(frame);
                true
            }
            None => false,
        }
    }

    /// No live card: its page was closed, and the card put away as the
    /// address it was last at (queue item 703). Whether one was shown.
    pub fn close_card(&mut self) -> bool {
        self.card_keys = false;
        self.thread_mut().is_some_and(Thread::put_away_card)
    }

    /// No live card, because its page could not be opened, for `why` — said
    /// in its turn's place — or went on its own, `why` then [`None`] and the
    /// card put away as its address (queue items 749, 703): the box takes
    /// the keyboard, so the next address
    /// typed reaches it. Pressing a site's title had taken the keyboard from
    /// the box, a card with no picture yet cannot be pressed for it, and the
    /// card's failing gave it to nothing: typed by hand, the next address
    /// went nowhere. Whether the first screen shows something else.
    pub fn card_failed(&mut self, why: Option<String>) -> bool {
        let closed = match why {
            Some(why) => {
                self.card_keys = false;
                self.thread_mut()
                    .is_some_and(|thread| thread.card_failed(why))
            }
            None => self.close_card(),
        };
        self.the_box.focus() || closed
    }

    /// The page shown live as a card, if the chat on screen's newest turn is
    /// one.
    pub fn card(&self) -> Option<&Card> {
        self.thread().and_then(Thread::card)
    }

    /// A person's wheel turned `by` CSS pixels down over the conversation,
    /// laid out at `viewport` (queue item 703): whether it scrolled.
    pub fn scroll(&mut self, by: f32, viewport: Size) -> bool {
        self.thread_mut()
            .is_some_and(|thread| thread.scroll(by, viewport))
    }

    /// The first screen's renderer measured `drawn` turns of the chat on
    /// screen to reach as `reach` says, laid out at `viewport`: whether
    /// keeping inside it scrolled the conversation.
    pub fn reached(&mut self, reach: Reach, drawn: usize, viewport: Size) -> bool {
        self.thread_mut()
            .is_some_and(|thread| thread.reached(reach, drawn, viewport))
    }

    /// Whether the first screen is a conversation: the chat on screen holds
    /// a turn.
    pub fn conversing(&self) -> bool {
        self.thread()
            .is_some_and(|thread| !thread.turns().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::conversation_decide::Refusal;
    use alo_renderer::message::Failure;
    use alo_renderer::panel::{Doing, NEW_CHAT};
    use alo_renderer::turn::Turn;

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
        let card = start.card().cloned().unwrap();
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
                .card()
                .cloned(),
            None
        );
        // Closed, the card is kept as the address its page was at (queue
        // item 703).
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .turns,
            vec![Turn::Visited {
                said: "example.com".to_owned(),
                url
            }]
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
    fn each_chat_keeps_its_own_turns_and_a_new_one_starts_empty() {
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let first = conversation.on_screen();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let _ = conversation.edit(Edit::Choose);
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        assert!(conversation.conversing());
        // New chat: empty, the card put away in the chat it was in.
        assert!(conversation.act(Pressed::NewChat));
        assert!(!conversation.conversing());
        assert_eq!(conversation.card(), None);
        assert!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .turns
                .is_empty()
        );
        let Some(first) = first else {
            panic!("a chat was on screen");
        };
        assert!(conversation.act(Pressed::Chat(first)));
        let turns = conversation
            .start(Size::new(1040.0, 820.0), Scale::ONE)
            .turns;
        assert!(
            matches!(turns.as_slice(), [Turn::Visited { said, .. }] if said == "example.com"),
            "{turns:?}"
        );
    }

    #[test]
    fn a_page_not_opened_says_why_in_its_turn_and_one_gone_leaves_its_address() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "a.example");
        let _ = conversation.edit(Edit::Choose);
        conversation.open_card(alo_url::parse("https://a.example/").unwrap());
        let _ = conversation.act(Pressed::Nothing);
        assert!(
            conversation.card_failed(Some("https://a.example/ could not be opened: no".into()))
        );
        assert!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .focused,
            "the box given the keyboard"
        );
        conversation.open_card(alo_url::parse("https://b.example/").unwrap());
        assert!(conversation.card_failed(None));
        let turns = conversation
            .start(Size::new(1040.0, 820.0), Scale::ONE)
            .turns;
        assert!(
            matches!(
                turns.as_slice(),
                [Turn::Failed { why, .. }, Turn::Visited { .. }] if why.ends_with(": no")
            ),
            "{turns:?}"
        );
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
    fn each_sites_mark_is_put_beside_it_once_by_its_place() {
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "ev");
        let _ = conversation.edit(Edit::Choose);
        assert!(conversation.searched(Searched::sites(
            1,
            "ev",
            results::Showing::Found(vec![found("https://one.example/"); 3]),
        )));
        let marks = |first: u8| Searched {
            number: 1,
            words: "ev".to_owned(),
            came: Came::Marks(vec![Some(vec![first]), None, Some(vec![3])]),
        };
        assert!(
            !conversation.searched(Searched {
                came: Came::Marks(vec![Some(vec![1])]),
                ..marks(1)
            }),
            "marks for another number of sites are no search's"
        );
        assert!(conversation.searched(marks(1)));
        let held: Vec<Option<Vec<u8>>> =
            match conversation.results().map(|results| &results.showing) {
                Some(results::Showing::Found(found)) => {
                    found.iter().map(|site| site.mark.clone()).collect()
                }
                _ => Vec::new(),
            };
        assert_eq!(held, vec![Some(vec![1]), None, Some(vec![3])]);
        assert!(
            !conversation.searched(marks(9)),
            "a turn already marked is not marked again"
        );
    }

    #[test]
    fn a_searchs_images_a_tab_and_an_image_are_pressed_as_the_browser_holds_them() {
        use alo_renderer::results::{Images, Pictured, Tab};
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "ev");
        let _ = conversation.edit(Edit::Choose);
        let chat = conversation.on_screen().unwrap();
        let sites = Searched {
            number: 1,
            words: "ev".to_owned(),
            came: Came::Sites {
                showing: results::Showing::Found(vec![found("https://one.example/"); 4]),
                images: Images::Asking,
            },
        };
        assert!(conversation.searched(sites));
        let pictures: Vec<Pictured> = (0..6)
            .map(|n| {
                Pictured::new(
                    format!("Picture {n}"),
                    alo_url::parse(&format!("https://p{n}.example/")).unwrap(),
                    "",
                )
                .holding(vec![0x89, b'P', b'N', b'G', n])
            })
            .collect();
        // Images come for any search still asking, even once a newer one
        // was numbered.
        let images = Searched {
            number: 7,
            words: "ev".to_owned(),
            came: Came::Images(Images::Found(pictures)),
        };
        assert!(conversation.searched(images));
        let shown = conversation.showing();
        assert_eq!(
            shown.tabs,
            vec![
                (chat, 0, Tab::All),
                (chat, 0, Tab::Sites),
                (chat, 0, Tab::Images),
                (chat, 0, Tab::Videos),
                (chat, 0, Tab::News)
            ]
        );
        assert_eq!(shown.more, vec![(chat, 0)]);
        assert_eq!(shown.see_all, vec![(chat, 0)]);
        assert_eq!(shown.pictured.len(), 5);
        // Only the five pictures All draws are sent.
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        let Some(Turn::Searched(sent)) = start.turns.first() else {
            panic!("no search sent");
        };
        assert_eq!(sent.picture_bytes(), 25);
        // A tab pressed shows it; an image pressed opens its page with its
        // title as what was said.
        assert!(conversation.act(Pressed::Tab {
            chat,
            turn: 0,
            tab: Tab::Images
        }));
        assert_eq!(conversation.showing().pictured.len(), 6);
        let page = alo_url::parse("https://p5.example/").unwrap();
        let _ = conversation.act(Pressed::Image(page.clone()));
        assert_eq!(conversation.going(), Some(&page));
        conversation.open_card(page);
        assert_eq!(
            conversation.card().map(|card| card.said.as_str()),
            Some("Picture 5")
        );
        assert!(!conversation.act(Pressed::Tab {
            chat: ChatId(99),
            turn: 0,
            tab: Tab::All
        }));
    }

    #[test]
    fn videos_and_news_are_asked_once_for_a_turn_when_their_tab_is_opened() {
        use alo_renderer::results::{Images, Stories, Tab, Videos};
        use alo_renderer::results_media::{Story, Video};
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "ev");
        let _ = conversation.edit(Edit::Choose);
        let chat = conversation.on_screen().unwrap();
        assert!(conversation.searched(Searched {
            number: 1,
            words: "ev".to_owned(),
            came: Came::Sites {
                showing: results::Showing::Found(vec![found("https://one.example/"); 4]),
                images: Images::NotAsked,
            },
        }));
        assert!(conversation.take_asks().is_empty(), "never with a search");
        let tab = |tab| Pressed::Tab { chat, turn: 0, tab };
        assert!(conversation.act(tab(Tab::Videos)));
        let asks = conversation.take_asks();
        assert_eq!(
            asks,
            vec![Further {
                number: 1,
                words: "ev".to_owned(),
                tab: Tab::Videos
            }]
        );
        assert!(conversation.take_asks().is_empty(), "handed once");
        // Away and back: not asked again, asking or answered.
        assert!(conversation.act(tab(Tab::All)));
        assert!(conversation.act(tab(Tab::Videos)));
        assert!(conversation.take_asks().is_empty());
        assert!(conversation.act(tab(Tab::News)));
        assert_eq!(
            conversation.take_asks(),
            vec![Further {
                number: 2,
                words: "ev".to_owned(),
                tab: Tab::News
            }]
        );
        // An answer under a number nobody asked under is nothing.
        assert!(!conversation.searched(Searched {
            number: 9,
            words: "ev".to_owned(),
            came: Came::Videos(Videos::Found(Vec::new())),
        }));
        let video = Video::new("A video", alo_url::parse("https://v.example/w").unwrap());
        assert!(conversation.searched(Searched {
            number: 1,
            words: "ev".to_owned(),
            came: Came::Videos(Videos::Found(vec![video])),
        }));
        let story = Story::new("A story", alo_url::parse("https://n.example/s").unwrap());
        assert!(conversation.searched(Searched {
            number: 2,
            words: "ev".to_owned(),
            came: Came::News(Stories::Unanswered("no".to_owned())),
        }));
        assert!(
            !conversation.searched(Searched {
                number: 2,
                words: "ev".to_owned(),
                came: Came::News(Stories::Found(vec![story])),
            }),
            "answered once"
        );
        assert!(conversation.act(tab(Tab::Videos)));
        assert!(conversation.take_asks().is_empty());
        // A video's title pressed opens its page, its title what was said.
        let shown = conversation.showing();
        let page = alo_url::parse("https://v.example/w").unwrap();
        assert_eq!(shown.found, vec![page.clone()]);
        let _ = conversation.act(Pressed::Found(page.clone()));
        assert_eq!(conversation.going(), Some(&page));
        conversation.open_card(page);
        assert_eq!(
            conversation.card().map(|card| card.said.as_str()),
            Some("A video")
        );
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
        assert_eq!(
            start.turns,
            vec![Turn::Searched(Results::searching("ev charger"))]
        );
        assert_eq!(conversation.searching(), Some((1, "ev charger")));
        // An older search's answer is not shown; the newest's is.
        let answer = |number| {
            Searched::sites(
                number,
                "ev charger",
                results::Showing::Found(vec![
                    found("https://one.example/"),
                    found("https://two.example/"),
                    found("https://three.example/"),
                    found("https://four.example/"),
                ]),
            )
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
        // A title pressed is chosen as an address is, and opens as a new
        // turn whose bubble is the title (queue item 703); the results stay.
        let two = alo_url::parse("https://two.example/").unwrap();
        let _ = conversation.act(Pressed::Found(two.clone()));
        assert_eq!(conversation.going(), Some(&two));
        conversation.open_card(two.clone());
        assert_eq!(
            conversation.card().map(|card| card.said.as_str()),
            Some("A site")
        );
        assert!(conversation.results().is_some(), "the results kept");
        assert_eq!(conversation.showing().found.len(), 3);
        // A newer page puts the card away as its address, pressable too.
        let one = alo_url::parse("https://one.example/").unwrap();
        let _ = conversation.act(Pressed::Found(one.clone()));
        conversation.open_card(one);
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.turns.len(), 3);
        assert_eq!(
            start.turns.get(1),
            Some(&Turn::Visited {
                said: "A site".to_owned(),
                url: two
            })
        );
        assert_eq!(conversation.showing().found.len(), 4);
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
        assert!(start.turns.is_empty());
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
            .card()
            .cloned()
            .unwrap();
        assert_eq!(
            card.url.to_string(),
            "https://example.com/wiki/Talk:Main_Page"
        );
        assert_eq!(card.said, "example.com", "what the person said stays");
    }

    #[test]
    fn the_card_says_its_page_is_loading_until_it_has_loaded() {
        let mut conversation = Conversation::default();
        let url = alo_url::parse("https://example.com/").unwrap();
        assert!(!conversation.card_loading(false), "no card");
        conversation.open_card(url);
        let loading = |conversation: &Conversation| conversation.card().map(|card| card.loading);
        assert_eq!(loading(&conversation), Some(true), "a new card is loading");
        assert!(!conversation.card_loading(true), "nothing changed");
        assert!(conversation.framed(vec![1]));
        assert_eq!(loading(&conversation), Some(true), "a frame is not a load");
        assert!(conversation.card_loading(false));
        assert_eq!(loading(&conversation), Some(false));
        assert!(!conversation.card_loading(false));
        // A link followed: loading again until it has.
        assert!(conversation.card_loading(true));
        assert_eq!(loading(&conversation), Some(true));
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

    #[test]
    fn a_press_on_an_offer_chooses_it_as_return_chooses_the_first() {
        // Queue item 714: the first offer for an address is going there.
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let offers = conversation.showing().offers;
        assert_eq!(offers, alo_renderer::typed::offers("example.com"));
        let first = offers.first().cloned().unwrap();
        assert!(conversation.act(Pressed::Offer(first.clone())));
        assert_eq!(conversation.chosen(), Some(&first));
        assert_eq!(
            conversation.going().map(ToString::to_string).as_deref(),
            Some("https://example.com/")
        );
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.typed, "", "emptied, as Return empties it");
        assert!(start.focused, "and the box keeps the keyboard");
        assert!(conversation.showing().offers.is_empty());

        // The second, searching, chosen though Return would choose the
        // first: set up, it searches what was typed.
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let search = conversation.showing().offers.get(1).cloned().unwrap();
        assert!(matches!(search, Offer::Search(_)), "{search:?}");
        assert!(conversation.act(Pressed::Offer(search.clone())));
        assert_eq!(conversation.chosen(), Some(&search));
        assert_eq!(conversation.going(), None);
        assert_eq!(
            conversation.searching().map(|(_, words)| words.to_owned()),
            Some("example.com".to_owned())
        );
        // Asking, not set up, leaves what was typed where it was.
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "what is x");
        let ask = conversation.showing().offers.last().cloned().unwrap();
        assert!(matches!(ask, Offer::Ask(_)), "{ask:?}");
        assert!(!conversation.act(Pressed::Offer(ask.clone())));
        assert_eq!(conversation.chosen(), Some(&ask));
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            "what is x"
        );
    }

    #[test]
    fn search_chats_takes_the_keyboard_from_the_box_and_shows_only_the_chats_named_so() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "abc");
        let panel =
            |conversation: &Conversation| conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert!(conversation.act(Pressed::SearchChats), "drawn again");
        assert!(!conversation.act(Pressed::SearchChats), "already had it");
        assert!(conversation.find().focused());
        let start = conversation.start(Size::new(1040.0, 820.0), Scale::ONE);
        assert!(!start.focused, "the box let it go");
        assert_eq!(start.typed, "abc", "keeping what it held");
        assert!(panel(&conversation).searching);
        // Keys now reach Search chats, never the box.
        type_in(&mut conversation, "NEW");
        assert_eq!(conversation.find().typed(), "NEW");
        assert_eq!(panel(&conversation).searched, "NEW");
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            "abc"
        );
        // Both chats are called New chat, so both are found; the panel and
        // what a press on it is decided against agree.
        assert_eq!(panel(&conversation).chats.len(), 2);
        assert_eq!(conversation.showing().chats.len(), 2);
        assert!(matches!(
            conversation.edit(Edit::Put("x".to_owned())),
            Typed::Changed
        ));
        assert!(panel(&conversation).chats.is_empty(), "none called NEWx");
        assert!(conversation.showing().chats.is_empty());
        assert!(matches!(conversation.edit(Edit::Erase), Typed::Changed));
        assert_eq!(conversation.showing().chats.len(), 2);
        assert!(matches!(conversation.edit(Edit::Choose), Typed::Unchanged));
        // A press on the box takes the keyboard back, keeping the words.
        assert!(conversation.act(Pressed::TheBox));
        assert!(!conversation.find().focused());
        assert!(!panel(&conversation).searching);
        assert_eq!(panel(&conversation).searched, "NEW");
        type_in(&mut conversation, "d");
        assert_eq!(
            conversation
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .typed,
            "abcd"
        );
        // A refused press leaves it where it was.
        let _ = conversation.act(Pressed::SearchChats);
        let _ = conversation.act(Pressed::Refused(Refusal::Unasked));
        assert!(conversation.find().focused());
        // Escape empties it, and then lets the keyboard go.
        let _ = conversation.edit(Edit::Clear);
        assert_eq!(conversation.find().typed(), "");
        let _ = conversation.edit(Edit::Clear);
        assert!(!conversation.find().focused());
    }

    #[test]
    fn a_chat_found_is_named_by_its_place_among_those_shown() {
        // Three chats, the newest first; Search chats finding all of them
        // keeps the order, and a press on a row names the chat drawn there.
        let mut conversation = Conversation::default();
        let oldest = conversation.on_screen();
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::SearchChats);
        type_in(&mut conversation, "chat");
        let showing = conversation.showing();
        assert_eq!(showing.chats.len(), 3);
        assert_eq!(showing.chats.last().copied(), oldest);
        let pressed = Pressed::Chat(showing.chats.get(2).copied().unwrap());
        assert!(conversation.act(pressed));
        assert_eq!(conversation.on_screen(), oldest);
        assert!(!conversation.find().focused(), "a chat pressed takes it");
    }

    #[test]
    fn hiding_the_panel_makes_it_the_rail_and_showing_it_brings_it_back() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::SearchChats);
        type_in(&mut conversation, "new");
        assert_eq!(conversation.side(), Side::Panel);
        assert!(conversation.act(Pressed::HideThePanel));
        assert!(!conversation.act(Pressed::HideThePanel), "already hidden");
        assert_eq!(conversation.side(), Side::Rail);
        assert!(!conversation.find().focused(), "the rail has no field");
        let rail = conversation.panel(Size::new(64.0, 820.0), Scale::TWO);
        assert!(rail.hidden);
        assert!(rail.chats.is_empty(), "the rail shows no chat");
        assert!((rail.scale - 2.0).abs() < f32::EPSILON);
        let showing = conversation.showing();
        assert!(showing.hidden);
        assert!(showing.chats.is_empty());
        // New chat from the rail is New chat.
        assert!(conversation.act(Pressed::NewChat));
        assert_eq!(conversation.side(), Side::Rail);
        assert!(conversation.act(Pressed::ShowThePanel));
        assert!(!conversation.act(Pressed::ShowThePanel), "already shown");
        assert_eq!(conversation.side(), Side::Panel);
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert!(!panel.hidden);
        assert_eq!(panel.searched, "new", "what was searched is kept");
        assert_eq!(panel.chats.len(), 2);
    }
}
