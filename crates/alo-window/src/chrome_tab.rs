/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page in Chrome, and the tree an agent reads of it (ADR 0058 § 4, queue
//! item 620).
//!
//! A tab is a target of its own in the [`Chrome`] it was opened in, spoken to
//! through a flattened session over the same pipe. It is read as this
//! browser's [`Snapshot`] by [`crate::chrome_tree`].
//!
//! # A frozen page touches no network
//!
//! The one way a page is opened today is [`Tab::open_frozen`]: the bytes a
//! corpus case holds, served at the address they were taken from. Every
//! request the tab makes is paused before it leaves Chrome (`Fetch.enable`
//! for every address); the page's own address is answered with its bytes,
//! and **every other is refused** and named in [`Tab::refused`] — a favicon,
//! a picture, anything a script asks for. So a case read through Chrome
//! cannot reach a server, exactly as one read through the engine cannot.
//!
//! Nor can a tab the page opens: a link with `target=_blank` or a
//! `window.open` would be a tab of Chrome's whose requests this tab never
//! sees. So while a frozen page is open, Chrome holds every new tab before it
//! loads anything (`Target.setAutoAttach`, waiting for the debugger); one the
//! page opened has every request paused, is let go — a held tab can hold the
//! page that opened it, whose renderer it may share, and a `window.open`
//! then never returns — and is closed with its requests unanswered, and
//! counted ([`Tab::closed`]). Measured against Chrome 155: the tab is told
//! of before it has an address, and is gone without asking for one.
//!
//! # A page from the network goes through Chrome's network, and our gate
//!
//! [`Tab::open`] opens a page at an address, and Chrome's own network carries
//! its requests (ADR 0060 § 1) — but none leaves until the browser process has
//! seen it: every paused request is judged by [`crate::chrome_gate`] and let
//! go (`Fetch.continueRequest`) or refused by name (`Fetch.failRequest`), in
//! [`Tab::went`] or [`Tab::refusals`]. A navigation the page, or a frame in
//! it, asks for that Chrome never pauses — a link to `mailto:` — is judged as
//! it is told of, and a refusal named and recorded. [`Tab::open_for_agent`] is the same, after the agent's
//! address is judged against the sites a person named for agents: one outside
//! them is refused as [`Refused::NotGranted`] **before a single command is
//! written** (ADR 0060 § 3).
//!
//! # Every target of the tab is held
//!
//! For either kind of page, the tab's session holds every frame and worker it
//! starts until that target's requests are paused ([`crate::chrome_hold`]),
//! so a frame from another site or a service worker cannot reach a network
//! the tab does not see — on the tab's own session alone, Chrome 155 let a
//! service worker's script and a cross-site frame's `fetch` through unpaused
//! (ADR 0060's measurement 1). A frozen page's service worker is held the
//! same way, its script paused in its own target and refused. A shared
//! worker is never told of to the tab's session, only to Chrome's own, and
//! is held from there. A target that cannot be held is [`Failed::Unheld`],
//! and the tab is closed.
//!
//! A service worker can stop and start again in the target it had, with no
//! new attach (queue item 655): stopped, a verb neither waits on it nor
//! hears it, and started again it waits, as at its first start, until it is
//! held again — asked everything it was asked then — and let go
//! ([`crate::chrome_hold`] has what was measured).
//!
//! A frame's own target is asked, too, to tell of the frames made in it and
//! of each document it commits, before it is let go: what each frame's
//! document asks for is that document's in the record
//! ([`crate::chrome_record::Causes`], queue item 636).
//!
//! # A tab in a shown Chrome is its window's page
//!
//! A Chrome started shown ([`Chrome::start_shown`]) has one window of its own
//! as it starts, holding `data:,`. Its tab is that window's page rather than
//! a new one — a new tab would open in a window of Chrome's with a tab strip
//! — and is laid out at the size asked by sizing the window around it
//! ([`crate::chrome_window::fit`]) instead of being told a size to pretend
//! it has — or at the size the screen gives the window, when that is less,
//! which is used rather than refused (queue item 671). Closing the tab
//! closes the window.
//!
//! # Waiting for the page, not the blank one before it
//!
//! A tab begins at `about:blank`, whose loading may still be told of after
//! the tab is attached. So the page is loaded when the newest document its
//! own frame began since it was sent there — or the loader `Page.navigate`
//! names — reaches its `load` stage, and a `load` told of any other is
//! passed over ([`crate::chrome_loading`]).
//!
//! A page opened as the conversation's card is waited for only until its
//! navigation has **committed** ([`Tab::open_as_a_card`], queue item 751):
//! it is streamed from its first paint, and whether it has loaded is read
//! as Chrome goes on telling of it ([`Tab::is_loading`]). Every other tab is
//! waited for until it has loaded, as an agent reads a page that has.

use crate::agent_sites::AgentSites;
use crate::asking::Refused;
use crate::chrome::{Chrome, Failed};
use crate::chrome_gate::{self, Refusal};
use crate::chrome_hold::{Holding, Kind, Unheld};
use crate::chrome_loading::Loading;
use crate::chrome_owed::{self, Owed};
use crate::chrome_protocol::{
    self, AxNode, Command, Declined, Malformed, Message, Requested, Within,
};
use crate::chrome_record::{Causes, Hop, purpose_of};
use crate::chrome_said::Heard;
use crate::chrome_screencast::{Casting, Frame as CastFrame};
use crate::chrome_tree::{self, Unread};
use crate::chrome_unfocusable::{self, Unfocusable};
use crate::chrome_window::{self, Unplaced};
use alo_layout::Rect;
use alo_net::activity::Happened;
use alo_net::cause::{ActionId, DocumentId};
use alo_net::request::Purpose;
use alo_renderer::Snapshot;
use alo_renderer::navigate::LONGEST_URL;
use alo_url::Url;
use core::fmt;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// A page in Chrome.
pub struct Tab<'c> {
    chrome: &'c mut Chrome,
    target: String,
    session: String,
    url: String,
    kept: Kept,
    refused: Vec<String>,
    /// Why each request the browser process's rules refused was refused.
    refusals: Vec<Refusal>,
    /// Every request let go, in the order it was paused.
    went: Vec<Went>,
    /// The frames and workers of the tab, held.
    holding: Holding,
    /// The requests this tab answered and awaits Chrome's word on, and
    /// whether each was the page.
    answering: HashMap<u64, bool>,
    /// The targets asked to tell of their sockets and let go before
    /// answering, by the command's id: each may still refuse.
    telling: HashMap<u64, Kind>,
    /// The navigations the page's own frame asked for, told of since they
    /// were last taken ([`Tab::take_requested`]).
    requested: Vec<Requested>,
    /// The moves the page's own frame made within the document it holds,
    /// told of since they were last taken ([`Tab::take_within`]).
    within: Vec<Within>,
    /// The form controls Chrome logged it could not give the keyboard, by
    /// the `objectId` each was handed over as, told of since they were last
    /// taken ([`Tab::take_unfocusable`]).
    unfocusable: Vec<Unfocusable>,
    /// The tabs the page opened, by id, each closed before it loaded
    /// anything.
    closed: HashSet<String>,
    /// What caused what in this tab, which each line of the record names.
    causes: Causes,
    /// The lines of requests let go while their frame went to a new
    /// document, each waiting for Chrome to say which document asked.
    owed: Owed,
    /// The agent's ask that opened the tab, when an agent asked.
    opened_by: Option<ActionId>,
    /// The last verb accepted on the tab.
    acted: Option<ActionId>,
    /// What the page's script has said while the verb running is heard
    /// ([`Tab::hear`]), or [`None`] while none is.
    heard: Option<Heard>,
    /// The frames and workers in targets of their own told to tell of what
    /// their script says while the verb running is heard, by session.
    hearing: Vec<String>,
    /// The address of every WebSocket the page opened, as Chrome spells it.
    sockets: Vec<String>,
    /// The address of the page the tab shows: where it was opened, until its
    /// own frame commits a document, and then that document's, and the
    /// address it last moved to within it — or [`None`], for an address
    /// that is not one this browser reads.
    address: Option<Url>,
    /// Whether Chrome has detached the tab's own session: its page was
    /// closed, by a person closing its window or by this process, and
    /// nothing sent through the session is answered after (item 672).
    detached: bool,
    /// The page streamed as frames, for the conversation's card (ADR 0076
    /// B, queue item 717): every frame answered as it is read, the newest
    /// kept.
    casting: Casting,
    /// What Chrome has told of the tab's own frame's documents: whether the
    /// page has committed and loaded (queue item 751).
    loading: Loading,
}

