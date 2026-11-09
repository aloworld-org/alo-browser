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
//! and its *New chat* is the panel's. Queue item 775 folds it to the rail
//! on its own in a narrow window, and unfolds it as the window widens,
//! the person's own choice kept apart ([`crate::conversation_fold`]): the
//! window tells it how wide it is ([`Conversation::fit`]).
//!
//! Queue item 854 adds *Settings* ([`crate::conversation_settings`]): its
//! row in the panel, or ⌘, (886), opens it over the chat on screen, which is
//! drawn under it as it was — its turns, its scroll and a live card kept —
//! until its ×, Escape, a press on the veil around it, a chat chosen or
//! *New chat*, the first three giving the keyboard back to where it was;
//! and a face chosen in it sets every word of both parts at once
//! ([`Conversation::face`]).
//!
//! Every other — a kept site, *Add a site*, a waiting chat — is decided
//! and kept as [`Conversation::last_pressed`], and **changes nothing yet**:
//! nothing keeps a site or a waiting chat.
//!
//! # The greeting
//!
//! The design greets a person by the time of day — *Good morning.* — and the
//! browser process is the one that would know it. It does not yet: this
//! engine reads no time zone (ADR 0036 § 3 keeps a page's local time UTC),
//! and a greeting of the morning said in the evening is a wrong sentence on
//! the first screen a person sees. So it says [`GREETING`], which is true at
//! any hour, until the machine's zone is read.

use crate::blocking_off::{self, Off};
use crate::chat_delete::Deleting;
use crate::chat_name;
use crate::chat_store::Kept;
use crate::chrome_history::Travel;
use crate::chrome_pin::Pinned;
use crate::conversation_agent::AgentSaid;
use crate::conversation_box::{TheBox, Typed};
use crate::conversation_find::FindChats;
use crate::conversation_fold::Fold;
use crate::conversation_keys::Scroll;
use crate::conversation_place::Side;
use crate::conversation_search::{Came, Searched};
use crate::conversation_settings::{Keyboard, Settings};
use crate::conversation_thread::Thread;
use crate::conversation_work::{Holds, Work};
use crate::scale::Scale;
use crate::typing::Edit;
use alo_layout::Size;
use alo_renderer::agent_limits::{Asks, Limits};
use alo_renderer::card::{Card, Shield};
use alo_renderer::card_menu::{CardMenu, MenuItem};
use alo_renderer::conversation_decide::{Decided, Shown};
use alo_renderer::conversation_scroll::{Reach, Room};
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::panel::{Chat, Closing, Panel, RAIL_CHATS, drawn_order, matching};
use alo_renderer::results::{self, Results, SetUp};
use alo_renderer::start::{KeptSite, Start};
use alo_renderer::typed::Offer;
use alo_url::Url;
use std::time::Instant;

/// One of alo's two fields a person types into (queue item 861).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The one box, on the first screen.
    TheBox,
    /// *Search chats*, in the panel.
    SearchChats,
}

/// What the first screen says first, at any hour.
pub const GREETING: &str = "Hello.";

/// What the agent's card says once a person pressed *Stop* (queue item
/// 704).
pub const STOPPED: &str = "You stopped alo. Nothing more was done on the page.";

/// A chat, as the browser process knows it: a number it gave the chat
/// when it was made, never reused — across launches too, since the chats
/// kept are numbered by it (ADR 0092) — and never sent to a renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatId(u64);

impl ChatId {
    /// Its number: what its kept file is named by
    /// ([`crate::chat_store::file_of`]).
    pub fn number(self) -> u64 {
        self.0
    }

    /// A chat of this number, for a test of another file.
    #[cfg(test)]
    pub(crate) fn for_a_test(number: u64) -> Self {
        Self(number)
    }
}

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
    /// Whether the panel is folded to the rail: hidden by a person, or the
    /// window too narrow for it (queue items 711, 775).
    fold: Fold,
    /// How many tabs were asked for (queue items 745, 746): the newest
    /// one's number.
    further: u64,
    /// The tabs opened whose results are to be asked for, not yet handed to
    /// the searcher.
    to_ask: Vec<Further>,
    /// The tabs asked for and not yet answered: each one's number, and the
    /// chat and turn it was opened in.
    asked: Vec<(u64, ChatId, usize)>,
    /// The agent at work on the card's page (queue item 704).
    work: Work,
    /// The chat whose row the pointer is over, offering *Delete* (queue
    /// item 789).
    hovered: Option<ChatId>,
    /// The chat deleted a moment ago, while *Undo* can bring it back.
    deleting: Option<Deleting>,
    /// The chats deleted and gone, whose files are now to be removed.
    gone: Vec<ChatId>,
    /// The sites blocking is switched off for (queue item 807): what a
    /// card's shield shows, and switches.
    off: Off,
    /// The site whose blocking a person just switched, not yet kept and
    /// its page not yet loaded again.
    switched: Option<String>,
    /// Whether the whole panel is shown over the conversation as a sheet,
    /// the window being too narrow for it beside the conversation (queue
    /// item 776).
    sheet: bool,
    /// *Settings*: whether it is the page on screen, and the face the
    /// interface is set in (queue item 854).
    settings: Settings,
    /// *Settings › Search and answers* (ADR 0106, queue item 905).
    services: crate::conversation_services::ServicesPage,
    /// Where the keyboard was as *Settings* opened, given back as it
    /// closes (queue item 886).
    keyboard_before_settings: Keyboard,
    /// The page the run of alo's agent asked to open (queue item 821): the
    /// run's number, the address, and what its card's bubble says — until
    /// the card for it opens, and then until its page is up.
    agent_opening: Option<AgentOpening>,
}

/// What holds the agent's run in the newest turn of the chat `on` among
/// `threads`: the live card, or a turn asked with no page (queue item 821).
fn holder_in(threads: &mut [(ChatId, Thread)], on: Option<ChatId>) -> Option<&mut dyn Holds> {
    threads
        .iter_mut()
        .find(|(id, _)| Some(*id) == on)
        .and_then(|(_, thread)| thread.holder_mut())
}

/// A page the run of alo's agent asked to open (queue item 821).
#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentOpening {
    /// The run's number.
    run: u64,
    /// Where.
    url: Url,
    /// Whether its card has opened, the run moved onto it.
    carded: bool,
    /// Its name as the address it was resolved to, which its card takes
    /// (queue item 881).
    pinned: Option<Pinned>,
}

/// A tab opened whose results are to be asked of the provider — Videos or
/// News, once for a turn (queue items 745, 746, ADR 0087) — or a further
/// page of a turn's sites, pressed for (761).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Further {
    /// The number its answer comes back under.
    pub number: u64,
    /// The turn's words, asked alone.
    pub words: String,
    /// Which tab: [`results::Tab::Videos`], [`results::Tab::News`], or
    /// [`results::Tab::Sites`] for a further page.
    pub tab: results::Tab,
    /// The further page asked, 1 to [`results::MOST_PAGES`], for Sites; 0
    /// for Videos and News.
    pub offset: usize,
}

impl Default for Conversation {
    /// One new chat, on screen, and the greeting.
    fn default() -> Self {
        Self {
            chats: vec![(ChatId(0), Chat::new("", "").current())],
            next: 1,
            greeting: GREETING.to_owned(),
            last_pressed: None,
            // The keyboard is the box's as the window opens (queue item
            // 875): a person's first keys reach it without a press.
            the_box: TheBox::with_the_keyboard(),
            chosen: None,
            said: String::new(),
            threads: vec![(ChatId(0), Thread::default())],
            card_keys: false,
            search: SetUp::Not(String::new()),
            searches: 0,
            find: FindChats::default(),
            fold: Fold::default(),
            further: 0,
            to_ask: Vec::new(),
            asked: Vec::new(),
            work: Work::default(),
            hovered: None,
            deleting: None,
            gone: Vec::new(),
            off: Off::default(),
            switched: None,
            sheet: false,
            settings: Settings::default(),
            services: crate::conversation_services::ServicesPage::default(),
            keyboard_before_settings: Keyboard::Nowhere,
            agent_opening: None,
        }
    }
}

impl Conversation {
    /// The chats panel as it is now, laid out at `viewport` on a screen of
    /// `scale`: the chats *Search chats* finds, and what is typed into it,
    /// or — hidden or folded — the rail, with the most recent chats (queue
    /// items 711, 760, 775). The row the pointer is over is hovered: an
    /// earlier chat's offers *Delete*, an open one's its × unless it is the
    /// empty chat on screen (789, 791), and on the rail its name stands
    /// beside it.
    pub fn panel(&self, viewport: Size, scale: Scale) -> Panel {
        Panel {
            chats: self
                .listed()
                .into_iter()
                .map(|(id, chat)| Chat {
                    // *Settings* on screen, no chat is (queue item 854).
                    current: chat.current && !self.settings.is_open(),
                    hovered: self.hovered == Some(*id),
                    closing: if self.closable(*id, chat) {
                        Closing::Offered
                    } else {
                        Closing::Withheld
                    },
                    ..chat.clone()
                })
                .collect(),
            viewport,
            scale: 1.0,
            searched: self.find.typed().to_owned(),
            searching: self.find.focused(),
            editing: self.find.field().editing(),
            hidden: self.railed(),
            deleted: self
                .deleting
                .as_ref()
                .filter(|_| !self.railed())
                .map(Deleting::name),
            face: self.settings.face(),
            settings: self.settings.is_open(),
        }
        .at_scale(scale.factor())
    }

    /// How wide the panel's renderer lays it out, in CSS pixels: the whole
    /// panel, shown or as the sheet; the rail; or the rail wider while it
    /// shows the name of the chat the pointer is over (queue item 760).
    pub fn panel_width(&self) -> u32 {
        if !self.railed() {
            crate::conversation_place::PANEL_WIDTH
        } else if self.rail_named() {
            alo_renderer::rail_document::NAMED_WIDTH
        } else {
            crate::conversation_place::RAIL_WIDTH
        }
    }

    /// Whether the rail shows the name of the chat the pointer is over,
    /// past its edge, over the first screen (queue item 760).
    pub fn rail_named(&self) -> bool {
        self.railed()
            && self
                .hovered
                .is_some_and(|id| self.listed().iter().any(|(each, _)| *each == id))
    }

    /// The chats the panel shows, with their identities, in the order the
    /// browser process keeps them: those whose names hold what *Search
    /// chats* holds — or, on the rail, the [`RAIL_CHATS`] most recent, in
    /// the order the panel would draw them, open ones first, whatever was
    /// searched (queue item 760).
    fn listed(&self) -> Vec<&(ChatId, Chat)> {
        if self.railed() {
            let open = self.chats.iter().filter(|(_, chat)| chat.open);
            let earlier = self.chats.iter().filter(|(_, chat)| !chat.open);
            return open.chain(earlier).take(RAIL_CHATS).collect();
        }
        let searched = self.find.typed();
        self.chats
            .iter()
            .filter(move |(_, chat)| matching(chat, searched))
            .collect()
    }

    /// Whether `chat`, `id`, hovered while open, offers its × (queue item
    /// 791): every open chat but the one on screen with nothing in it,
    /// which would only come back as *New chat*.
    fn closable(&self, id: ChatId, chat: &Chat) -> bool {
        chat.open && !(chat.current && self.empty(id))
    }

    /// Whether the chat `id` holds nothing: no name and no turn.
    fn empty(&self, id: ChatId) -> bool {
        self.chats
            .iter()
            .any(|(each, chat)| *each == id && chat.name.is_empty())
            && self
                .threads
                .iter()
                .find(|(each, _)| *each == id)
                .is_none_or(|(_, thread)| thread.turns().is_empty())
    }

    /// What stands down the window's left: the panel, or its rail while a
    /// person has hidden it (queue item 711), the window is too narrow for
    /// it (queue item 775) or alo's agent is at work (queue item 819).
    pub fn side(&self) -> Side {
        self.fold.side_at_work(self.at_work())
    }

    /// Whether alo's agent is at work on the card's page — acting, or
    /// asking the person — as the card the first screen draws says: the
    /// panel is then the rail, and the first screen the split view where
    /// it is wide enough (queue item 819).
    pub fn at_work(&self) -> bool {
        self.card()
            .and_then(|card| card.agent.as_ref())
            .is_some_and(alo_renderer::agent_work::AtWork::is_working)
    }

    /// Whether the panel is folded, and why: what a person chose kept
    /// apart from the window's width.
    pub fn fold(&self) -> Fold {
        self.fold
    }

    /// The window is `width` CSS pixels wide now (queue item 775): whether
    /// that folded the panel to its rail or unfolded it, and so whether the
    /// window is to be laid out again. Folded, *Search chats* is not drawn,
    /// so it lets the keyboard go; what was typed in it is kept.
    pub fn fit(&mut self, width: u32) -> bool {
        let changed = self.fold.fit(width);
        // Wide enough again, the panel stands whole beside the conversation:
        // no sheet over it (queue item 776).
        if !self.fold.narrow() {
            self.sheet = false;
        }
        if changed && self.railed() {
            let _ = self.find.blur();
        }
        changed
    }

    /// Whether the panel is drawn as its rail: folded, and not shown over
    /// the conversation as the sheet.
    fn railed(&self) -> bool {
        self.side() == Side::Rail && !self.sheet()
    }