/// How long opening a tab waits for its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Until {
    /// Until the newest document of its own frame has loaded.
    Loaded,
    /// Until its navigation has committed: the conversation's card, drawn
    /// from the page's first paint (queue item 751).
    Committed,
}

/// The most moves within its document a page's frame is remembered to have
/// made between two presses: a script can call `history.pushState` a
/// million times in one task, and a press asks only after the first.
const MOST_WITHIN: usize = 16;

/// What a tab's requests are answered from.
enum Kept {
    /// A frozen page's bytes: its own address is answered with them, and
    /// every other refused.
    Frozen(Vec<u8>),
    /// Chrome's own network, behind [`crate::chrome_gate`].
    Network,
}

/// A request the browser process let Chrome send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Went {
    /// Where to, as Chrome spells it.
    pub url: String,
    /// The target it was paused in.
    pub by: Kind,
}

/// Why a page was not opened.
#[derive(Debug, Clone, PartialEq)]
pub enum Unopened {
    /// Asking Chrome failed.
    Failed(Failed),
    /// Chrome did not go to the page, and said why.
    NotNavigated {
        /// Chrome's own words, as `net::ERR_ABORTED`.
        reason: String,
    },
    /// The address is not on the network, which is all [`Tab::open`] opens.
    NotTheNetwork {
        /// Its scheme.
        scheme: String,
    },
    /// The agent that named it may not reach it, or what it named is not an
    /// address; nothing was written to Chrome.
    Refused(Refused),
    /// A shown Chrome's window could not be sized to hold a page at all
    /// ([`crate::chrome_window::fit`]): a size of nothing was asked, or the
    /// window was given room for none. Room for less than was asked is not
    /// this; the page is opened at it.
    Unfitted(Box<Unplaced>),
}

impl fmt::Display for Unopened {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(why) => why.fmt(f),
            Self::NotNavigated { reason } => write!(f, "Chrome did not open the page: {reason}"),
            Self::NotTheNetwork { scheme } => write!(
                f,
                "a {scheme}: address is not a page from the network, which is all this opens"
            ),
            Self::Refused(refused) => refused.fmt(f),
            Self::Unfitted(why) => why.fmt(f),
        }
    }
}

impl From<Failed> for Unopened {
    fn from(why: Failed) -> Self {
        Self::Failed(why)
    }
}

impl<'c> Tab<'c> {
    /// Open `page`, the bytes of a frozen page, at `url`, laid out `width` by
    /// `height` CSS pixels, and wait for it to load.
    ///
    /// # Errors
    ///
    /// [`Unopened`], when Chrome does not open a tab, does not go to the
    /// page, or does not finish loading it within its patience.
    pub fn open_frozen(
        chrome: &'c mut Chrome,
        url: &str,
        page: Vec<u8>,
        size: (u32, u32),
    ) -> Result<Self, Unopened> {
        Self::begin(chrome, url, (Kept::Frozen(page), Until::Loaded), None, size)
    }

    /// Open the page at `address`, an `http` or `https` one, through
    /// Chrome's own network, laid out `width` by `height` CSS pixels, and
    /// wait for it to load: every request it makes judged before Chrome is
    /// let send it.
    ///
    /// # Errors
    ///
    /// [`Unopened::NotTheNetwork`] for any other address, before anything is
    /// written to Chrome; or an [`Unopened`] as [`Tab::open_frozen`] says.
    pub fn open(chrome: &'c mut Chrome, address: &Url, size: (u32, u32)) -> Result<Self, Unopened> {
        on_the_network(address)?;
        Self::begin(
            chrome,
            &address.to_string(),
            (Kept::Network, Until::Loaded),
            None,
            size,
        )
    }

    /// Open the page at `address` as [`Tab::open`] does, for the
    /// conversation's card (ADR 0076 B, queue item 751): waited for only
    /// until its navigation has committed, so that it is streamed from its
    /// first paint rather than its `load` — a page whose `load` is held, or
    /// never comes, still draws in the card. Whether it has loaded since is
    /// [`Tab::is_loading`].
    ///
    /// # Errors
    ///
    /// As [`Tab::open`]: a navigation that does not commit within Chrome's
    /// patience — a server that never answers — is not opened.
    pub fn open_as_a_card(
        chrome: &'c mut Chrome,
        address: &Url,
        size: (u32, u32),
    ) -> Result<Self, Unopened> {
        on_the_network(address)?;
        Self::begin(
            chrome,
            &address.to_string(),
            (Kept::Network, Until::Committed),
            None,
            size,
        )
    }

    /// Open the page at `address`, which an agent named, as [`Tab::open`]
    /// does — when an agent kept to `sites` may reach it.
    ///
    /// The ask is an agent action (ADR 0063), minted once it is accepted —
    /// the address parsed, on the network and within the agent's sites —
    /// and before Chrome is told anything: the tab's first page is caused by
    /// it, done in the tab, and [`Tab::opened_by`] names it. An ask refused
    /// before then mints nothing.
    ///
    /// # Errors
    ///
    /// [`Refused::NotAnAddress`] or [`Refused::NotGranted`], by its site,
    /// in [`Unopened::Refused`], before anything is written to Chrome; or an
    /// [`Unopened`] as [`Tab::open`] says.
    pub fn open_for_agent(
        chrome: &'c mut Chrome,
        address: &str,
        sites: &AgentSites,
        size: (u32, u32),
    ) -> Result<Self, Unopened> {
        let Ok(url) = alo_url::parse(address) else {
            return Err(Unopened::Refused(Refused::NotAnAddress {
                tab: address.to_owned(),
            }));
        };
        if let Err(site) = sites.reach(&url) {
            return Err(Unopened::Refused(Refused::NotGranted {
                tab: address.to_owned(),
                site,
            }));
        }
        on_the_network(&url)?;
        let action = chrome.record_mut().an_action();
        Self::begin(
            chrome,
            &url.to_string(),
            (Kept::Network, Until::Loaded),
            Some(action),
            size,
        )
    }