    /// Whether the whole panel is shown over the conversation as a sheet
    /// (queue item 776): *Show the panel* pressed while the window is too
    /// narrow for it, and not yet closed — never while alo's agent works,
    /// which keeps the rail for the run.
    pub fn sheet(&self) -> bool {
        self.sheet && self.fold.narrow() && !self.at_work()
    }

    /// Close the sheet — Escape, or a press outside it: whether it was
    /// shown. What *Search chats* holds is kept, and it lets the keyboard
    /// go, as when the panel folds.
    pub fn close_sheet(&mut self) -> bool {
        let shown = self.sheet();
        self.sheet = false;
        if shown {
            let _ = self.find.blur();
            self.hovered = None;
        }
        shown
    }

    /// What is typed into *Search chats*, and whether it has the keyboard.
    pub fn find(&self) -> &FindChats {
        &self.find
    }

    /// What the panel and the first screen show as they are now, each list
    /// in the order its part draws it: what a claim on either is decided
    /// against once they are painted.
    pub fn showing(&self) -> Showing {
        let listed: Vec<&(ChatId, Chat)> = self.listed();
        // The row the pointer is over, in the whole panel: an earlier one
        // offers Delete, an open one its × (queue items 789, 791). The rail
        // offers neither.
        let row = self
            .hovered
            .and_then(|id| listed.iter().find(|(each, _)| *each == id))
            .filter(|_| !self.railed());
        let chats: Vec<Chat> = listed.iter().map(|(_, chat)| chat.clone()).collect();
        let shown = Shown {
            chats: drawn_order(&chats)
                .into_iter()
                .filter_map(|place| listed.get(place).map(|(id, _)| *id))
                .collect(),
            sites: Vec::new(),
            waiting: Vec::new(),
            card: self.card().is_some(),
            found: self.thread().map(Thread::links).unwrap_or_default(),
            offers: self.the_box.offers(self.asks_first()),
            hidden: self.railed(),
            tabs: self
                .on_this_chat(Thread::tabs)
                .into_iter()
                .map(|(id, (turn, tab))| (id, turn, tab))
                .collect(),
            more: self.on_this_chat(Thread::more_sites),
            see_all: self.on_this_chat(Thread::see_all),
            show_more: self.on_this_chat(Thread::show_more),
            full_answer: self.on_this_chat(Thread::full_answers),
            pictured: self
                .thread()
                .map(Thread::pictured_pages)
                .unwrap_or_default(),
            working: self.work.working(self.holder()),
            paused: self.work.paused(self.holder()),
            asking: self.work.asking_now(self.holder()),
            choices: Work::choices(self.holder()),
            hovered: row.filter(|(_, chat)| !chat.open).map(|(id, _)| *id),
            closing: row
                .filter(|(id, chat)| self.closable(*id, chat))
                .map(|(id, _)| *id),
            undo: self
                .deleting
                .as_ref()
                .filter(|_| !self.railed())
                .map(|deleting| deleting.id),
            shield: self
                .card()
                .filter(|card| card.shield.is_some())
                .and_then(|card| blocking_off::site_of(&card.url.serialised)),
            menu: self
                .card_menu()
                .map(|menu| menu.items.clone())
                .unwrap_or_default(),
            faces: Vec::new(),
            limits: Vec::new(),
            asks: Vec::new(),
            services: Vec::new(),
        };
        if !self.settings.is_open() {
            return shown;
        }
        // *Settings* over the chat on screen (queue items 854, 886): the
        // first screen shows its faces and nothing of the chat.
        Shown {
            chats: shown.chats,
            hidden: shown.hidden,
            hovered: shown.hovered,
            closing: shown.closing,
            undo: shown.undo,
            faces: InterfaceFace::ALL.to_vec(),
            // *alo at work* (queue item 878).
            limits: self.settings.limits().presses(),
            asks: Asks::ALL.to_vec(),
            // *Search and answers* (queue item 905), as it is drawn.
            services: self.services.shown().pressables(),
            ..Shown::default()
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
    /// (queue item 711). *New chat* gives the box the keyboard, as the
    /// window's opening does (queue item 875). A press on anything but the
    /// box, *New chat*, *Go* and an offer takes the keyboard from the box,
    /// and on anything but *Search chats* from it. Everything else changes nothing yet (see the module's
    /// words), and neither does a chat this does not hold, which the
    /// decision refused unless it went between being decided and being
    /// acted on, nor a press refused or unanswered.
    pub fn act(&mut self, pressed: Pressed) -> bool {
        // Where the keyboard was before the press took it, for *Settings*
        // to give back (queue item 886).
        let keyboard = self.keyboard();
        let changed = match &pressed {
            // A new chat's box has the keyboard, as the window's first does
            // (queue item 875).
            Pressed::TheBox | Pressed::NewChat => self.the_box.focus(),
            Pressed::Go => {
                let choice = self.the_box.choice(self.asks_first());
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
            _ => self.pointer_pressed(matches!(pressed, Pressed::SearchChats)) || changed,
        };
        let changed = match &pressed {
            Pressed::Chat(_)
            | Pressed::Settings
            | Pressed::Face(_)
            | Pressed::LeaveSettings
            | Pressed::Stepped(_)
            | Pressed::Asks(_)
            | Pressed::Service(_) => self.in_settings(&pressed, keyboard) || changed,
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
                    self.ask_further((*chat, *turn), words, *tab, 0);
                }
                shown || changed
            }
            // *Show more sites*: the turn's next page asked, one at a time
            // (761) — a paid request, made only for this press.
            Pressed::ShowMoreSites { chat, turn } => {
                let Some((_, thread)) = self.threads.iter_mut().find(|(each, _)| each == chat)
                else {
                    return self.pressed(pressed, changed);
                };
                let Some((words, offset)) = thread.ask_page(*turn) else {
                    return self.pressed(pressed, changed);
                };
                self.ask_further((*chat, *turn), words, results::Tab::Sites, offset);
                true
            }
            Pressed::NewChat => {
                self.sheet = false;
                let _ = self.settings.close();
                self.services.leave();
                self.new_chat();
                true
            }
            Pressed::DeleteChat(id) => self.delete(*id, Instant::now()) || changed,
            Pressed::CloseChat(id) => self.close(*id) || changed,
            Pressed::Shield(site) => {
                let site = site.clone();
                self.switch_blocking(&site) || changed
            }
            Pressed::Undo(id) => self.undo(*id) || changed,
            Pressed::HideThePanel => self.hide_the_panel() || changed,
            Pressed::ShowThePanel => self.show_the_panel() || changed,
            // The agent (queue item 704): *Stop* says so on the card at
            // once; a choice a stuck run offered is asked of it anew.
            Pressed::Stop(_) => self.stop_agent(STOPPED).is_some() || changed,
            // *Let alo continue* (queue item 825): the run held acts again.
            Pressed::Continue(run) => {
                let run = *run;
                self.carry_on_agent(run).is_some() || changed
            }
            Pressed::Choice(choice) => {
                let choice = choice.clone();
                self.ask_alo(&choice) || changed
            }
            // alo's menu over the card's page (queue item 798): it closes,
            // whatever its line; a link opens as a new chat's first turn,
            // words selected are searched as this chat's next. Copying is
            // the window's, which holds the page and the clipboard.
            Pressed::MenuItem(item) => {
                let item = item.clone();
                self.menu_pressed(item) || changed
            }
            _ => changed,
        };
        self.pressed(pressed, changed)
    }

    /// The pointer pressed on either part, the card's bar among them —
    /// on *Search chats* when `on_search`: it has the keyboard then, and
    /// loses it otherwise; the press is not on the card's page, which no
    /// longer has the keyboard; and the keyboard's ring on *Settings* is
    /// put away (queue item 856). Whether *Search chats* or the ring
    /// changed.
    fn pointer_pressed(&mut self, on_search: bool) -> bool {
        let search = if on_search {
            self.find.focus()
        } else {
            self.find.blur()
        };
        self.card_keys = false;
        let unkeyed = self.settings.unkey();
        search || unkeyed
    }

    /// Ask `tab` of the turn `turn` of `chat` for `words`, at `offset`: a
    /// tab opened for the first time, or a further page of sites.
    fn ask_further(
        &mut self,
        (chat, turn): (ChatId, usize),
        words: String,
        tab: results::Tab,
        offset: usize,
    ) {
        self.further += 1;
        self.asked.push((self.further, chat, turn));
        self.to_ask.push(Further {
            number: self.further,
            words,
            tab,
            offset,
        });
    }

    /// A chat chosen in the panel, or *Settings* pressed (queue item 854):
    /// a chat chosen is on screen, closing the sheet it was chosen in (776)
    /// and *Settings*; *Settings* opens over the chat on screen, closing
    /// the sheet as a chat chosen does, and keeps where the `keyboard` was
    /// to give it back (886); a face chosen sets everything in it; its ×, or
    /// a press on the veil around it, closes it. Whether anything changed.
    fn in_settings(&mut self, pressed: &Pressed, keyboard: Keyboard) -> bool {
        match pressed {
            Pressed::Chat(id) => {
                let sheet = core::mem::replace(&mut self.sheet, false);
                let settings = self.settings.close();
                self.services.leave();
                self.put_on_screen(*id) || sheet || settings
            }
            Pressed::Settings => {
                let sheet = core::mem::replace(&mut self.sheet, false);
                let opened = self.settings.open();
                if opened {
                    self.keyboard_before_settings = keyboard;
                }
                opened || sheet
            }
            // A press anywhere in *Settings* but a field takes the keyboard
            // out of *Search and answers*' field (905).
            Pressed::Face(face) => self.services.type_in(None) | self.settings.choose(*face),
            // *alo at work* (queue item 878).
            Pressed::Stepped(stepped) => {
                self.services.type_in(None) | self.settings.press(*stepped)
            }
            Pressed::Asks(asks) => self.services.type_in(None) | self.settings.choose_asks(*asks),
            // *Search and answers* (queue item 905): a field pressed takes
            // the keyboard; a press anywhere else leaves it.
            Pressed::Service(pressed) => {
                let left = match pressed {
                    alo_renderer::settings_services::ServicePress::Field(_) => false,
                    _ => self.services.type_in(None),
                };
                self.services.press(*pressed) || left
            }
            Pressed::LeaveSettings => self.close_settings(),
            _ => false,
        }
    }

    /// *Hide the panel*: the rail, a person's choice — and from the sheet,
    /// the sheet closed too (queue item 776). Whether anything changed.
    fn hide_the_panel(&mut self) -> bool {
        let sheet = core::mem::replace(&mut self.sheet, false);
        self.fold.hide() || sheet
    }

    /// *Show the panel*: the panel beside the conversation — or, in a
    /// window too narrow for it, over it as the sheet, the choice kept for
    /// when the window is wide enough (queue items 775, 776). Whether
    /// anything changed.
    fn show_the_panel(&mut self) -> bool {
        let side = self.fold.show();
        if self.fold.narrow() && !self.at_work() {
            self.sheet = true;
        }
        side || self.sheet()
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
            return Typed::of(self.find.edit(edit));
        }
        let typed = self.the_box.edit(edit, self.asks_first());
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
                self.name_on_screen();
                self.card_keys = false;
                let _ = self.the_box.empty();
                true
            }
            // Asking alo, while a card is open (queue item 704): it works
            // on the card's page, and the box is emptied as for a search.
            Some(Offer::Ask(words)) => {
                let words = words.clone();
                if self.ask_alo(&words) {
                    self.name_on_screen();
                    let _ = self.the_box.empty();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Whether the model can be asked to work on a card's page is now
    /// `set_up` (queue item 704).
    pub fn set_agent(&mut self, set_up: SetUp) {
        self.work.set_up(set_up);
    }

    /// Whether asking alo can be done now: set up — with a card open or
    /// none (queue item 821).
    pub fn asking(&self) -> SetUp {
        self.work.asking()
    }

    /// Whether the box offers asking alo before searching: while it can be
    /// done (`docs/design/shell.md`: a request typed while a card is open
    /// is the agent's).
    fn asks_first(&self) -> bool {
        // With no card open, searching stays first and asking alo — which
        // then finds the site itself — is the second line (queue item 821).
        self.card().is_some() && self.asking() == SetUp::Ready
    }

    /// Start the agent on the card's page for what was `asked` — or, with
    /// no card open (queue item 821), in a turn of its own, from which it
    /// finds the site: whether it started.
    fn ask_alo(&mut self, asked: &str) -> bool {
        if self.work.asking() != SetUp::Ready {
            return false;
        }
        let on = self.on_screen();
        let Some(thread) = self
            .threads
            .iter_mut()
            .find(|(id, _)| Some(*id) == on)
            .map(|(_, thread)| thread)
        else {
            return false;
        };
        if thread.card().is_none() {
            thread.begin_asked(asked.to_owned());
        }
        let started = match thread.holder_mut() {
            Some(holder) => self.work.start(holder, asked),
            None => false,
        };
        self.agent_opening = None;
        if started {
            // The panel is the rail for the run (queue item 819): *Search
            // chats* is not drawn, so it lets the keyboard go; what was
            // typed in it is kept. No sheet stands over the run.
            let _ = self.find.blur();
            self.sheet = false;
        }
        started
    }

    /// The run of the agent started and not yet handed to the window, once:
    /// its number, and what the person asked.
    pub fn take_run(&mut self) -> Option<(u64, String)> {
        self.work.take_start()
    }

    /// The run of the agent at work on the card — or in a turn asked with
    /// no page (queue item 821) — if one is.
    pub fn working(&self) -> Option<u64> {
        self.work.working(self.holder())
    }

    /// The run of the agent the window keeps a thread for: at work on the
    /// card, or held while its page is the person's (queue item 825).
    pub fn agent_run(&self) -> Option<u64> {
        self.work.at_work(self.holder())
    }

    /// What holds the agent's run in the chat on screen's newest turn: the
    /// live card, or a turn asked with no page (queue item 821).
    fn holder(&self) -> Option<&dyn Holds> {
        self.thread().and_then(Thread::holder)
    }

    /// What a run of the agent said: shown on the card when it is the run
    /// at work there. Whether the first screen shows something else.
    pub fn agent_said(&mut self, said: AgentSaid) -> bool {
        let on = self.on_screen();
        let card = holder_in(&mut self.threads, on);
        self.work.heard(card, said)
    }

    /// The person took the card's page over (queue item 825): the run at
    /// work held, its card saying so with *Let alo continue*. Its number,
    /// if one was at work.
    pub fn pause_agent(&mut self) -> Option<u64> {
        // Only a card's page can be taken over: a turn asked with no page
        // has none.
        self.card()?;
        let on = self.on_screen();
        let card = holder_in(&mut self.threads, on);
        self.work.pause(card)
    }

    /// *Let alo continue* for the run numbered `run`: it acts again, if it
    /// is the one held. Its number, if it was.
    pub fn carry_on_agent(&mut self, run: u64) -> Option<u64> {
        let on = self.on_screen();
        let card = holder_in(&mut self.threads, on);
        self.work.carry_on(card, run)
    }

    /// Stop the run of the agent at work, saying `why` on its card: its
    /// number, if one was.
    pub fn stop_agent(&mut self, why: &str) -> Option<u64> {
        self.agent_opening = None;
        let on = self.on_screen();
        let card = holder_in(&mut self.threads, on);
        self.work.stop(card, why)
    }

    /// The run of alo's agent numbered `run` asks to open `url` (queue item
    /// 821), its card's bubble `said`: chosen as a site is, to be opened by
    /// the window as a card the run is moved onto — never stopping the run,
    /// as another page a person opens does — its name taken as `pinned`
    /// holds, when one is (queue item 881). Whether it is the run at work.
    pub fn agent_opens(
        &mut self,
        run: u64,
        (url, pinned): (Url, Option<Pinned>),
        said: String,
    ) -> bool {
        if self.work.working(self.holder()) != Some(run) {
            return false;
        }
        self.agent_opening = Some(AgentOpening {
            run,
            url: url.clone(),
            carded: false,
            pinned,
        });
        self.said = said;
        self.chosen = Some(Offer::Go(url));
        true
    }

    /// Whether the page about to be opened is the one the run of alo's
    /// agent asked for, its card not yet open (queue item 821): the window
    /// then does not stop the run for it.
    pub fn opening_for_the_run(&self) -> bool {
        self.agent_opening
            .as_ref()
            .is_some_and(|opening| !opening.carded && self.going() == Some(&opening.url))
    }

    /// The name the page about to be opened is to be loaded as, when it is
    /// the one the run of alo's agent asked for and the browser process
    /// resolved its name (queue item 881): never for a page a person
    /// chose.
    pub fn pinned_for_the_run(&self) -> Option<&Pinned> {
        if !self.opening_for_the_run() {
            return None;
        }
        self.agent_opening
            .as_ref()
            .and_then(|opening| opening.pinned.as_ref())
    }

    /// The card the run of alo's agent asked for is up, or could not be
    /// opened (queue item 821): the run to be told, by its number, once.
    pub fn take_agent_opened(&mut self) -> Option<u64> {
        self.agent_opening
            .take_if(|opening| opening.carded)
            .map(|opening| opening.run)
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
            Came::Sites {
                showing,
                images,
                more,
                answer,
            } => {
                if searched.number != self.searches {
                    return false;
                }
                self.thread_mut().is_some_and(|thread| {
                    thread.searched(&searched.words, showing, images, more, answer)
                })
            }
            // Its answer (queue item 707), shown in the turn still asking for
            // it, of any chat, as its images are.
            Came::Answer { answer, .. } => {
                let on_screen = self.on_screen();
                let mut shown = false;
                for (id, thread) in &mut self.threads {
                    if thread.answered(&searched.words, answer.clone()) {
                        shown = shown || Some(*id) == on_screen;
                        break;
                    }
                }
                shown
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
            Came::Page(page) => {
                self.further_came(searched.number, |thread, turn| thread.page_came(turn, page))
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
        // A chat from before, pressed, is in use again: drawn under *Open*
        // (queue item 757, `docs/design/shell.md`) — first there, the most
        // recent, as a new chat is, and so first on the rail (760).
        if let Some(place) = self
            .chats
            .iter()
            .position(|(each, chat)| *each == id && !chat.open)
        {
            let again = self.chats.remove(place);
            self.chats.insert(0, again);
        }
        for (each, chat) in &mut self.chats {
            chat.current = *each == id;
            chat.open |= *each == id;
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

    /// The pointer is over the row of `hovered`, or no row (queue item
    /// 789): whether the panel is to be drawn again, its row offering
    /// *Delete* or its × — or, on the rail, its name beside it (760). A
    /// chat the panel does not list is no row.
    pub fn hover(&mut self, hovered: Option<ChatId>) -> bool {
        let hovered = hovered.filter(|id| self.listed().iter().any(|(each, _)| each == id));
        core::mem::replace(&mut self.hovered, hovered) != hovered
    }

    /// Delete `id` `now` (queue item 789, [`crate::chat_delete`]): out of
    /// the panel at once, held for its *Undo*. A chat deleted before it
    /// and still held is gone now. The chat on screen deleted, its card is
    /// put away first and a new chat is on screen — one with nothing in
    /// it already open, or a new one. Whether anything changed.
    pub fn delete(&mut self, id: ChatId, now: Instant) -> bool {
        let Some(place) = self.chats.iter().position(|(each, _)| *each == id) else {
            return false;
        };
        if let Some(before) = self.deleting.take() {
            self.gone.push(before.id);
        }
        let on_screen = self.on_screen() == Some(id);
        if on_screen {
            let _ = self.close_card();
        }
        let (_, chat) = self.chats.remove(place);
        let thread = self
            .threads
            .iter()
            .position(|(each, _)| *each == id)
            .map(|at| self.threads.remove(at).1)
            .unwrap_or_default();
        self.hovered = None;
        let _ = self.find.blur();
        self.deleting = Some(Deleting::at(
            id,
            Chat {
                current: false,
                hovered: false,
                ..chat
            },
            thread,
            place,
            now,
        ));
        if on_screen {
            let empty = self.chats.iter().find_map(|(each, chat)| {
                let blank = chat.open
                    && chat.name.is_empty()
                    && self
                        .threads
                        .iter()
                        .any(|(other, thread)| other == each && thread.turns().is_empty());
                blank.then_some(*each)
            });
            match empty {
                Some(empty) => {
                    let _ = self.put_on_screen(empty);
                }
                None => self.new_chat(),
            }
        }
        true
    }

    /// *Undo*: `id`, deleted a moment ago, back where it stood, its turns as
    /// they were, not on screen. Whether it was still held.
    pub fn undo(&mut self, id: ChatId) -> bool {
        if self.deleting.as_ref().map(|deleting| deleting.id) != Some(id) {
            return false;
        }
        let Some(deleting) = self.deleting.take() else {
            return false;
        };
        let place = deleting.place.min(self.chats.len());
        self.chats.insert(place, (deleting.id, deleting.chat));
        self.threads.push((deleting.id, deleting.thread));
        true
    }

    /// Close the open chat `id` (queue item 791): it moves under
    /// *Earlier*, first there, its turns kept — its live card put away as
    /// its address — and nothing deleted. One with nothing said in it goes,
    /// since nothing would be kept of it. The chat on screen closed, the
    /// open chat below it in the panel is put on screen, or the one above,
    /// or else a new chat; the empty chat on screen is not closed, since it
    /// would only come back. Whether anything changed.
    pub fn close(&mut self, id: ChatId) -> bool {
        let Some(place) = self
            .chats
            .iter()
            .position(|(each, chat)| *each == id && chat.open)
        else {
            return false;
        };
        let empty = self.empty(id);
        let on_screen = self.on_screen() == Some(id);
        if on_screen && empty {
            return false;
        }
        let open: Vec<ChatId> = self
            .chats
            .iter()
            .filter(|(_, chat)| chat.open)
            .map(|(each, _)| *each)
            .collect();
        let next = open.iter().position(|each| *each == id).and_then(|at| {
            open.get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|above| open.get(above)))
                .copied()
        });
        if on_screen {
            let _ = self.close_card();
        }
        self.hovered = None;
        let (_, chat) = self.chats.remove(place);
        if empty {
            self.threads.retain(|(each, _)| *each != id);
        } else {
            self.chats.insert(
                0,
                (
                    id,
                    Chat {
                        open: false,
                        current: false,
                        hovered: false,
                        ..chat
                    },
                ),
            );
        }
        if on_screen {
            match next {
                Some(next) => {
                    let _ = self.put_on_screen(next);
                }
                None => self.new_chat(),
            }
        }
        true
    }

    /// Whether a chat deleted is held for its *Undo*, and until when: the
    /// window wakes then.
    pub fn undo_until(&self) -> Option<Instant> {
        self.deleting.as_ref().map(|deleting| deleting.until)
    }

    /// The time is `now`: a chat deleted whose *Undo* has passed is gone.
    /// Whether the panel is to be drawn again, its *Undo* no longer there.
    pub fn expire(&mut self, now: Instant) -> bool {
        if !self
            .deleting
            .as_ref()
            .is_some_and(|deleting| deleting.passed(now))
        {
            return false;
        }
        if let Some(deleting) = self.deleting.take() {
            self.gone.push(deleting.id);
        }
        true
    }

    /// The window is closing: a chat deleted and still held is gone now.
    pub fn let_go(&mut self) {
        if let Some(deleting) = self.deleting.take() {
            self.gone.push(deleting.id);
        }
    }

    /// The chats deleted and gone since this was last asked, each once:
    /// their files are now to be removed.
    pub fn take_gone(&mut self) -> Vec<ChatId> {
        std::mem::take(&mut self.gone)
    }

    /// Name the chat on screen by the first words said in it (queue item
    /// 758, [`chat_name`]), once it holds a turn, if it has no name yet.
    fn name_on_screen(&mut self) {
        let Some(id) = self.on_screen() else {
            return;
        };
        let Some(named) = self
            .thread()
            .and_then(|thread| thread.turns().first())
            .map(|first| chat_name::of(first.said()))
        else {
            return;
        };
        if let Some((_, chat)) = self.chats.iter_mut().find(|(each, _)| *each == id)
            && chat.name.is_empty()
        {
            chat.name = named;
        }
    }

    /// The same conversation, holding the chats `kept` from before (queue
    /// item 757, ADR 0092), newest first, each under *Earlier* with its
    /// name and turns, after the new chat on screen — numbered past
    /// `highest`, the highest number any chat's file holds, so that no chat
    /// made now is written over one kept.
    #[must_use]
    pub fn keeping(mut self, kept: Vec<Kept>, highest: Option<u64>) -> Self {
        let past = highest.map_or(0, |highest| highest.saturating_add(1));
        let mut next = self.next.max(past);
        // The chats made so far hold no turn: numbered past the kept.
        for (id, _) in &mut self.chats {
            if id.0 < past {
                let to = ChatId(next);
                next += 1;
                for (each, _) in &mut self.threads {
                    if each == id {
                        *each = to;
                    }
                }
                *id = to;
            }
        }
        self.next = next;
        for one in kept {
            let id = ChatId(one.number);
            if self.chats.iter().any(|(each, _)| *each == id) {
                continue;
            }
            let thread = Thread::kept(one.record.turns);
            // A name 758's rule made is named again by 859's (ADR 0092's
            // amendment of 7 November 2026); any other is kept.
            let name = chat_name::read_back(
                &one.record.name,
                thread.turns().first().map(alo_renderer::turn::Turn::said),
            );
            self.chats.push((id, Chat::new(name, "").earlier()));
            self.threads.push((id, thread));
        }
        self
    }

    /// Every chat that holds a turn, with its name and turns: what is kept
    /// of the conversation across launches (queue item 757).
    pub fn to_keep(&self) -> impl Iterator<Item = (ChatId, &str, &Thread)> {
        self.threads
            .iter()
            .filter(|(_, thread)| !thread.turns().is_empty())
            .filter_map(|(id, thread)| {
                self.chats
                    .iter()
                    .find(|(each, _)| each == id)
                    .map(|(_, chat)| (*id, chat.name.as_str(), thread))
            })
    }

    /// The first screen as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn start(&self, viewport: Size, scale: Scale) -> Start {
        let start = Start {
            greeting: self.greeting.clone(),
            sites: Vec::new(),
            waiting: Vec::new(),
            viewport,
            scale: 1.0,
            typed: self.the_box.typed().to_owned(),
            focused: self.the_box.focused(),
            editing: self.the_box.field().editing(),
            offers: self.the_box.offers(self.asks_first()),
            // Only the pictures each tab draws, the newest turns' first,
            // within the first screen's budget (queue item 743).
            turns: self
                .thread()
                .map(|thread| results::within_budget(thread.turns()))
                .unwrap_or_default(),
            down: self.thread().map_or(0.0, Thread::down),
            bar: self
                .thread()
                .and_then(|thread| thread.bar(viewport, scale.factor())),
            search: self.search.clone(),
            asking: self.asking(),
            face: self.settings.face(),
            settings: false,
            settings_keyed: None,
            limits: self.settings.limits(),
            settings_down: 0.0,
            settings_bar: None,
            services: None,
        }
        .at_scale(scale.factor());
        if !self.settings.is_open() {
            return start;
        }
        // *Settings* over the chat on screen, which is drawn under it as it
        // was (queue item 886), scrolled in its own panel (queue item 880).
        let scrolled = self.settings.scrolled();
        Start {
            settings: true,
            settings_keyed: self.settings.keyed(),
            settings_down: scrolled.down(),
            settings_bar: scrolled.bar(Room::settings(viewport).height, scale.factor()),
            services: Some(self.services.shown()),
            ..start
        }
    }

    /// A key on *Settings* while it is open (queue item 856,
    /// [`crate::settings_keys`]): the keyboard moved, a face chosen by an
    /// arrow, or its × pressed, closing it. Whether anything
    /// changed, to be drawn again.
    pub fn settings_key(&mut self, key: crate::settings_keys::SettingsKey) -> bool {
        use crate::settings_keys::SettingsKey;
        use alo_renderer::settings_services::ServicePress;
        use alo_renderer::start::SettingsKeyed;
        if !self.settings.is_open() {
            return false;
        }
        match key {
            SettingsKey::Tab { back } => {
                // Through *Search and answers* too, as it is drawn (905):
                // into a field it moves to, out of one it leaves.
                let stops: Vec<SettingsKeyed> = self
                    .services
                    .shown()
                    .stops()
                    .into_iter()
                    .map(SettingsKeyed::Service)
                    .collect();
                let moved = self.settings.tab_through(back, &stops);
                let field = match self.settings.keyed() {
                    Some(SettingsKeyed::Service(ServicePress::Field(entry))) => Some(entry),
                    _ => None,
                };
                let _ = self.services.type_in(field);
                moved
            }
            SettingsKey::Step { on } => match self.settings.keyed() {
                // A chooser: the next service, or the one before (905).
                Some(SettingsKeyed::Service(
                    pressed @ (ServicePress::Search(_) | ServicePress::Answers(_)),
                )) => {
                    let (chosen, now) = self
                        .services
                        .step(matches!(pressed, ServicePress::Search(_)), on);
                    self.settings.key_on(SettingsKeyed::Service(now));
                    chosen || now != pressed
                }
                Some(SettingsKeyed::Service(_)) => false,
                _ => self.settings.step(on),
            },
            SettingsKey::Press => match self.settings.keyed() {
                Some(SettingsKeyed::Close) => self.close_settings(),
                Some(SettingsKeyed::Service(pressed)) => {
                    let changed = self.services.press(pressed);
                    self.services_keyed_kept();
                    changed
                }
                _ => false,
            },
        }
    }

    /// The keyboard kept on something of *Search and answers* still drawn,
    /// after a press changed what is: else on its chooser of search.
    fn services_keyed_kept(&mut self) {
        use alo_renderer::start::SettingsKeyed;
        if let Some(SettingsKeyed::Service(on)) = self.settings.keyed() {
            let stops = self.services.shown().stops();
            if !stops.contains(&on) {
                let first = stops.first().copied();
                if let Some(first) = first {
                    self.settings.key_on(SettingsKeyed::Service(first));
                }
            }
        }
    }

    /// *Settings › Search and answers* as it is now (queue item 905).
    pub fn services(&self) -> &crate::conversation_services::ServicesPage {
        &self.services
    }

    /// *Settings › Search and answers*, to be typed into or told what the
    /// window did.
    pub fn services_mut(&mut self) -> &mut crate::conversation_services::ServicesPage {
        &mut self.services
    }

    /// The same conversation, its *Search and answers* `page` (queue item
    /// 905): the services named and whose keys are used, as the window
    /// opens.
    #[must_use]
    pub fn serving(mut self, page: crate::conversation_services::ServicesPage) -> Self {
        self.services = page;
        self
    }

    /// Show the page at `url` as a card, answering what the person last
    /// chose, with no frame yet (queue item 717), as a new turn (703): any
    /// card shown put away as its address first, since one page is open at
    /// a time (ADR 0075 § 4).
    pub fn open_card(&mut self, url: Url) {
        let said = self.said.clone();
        let shield = self.shield_for(&url);
        // The page the run of alo's agent asked for (queue item 821): the
        // run moved onto its card, as if the person had opened it.
        let for_the_run = self
            .agent_opening
            .as_mut()
            .filter(|opening| !opening.carded && opening.url == url);
        let carded = match for_the_run {
            Some(opening) => {
                opening.carded = true;
                let went_on = format!("alo went on to {}.", alo_url::reading::host(&url));
                self.thread_mut().is_some_and(|thread| {
                    thread.open_card_for_the_run(said.clone(), url.clone(), &went_on)
                })
            }
            None => false,
        };
        if let Some(thread) = self.thread_mut() {
            if !carded {
                thread.open_card(said, url);
            }
            if let Some(card) = thread.card_mut() {
                card.shield = Some(shield);
            }
        }
        self.name_on_screen();
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

    /// Which field has the keyboard, if one does: *Search chats* or the
    /// one box (queue item 861).
    pub fn field_focused(&self) -> Option<Field> {
        if self.find.focused() {
            Some(Field::SearchChats)
        } else if self.the_box.focused() {
            Some(Field::TheBox)
        } else {
            None
        }
    }

    /// The field that has the keyboard, to place its caret, select in it
    /// or compose in it (queue items 861, 848): none while neither has it.
    pub fn field_mut(&mut self) -> Option<&mut crate::text_field::TextField> {
        if self.find.focused() {
            self.find.field_mut()
        } else {
            self.the_box.field_mut()
        }
    }

    /// The field `which`, as it stands.
    pub fn field(&self, which: Field) -> &crate::text_field::TextField {
        match which {
            Field::TheBox => self.the_box.field(),
            Field::SearchChats => self.find.field(),
        }
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
        let shield = self.shield_for(&url);
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) if card.url != url => {
                card.url = url;
                // Another page: nothing refused on it yet (queue item 807).
                if card.shield.is_some() {
                    card.shield = Some(shield);
                }
                true
            }
            _ => false,
        }
    }

    /// A fresh shield for a page at `url`: nothing refused yet, and on
    /// unless a person switched blocking off for its site (queue item 807).
    fn shield_for(&self, url: &Url) -> Shield {
        Shield {
            blocked: 0,
            on: !self.off.is_off(&url.serialised),
        }
    }

    /// The sites blocking is switched off for are `off` (queue item 807):
    /// the window's, shared with every card's Chromium.
    pub fn set_blocking_off(&mut self, off: Off) {
        self.off = off;
    }

    /// The lists refused a request of the live card's page: counted on its
    /// shield. Whether the card shows something else for it.
    pub fn blocked(&mut self) -> bool {
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(Card {
                shield: Some(shield),
                ..
            }) if shield.on => {
                shield.blocked = shield.blocked.saturating_add(1);
                true
            }
            _ => false,
        }
    }

    /// The shield pressed for `site` (queue item 807): blocking switched
    /// off for it, or on again, the count begun anew, and the switch kept
    /// for the window to keep and load the page again with
    /// ([`Conversation::take_switched`]).
    fn switch_blocking(&mut self, site: &str) -> bool {
        let off = !self.off.is_off_for(site);
        self.off.switch(site, off);
        if let Some(Card {
            shield: Some(shield),
            ..
        }) = self.thread_mut().and_then(Thread::card_mut)
        {
            *shield = Shield {
                blocked: 0,
                on: !off,
            };
        }
        self.switched = Some(site.to_owned());
        true
    }

    /// Whether blocking is switched off for `site`.
    pub fn shield_off(&self, site: &str) -> bool {
        self.off.is_off_for(site)
    }

    /// The sites blocking is switched off for.
    pub fn blocking_off(&self) -> &Off {
        &self.off
    }

    /// The site whose blocking was just switched, once: the window keeps
    /// the switches and loads the card's page again.
    pub fn take_switched(&mut self) -> Option<String> {
        self.switched.take()
    }

    /// A line of alo's menu over the card's page pressed (queue item 798):
    /// the menu closes; a link opens as a new chat's first turn, and words
    /// selected are searched as this chat's next. Whether the first screen
    /// shows something else.
    fn menu_pressed(&mut self, item: MenuItem) -> bool {
        let closed = self.close_card_menu();
        match item {
            MenuItem::OpenInNewChat(address) => {
                self.sheet = false;
                self.new_chat();
                self.open_link(&address);
                true
            }
            MenuItem::Search(words) => self.search_for(&words) || closed,
            MenuItem::Copy { .. } | MenuItem::CopyLink(_) => closed,
        }
    }

    /// alo's menu over the live card's page, while it is open (queue item
    /// 798).
    pub fn card_menu(&self) -> Option<&CardMenu> {
        self.card().and_then(|card| card.menu.as_ref())
    }

    /// Open `menu` over the live card's page, in place of any open: whether
    /// there was a card to open it over.
    pub fn open_card_menu(&mut self, menu: CardMenu) -> bool {
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) => {
                card.menu = Some(menu);
                true
            }
            None => false,
        }
    }

    /// Close alo's menu over the live card's page: whether one was open.
    pub fn close_card_menu(&mut self) -> bool {
        self.thread_mut()
            .and_then(Thread::card_mut)
            .and_then(|card| card.menu.take())
            .is_some()
    }

    /// Open the link at `address` as the conversation's next turn (queue
    /// item 798): chosen as an address typed is, the turn's bubble its
    /// address; the window opens it ([`Conversation::going`]). An address
    /// that is not a site's is not chosen. Whether it was.
    pub fn open_link(&mut self, address: &str) -> bool {
        let Ok(url) = alo_url::parse(address) else {
            return false;
        };
        if !matches!(url.scheme.as_str(), "http" | "https") {
            return false;
        }
        self.said.clone_from(&url.serialised);
        self.chosen = Some(Offer::Go(url));
        true
    }

    /// Search for `words` as the conversation's next turn (queue item 798's
    /// *Search for “…”*), as the box searches what was typed: whether the
    /// first screen shows something else for it.
    pub fn search_for(&mut self, words: &str) -> bool {
        let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
        if words.is_empty() {
            return false;
        }
        self.said.clone_from(&words);
        self.chosen = Some(Offer::Search(words.clone()));
        if self.search != SetUp::Ready {
            return false;
        }
        self.searches += 1;
        if let Some(thread) = self.thread_mut() {
            thread.search(words);
        }
        self.name_on_screen();
        self.card_keys = false;
        true
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

    /// The card's page can go back or forward as `travel` says (queue item
    /// 737): the card's *Back* and *Forward* are pressable only where there
    /// is somewhere to go. Whether the card shows something else for it.
    pub fn card_travel(&mut self, travel: Travel) -> bool {
        match self.thread_mut().and_then(Thread::card_mut) {
            Some(card) if (card.back, card.forward) != (travel.back, travel.forward) => {
                card.back = travel.back;
                card.forward = travel.forward;
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
        // Nothing of the conversation is drawn under *Settings* (854), which
        // scrolls itself (queue item 880).
        if let Some(scrolled) = self.settings.scrolled_mut() {
            return scrolled.by(by, Room::settings(viewport).height);
        }
        self.thread_mut()
            .is_some_and(|thread| thread.scroll(by, viewport))
    }

    /// Where the page on screen that scrolls is seen, on a first screen
    /// laid out at `viewport`: *Settings*' room while it is open (queue
    /// item 880), the conversation's otherwise.
    pub fn room(&self, viewport: Size) -> Room {
        if self.settings.is_open() {
            Room::settings(viewport)
        } else {
            Room::conversation(viewport)
        }
    }

    /// How far the page on screen that scrolls is scrolled and how far it
    /// reaches, if it scrolls: *Settings* while it is open, the chat on
    /// screen's conversation otherwise.
    pub fn scrolled(&self) -> Option<crate::scrolled::Scrolled> {
        if self.settings.is_open() {
            return Some(self.settings.scrolled());
        }
        self.thread().map(Thread::scrolled)
    }

    /// Whether a person's paging keys scroll the conversation now (queue
    /// item 754, [`crate::conversation_keys`]): it holds a turn, and nothing
    /// holding the keyboard would take them — neither the card's page,
    /// nor *Search chats*, nor the box while it holds what was typed.
    pub fn keys_scroll(&self) -> bool {
        // *Settings* scrolls by them too (queue item 880), unless *Search
        // chats* has the keyboard.
        if self.settings.is_open() {
            return !self.find.focused();
        }
        self.conversing()
            && !self.keys_to_the_card()
            && !self.find.focused()
            && (!self.the_box.focused() || self.the_box.typed().is_empty())
    }

    /// Scroll as a person's key asked, laid out at `viewport` (queue item
    /// 754): whether it scrolled.
    pub fn scroll_by_key(&mut self, scroll: Scroll, viewport: Size) -> bool {
        if let Some(scrolled) = self.settings.scrolled_mut() {
            let room = Room::settings(viewport).height;
            return match scroll {
                Scroll::By(by) => scrolled.by(by, room),
                Scroll::ToFirst => scrolled.to_end(true, room),
                Scroll::ToNewest => scrolled.to_end(false, room),
            };
        }
        self.thread_mut().is_some_and(|thread| match scroll {
            Scroll::By(by) => thread.scroll(by, viewport),
            Scroll::ToFirst => thread.scroll_to_end(true, viewport),
            Scroll::ToNewest => thread.scroll_to_end(false, viewport),
        })
    }

    /// A wheel's lines turned `by` CSS pixels down over the conversation
    /// `now`, laid out at `viewport` on a screen of `scale` (queue item
    /// 756): whether a glide is under way for them.
    pub fn glide_lines(&mut self, by: f32, viewport: Size, now: Instant, scale: f32) -> bool {
        if let Some(scrolled) = self.settings.scrolled_mut() {
            return scrolled.glide_lines(by, Room::settings(viewport).height, now, scale);
        }
        self.thread_mut()
            .is_some_and(|thread| thread.glide_lines(by, viewport, now, scale))
    }

    /// Whether the chat on screen's conversation is gliding.
    pub fn gliding(&self) -> bool {
        if self.settings.is_open() {
            return self.settings.scrolled().gliding();
        }
        self.thread().is_some_and(Thread::gliding)
    }

    /// The chat on screen's glide — or *Settings*' (queue item 880) — moved
    /// on to `now`: whether it scrolled.
    pub fn glide(&mut self, now: Instant, viewport: Size, scale: f32) -> bool {
        if let Some(scrolled) = self.settings.scrolled_mut() {
            return scrolled.glide(now, Room::settings(viewport).height, scale);
        }
        self.thread_mut()
            .is_some_and(|thread| thread.glide(now, viewport, scale))
    }

    /// The first screen's renderer measured `drawn` turns of the chat on
    /// screen to reach as `reach` says, laid out at `viewport`: whether
    /// keeping inside it scrolled the conversation.
    pub fn reached(&mut self, reach: Reach, drawn: usize, viewport: Size) -> bool {
        self.thread_mut()
            .is_some_and(|thread| thread.reached(reach, drawn, viewport))
    }

    /// The first screen's renderer measured *Settings*, drawn as `drawn`, to
    /// reach as `reach` says, laid out at `viewport` (queue item 880):
    /// whether that scrolled it — kept inside it, or what Tab moved the
    /// keyboard to brought into view. Nothing while it is closed.
    pub fn settings_reached(
        &mut self,
        reach: Reach,
        drawn: crate::conversation_drawing::SettingsDrawn,
        viewport: Size,
    ) -> bool {
        self.settings
            .reached(reach, drawn, Room::settings(viewport).height)
    }

    /// *Settings* as it is now: whether it is on screen, and the face the
    /// interface is set in (queue item 854).
    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// The same conversation, set in `face` — read back from where it was
    /// kept as the window opens (queue item 854).
    #[must_use]
    pub fn set_in(mut self, face: InterfaceFace) -> Self {
        self.settings = Settings::set_in(face).with_limits(self.settings.limits());
        self
    }

    /// The same conversation, alo's agent kept to `limits` — read back from
    /// where they were kept as the window opens (queue item 878).
    #[must_use]
    pub fn limited_to(mut self, limits: Limits) -> Self {
        self.settings = self.settings.with_limits(limits);
        self
    }

    /// Whether a person changed alo's agent's limits in *Settings* since
    /// this was last asked, once: they are then to be kept.
    pub fn take_limits_changed(&mut self) -> bool {
        self.settings.take_limits_changed()
    }

    /// Close *Settings* — Escape, its ×, a press on the veil around it —
    /// back to the chat on screen, the keyboard given back to where it was
    /// as it opened (queue item 886): whether it was open.
    pub fn close_settings(&mut self) -> bool {
        if !self.settings.close() {
            return false;
        }
        self.services.leave();
        match core::mem::take(&mut self.keyboard_before_settings) {
            Keyboard::TheBox => {
                let _ = self.the_box.focus();
            }
            Keyboard::SearchChats => {
                let _ = self.find.focus();
            }
            Keyboard::TheCard => self.card_keys = self.card().is_some(),
            Keyboard::Nowhere => {}
        }
        true
    }

    /// Where the keyboard is now: what *Settings* gives it back to as it
    /// closes (queue item 886).
    fn keyboard(&self) -> Keyboard {
        if self.find.focused() {
            Keyboard::SearchChats
        } else if self.the_box.focused() {
            Keyboard::TheBox
        } else if self.keys_to_the_card() {
            Keyboard::TheCard
        } else {
            Keyboard::Nowhere
        }
    }

    /// The face a person chose in *Settings* since this was last asked, to
    /// be kept beside the chats.
    pub fn take_face_chosen(&mut self) -> Option<InterfaceFace> {
        self.settings.take_chosen()
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
    use crate::blocking_off::Off;
    use alo_renderer::card::Shield;
    use alo_renderer::conversation_decide::Refusal;
    use alo_renderer::message::Failure;
    use alo_renderer::panel::{Doing, NEW_CHAT};
    use alo_renderer::turn::Turn;
    use std::time::Duration;

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
    fn a_chat_deleted_leaves_the_panel_at_once_and_comes_back_with_undo() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::NewChat);
        let showing = conversation.showing();
        let [newest, middle, oldest] = showing.chats.as_slice() else {
            panic!("{showing:?}");
        };
        let (newest, middle, oldest) = (*newest, *middle, *oldest);
        // Hovered: its row is hovered — open, it offers its ×, not Delete
        // (queue item 791) — and a chat not listed is no row.
        assert!(conversation.hover(Some(middle)));
        assert!(!conversation.hover(Some(middle)), "the same row again");
        assert_eq!(conversation.showing().hovered, None);
        assert_eq!(conversation.showing().closing, Some(middle));
        let hovered: Vec<bool> = conversation
            .panel(Size::new(240.0, 820.0), Scale::ONE)
            .chats
            .iter()
            .map(|chat| chat.hovered)
            .collect();
        assert_eq!(hovered, vec![false, true, false]);
        let now = Instant::now();
        assert!(conversation.act(Pressed::DeleteChat(middle)));
        assert_eq!(conversation.showing().chats, vec![newest, oldest]);
        assert_eq!(conversation.showing().hovered, None);
        assert_eq!(conversation.showing().undo, Some(middle));
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert_eq!(panel.deleted.as_deref(), Some(NEW_CHAT));
        assert!(
            conversation
                .undo_until()
                .is_some_and(|until| until >= now + crate::chat_delete::UNDO_FOR),
            "held for its five seconds"
        );
        assert!(
            !conversation.hover(Some(middle)),
            "a deleted chat is no row"
        );
        // Undo: back where it stood, not on screen.
        assert!(conversation.act(Pressed::Undo(middle)));
        assert_eq!(conversation.showing().chats, vec![newest, middle, oldest]);
        assert_eq!(conversation.on_screen(), Some(newest));
        assert!(!conversation.undo(middle), "brought back once");
        assert_eq!(conversation.undo_until(), None);
        assert!(conversation.take_gone().is_empty(), "nothing is gone");
    }

    #[test]
    fn a_chat_deleted_is_gone_when_its_undo_passes_another_is_deleted_or_alo_closes() {
        let mut conversation = Conversation::default();
        for _ in 0..3 {
            let _ = conversation.act(Pressed::NewChat);
        }
        let ids = conversation.showing().chats;
        let now = Instant::now();
        assert!(conversation.delete(ids[1], now));
        assert!(!conversation.expire(now + Duration::from_millis(4_999)));
        assert!(conversation.take_gone().is_empty());
        assert!(conversation.expire(now + Duration::from_secs(5)));
        assert_eq!(conversation.take_gone(), vec![ids[1]]);
        assert!(conversation.take_gone().is_empty(), "each once");
        assert!(!conversation.undo(ids[1]), "too late");
        // A second deleted while the first is held: the first is gone.
        assert!(conversation.delete(ids[2], now));
        assert!(conversation.delete(ids[3], now));
        assert_eq!(conversation.take_gone(), vec![ids[2]]);
        assert_eq!(conversation.showing().undo, Some(ids[3]));
        conversation.let_go();
        assert_eq!(conversation.take_gone(), vec![ids[3]]);
        assert!(!conversation.delete(ids[3], now), "no such chat any more");
    }

    #[test]
    fn the_chat_on_screen_deleted_puts_an_empty_open_chat_on_screen_or_a_new_one() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::NewChat);
        let [on_screen, other] = conversation.showing().chats[..] else {
            panic!("two chats");
        };
        assert_eq!(conversation.on_screen(), Some(on_screen));
        // Another empty open chat is there: it is put on screen.
        assert!(conversation.delete(on_screen, Instant::now()));
        assert_eq!(conversation.on_screen(), Some(other));
        assert_eq!(conversation.showing().chats, vec![other]);
        // None is left but the one on screen, holding a turn: a new one.
        let _ = conversation.act(Pressed::TheBox);
        let _ = conversation.edit(Edit::Put("example.com".to_owned()));
        assert!(matches!(conversation.edit(Edit::Choose), Typed::Chose(_)));
        let url = conversation.going().cloned().unwrap();
        conversation.open_card(url);
        assert!(conversation.card().is_some());
        assert!(conversation.delete(other, Instant::now()));
        assert!(conversation.card().is_none(), "its card put away with it");
        let [fresh] = conversation.showing().chats[..] else {
            panic!("one chat");
        };
        assert_ne!(fresh, other);
        assert_eq!(conversation.on_screen(), Some(fresh));
        assert_eq!(rows(&conversation), (vec![NEW_CHAT.to_owned()], vec![true]));
        // Brought back: its page kept as its address, not a live card.
        assert!(conversation.undo(other));
        assert!(conversation.card().is_none());
        assert_eq!(conversation.on_screen(), Some(fresh));
    }

    #[test]
    fn a_cards_shield_counts_its_pages_refusals_and_switches_its_site() {
        let mut conversation = Conversation::default();
        conversation.set_blocking_off(Off::of(["bbc.com".to_owned()]));
        let _ = conversation.act(Pressed::TheBox);
        let _ = conversation.edit(Edit::Put("www.theguardian.com".to_owned()));
        assert!(matches!(conversation.edit(Edit::Choose), Typed::Chose(_)));
        let url = conversation.going().cloned().unwrap();
        conversation.open_card(url);
        let shield = |conversation: &Conversation| conversation.card().and_then(|card| card.shield);
        assert_eq!(
            shield(&conversation),
            Some(Shield {
                blocked: 0,
                on: true
            })
        );
        assert!(conversation.blocked() && conversation.blocked());
        assert_eq!(
            shield(&conversation),
            Some(Shield {
                blocked: 2,
                on: true
            })
        );
        assert_eq!(
            conversation.showing().shield.as_deref(),
            Some("theguardian.com")
        );
        // Another page of the card: counted anew, and off where switched.
        assert!(conversation.moved_card(alo_url::parse("https://www.bbc.com/news").unwrap()));
        assert_eq!(
            shield(&conversation),
            Some(Shield {
                blocked: 0,
                on: false
            })
        );
        assert!(!conversation.blocked(), "nothing is counted while off");
        // Pressed: on again for the BBC, kept for the window once.
        assert!(conversation.act(Pressed::Shield("bbc.com".to_owned())));
        assert_eq!(
            shield(&conversation),
            Some(Shield {
                blocked: 0,
                on: true
            })
        );
        assert!(!conversation.shield_off("bbc.com"));
        assert_eq!(conversation.take_switched().as_deref(), Some("bbc.com"));
        assert_eq!(conversation.take_switched(), None);
        assert!(conversation.act(Pressed::Shield("bbc.com".to_owned())));
        assert!(conversation.shield_off("bbc.com"));
        assert_eq!(
            conversation.blocking_off().sites(),
            vec!["bbc.com".to_owned()]
        );
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
                url,
                ran: Vec::new(),
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
    fn the_box_has_the_keyboard_as_the_window_opens_and_on_every_new_chat() {
        // Queue item 875: a person's first keys reach the box unpressed.
        let mut conversation = Conversation::default();
        assert_eq!(conversation.field_focused(), Some(Field::TheBox));
        // A press elsewhere takes it, as before; New chat gives it back.
        let _ = conversation.act(Pressed::SearchChats);
        assert_eq!(conversation.field_focused(), Some(Field::SearchChats));
        assert!(conversation.act(Pressed::NewChat));
        assert_eq!(conversation.field_focused(), Some(Field::TheBox));
        let _ = conversation.act(Pressed::Settings);
        assert_eq!(conversation.field_focused(), None);
        let _ = conversation.act(Pressed::NewChat);
        assert_eq!(conversation.field_focused(), Some(Field::TheBox));
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
    fn the_paging_keys_scroll_only_while_nothing_holding_the_keyboard_would_take_them() {
        let viewport = Size::new(1040.0, 820.0);
        let down = |conversation: &Conversation| conversation.thread().map(|t| t.down().to_bits());
        let mut conversation = Conversation::default();
        assert!(!conversation.keys_scroll(), "nothing to scroll");
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "a");
        let _ = conversation.edit(Edit::Choose);
        type_in(&mut conversation, "b");
        let _ = conversation.edit(Edit::Choose);
        assert!(
            !conversation.scroll_by_key(Scroll::ToFirst, viewport),
            "no reach yet"
        );
        let reach = Reach {
            earlier: 900.0,
            newest: 1000.0,
        };
        let _ = conversation.reached(reach, 2, viewport);
        // The box has the keyboard, empty: the keys are the conversation's.
        assert!(conversation.keys_scroll());
        assert!(conversation.scroll_by_key(Scroll::By(-636.0), viewport));
        assert_eq!(down(&conversation), Some((-636.0_f32).to_bits()));
        assert!(conversation.scroll_by_key(Scroll::By(-636.0), viewport));
        assert_eq!(
            down(&conversation),
            Some((-900.0_f32).to_bits()),
            "the first turn's top"
        );
        assert!(
            !conversation.scroll_by_key(Scroll::ToFirst, viewport),
            "already there"
        );
        assert!(conversation.scroll_by_key(Scroll::ToNewest, viewport));
        assert_eq!(
            down(&conversation),
            Some(324.0_f32.to_bits()),
            "the newest turn's foot at the room's foot: 1000 less 676"
        );
        assert!(conversation.scroll_by_key(Scroll::ToFirst, viewport));
        // Words in the box: its keys are its own.
        type_in(&mut conversation, "x");
        assert!(!conversation.keys_scroll());
        let _ = conversation.edit(Edit::Erase(
            crate::text_field::Unit::Character,
            crate::text_field::Toward::Start,
        ));
        assert!(conversation.keys_scroll(), "empty again");
        // Nothing has the keyboard: the conversation's.
        let _ = conversation.act(Pressed::Nothing);
        assert!(conversation.keys_scroll());
        // *Search chats* keeps its own.
        let _ = conversation.act(Pressed::SearchChats);
        assert!(!conversation.keys_scroll());
        let _ = conversation.act(Pressed::Nothing);
        // The card's page keeps every key while it has the keyboard.
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        let _ = conversation.press_the_page();
        assert!(conversation.keys_to_the_card());
        assert!(!conversation.keys_scroll());
        let _ = conversation.act(Pressed::Nothing);
        assert!(
            conversation.keys_scroll(),
            "a press elsewhere takes it from the page"
        );
    }

    #[test]
    fn a_chat_is_named_by_the_first_words_said_in_it_and_never_renamed() {
        let mut conversation = Conversation::default();
        conversation.set_search(SetUp::Ready);
        let _ = conversation.act(Pressed::TheBox);
        assert_eq!(rows(&conversation).0, vec![NEW_CHAT.to_owned()]);
        type_in(&mut conversation, "  ev   chargers near me ");
        let _ = conversation.edit(Edit::Choose);
        assert_eq!(
            rows(&conversation).0,
            vec!["ev chargers near me".to_owned()]
        );
        // A later turn does not rename it.
        type_in(&mut conversation, "heat pumps");
        let _ = conversation.edit(Edit::Choose);
        assert_eq!(conversation.thread().map(|t| t.turns().len()), Some(2));
        assert_eq!(
            rows(&conversation).0,
            vec!["ev chargers near me".to_owned()]
        );
        // A new chat is *New chat* until it holds a turn: here a page
        // opened from the box, named by the address typed.
        assert!(conversation.act(Pressed::NewChat));
        assert_eq!(
            rows(&conversation).0,
            vec![NEW_CHAT.to_owned(), "ev chargers near me".to_owned()]
        );
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let _ = conversation.edit(Edit::Choose);
        assert_eq!(rows(&conversation).0[0], NEW_CHAT, "chosen, not yet open");
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        assert_eq!(
            rows(&conversation).0,
            vec!["example.com".to_owned(), "ev chargers near me".to_owned()]
        );
        // *Search chats* finds a chat by its name.
        let _ = conversation.act(Pressed::SearchChats);
        for letter in "CHARGERS".chars() {
            let _ = conversation.edit(Edit::Put(letter.to_string()));
        }
        assert_eq!(
            rows(&conversation).0,
            vec!["ev chargers near me".to_owned()]
        );
    }

    #[test]
    fn chats_kept_from_before_are_earlier_numbered_past_and_open_once_pressed() {
        use crate::chat_record::Record;
        let visited = |said: &str| Turn::Visited {
            said: said.to_owned(),
            url: alo_url::parse("https://a.example/").unwrap(),
            ran: Vec::new(),
        };
        let kept = vec![
            Kept {
                number: 9,
                record: Record {
                    name: "newer".to_owned(),
                    turns: vec![visited("b"), visited("c")],
                },
            },
            Kept {
                number: 4,
                record: Record {
                    name: "older".to_owned(),
                    turns: vec![visited("a")],
                },
            },
        ];
        // A damaged file numbered 11 holds the number past them.
        let mut conversation = Conversation::default().keeping(kept, Some(11));
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        let shown: Vec<(&str, bool, bool)> = panel
            .chats
            .iter()
            .map(|chat| (chat.label(), chat.open, chat.current))
            .collect();
        assert_eq!(
            shown,
            vec![
                (NEW_CHAT, true, true),
                ("newer", false, false),
                ("older", false, false)
            ]
        );
        assert_eq!(conversation.on_screen().map(ChatId::number), Some(12));
        assert!(!conversation.conversing(), "a new chat on screen");
        // Only chats holding turns are kept, by their numbers.
        let to_keep: Vec<(u64, &str, usize)> = conversation
            .to_keep()
            .map(|(id, name, thread)| (id.number(), name, thread.turns().len()))
            .collect();
        assert_eq!(to_keep, vec![(9, "newer", 2), (4, "older", 1)]);
        // Pressed, an earlier chat is on screen and open, its turns shown
        // from the newest's top.
        let showing = conversation.showing();
        let older = showing.chats.get(2).copied().expect("drawn third");
        assert!(conversation.act(Pressed::Chat(older)));
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert!(
            panel
                .chats
                .iter()
                .any(|chat| chat.label() == "older" && chat.open && chat.current)
        );
        assert_eq!(
            conversation.thread().map(Thread::down).map(f32::to_bits),
            Some(0)
        );
        assert_eq!(conversation.thread().map(|t| t.turns().len()), Some(1));
        // New chats are numbered past every kept one.
        assert!(conversation.act(Pressed::NewChat));
        assert_eq!(conversation.on_screen().map(ChatId::number), Some(13));
    }

    #[test]
    fn a_chat_of_two_addresses_is_named_by_the_first_ones_host_alone() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "en.wikipedia.org/wiki/Kampala");
        let _ = conversation.edit(Edit::Choose);
        conversation.open_card(alo_url::parse("https://en.wikipedia.org/wiki/Kampala").unwrap());
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "https://www.bbc.com/news");
        let _ = conversation.edit(Edit::Choose);
        conversation.open_card(alo_url::parse("https://www.bbc.com/news").unwrap());
        assert_eq!(conversation.thread().map(|t| t.turns().len()), Some(2));
        assert_eq!(rows(&conversation).0, vec!["en.wikipedia.org".to_owned()]);
        // Kept and read back, it is named the same.
        let kept: Vec<(u64, String)> = conversation
            .to_keep()
            .map(|(id, name, _)| (id.number(), name.to_owned()))
            .collect();
        assert_eq!(kept, vec![(0, "en.wikipedia.org".to_owned())]);
    }

    #[test]
    fn a_name_kept_by_the_rule_before_is_named_again_as_it_is_read() {
        use crate::chat_record::Record;
        let visited = |said: &str, url: &str| Turn::Visited {
            said: said.to_owned(),
            url: alo_url::parse(url).unwrap(),
            ran: Vec::new(),
        };
        let glued = "https://en.wikipedia.orghttps://en.wikipedia.org";
        let kept = vec![
            // Batch 16's chat, as 758's rule named it.
            Kept {
                number: 2,
                record: Record {
                    name: glued.to_owned(),
                    turns: vec![
                        visited(glued, "https://en.wikipedia.org/"),
                        visited("www.bbc.com/news", "https://www.bbc.com/news"),
                    ],
                },
            },
            // A name not 758's of its first turn: kept as it is.
            Kept {
                number: 1,
                record: Record {
                    name: "https://example.com".to_owned(),
                    turns: vec![visited("example.com", "https://example.com/")],
                },
            },
        ];
        let conversation = Conversation::default().keeping(kept, None);
        assert_eq!(
            rows(&conversation).0,
            vec![
                NEW_CHAT.to_owned(),
                "en.wikipedia.org".to_owned(),
                "https://example.com".to_owned()
            ]
        );
        // And written so the next time it is kept.
        let names: Vec<&str> = conversation.to_keep().map(|(_, name, _)| name).collect();
        assert_eq!(names, vec!["en.wikipedia.org", "https://example.com"]);
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
        let _ = conversation.the_box.blur();
        let before = conversation.clone();
        assert!(!conversation.act(Pressed::Chat(ChatId(99))));
        assert_eq!(rows(&conversation), rows(&before));
        assert_eq!(conversation.on_screen(), before.on_screen());
    }

    #[test]
    fn every_other_press_is_kept_and_changes_nothing_yet() {
        for pressed in [
            Pressed::Site(KeptSite::new("Mail", "https://mail.example/")),
            Pressed::AddASite,
            Pressed::Waiting(ChatId(0)),
            Pressed::Go,
            Pressed::Nothing,
            Pressed::Refused(Refusal::Unasked),
            Pressed::Failed(Failure::NotTheConversation),
        ] {
            let mut conversation = Conversation::default();
            // Not the box's keyboard, given as the window opens (875), to
            // take away.
            let _ = conversation.the_box.blur();
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
                more: false,
                answer: alo_renderer::results_answer::Answer::NotAsked,
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
    fn show_more_sites_asks_the_next_page_once_per_press_and_appends_it() {
        use alo_renderer::results::{Images, Tab};
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
                showing: results::Showing::Found(vec![found("https://one.example/"); 20]),
                images: Images::NotAsked,
                more: true,
                answer: alo_renderer::results_answer::Answer::NotAsked,
            },
        }));
        let more = Pressed::ShowMoreSites { chat, turn: 0 };
        // Offered, but drawn only on Sites: nothing to press on All.
        assert!(conversation.showing().show_more.is_empty());
        assert!(conversation.act(Pressed::Tab {
            chat,
            turn: 0,
            tab: Tab::Sites
        }));
        assert_eq!(conversation.showing().show_more, vec![(chat, 0)]);
        assert!(conversation.act(more.clone()));
        assert_eq!(
            conversation.take_asks(),
            vec![Further {
                number: 1,
                words: "ev".to_owned(),
                tab: Tab::Sites,
                offset: 1
            }]
        );
        // One at a time: a second press while it is asked asks nothing, and
        // the button is no longer one to press.
        assert!(!conversation.act(more.clone()));
        assert!(conversation.take_asks().is_empty());
        assert!(conversation.showing().show_more.is_empty());
        let page = vec![found("https://two.example/"); 20];
        assert!(conversation.searched(Searched {
            number: 1,
            words: "ev".to_owned(),
            came: Came::Page(Ok((page, true))),
        }));
        assert_eq!(
            conversation.results().map(|r| r.drawn_sites().len()),
            Some(40)
        );
        assert!(conversation.act(more));
        assert_eq!(
            conversation.take_asks().first().map(|ask| ask.offset),
            Some(2)
        );
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
                more: false,
                answer: alo_renderer::results_answer::Answer::NotAsked,
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
                tab: Tab::Videos,
                offset: 0
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
                tab: Tab::News,
                offset: 0
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
                url: two,
                ran: Vec::new(),
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
        // Given the keyboard as the window opens (875), and pressed away.
        let _ = conversation.act(Pressed::HideThePanel);
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
            Some("http://localhost:8080/"),
            "this machine's, read as http (874)"
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
    fn the_cards_back_and_forward_are_where_its_page_can_go() {
        let mut conversation = Conversation::default();
        let travel = |back, forward| Travel { back, forward };
        assert!(!conversation.card_travel(travel(true, false)), "no card");
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        let shown =
            |conversation: &Conversation| conversation.card().map(|card| (card.back, card.forward));
        assert_eq!(shown(&conversation), Some((false, false)), "a new card");
        assert!(!conversation.card_travel(travel(false, false)), "unchanged");
        assert!(conversation.card_travel(travel(true, false)));
        assert_eq!(shown(&conversation), Some((true, false)));
        assert!(conversation.card_travel(travel(false, true)));
        assert_eq!(shown(&conversation), Some((false, true)));
        assert!(!conversation.card_travel(travel(false, true)));
    }

    #[test]
    fn a_press_elsewhere_takes_the_keyboard_and_hides_the_offers() {
        for elsewhere in [Pressed::Nothing, Pressed::AddASite] {
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
        assert!(matches!(
            conversation.edit(Edit::Erase(
                crate::text_field::Unit::Character,
                crate::text_field::Toward::Start
            )),
            Typed::Changed
        ));
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
        assert_eq!(rail.chats.len(), 1, "the rail shows its chats (760)");
        assert!((rail.scale - 2.0).abs() < f32::EPSILON);
        let showing = conversation.showing();
        assert!(showing.hidden);
        assert_eq!(showing.chats.len(), 1);
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

    #[test]
    fn a_narrow_window_folds_the_panel_to_the_rail_and_a_wider_one_brings_it_back() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::SearchChats);
        type_in(&mut conversation, "new");
        assert!(!conversation.fit(760), "760 holds the panel");
        assert_eq!(conversation.side(), Side::Panel);
        assert!(conversation.find().focused());
        assert!(conversation.fit(759), "759 folds it");
        assert_eq!(conversation.side(), Side::Rail);
        assert!(!conversation.find().focused(), "the rail has no field");
        let rail = conversation.panel(Size::new(64.0, 820.0), Scale::ONE);
        assert!(rail.hidden, "drawn as the rail");
        assert_eq!(rail.chats.len(), 1, "its chats, whatever was searched");
        assert!(conversation.showing().hidden, "and decided against as it");
        assert!(!conversation.fold().hidden(), "nobody hid it");
        assert!(conversation.fit(1280), "widened, it unfolds");
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert!(!panel.hidden);
        assert_eq!(panel.searched, "new", "what was searched is kept");
        assert_eq!(panel.chats.len(), 1);
    }

    #[test]
    fn a_person_who_hid_the_panel_keeps_it_hidden_past_the_fold_both_ways() {
        let mut conversation = Conversation::default();
        assert!(conversation.act(Pressed::HideThePanel));
        assert!(!conversation.fit(700), "already the rail");
        assert!(!conversation.fit(1280), "a person hid it");
        assert_eq!(conversation.side(), Side::Rail);
        // Shown while narrow: kept, the panel over the conversation as the
        // sheet meanwhile (queue item 776), and beside it once the window
        // is wide.
        let _ = conversation.fit(700);
        assert!(conversation.act(Pressed::ShowThePanel), "the sheet");
        assert_eq!(conversation.side(), Side::Rail);
        assert!(conversation.sheet());
        assert!(!conversation.fold().hidden());
        assert!(conversation.fit(760));
        assert_eq!(conversation.side(), Side::Panel);
        assert!(!conversation.sheet());
    }

    /// A conversation of chats called by `names`, each holding one turn,
    /// the first on screen, and `earlier` kept from before under *Earlier*.
    fn named_chats(names: &[&str], earlier: &[&str]) -> Conversation {
        use crate::chat_record::Record;
        let visited = |said: &str| Turn::Visited {
            said: said.to_owned(),
            url: alo_url::parse("https://a.example/").unwrap(),
            ran: Vec::new(),
        };
        let kept = earlier
            .iter()
            .enumerate()
            .map(|(at, name)| Kept {
                number: 100 - u64::try_from(at).unwrap_or(0),
                record: Record {
                    name: (*name).to_owned(),
                    turns: vec![visited(name)],
                },
            })
            .collect();
        let mut conversation = Conversation::default().keeping(kept, Some(100));
        // Made oldest first, so the first named is the newest, on screen.
        for name in names.iter().rev() {
            let _ = conversation.act(Pressed::NewChat);
            let id = conversation.on_screen().unwrap();
            if let Some((_, chat)) = conversation.chats.iter_mut().find(|(each, _)| *each == id) {
                chat.name = (*name).to_owned();
            }
            if let Some((_, thread)) = conversation
                .threads
                .iter_mut()
                .find(|(each, _)| *each == id)
            {
                *thread = Thread::kept(vec![visited(name)]);
            }
        }
        // The empty chat the conversation began with goes: nothing in it.
        let first = conversation
            .chats
            .iter()
            .find(|(_, chat)| chat.name.is_empty())
            .map(|(id, _)| *id);
        if let Some(first) = first {
            assert!(conversation.close(first));
        }
        conversation
    }

    /// Each chat's label, whether open, and whether on screen, as drawn.
    fn drawn(conversation: &Conversation) -> Vec<(String, bool, bool)> {
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        drawn_order(&panel.chats)
            .into_iter()
            .filter_map(|at| panel.chats.get(at))
            .map(|chat| (chat.label().to_owned(), chat.open, chat.current))
            .collect()
    }

    fn id_of(conversation: &Conversation, name: &str) -> ChatId {
        conversation
            .chats
            .iter()
            .find(|(_, chat)| chat.label() == name)
            .map_or_else(|| panic!("no chat called {name}"), |(id, _)| *id)
    }

    #[test]
    fn a_closed_chat_moves_to_the_top_of_earlier_and_nothing_is_deleted() {
        let mut conversation = named_chats(&["Lisbon", "Invoice", "EV"], &["Phones"]);
        assert_eq!(
            drawn(&conversation),
            vec![
                ("Lisbon".to_owned(), true, true),
                ("Invoice".to_owned(), true, false),
                ("EV".to_owned(), true, false),
                ("Phones".to_owned(), false, false),
            ]
        );
        let invoice = id_of(&conversation, "Invoice");
        // Hovered, an open row offers its ×, and no Delete.
        assert!(conversation.hover(Some(invoice)));
        assert_eq!(conversation.showing().closing, Some(invoice));
        assert_eq!(conversation.showing().hovered, None);
        assert!(conversation.act(Pressed::CloseChat(invoice)));
        assert_eq!(
            drawn(&conversation),
            vec![
                ("Lisbon".to_owned(), true, true),
                ("EV".to_owned(), true, false),
                ("Invoice".to_owned(), false, false),
                ("Phones".to_owned(), false, false),
            ],
            "first under Earlier"
        );
        // Kept, turns and all: nothing is deleted, nothing gone.
        assert!(conversation.take_gone().is_empty());
        assert_eq!(conversation.to_keep().count(), 4);
        // Hovered now, it is an earlier row: it offers Delete, and no ×.
        assert!(conversation.hover(Some(invoice)));
        assert_eq!(conversation.showing().hovered, Some(invoice));
        assert_eq!(conversation.showing().closing, None);
        // Closed twice is closed once; pressed, it is open again.
        assert!(!conversation.close(invoice));
        assert!(conversation.act(Pressed::Chat(invoice)));
        assert_eq!(conversation.on_screen(), Some(invoice));
        assert!(drawn(&conversation).contains(&("Invoice".to_owned(), true, true)));
    }

    #[test]
    fn the_chat_on_screen_closed_puts_the_one_below_on_screen_or_above_or_a_new_one() {
        let mut conversation = named_chats(&["Lisbon", "Invoice", "EV"], &[]);
        let (lisbon, invoice, ev) = (
            id_of(&conversation, "Lisbon"),
            id_of(&conversation, "Invoice"),
            id_of(&conversation, "EV"),
        );
        // The one below it.
        assert!(conversation.close(lisbon));
        assert_eq!(conversation.on_screen(), Some(invoice));
        // The last open one: the one above it.
        assert!(conversation.act(Pressed::Chat(ev)));
        assert!(conversation.close(ev));
        assert_eq!(conversation.on_screen(), Some(invoice));
        // None left open: a new chat, empty, on screen — and that one,
        // empty and on screen, offers no × and is not closed.
        assert!(conversation.close(invoice));
        let fresh = conversation.on_screen().unwrap();
        assert!(![lisbon, invoice, ev].contains(&fresh));
        assert_eq!(
            drawn(&conversation).first(),
            Some(&(NEW_CHAT.to_owned(), true, true))
        );
        assert!(conversation.hover(Some(fresh)));
        assert_eq!(conversation.showing().closing, None);
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert_eq!(panel.chats[0].closing, Closing::Withheld);
        assert!(!conversation.close(fresh));
        // Everything closed is under Earlier, newest closed first.
        let earlier: Vec<String> = drawn(&conversation)
            .into_iter()
            .filter(|(_, open, _)| !open)
            .map(|(name, _, _)| name)
            .collect();
        assert_eq!(earlier, vec!["Invoice", "EV", "Lisbon"]);
    }

    #[test]
    fn an_empty_chat_closed_goes_and_a_card_on_screen_closed_is_put_away() {
        let mut conversation = named_chats(&["Lisbon"], &[]);
        let _ = conversation.act(Pressed::NewChat);
        let empty = conversation.on_screen().unwrap();
        let lisbon = id_of(&conversation, "Lisbon");
        assert!(conversation.act(Pressed::Chat(lisbon)));
        // Not on screen, empty: closable, and closed it goes.
        assert!(conversation.close(empty));
        assert_eq!(drawn(&conversation).len(), 1);
        assert!(!conversation.chats.iter().any(|(id, _)| *id == empty));
        assert!(!conversation.threads.iter().any(|(id, _)| *id == empty));
        // A live card in the chat on screen is put away as its address.
        conversation.open_card(alo_url::parse("https://example.com/").unwrap());
        assert!(conversation.card().is_some());
        assert!(conversation.close(lisbon));
        assert!(conversation.card().is_none());
        let kept: Vec<usize> = conversation
            .to_keep()
            .map(|(_, _, thread)| thread.turns().len())
            .collect();
        assert_eq!(kept, vec![2], "its turns, the card's address among them");
    }

    #[test]
    fn the_rail_holds_the_five_most_recent_chats_open_first_and_names_the_hovered() {
        let mut conversation = named_chats(
            &["Lisbon", "Invoice", "EV"],
            &["Phones", "Birthday", "Taxes"],
        );
        let _ = conversation.act(Pressed::SearchChats);
        type_in(&mut conversation, "zzz");
        assert!(conversation.act(Pressed::HideThePanel));
        let rail = conversation.panel(Size::new(64.0, 820.0), Scale::ONE);
        assert!(rail.hidden);
        let names: Vec<&str> = rail.chats.iter().map(Chat::label).collect();
        assert_eq!(
            names,
            vec!["Lisbon", "Invoice", "EV", "Phones", "Birthday"],
            "whatever was searched"
        );
        let showing = conversation.showing();
        assert_eq!(showing.chats.len(), 5);
        assert_eq!(conversation.panel_width(), 64);
        // Hovered: its name beside it, the rail laid out wider; it offers
        // neither × nor Delete.
        let ev = id_of(&conversation, "EV");
        assert!(conversation.hover(Some(ev)));
        assert!(conversation.rail_named());
        assert_eq!(conversation.panel_width(), 320);
        assert_eq!(conversation.showing().closing, None);
        assert_eq!(conversation.showing().hovered, None);
        // A press on it, by its place on the rail, puts it on screen.
        let at = conversation.showing().chats.iter().position(|id| *id == ev);
        assert_eq!(at, Some(2));
        assert!(conversation.act(Pressed::Chat(ev)));
        assert_eq!(conversation.on_screen(), Some(ev));
        // A chat the rail does not show is no row on it.
        let taxes = id_of(&conversation, "Taxes");
        assert!(conversation.hover(None));
        assert!(!conversation.hover(Some(taxes)));
        assert!(!conversation.rail_named());
        // A chat from before chosen is open again, and the most recent:
        // first on the rail, the last of the five gone from it.
        let phones = id_of(&conversation, "Phones");
        assert!(conversation.act(Pressed::Chat(phones)));
        let rail = conversation.panel(Size::new(64.0, 820.0), Scale::ONE);
        let names: Vec<&str> = rail.chats.iter().map(Chat::label).collect();
        assert_eq!(names, vec!["Phones", "Lisbon", "Invoice", "EV", "Birthday"]);
    }

    #[test]
    fn show_in_a_narrow_window_is_the_sheet_and_choosing_in_it_closes_it() {
        let mut conversation = named_chats(&["Lisbon", "Invoice"], &["Phones"]);
        let _ = conversation.fit(700);
        assert!(!conversation.sheet());
        assert!(conversation.act(Pressed::ShowThePanel));
        assert!(conversation.sheet());
        // The conversation still beside the rail; the panel drawn whole,
        // decided against as whole, its rows hoverable.
        assert_eq!(conversation.side(), Side::Rail);
        assert!(
            !conversation
                .panel(Size::new(240.0, 820.0), Scale::ONE)
                .hidden
        );
        assert!(!conversation.showing().hidden);
        assert_eq!(conversation.panel_width(), 240);
        // Search chats in it keeps it out; a chat chosen closes it.
        assert!(conversation.act(Pressed::SearchChats));
        assert!(conversation.sheet());
        let invoice = id_of(&conversation, "Invoice");
        assert!(conversation.act(Pressed::Chat(invoice)));
        assert!(!conversation.sheet());
        assert!(conversation.showing().hidden, "the rail again");
        // New chat closes it.
        assert!(conversation.act(Pressed::ShowThePanel));
        assert!(conversation.act(Pressed::NewChat));
        assert!(!conversation.sheet());
        // Hide the panel closes it, and is the person's choice.
        assert!(conversation.act(Pressed::ShowThePanel));
        assert!(conversation.act(Pressed::HideThePanel));
        assert!(!conversation.sheet());
        assert!(conversation.fold().hidden());
        // Escape or a press outside: closed, Search chats letting go.
        assert!(conversation.act(Pressed::ShowThePanel));
        assert!(conversation.act(Pressed::SearchChats));
        assert!(conversation.close_sheet());
        assert!(!conversation.close_sheet(), "closed once");
        assert!(!conversation.find().focused());
        // Wide, Show the panel is the panel beside the conversation, no
        // sheet.
        let _ = conversation.fit(1280);
        assert!(!conversation.act(Pressed::ShowThePanel));
        assert!(!conversation.sheet());
        assert_eq!(conversation.side(), Side::Panel);
    }

    /// Queue items 854, 886: *Settings* opens over the chat on screen,
    /// which is drawn under it as it was and left so, and closes back to
    /// it, the keyboard given back to the box.
    #[test]
    fn settings_stands_over_the_chat_on_screen_and_leaves_it_as_it_was() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "example.com");
        let room = Size::new(1040.0, 820.0);
        let before = conversation.start(room, Scale::ONE);
        assert!(conversation.act(Pressed::Settings));
        assert!(conversation.settings().is_open());
        assert!(!conversation.act(Pressed::Settings), "open already");
        let start = conversation.start(room, Scale::ONE);
        assert!(start.settings);
        assert_eq!(start.typed, before.typed, "drawn under it as it was");
        assert_eq!(start.turns, before.turns);
        assert!(!start.focused, "the box let the keyboard go");
        // The panel: no chat current, Settings is.
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        assert!(panel.settings);
        assert!(panel.chats.iter().all(|chat| !chat.current));
        // What a press is decided against: the faces, nothing of the chat.
        let showing = conversation.showing();
        assert_eq!(showing.faces, InterfaceFace::ALL.to_vec());
        assert!(!showing.card && showing.offers.is_empty() && showing.found.is_empty());
        assert_eq!(
            showing.chats.len(),
            1,
            "the panel's chats are still pressed"
        );
        // Nothing of the conversation scrolls: the paging keys and the
        // wheel are *Settings*' (queue item 880), which scrolls nowhere
        // until its reach is measured.
        let down = conversation.thread().map(Thread::down);
        assert!(conversation.keys_scroll());
        assert!(!conversation.scroll(120.0, room));
        assert_eq!(conversation.thread().map(Thread::down), down);
        // Its × or the veil: as it was, what was typed kept, and the
        // keyboard back in the box.
        assert!(conversation.act(Pressed::LeaveSettings));
        assert!(!conversation.settings().is_open());
        let after = conversation.start(room, Scale::ONE);
        assert_eq!(after.typed, before.typed);
        assert!(!after.settings);
        assert!(after.focused, "the keyboard given back");
        assert!(conversation.showing().faces.is_empty());
    }

    #[test]
    fn a_chat_new_chat_or_escape_closes_settings_and_a_chat_chosen_is_on_screen() {
        let mut conversation = named_chats(&["Lisbon", "Invoice"], &[]);
        let lisbon = id_of(&conversation, "Lisbon");
        let invoice = id_of(&conversation, "Invoice");
        assert!(conversation.act(Pressed::Chat(invoice)));
        // The chat already on screen, pressed: Settings closes all the same.
        assert!(conversation.act(Pressed::Settings));
        assert!(conversation.act(Pressed::Chat(invoice)));
        assert!(!conversation.settings().is_open());
        assert_eq!(conversation.on_screen(), Some(invoice));
        assert!(conversation.act(Pressed::Settings));
        assert!(conversation.act(Pressed::Chat(lisbon)));
        assert_eq!(conversation.on_screen(), Some(lisbon));
        assert!(!conversation.settings().is_open());
        assert!(conversation.act(Pressed::Settings));
        assert!(conversation.act(Pressed::NewChat));
        assert!(!conversation.settings().is_open());
        assert!(conversation.act(Pressed::Settings));
        assert!(conversation.close_settings(), "Escape");
        assert!(!conversation.close_settings(), "closed already");
    }

    #[test]
    fn a_face_chosen_sets_both_parts_at_once_and_is_kept_once() {
        let mut conversation = Conversation::default();
        assert_eq!(conversation.settings().face(), InterfaceFace::Inter);
        let _ = conversation.act(Pressed::Settings);
        assert!(conversation.act(Pressed::Face(InterfaceFace::PlusJakartaSans)));
        assert!(conversation.settings().is_open(), "choosing keeps it open");
        let room = Size::new(1040.0, 820.0);
        assert_eq!(
            conversation.start(room, Scale::ONE).face,
            InterfaceFace::PlusJakartaSans
        );
        assert_eq!(
            conversation.panel(Size::new(240.0, 820.0), Scale::ONE).face,
            InterfaceFace::PlusJakartaSans
        );
        assert_eq!(
            conversation.take_face_chosen(),
            Some(InterfaceFace::PlusJakartaSans)
        );
        assert_eq!(conversation.take_face_chosen(), None);
        assert!(!conversation.act(Pressed::Face(InterfaceFace::PlusJakartaSans)));
        assert_eq!(
            conversation.take_face_chosen(),
            None,
            "the same face is no choice"
        );
        // Closed, the chat is drawn in it too; set in a face read back, it
        // starts so, nothing to keep.
        let _ = conversation.act(Pressed::LeaveSettings);
        assert_eq!(
            conversation.start(room, Scale::ONE).face,
            InterfaceFace::PlusJakartaSans
        );
        let kept = Conversation::default().set_in(InterfaceFace::Geist);
        assert_eq!(kept.panel(room, Scale::ONE).face, InterfaceFace::Geist);
        assert_eq!(kept.clone().take_face_chosen(), None);
    }

    #[test]
    fn asked_with_no_page_the_run_is_a_turn_of_its_own_then_the_card_it_opens() {
        // Queue item 821.
        let mut conversation = Conversation::default();
        conversation.set_agent(SetUp::Ready);
        assert_eq!(conversation.asking(), SetUp::Ready, "no page needed");
        let _ = conversation.act(Pressed::TheBox);
        type_in(&mut conversation, "find the Uganda Museum's hours");
        let ask = conversation.showing().offers.last().cloned().unwrap();
        assert!(matches!(ask, Offer::Ask(_)), "{ask:?}");
        assert!(conversation.act(Pressed::Offer(ask)));
        let (run, asked) = conversation.take_run().expect("started");
        assert_eq!(asked, "find the Uganda Museum's hours");
        assert_eq!(conversation.working(), Some(run));
        let room = Size::new(1040.0, 820.0);
        let start = conversation.start(room, Scale::ONE);
        assert!(
            matches!(start.turns.last(), Some(Turn::Asked(asked)) if asked.agent.is_some()),
            "{:?}",
            start.turns
        );
        assert_eq!(start.card(), None, "no page, no split");
        // Another run's open is nothing; its own is chosen as an address,
        // and the card opened for it holds the run.
        let museum = alo_url::parse("https://ugandamuseum.go.ug/").unwrap();
        let pinned = Pinned::new(
            "ugandamuseum.go.ug",
            std::net::IpAddr::from([93, 184, 215, 14]),
        );
        let asked = (museum.clone(), pinned.clone());
        assert!(!conversation.agent_opens(run + 1, asked.clone(), "Uganda Museum".to_owned()));
        assert!(conversation.agent_opens(run, asked, "Uganda Museum".to_owned()));
        assert!(
            conversation.opening_for_the_run(),
            "the window does not stop it"
        );
        assert_eq!(conversation.going(), Some(&museum));
        // Opened as the address the browser process resolved (881).
        assert_eq!(conversation.pinned_for_the_run(), pinned.as_ref());
        assert_eq!(conversation.take_agent_opened(), None, "not yet carded");
        conversation.open_card(museum.clone());
        assert!(!conversation.opening_for_the_run());
        assert_eq!(conversation.pinned_for_the_run(), None, "carded");
        let card = conversation.card().expect("a card");
        assert_eq!(card.said, "find the Uganda Museum's hours");
        assert!(card.agent.is_some());
        assert_eq!(
            conversation.working(),
            Some(run),
            "still at work, on the card"
        );
        assert_eq!(conversation.take_agent_opened(), Some(run));
        assert_eq!(conversation.take_agent_opened(), None, "told once");
        assert_eq!(
            conversation.start(room, Scale::ONE).turns.len(),
            1,
            "the turn asked became the card"
        );
        // Stop, from the card, stops it.
        assert_eq!(conversation.stop_agent(STOPPED), Some(run));
    }

    #[test]
    fn alo_at_works_presses_move_its_numbers_and_choices_and_are_kept_once() {
        // Queue item 878.
        use alo_renderer::agent_limits::{Limit, Stepped};
        let mut conversation = Conversation::default();
        assert!(conversation.showing().limits.is_empty(), "not drawn");
        let _ = conversation.act(Pressed::Settings);
        let shown = conversation.showing();
        assert_eq!(shown.limits, Limits::default().presses());
        assert_eq!(shown.asks, Asks::ALL.to_vec());
        let more_sites = Stepped {
            limit: Limit::Sites,
            more: true,
        };
        assert!(conversation.act(Pressed::Stepped(more_sites)));
        assert!(conversation.act(Pressed::Asks(Asks::Every)));
        assert!(!conversation.act(Pressed::Asks(Asks::Every)), "the same");
        let limits = conversation.settings().limits();
        assert_eq!((limits.sites, limits.asks), (4, Asks::Every));
        let room = Size::new(1040.0, 820.0);
        assert_eq!(conversation.start(room, Scale::ONE).limits, limits);
        assert!(conversation.take_limits_changed());
        assert!(!conversation.take_limits_changed());
        // Read back as the window opens, nothing to keep.
        let mut kept = Conversation::default()
            .limited_to(limits)
            .set_in(InterfaceFace::Geist);
        assert_eq!(
            kept.settings().limits(),
            limits,
            "a face read after keeps them"
        );
        assert!(!kept.take_limits_changed());
    }

    /// Queue item 905: Tab goes on from *alo at work* through *Search and
    /// answers* as it is drawn — its chooser, the field and *Keep* — and
    /// round to *Close*; the arrows on a chooser choose; Tab into a field
    /// puts the keyboard in it, and out of it takes it out.
    #[test]
    fn tab_goes_on_through_search_and_answers_and_the_arrows_choose_a_service() {
        use crate::settings_keys::SettingsKey;
        use alo_renderer::settings_services::{Entry, ServicePress};
        use alo_renderer::start::SettingsKeyed;
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::Settings);
        let shown = conversation.showing();
        assert_eq!(
            shown.services,
            vec![
                ServicePress::Search(0),
                ServicePress::Search(1),
                ServicePress::Field(Entry::SearchKey),
                ServicePress::Keep(Entry::SearchKey),
                ServicePress::Answers(0),
                ServicePress::Field(Entry::AiKey),
                ServicePress::Keep(Entry::AiKey),
            ]
        );
        let mut went = Vec::new();
        for _ in 0..11 {
            assert!(conversation.settings_key(SettingsKey::Tab { back: false }));
            went.extend(conversation.settings().keyed());
        }
        let service = SettingsKeyed::Service;
        assert_eq!(
            went,
            [
                SettingsKeyed::Close,
                SettingsKeyed::Font,
                SettingsKeyed::Sites,
                SettingsKeyed::Actions,
                SettingsKeyed::Asks,
                service(ServicePress::Search(0)),
                service(ServicePress::Field(Entry::SearchKey)),
                service(ServicePress::Keep(Entry::SearchKey)),
                service(ServicePress::Answers(0)),
                service(ServicePress::Field(Entry::AiKey)),
                service(ServicePress::Keep(Entry::AiKey)),
            ]
        );
        assert_eq!(conversation.services().typing(), None, "out of the field");
        assert!(conversation.settings_key(SettingsKey::Tab { back: true }));
        assert_eq!(
            conversation.services().typing(),
            Some(Entry::AiKey),
            "into it"
        );
        // Back to the chooser of search: → chooses the next, the keyboard
        // following it; the field it takes is drawn in place of the key's.
        for _ in 0..4 {
            assert!(conversation.settings_key(SettingsKey::Tab { back: true }));
        }
        assert_eq!(
            conversation.settings().keyed(),
            Some(service(ServicePress::Search(0)))
        );
        assert!(conversation.settings_key(SettingsKey::Step { on: true }));
        assert_eq!(
            conversation.settings().keyed(),
            Some(service(ServicePress::Search(1)))
        );
        assert!(
            conversation
                .showing()
                .services
                .contains(&ServicePress::Field(Entry::Address))
        );
        assert_eq!(
            conversation.services_mut().take_to_do(),
            [crate::conversation_services::ToDo::KeepServices]
        );
        // Space on *Keep* of the address presses it; closing forgets.
        assert!(conversation.settings_key(SettingsKey::Tab { back: false }));
        assert_eq!(conversation.services().typing(), Some(Entry::Address));
        assert!(conversation.services_mut().put("not an address"));
        assert!(conversation.settings_key(SettingsKey::Tab { back: false }));
        assert!(conversation.settings_key(SettingsKey::Press));
        assert!(conversation.services().shown().said.is_some());
        assert!(conversation.close_settings());
        assert_eq!(conversation.services().shown().said, None);
    }

    #[test]
    fn settings_pressed_in_the_sheet_closes_the_sheet() {
        let mut conversation = Conversation::default();
        let _ = conversation.fit(700);
        let _ = conversation.act(Pressed::ShowThePanel);
        assert!(conversation.sheet());
        assert!(conversation.act(Pressed::Settings));
        assert!(!conversation.sheet());
        assert!(conversation.settings().is_open());
    }
}