    /// A tab at `url`, answered from `kept`, waited for `until` it has
    /// loaded or committed: opened as `opening`, an agent's ask, or by the
    /// person when [`None`].
    fn begin(
        chrome: &'c mut Chrome,
        url: &str,
        (kept, until): (Kept, Until),
        opening: Option<ActionId>,
        (width, height): (u32, u32),
    ) -> Result<Self, Unopened> {
        let target = if chrome.is_shown() {
            chrome_window::page_of_the_window(chrome)?
        } else {
            let blank = Command::CreateTarget {
                url: "about:blank".to_owned(),
            };
            chrome_protocol::target(&chrome.ask(None, &blank)?).map_err(Failed::Malformed)?
        };
        let attach = Command::AttachToTarget {
            target: target.clone(),
        };
        let session = match chrome
            .ask(None, &attach)
            .and_then(|result| chrome_protocol::session(&result).map_err(Failed::Malformed))
        {
            Ok(session) => session,
            Err(why) => {
                let _ = chrome.send(None, &Command::CloseTarget { target });
                return Err(why.into());
            }
        };
        let tab = chrome.record_mut().a_tab();
        let causes = match opening {
            Some(action) => Causes::opened_by(action, tab, &target),
            None => Causes::of(tab, &target),
        };
        // From here the tab is closed when this is dropped, whatever fails.
        let mut tab = Self {
            chrome,
            target,
            session,
            url: url.to_owned(),
            kept,
            refused: Vec::new(),
            refusals: Vec::new(),
            went: Vec::new(),
            holding: Holding::default(),
            answering: HashMap::new(),
            telling: HashMap::new(),
            requested: Vec::new(),
            within: Vec::new(),
            unfocusable: Vec::new(),
            closed: HashSet::new(),
            causes,
            owed: Owed::default(),
            opened_by: opening,
            acted: None,
            heard: None,
            hearing: Vec::new(),
            sockets: Vec::new(),
            address: alo_url::parse(url).ok(),
            detached: false,
            casting: Casting::default(),
            loading: Loading::default(),
        };
        // Only now, so that the tab itself is not held.
        tab.chrome.ask(None, &Command::HoldNewTabs { hold: true })?;
        for command in [
            Command::PauseEveryRequest,
            Command::HoldNewTabs { hold: true },
            Command::TellOfLoading,
            Command::TellOfStages,
        ] {
            tab.ask(&command)?;
        }
        if tab.is_shown() {
            chrome_window::fit(&mut tab, width, height)
                .map_err(|why| Unopened::Unfitted(Box::new(why)))?;
        } else {
            tab.ask(&Command::LayOutAt { width, height })?;
        }
        if tab.is_from_the_network() {
            tab.ask(&Command::TellOfSockets)?;
        }
        tab.go(until)?;
        Ok(tab)
    }

    /// Every address the page asked for and was refused, in the order it
    /// asked, as Chrome spells them.
    pub fn refused(&self) -> &[String] {
        &self.refused
    }

    /// Why each request, or each navigation asked for, of a page from the
    /// network was refused, in the order it was asked for. A frozen page's
    /// are refused for being not the page, and are only in [`Tab::refused`].
    pub fn refusals(&self) -> &[Refusal] {
        &self.refusals
    }

    /// Every request of a page from the network that was let go, in the
    /// order Chrome paused it, and which target it was paused in.
    pub fn went(&self) -> &[Went] {
        &self.went
    }

    /// The address of every WebSocket a page from the network opened, in
    /// the order Chrome told of it, as Chrome spells it: each opened
    /// without being decided (ADR 0060 § 3), and in the record as such.
    pub fn sockets(&self) -> &[String] {
        &self.sockets
    }

    /// The action an agent's ask to open this tab was minted as, when it
    /// was opened at an agent's word ([`Tab::open_for_agent`]): what the
    /// record names as the cause of its first page (ADR 0063). [`None`] for
    /// a tab the person opened.
    pub fn opened_by(&self) -> Option<ActionId> {
        self.opened_by
    }

    /// The action the last verb accepted on this tab was minted as: what
    /// the record names as the cause of what it did.
    pub fn acted(&self) -> Option<ActionId> {
        self.acted
    }

    /// A verb has been accepted on this tab, as the action this answers:
    /// until [`Tab::done_acting`], what the page asks for is the agent's
    /// (ADR 0012 § 4).
    pub(crate) fn acting(&mut self) -> ActionId {
        let action = self.chrome.record_mut().an_action();
        self.acted = Some(action);
        self.causes.acting(Some(action));
        action
    }

    /// The verb has been carried out, or refused.
    pub(crate) fn done_acting(&mut self) {
        self.causes.acting(None);
    }

    /// Whether this is a page from the network, whose requests are judged
    /// and recorded rather than answered from a frozen page's bytes.
    fn is_from_the_network(&self) -> bool {
        matches!(self.kept, Kept::Network)
    }

    /// The address of the page the tab shows, as an agent names the tab
    /// (ADR 0055 § 5): where it was opened until its own frame commits a
    /// document, and that document's from then — so a page that went
    /// somewhere else is named only by where it is now (queue item 497).
    /// A move within the document — a link to a part of it, a script's
    /// `history.pushState` — names it by where it moved, fragment and all,
    /// though no document was committed (queue item 640). [`None`] for an
    /// address this browser cannot read as one, which cannot be named at
    /// all.
    pub fn address(&self) -> Option<&Url> {
        self.address.as_ref()
    }

    /// The document the tab's own frame holds, once one has committed, as
    /// [`Chrome::record`] names it.
    pub fn document(&self) -> Option<DocumentId> {
        self.causes.page()
    }

    /// How many tabs the page opened — a link with `target=_blank`, a
    /// `window.open` — each closed before it loaded anything.
    pub fn closed(&self) -> usize {
        self.closed.len()
    }

    /// The page, read as the tree an agent reads.
    ///
    /// # Errors
    ///
    /// [`Unread`], when Chrome does not answer, or its tree is not a tree.
    pub fn read(&mut self) -> Result<Snapshot, Unread> {
        self.read_whole().map(|(snapshot, _)| snapshot)
    }

    /// The page read as [`Tab::read`] reads it, and Chrome's tree it was
    /// read from.
    pub(crate) fn read_whole(&mut self) -> Result<(Snapshot, Vec<AxNode>), Unread> {
        let nodes =
            chrome_protocol::ax_nodes(&self.ask(&Command::ReadTree)?).map_err(Failed::Malformed)?;
        let window =
            chrome_protocol::window(&self.ask(&Command::Window)?).map_err(Failed::Malformed)?;
        let snapshot = chrome_tree::snapshot(&nodes, window, &mut |node| self.drawn(node))?;
        Ok((snapshot, nodes))
    }

    /// The navigations the page's own frame asked for since this was last
    /// asked, in the order Chrome told of them.
    pub(crate) fn take_requested(&mut self) -> Vec<Requested> {
        std::mem::take(&mut self.requested)
    }

    /// The moves the page's own frame made within its document since this
    /// was last asked, in the order Chrome told of them.
    pub(crate) fn take_within(&mut self) -> Vec<Within> {
        std::mem::take(&mut self.within)
    }

    /// The form controls Chrome logged, of the page's own document, that it
    /// could not give the keyboard to since this was last asked, in the
    /// order Chrome told of them ([`crate::chrome_unfocusable`]). Chrome
    /// logs only while told to ([`Command::TellOfTheLog`]).
    pub(crate) fn take_unfocusable(&mut self) -> Vec<Unfocusable> {
        std::mem::take(&mut self.unfocusable)
    }

    /// Where Chrome drew the DOM node `node`: nowhere, when it declines to
    /// say, which it does for a node with no box.
    pub(crate) fn drawn(&mut self, node: u64) -> Result<Vec<Rect>, Failed> {
        let quads = match self.ask(&Command::WhereDrawn { node }) {
            Ok(result) => chrome_protocol::quads(&result).map_err(Failed::Malformed)?,
            Err(Failed::Declined(_)) => return Ok(Vec::new()),
            Err(why) => return Err(why),
        };
        quads
            .iter()
            .map(|quad| {
                chrome_tree::rect_of(quad)
                    .ok_or(Failed::Malformed(Malformed::BadField { field: "quads" }))
            })
            .collect()
    }

    /// Whether the tab is a shown Chrome's window's page.
    pub(crate) fn is_shown(&self) -> bool {
        self.chrome.is_shown()
    }

    /// The process of the Chrome this tab is in: the owner the window
    /// server names its window by (item 672).
    pub(crate) fn chrome_process(&self) -> u32 {
        self.chrome.process()
    }

    /// The tab, by its target id in Chrome.
    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    /// How long Chrome is waited for.
    pub(crate) fn patience(&self) -> std::time::Duration {
        self.chrome.patience()
    }

    /// Send `command` to Chrome itself, rather than through this tab's
    /// session, and wait for its result — answering every request paused
    /// meanwhile, as [`Tab::ask`] does.
    pub(crate) fn ask_chrome(&mut self, command: &Command) -> Result<Vec<u8>, Failed> {
        let asked = self.chrome.send(None, command)?;
        self.answer_to(asked, false)
    }

    /// Go to the page, and wait `until` the document its own frame holds —
    /// the loader the navigation began, or the newest its frame began after
    /// it ([`crate::chrome_loading`]) — has loaded, or the navigation has
    /// committed.
    ///
    /// A page may replace itself before it has loaded — a script's
    /// `location.replace`, as `www.reddit.com`'s challenge does — and the
    /// first loader then never loads. Queue item 749 found the card waiting
    /// out Chrome's whole patience for it, and closing.
    fn go(&mut self, until: Until) -> Result<(), Unopened> {
        self.loading.navigating();
        let asked = self.chrome.send(
            Some(&self.session),
            &Command::Navigate {
                url: self.url.clone(),
            },
        )?;
        let by = Instant::now() + self.chrome.patience();
        loop {
            let done = match until {
                Until::Loaded => self.loading.loaded(),
                Until::Committed => self.loading.committed(),
            };
            if done {
                return Ok(());
            }
            match self.next(by)? {
                Some(Message::Skipped { id: Some(id), .. }) if id == asked => {
                    return Err(Failed::TooLong.into());
                }
                Some(Message::Reply { id, result, .. }) if id == asked => {
                    let result = result.map_err(Failed::Declined)?;
                    match chrome_protocol::navigated(&result).map_err(Failed::Malformed)? {
                        Ok(loader) => self.loading.navigated(loader),
                        Err(reason) => return Err(Unopened::NotNavigated { reason }),
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether the page is still loading: the newest document its own frame
    /// began has not been told to have loaded (queue item 751) — as Chrome
    /// last said, which is read whenever the tab is ([`Tab::catch_up`]).
    pub fn is_loading(&self) -> bool {
        !self.loading.loaded()
    }

    /// Take every message Chrome has sent already — answering each request
    /// paused and each target told of, as when a command is waited for —
    /// and wait for nothing more: so that the tab's address and document
    /// are what Chrome last said they are, though nothing has been asked of
    /// it since. A page that goes elsewhere on its own — a script, a refresh
    /// — is told of whenever it does, and until this is asked it is read
    /// nowhere.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome has gone or what it sent cannot be read.
    pub(crate) fn catch_up(&mut self) -> Result<(), Failed> {
        loop {
            match self.next(Instant::now()) {
                Ok(_) => {}
                Err(Failed::Silent { .. }) => return Ok(()),
                Err(why) => return Err(why),
            }
        }
    }

    /// Send `command` through this tab's session and wait for its result,
    /// answering every request paused meanwhile.
    ///
    /// A session Chrome has detached answers nothing: measured under load
    /// against Chrome 155, a command sent through it as its page closed
    /// was never answered, where without load it was declined. So once
    /// Chrome has said the session is detached — before this is sent, or
    /// while its answer is waited for — this is [`Failed::Detached`] at
    /// once rather than after Chrome's whole patience.
    ///
    /// # Errors
    ///
    /// [`Failed::Detached`], or another [`Failed`].
    pub(crate) fn ask(&mut self, command: &Command) -> Result<Vec<u8>, Failed> {
        if self.detached {
            return Err(Failed::Detached);
        }
        let asked = self.chrome.send(Some(&self.session), command)?;
        self.answer_to(asked, true)
    }

    /// Wait for the result of the command sent as `asked`, answering every
    /// request paused meanwhile — or, for a command sent `through_session`,
    /// until Chrome says the tab's session is detached.
    fn answer_to(&mut self, asked: u64, through_session: bool) -> Result<Vec<u8>, Failed> {
        let until = Instant::now() + self.chrome.patience();
        loop {
            match self.next(until)? {
                Some(Message::Reply { id, result, .. }) if id == asked => {
                    return result.map_err(Failed::Declined);
                }
                Some(Message::Skipped { id: Some(id), .. }) if id == asked => {
                    return Err(Failed::TooLong);
                }
                _ if through_session && self.detached => return Err(Failed::Detached),
                _ => {}
            }
        }
    }

    /// Stream the page as PNG frames, each at most `width` by `height`
    /// device pixels (ADR 0076 B, queue item 717): every frame told of from
    /// now on is answered as it is read, here or while any command waits,
    /// and the newest kept for [`Tab::take_frame`].
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome declines or does not answer.
    pub fn cast(&mut self, width: u32, height: u32) -> Result<(), Failed> {
        self.ask(&Command::Cast { width, height })?;
        self.casting.start();
        Ok(())
    }

    /// Stream no more, once Chrome has answered that it stopped: a frame it
    /// had in flight is still answered and counted, and never kept. Nothing
    /// is asked of a tab not streaming.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome declines or does not answer.
    pub fn uncast(&mut self) -> Result<(), Failed> {
        if !self.casting.is_on() {
            return Ok(());
        }
        let stopped = self.ask(&Command::StopCasting).map(drop);
        // Whatever Chrome answered, nothing more is kept.
        self.casting.stop();
        stopped
    }

    /// The newest frame read since this was last asked, if the page is
    /// streamed and one was — after taking what Chrome has sent already
    /// ([`Tab::catch_up`]).
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome has gone or what it sent cannot be read.
    pub fn take_frame(&mut self) -> Result<Option<CastFrame>, Failed> {
        self.catch_up()?;
        // A frame that is not base64 of a PNG was answered, and is not shown.
        Ok(self.casting.take().ok().flatten())
    }

    /// The stream, as the tab keeps it: whether it is on, and what was read.
    pub fn casting(&self) -> &Casting {
        &self.casting
    }

    /// Whether `message` is a frame streamed of the page — answered, and
    /// kept if it is the newest — or the reply to such an answer.
    fn cast_in(&mut self, message: &Message) -> Result<bool, Failed> {
        match message {
            Message::Event {
                method,
                session: Some(session),
                params,
            } if method == "Page.screencastFrame" && *session == self.session => {
                let frame = chrome_protocol::cast_frame(params).map_err(Failed::Malformed)?;
                let id = self.chrome.send(
                    Some(&self.session),
                    &Command::FrameSeen { frame: frame.frame },
                )?;
                self.casting.answering(id);
                self.casting.read(frame.data);
                Ok(true)
            }
            Message::Reply { id, .. } => Ok(self.casting.answered(*id)),
            _ => Ok(false),
        }
    }

    /// Whether Chrome has detached the tab's own session, its page closed.
    pub(crate) fn is_detached(&self) -> bool {
        self.detached
    }

    /// A session detached, as `Target.detachedFromTarget` tells of it —
    /// told through Chrome's own session when `told_to_chrome`: a held
    /// target's, forgotten, or the tab's own, which answers nothing after.
    fn detached_from(&mut self, told_to_chrome: bool, params: &[u8]) -> Result<(), Failed> {
        let gone = chrome_protocol::detached(params).map_err(Failed::Malformed)?;
        if told_to_chrome && gone == self.session {
            self.detached = true;
        }
        self.holding.forget(&gone);
        self.hearing.retain(|heard| *heard != gone);
        Ok(())
    }

    /// From now until [`Tab::heard`], hear what the page's script says, each
    /// line about `about` — and nothing it said before.
    ///
    /// Chrome is told of nothing a page's script says between verbs:
    /// `Runtime` is on only while one runs. Every value a page gives its
    /// console is described to a client that has it on, and a page decides
    /// how long that is, so a page that logs between verbs costs Chrome
    /// nothing of ours.
    ///
    /// The page's script is more than its own document's: a frame from
    /// another site and a dedicated worker each say what they say through a
    /// target of their own (item 652), and so do a shared worker and a
    /// service worker (item 654), and each is told to forget and then to
    /// tell, as the tab's own session is. Measured against Chrome 155: each
    /// answers, and each told to tell without forgetting first tells again
    /// of everything it said since it started. One started while the verb
    /// runs is told to tell as it is held ([`Tab::attached`]).
    ///
    /// A shared or service worker goes on while the verb holds the page's
    /// own thread, and what each throws is told of then — unlike a
    /// dedicated worker's, which Chrome hands the page's thread first. A
    /// service worker that has stopped answers nothing until it starts
    /// again, so it is neither told nor waited on (item 655); Chrome 155
    /// did not stop one held by this tab, idle for forty seconds, past the
    /// thirty after which it stops one nobody holds.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome cannot be asked.
    pub(crate) fn hear(&mut self, about: &'static str) -> Result<(), Failed> {
        let heard = self.holding.heard();
        self.ask(&Command::ForgetWhatWasSaid)?;
        for session in &heard {
            self.ask_held(session, &Command::ForgetWhatWasSaid)?;
        }
        self.heard = Some(Heard::about(about));
        self.ask(&Command::TellOfScript)?;
        for session in heard {
            // Heard from before it is asked: what it said since it forgot
            // is told of before its answer.
            self.hearing.push(session.clone());
            if !self.ask_held(&session, &Command::TellOfScript)? {
                self.hearing.retain(|other| *other != session);
            }
        }
        Ok(())
    }

    /// Send `command` to the target held as `session` and wait for its
    /// result: `true` once it is answered, or `false` when the target ended
    /// or stopped before it was — a frame taken out of its page, a worker
    /// that closed, a service worker stopped — whether Chrome refused it for
    /// that or never answered.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome cannot be asked, or declines a target still
    /// held.
    fn ask_held(&mut self, session: &str, command: &Command) -> Result<bool, Failed> {
        let asked = self.chrome.send(Some(session), command)?;
        let until = Instant::now() + self.chrome.patience();
        loop {
            let message = self.next(until)?;
            let gone = self.holding.kind(session).is_none() || self.holding.is_stopped(session);
            match message {
                Some(Message::Reply { id, result, .. }) if id == asked => {
                    return match result {
                        Ok(_) => Ok(true),
                        Err(_) if gone => Ok(false),
                        Err(declined) => Err(Failed::Declined(declined)),
                    };
                }
                Some(Message::Skipped { id: Some(id), .. }) if id == asked => {
                    return Err(Failed::TooLong);
                }
                // A target that ended may never answer what it was sent.
                _ if gone => return Ok(false),
                _ => {}
            }
        }
    }

    /// What the page's script said since [`Tab::hear`], in the engine's
    /// words ([`crate::chrome_said`]) — nothing, if it was not asked.
    ///
    /// Answered once Chrome has said `Runtime` is off, which the page's own
    /// renderer answers after everything it told of before: so a thing the
    /// page said in the verb's task has been heard (measured against Chrome
    /// 155). A promise rejected and not handled is told of a task later, and
    /// is heard only if that task ran before.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome cannot be asked; what was heard is dropped.
    pub(crate) fn heard(&mut self) -> Result<Vec<String>, Failed> {
        if self.heard.is_none() {
            return Ok(Vec::new());
        }
        // Each frame and worker heard is a barrier of its own: its renderer
        // answers after everything it told of before, whatever the page's
        // did. Every one is told to stop, whichever failed first.
        let mut off = self.ask(&Command::StopTellingOfScript).map(drop);
        for session in core::mem::take(&mut self.hearing) {
            let stopped = self.ask_held(&session, &Command::StopTellingOfScript);
            off = off.and(stopped.map(drop));
        }
        let heard = self.heard.take();
        off?;
        Ok(heard.map(Heard::lines).unwrap_or_default())
    }

    /// Something the page's script said, told of as `method` in `params`,
    /// heard if a verb is running.
    fn said(&mut self, method: &str, params: &[u8]) {
        let Some(heard) = &mut self.heard else {
            return;
        };
        let read = if method == "Runtime.consoleAPICalled" {
            chrome_protocol::console_called(params)
        } else {
            chrome_protocol::thrown(params)
        };
        // The page chose these bytes: what cannot be read is said to be
        // so, and the verb goes on.
        match read {
            Ok(said) => heard.hear(&said),
            Err(why) => heard.unread(&why),
        }
    }

    /// The next message for this tab that is not about a paused request or a
    /// held target — those are answered here — or [`None`] for one that was.
    fn next(&mut self, until: Instant) -> Result<Option<Message>, Failed> {
        let message = self.chrome.receive(until)?;
        // A frame streamed, and a stage of the page's documents.
        if self.cast_in(&message)? || self.staged_in(&message)? {
            return Ok(None);
        }
        let held = self.held_by(&message);
        // What a page's script said, what Chrome logged of it, and a request
        // a renderer of the tab told of — what a line owed waits for.
        if self.heard_in(&message) || self.logged_in(&message)? || self.told_in(&message, held)? {
            return Ok(None);
        }
        match message {
            Message::Event {
                method,
                session: Some(session),
                params,
            } if method == "Fetch.requestPaused" && held.is_some() => {
                let paused = chrome_protocol::paused(&params).map_err(Failed::Malformed)?;
                let (answer, page) = self.answer(paused, held.unwrap_or(Kind::Page));
                let id = self.chrome.send(Some(&session), &answer)?;
                self.answering.insert(id, page);
                Ok(None)
            }
            // A navigation the page asked for. Its main frame has the tab's
            // own id; a frame inside it asking is not the page going
            // anywhere.
            Message::Event {
                method,
                session: Some(session),
                params,
            } if is_a_frames(held) && method == "Page.frameRequestedNavigation" => {
                self.asked_to_go(&session, &params)?;
                Ok(None)
            }
            // A frame of the tab holds a new document, from now: the tab's
            // own frame, told of through the tab's session, or a frame
            // inside it, through that or the frame's own target.
            Message::Event {
                method,
                session: Some(session),
                params,
            } if is_a_frames(held) && method == "Page.frameNavigated" => {
                self.committed(&session, &params)?;
                Ok(None)
            }
            // A frame made inside one of the tab's, or taken out of its page.
            Message::Event { method, params, .. }
                if is_a_frames(held)
                    && matches!(method.as_str(), "Page.frameAttached" | "Page.frameDetached") =>
            {
                self.made_or_gone(&method, &params)?;
                Ok(None)
            }
            // The tab's own frame moved within the document it holds: no
            // request, no commit, and an address of its own from now.
            Message::Event {
                method,
                session: Some(session),
                params,
            } if session == self.session && method == "Page.navigatedWithinDocument" => {
                self.moved_within(&params)?;
                Ok(None)
            }
            // A WebSocket, told of before its handshake and held by nothing.
            Message::Event { method, params, .. }
                if method == "Network.webSocketCreated" && held.is_some() =>
            {
                let url = chrome_protocol::socket(&params).map_err(Failed::Malformed)?;
                if self.is_from_the_network() {
                    self.write_socket(&url);
                }
                self.sockets.push(url);
                Ok(None)
            }
            Message::Event {
                method,
                session,
                params,
            } if method == "Target.attachedToTarget" && (session.is_none() || held.is_some()) => {
                let attached = chrome_protocol::attached(&params).map_err(Failed::Malformed)?;
                self.attached(attached, session.is_none())?;
                Ok(None)
            }
            // A service worker held has stopped, or started again, in the
            // target it had.
            Message::Event {
                method,
                session: Some(session),
                ..
            } if held == Some(Kind::ServiceWorker)
                && matches!(
                    method.as_str(),
                    "Inspector.targetCrashed" | "Inspector.targetReloadedAfterCrash"
                ) =>
            {
                self.stopped_or_started(&method, &session)?;
                Ok(None)
            }
            // A shared worker's end is told of to Chrome's own session.
            Message::Event {
                method,
                session,
                params,
            } if method == "Target.detachedFromTarget" && (session.is_none() || held.is_some()) => {
                self.detached_from(session.is_none(), &params)?;
                Ok(None)
            }
            Message::Reply { id, result, .. } if self.awaits(id) => {
                self.answered(id, result)?;
                Ok(None)
            }
            // Told of a target not this tab's, or no longer held. A reply is
            // handed on whatever its session, because a command sent to a
            // target that has since ended is answered with a refusal its
            // asker waits for ([`Tab::ask_held`]).
            Message::Event {
                session: Some(_), ..
            } if held.is_none() => Ok(None),
            other => Ok(Some(other)),
        }
    }

    /// Whether `message` is a stage of a document of the tab's, told through
    /// its own session — kept, for whether its page has committed and
    /// loaded ([`crate::chrome_loading`]).
    ///
    /// # Errors
    ///
    /// [`Failed::Malformed`], for a stage that is not one.
    fn staged_in(&mut self, message: &Message) -> Result<bool, Failed> {
        match message {
            Message::Event {
                method,
                session: Some(session),
                params,
            } if *session == self.session && method == "Page.lifecycleEvent" => {
                let stage = chrome_protocol::stage(params).map_err(Failed::Malformed)?;
                self.loading.told(&self.target, stage);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Which of the tab's targets `message` came through: the tab's own
    /// session, or one of a target of it held — or [`None`].
    fn held_by(&self, message: &Message) -> Option<Kind> {
        match message {
            Message::Event {
                session: Some(session),
                ..
            }
            | Message::Reply {
                session: Some(session),
                ..
            } => {
                if *session == self.session {
                    Some(Kind::Page)
                } else {
                    self.holding.kind(session)
                }
            }
            _ => None,
        }
    }

    /// Whether `message` is something the page's script said — heard, if a
    /// verb is — or a message too long to read that was.
    ///
    /// What script says is heard from the tab's own session and from each
    /// frame's and worker's told to tell of it while a verb runs;
    /// a `Runtime` event skipped for its length, whose session is not known,
    /// is the page's.
    fn heard_in(&mut self, message: &Message) -> bool {
        match message {
            Message::Event {
                method,
                session: Some(session),
                params,
            } if (*session == self.session || self.hearing.contains(session))
                && matches!(
                    method.as_str(),
                    "Runtime.consoleAPICalled" | "Runtime.exceptionThrown"
                ) =>
            {
                self.said(method, params);
                true
            }
            Message::Skipped {
                method: Some(method),
                ..
            } if method.starts_with("Runtime.") => {
                if let Some(heard) = &mut self.heard {
                    heard.too_long();
                }
                true
            }
            _ => false,
        }
    }

    /// Whether `message` is something Chrome logged of the page's own
    /// document — kept, when it is a form control Chrome could not give the
    /// keyboard ([`crate::chrome_unfocusable`]).
    ///
    /// # Errors
    ///
    /// [`Failed::Malformed`], for an entry that is not one.
    fn logged_in(&mut self, message: &Message) -> Result<bool, Failed> {
        let Message::Event {
            method,
            session: Some(session),
            params,
        } = message
        else {
            return Ok(false);
        };
        if *session != self.session || method != "Log.entryAdded" {
            return Ok(false);
        }
        let logged = chrome_protocol::logged(params).map_err(Failed::Malformed)?;
        if let Some(control) = chrome_unfocusable::control(logged) {
            self.unfocusable.push(control);
        }
        Ok(true)
    }

    /// Whether `id` is a command this tab sent on its own account, whose
    /// answer it waits for rather than its caller.
    fn awaits(&self, id: u64) -> bool {
        self.owed.awaits(id)
            || self.holding.awaits(id)
            || self.telling.contains_key(&id)
            || self.answering.contains_key(&id)
    }

    /// Chrome's answer to `id`, a command [`Tab::awaits`].
    fn answered(&mut self, id: u64, result: Result<Vec<u8>, Declined>) -> Result<(), Failed> {
        // A frame's renderer, answering after every request it told of: a
        // line owed still was never told of. A refusal says the same — the
        // target has gone, and with it whatever would have told.
        if self.owed.awaits(id) {
            let settled = self.owed.answered(id);
            self.settle(settled);
            return Ok(());
        }
        // Chrome's word on holding a target: once it has answered all the
        // target was asked, the target is let go.
        if self.holding.awaits(id) {
            if let Some(ready) = self
                .holding
                .answered(id, result.map(|_| ()))
                .map_err(|why| Failed::Unheld(Box::new(why)))?
            {
                self.chrome.send(Some(&ready), &Command::LetGo)?;
            }
            return Ok(());
        }
        // A target let go before it answered whether it would tell of its
        // WebSockets: one that will not is one whose sockets would open
        // unrecorded.
        if let Some(kind) = self.telling.remove(&id) {
            return result.map(|_| ()).map_err(|declined| {
                Failed::Unheld(Box::new(Unheld {
                    kind: kind.chrome_word().to_owned(),
                    declined: Some(declined),
                }))
            });
        }
        // Chrome's word on a request this tab answered. The page not taken is
        // a page not shown; a refusal not taken is a request that had already
        // gone, which is what a refusal wanted; a request let go and not
        // taken was one Chrome had already given up.
        if self.answering.remove(&id) == Some(true) {
            result.map_err(Failed::Declined)?;
        }
        Ok(())
    }

    /// A navigation a frame of the page asked for, told of in `params`
    /// through `session`. Its main frame has the tab's own id; a frame inside
    /// it asking is not the page going anywhere, and is the agent's while a
    /// verb is carried out in the frame's document.
    ///
    /// Either is judged as it is told of when the page is from the network,
    /// because Chrome never pauses a scheme it hands elsewhere, and a refusal
    /// is written with the asking frame's document as its cause. Measured
    /// against Chrome 155: a same-site frame sending itself elsewhere is told
    /// of through the tab's session, a frame from another site through its
    /// own target, and a page sending its other-site frame elsewhere through
    /// the tab's — each once, naming the frame that is to go, never who
    /// asked — and Chrome sent none of them to `mailto:`.
    fn asked_to_go(&mut self, session: &str, params: &[u8]) -> Result<(), Failed> {
        let requested = chrome_protocol::requested(params).map_err(Failed::Malformed)?;
        let own = requested.frame == self.target;
        if own && session != self.session {
            if requested.disposition == "currentTab" {
                self.causes.asked_to_go(&requested.frame, &requested.url);
            }
            return Ok(());
        }
        let judged = self.is_from_the_network();
        match judged.then(|| chrome_gate::judge(&requested.url)) {
            Some(Err(refusal)) => {
                let method = if requested.reason == "formSubmissionPost" {
                    "POST"
                } else {
                    "GET"
                };
                let cause = self.causes.of_frame(&requested.frame);
                self.write_refused(&requested.url, method, Purpose::Document, cause, &refusal);
                self.refused.push(requested.url.clone());
                self.refusals.push(refusal);
            }
            _ if requested.disposition == "currentTab" && (judged || !own) => {
                self.causes.asked_to_go(&requested.frame, &requested.url);
            }
            _ => {}
        }
        if own {
            self.requested.push(requested);
        }
        Ok(())
    }

    /// The service worker held as `session` stopped, told of as `method`
    /// `Inspector.targetCrashed` — it answers nothing until it starts again,
    /// and is not heard — or started again, as any other `method`: waiting,
    /// in the target it had, so it is held again and let go once it has
    /// answered.
    fn stopped_or_started(&mut self, method: &str, session: &str) -> Result<(), Failed> {
        if method == "Inspector.targetCrashed" {
            self.holding.stopped(session);
            self.hearing.retain(|heard| heard != session);
        } else {
            let asked = self.ask_to_hold(session, Kind::ServiceWorker, true)?;
            self.holding.hold_again(session, &asked);
        }
        Ok(())
    }

    /// A frame of the tab that holds a new document, told of in `params`
    /// through `session`: the frame's document from now, and when it is the
    /// tab's own, the tab's. A frame inside another is told of with the
    /// frame holding it, in its own target as in the tab's (measured against
    /// Chrome 155).
    ///
    /// When a line is owed for the frame, the renderer that told of the
    /// commit is asked to answer after everything it told of before
    /// ([`chrome_owed`]): what it has not told of a loader for by then, it
    /// never will.
    fn committed(&mut self, session: &str, params: &[u8]) -> Result<(), Failed> {
        let committed = chrome_protocol::committed(params).map_err(Failed::Malformed)?;
        let own = committed.frame == self.target;
        if own == committed.inside {
            return Ok(());
        }
        self.causes
            .committed(self.chrome.record_mut(), &committed.frame);
        self.forget_owed();
        if self.owed.waits_on(&committed.frame) {
            let asked = self
                .chrome
                .send(Some(session), &Command::HearTheNetworkOut)?;
            self.owed.asked(asked, &committed.frame);
        }
        if own {
            self.address = alo_url::parse(&committed.url).ok();
        }
        Ok(())
    }

    /// Whether `message` is a request a renderer of the tab told of
    /// (`Network.requestWillBeSent`, through a session `held`) — and when
    /// its line is owed, written: as the document its loader names when
    /// that is the one its frame was getting, and otherwise as it was kept.
    ///
    /// # Errors
    ///
    /// [`Failed::Malformed`], for one that cannot be read while a line is
    /// owed; none is read while none is.
    fn told_in(&mut self, message: &Message, held: Option<Kind>) -> Result<bool, Failed> {
        let Message::Event { method, params, .. } = message else {
            return Ok(false);
        };
        if held.is_none() || method != "Network.requestWillBeSent" {
            return Ok(false);
        }
        if self.owed.is_empty() {
            return Ok(true);
        }
        let (network, loader) = chrome_protocol::told(params).map_err(Failed::Malformed)?;
        if let Some((line, cause)) = self.owed.told(&network, &loader) {
            self.write_owed(line, cause);
        }
        Ok(true)
    }

    /// Write the lines owed for frames this tab no longer remembers, as
    /// each was kept: no renderer of theirs will be asked.
    fn forget_owed(&mut self) {
        let causes = &self.causes;
        let gone = self.owed.forgotten(|frame| !causes.remembers(frame));
        self.settle(gone);
    }

    /// Write each of `lines`, as it was kept.
    fn settle(&mut self, lines: Vec<chrome_owed::Line>) {
        for line in lines {
            let kept = line.kept.clone();
            self.write_owed(line, kept);
        }
    }

    /// Write `line`, caused by `cause`.
    fn write_owed(&mut self, line: chrome_owed::Line, cause: alo_net::Cause) {
        self.chrome
            .record_mut()
            .write(line.url, &line.method, line.purpose, cause, line.happened);
    }

    /// A frame made inside one of the tab's, told of in `params` as
    /// `Page.frameAttached` before it asks for anything, or one gone, as
    /// `Page.frameDetached`: taken out of its page when Chrome says
    /// `remove`, and when it says `swap` gone on in a target of its own, and
    /// still the tab's.
    fn made_or_gone(&mut self, method: &str, params: &[u8]) -> Result<(), Failed> {
        if method == "Page.frameAttached" {
            let (frame, holder) = chrome_protocol::frame_made(params).map_err(Failed::Malformed)?;
            self.causes.made(&frame, &holder);
        } else {
            let (frame, why) = chrome_protocol::frame_gone(params).map_err(Failed::Malformed)?;
            if why == "remove" {
                self.causes.removed(&frame);
                self.forget_owed();
            }
        }
        Ok(())
    }

    /// A frame of the tab that moved within its document, told of in
    /// `params`: when it is the tab's own, the tab is at that address from
    /// now, and its document is the one it held.
    fn moved_within(&mut self, params: &[u8]) -> Result<(), Failed> {
        let within = chrome_protocol::within(params).map_err(Failed::Malformed)?;
        if within.frame == self.target {
            self.address = alo_url::parse(&within.url).ok();
            // The first of a task's moves are the ones a press asks after: a
            // link's own comes before anything a script did in its task.
            if self.within.len() < MOST_WITHIN {
                self.within.push(within);
            }
        }
        Ok(())
    }

    /// How to answer `paused`, a request paused in a target of kind `by`, and
    /// whether it is a frozen page's own.
    fn answer(&mut self, paused: chrome_protocol::Paused, by: Kind) -> (Command, bool) {
        let request = paused.request;
        match &self.kept {
            Kept::Frozen(page) if paused.url == self.url => (
                Command::Fulfil {
                    request,
                    body: page.clone(),
                },
                true,
            ),
            Kept::Frozen(_) => {
                self.refused.push(paused.url);
                (Command::Refuse { request }, false)
            }
            Kept::Network => {
                // A frame's navigation is paused in the session of the frame
                // holding it, or the frame's own, and is for the frame.
                let hop = match (by, paused.kind.as_str(), paused.redirected) {
                    (Kind::Page | Kind::Frame, "Document", false) => Hop::First,
                    (Kind::Page | Kind::Frame, "Document", true) => Hop::Redirected,
                    _ => Hop::None,
                };
                // Paused while its frame goes to a new document, a request
                // may be either's: Chrome is yet to say whose (item 647).
                // A frame this tab does not remember is its own frame's.
                let frame = if self.causes.remembers(&paused.frame) {
                    &paused.frame
                } else {
                    &self.target
                };
                let window = match (hop, &paused.network) {
                    (Hop::None, Some(network)) => {
                        self.causes.window(frame).map(|(arriving, loader)| Window {
                            network: network.clone(),
                            frame: frame.clone(),
                            arriving,
                            loader: loader.to_owned(),
                        })
                    }
                    _ => None,
                };
                // Paused with no request id through a frame's session, a
                // request is a page's being left — the frame's, until its
                // next navigation, after the commit of another (item 657).
                let cause = if hop == Hop::None
                    && paused.network.is_none()
                    && matches!(by, Kind::Page | Kind::Frame)
                {
                    self.causes.of_unnamed(&paused.frame)
                } else {
                    self.causes.of_request(
                        self.chrome.record_mut(),
                        &paused.frame,
                        &paused.url,
                        hop,
                        paused.network.as_deref(),
                    )
                };
                let purpose = purpose_of(&paused.kind);
                match chrome_gate::judge(&paused.url) {
                    Ok(url) => {
                        self.write_or_owe(
                            url,
                            &paused.method,
                            purpose,
                            cause,
                            Happened::LetGo,
                            window,
                        );
                        self.went.push(Went {
                            url: paused.url,
                            by,
                        });
                        (Command::Continue { request }, false)
                    }
                    Err(refusal) => {
                        if hop != Hop::None
                            && let Some(arriving) = self.causes.ended(&paused.frame)
                        {
                            let settled = self.owed.ended(&paused.frame, arriving);
                            self.settle(settled);
                        }
                        if let Some(url) = recordable(&paused.url) {
                            let happened = Happened::Refused {
                                rule: refusal.rule.to_string(),
                            };
                            self.write_or_owe(
                                url,
                                &paused.method,
                                purpose,
                                cause,
                                happened,
                                window,
                            );
                        }
                        self.refused.push(paused.url);
                        self.refusals.push(refusal);
                        (Command::Refuse { request }, false)
                    }
                }
            }
        }
    }

    /// Write down that `method` `url`, of kind `purpose`, was asked for by
    /// `cause`, and what `happened` to it — or, when it was paused in the
    /// `window` of its frame going to a new document, owe the line until
    /// Chrome says which document asked ([`chrome_owed`]).
    fn write_or_owe(
        &mut self,
        url: Url,
        method: &str,
        purpose: Purpose,
        cause: alo_net::Cause,
        happened: Happened,
        window: Option<Window>,
    ) {
        let Some(window) = window else {
            self.chrome
                .record_mut()
                .write(url, method, purpose, cause, happened);
            return;
        };
        let line = chrome_owed::Line {
            url,
            method: method.to_owned(),
            purpose,
            happened,
            kept: cause,
            frame: window.frame,
            arriving: window.arriving,
            loader: window.loader,
        };
        if let Some(line) = self.owed.owe(&window.network, line) {
            self.settle(vec![line]);
        }
    }

    /// Write down that `asked` was refused, by `refusal`'s rule: not when
    /// it is too long to be an address or does not parse as one, which
    /// names nothing a line could hold — the refusal says it instead, as
    /// the engine's does ([`alo_renderer::navigate::Refusal::record`]).
    fn write_refused(
        &mut self,
        asked: &str,
        method: &str,
        purpose: Purpose,
        cause: alo_net::Cause,
        refusal: &Refusal,
    ) {
        if let Some(url) = recordable(asked) {
            self.chrome.record_mut().write(
                url,
                method,
                purpose,
                cause,
                Happened::Refused {
                    rule: refusal.rule.to_string(),
                },
            );
        }
    }

    /// Write down that the page opened a WebSocket at `asked`, which
    /// nothing here decided. One whose address does not parse is not a
    /// line, and is still in [`Tab::sockets`].
    fn write_socket(&mut self, asked: &str) {
        if let Some(url) = recordable(asked) {
            let cause = self.causes.of_page();
            self.chrome.record_mut().write(
                url,
                "GET",
                Purpose::Unnamed,
                cause,
                Happened::Undecided,
            );
        }
    }

    /// A target Chrome attached, told of through Chrome's own session when
    /// `by_chrome`, or through this tab's or a target of it held.
    fn attached(
        &mut self,
        attached: chrome_protocol::Attached,
        by_chrome: bool,
    ) -> Result<(), Failed> {
        let opened_by_page = attached.opener.as_deref() == Some(self.target.as_str());
        if attached.kind == "page" {
            // Told of by Chrome, only a tab this page opened is this tab's
            // to close; Chrome attaches every other tab too, and those are
            // not ours to answer.
            if (opened_by_page || !by_chrome) && attached.target != self.target {
                self.close_opened(attached)?;
            }
            return Ok(());
        }
        // A frame or worker Chrome tells of to itself is told of to the tab
        // that holds it too, and is held there — except a shared worker,
        // which is told of to Chrome's own session only, waiting, measured
        // against Chrome 155 (item 637). It is held here as this tab's: a
        // `Tab` borrows its `Chrome` whole, so this is the one tab open in
        // it, and a tab its page opens is closed before it loads anything.
        if by_chrome && attached.kind != Kind::SharedWorker.chrome_word() {
            return Ok(());
        }
        let Some(kind) = Kind::of(&attached.kind) else {
            return Err(Failed::Unheld(Box::new(Unheld {
                kind: attached.kind,
                declined: None,
            })));
        };
        let asked = self.ask_to_hold(&attached.session, kind, false)?;
        self.holding
            .hold(&attached.session, &attached.target, kind, &asked);
        Ok(())
    }

    /// Ask the target held as `session`, of kind `kind`, to hold what it
    /// starts and pause what it asks for — `again`, when it is a service
    /// worker that stopped and has started again — and answer the commands
    /// it is to be let go after.
    ///
    /// Started again, a service worker answers all of it before it runs,
    /// and had kept what it was asked at its first start (measured, Chrome
    /// 155): it is asked again so that what holds it is what was asked of
    /// it, not what Chrome happened to keep.
    fn ask_to_hold(&mut self, session: &str, kind: Kind, again: bool) -> Result<Vec<u64>, Failed> {
        let held = Some(session);
        let answers = again || kind.answers_before_it_runs();
        let mut asked = Vec::new();
        if kind.pauses_its_own() {
            asked.push(self.chrome.send(held, &Command::PauseEveryRequest)?);
        }
        asked.push(
            self.chrome
                .send(held, &Command::HoldNewTabs { hold: true })?,
        );
        // A frame tells of the frames made in it and of what it commits only
        // when asked, and answers before it runs (measured, Chrome 155): so
        // what its document asks for is that document's from the first.
        if kind == Kind::Frame {
            asked.push(self.chrome.send(held, &Command::TellOfLoading)?);
        }
        // Its WebSockets are told of through its own session, measured for
        // a frame, a dedicated worker and a service worker alike. A
        // service worker answers only once it runs, so it is not held for
        // the answer: asked before it is let go, it still tells of a
        // socket its script opens first thing (measured, Chrome 155).
        // Started while a verb is heard, it is told to tell of what its
        // script says before it says anything: held, a frame, a dedicated
        // worker and a shared worker each answer `Runtime.enable` before
        // they run (measured, Chrome 155). A service worker answers it only
        // once it runs, so it is not held for the answer, which nothing
        // here waits for. One started again forgets first what it said
        // before it stopped, and while no verb is heard is told to stop
        // telling, in case it stopped while one was.
        if self.heard.is_some() && kind.is_heard() {
            if again {
                asked.push(self.chrome.send(held, &Command::ForgetWhatWasSaid)?);
            }
            let id = self.chrome.send(held, &Command::TellOfScript)?;
            if answers {
                asked.push(id);
            }
            self.hearing.retain(|heard| heard != session);
            self.hearing.push(session.to_owned());
        } else if again {
            asked.push(self.chrome.send(held, &Command::StopTellingOfScript)?);
        }
        if self.is_from_the_network() {
            let id = self.chrome.send(held, &Command::TellOfSockets)?;
            if answers {
                asked.push(id);
            } else {
                self.telling.insert(id, kind);
            }
        }
        Ok(asked)
    }

    /// Stop every service worker the tab holds, by closing its target —
    /// which, measured against Chrome 155, stops it and keeps the target,
    /// as Chrome stopping it does — for a test of what a tab does with one
    /// that stops and starts again (queue item 655), and answer how many.
    /// Nothing but a test has a reason to stop a page's service worker, and
    /// the name says so.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome cannot be asked or declines.
    pub fn stop_service_workers_for_tests(&mut self) -> Result<usize, Failed> {
        let targets = self.holding.targets(Kind::ServiceWorker);
        for target in &targets {
            self.ask_chrome(&Command::CloseTarget {
                target: target.clone(),
            })?;
        }
        Ok(targets.len())
    }

    /// A tab the page opened, held before it loads anything: every request
    /// it would make is paused, it is let go — held, it may hold the page
    /// that opened it, whose renderer it can share — and it is closed with
    /// its requests unanswered.
    fn close_opened(&mut self, attached: chrome_protocol::Attached) -> Result<(), Failed> {
        let held = Some(attached.session.as_str());
        self.chrome.send(held, &Command::PauseEveryRequest)?;
        self.chrome.send(held, &Command::LetGo)?;
        if self.closed.insert(attached.target.clone()) {
            self.chrome.send(
                None,
                &Command::CloseTarget {
                    target: attached.target,
                },
            )?;
        }
        Ok(())
    }
}

/// Where a request paused falls when its frame is going to a new document:
/// the id its renderer tells of it by, the frame, and the document arriving
/// there with its loader.
struct Window {
    network: String,
    frame: String,
    arriving: DocumentId,
    loader: String,
}

/// Whether `address` is a page from the network, which is all
/// [`Tab::open`] and [`Tab::open_for_agent`] open.
fn on_the_network(address: &Url) -> Result<(), Unopened> {
    if matches!(address.scheme.as_str(), "http" | "https") {
        Ok(())
    } else {
        Err(Unopened::NotTheNetwork {
            scheme: address.scheme.clone(),
        })
    }
}

/// Whether a message through a session held as `held` is one a frame of the
/// tab tells of: the tab's own, or a frame of it in a target of its own.
fn is_a_frames(held: Option<Kind>) -> bool {
    matches!(held, Some(Kind::Page | Kind::Frame))
}

/// `asked` as an address a line of the record can hold: within ADR 0020's
/// bound, and parsed by this process.
fn recordable(asked: &str) -> Option<Url> {
    if asked.len() > LONGEST_URL {
        return None;
    }
    alo_url::parse(asked).ok()
}

impl Drop for Tab<'_> {
    fn drop(&mut self) {
        // Every line is written: what was never told of, as it was kept.
        let owed = self.owed.all();
        self.settle(owed);
        // Not waited for: Chrome stops holding new tabs and closes this one,
        // in that order, and the replies are passed over by whatever asks
        // next.
        let _ = self
            .chrome
            .send(None, &Command::HoldNewTabs { hold: false });
        let _ = self.chrome.send(
            None,
            &Command::CloseTarget {
                target: self.target.clone(),
            },
        );
    }
}
