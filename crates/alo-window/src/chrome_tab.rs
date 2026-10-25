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
//! it has. Closing the tab closes the window.
//!
//! # Waiting for the page, not the blank one before it
//!
//! A tab begins at `about:blank`, whose loading may still be told of after
//! the tab is attached. So the page is loaded when its **own** loader — the
//! one `Page.navigate` names — reaches its `load` stage, and a `load` told
//! of any other is passed over.

use crate::agent_sites::AgentSites;
use crate::asking::Refused;
use crate::chrome::{Chrome, Failed};
use crate::chrome_gate::{self, Refusal};
use crate::chrome_hold::{Holding, Kind, Unheld};
use crate::chrome_protocol::{
    self, AxNode, Command, Declined, Malformed, Message, Requested, Within,
};
use crate::chrome_record::{Causes, Hop, purpose_of};
use crate::chrome_said::Heard;
use crate::chrome_tree::{self, Unread};
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
    /// The tabs the page opened, by id, each closed before it loaded
    /// anything.
    closed: HashSet<String>,
    /// What caused what in this tab, which each line of the record names.
    causes: Causes,
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
    /// A shown Chrome's window could not be sized to hold the page at the
    /// size asked ([`crate::chrome_window::fit`]).
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
        Self::begin(chrome, url, Kept::Frozen(page), size)
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
        if !matches!(address.scheme.as_str(), "http" | "https") {
            return Err(Unopened::NotTheNetwork {
                scheme: address.scheme.clone(),
            });
        }
        Self::begin(chrome, &address.to_string(), Kept::Network, size)
    }

    /// Open the page at `address`, which an agent named, as [`Tab::open`]
    /// does — when an agent kept to `sites` may reach it.
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
        Self::open(chrome, &url, size)
    }

    /// A tab at `url`, answered from `kept`, loaded.
    fn begin(
        chrome: &'c mut Chrome,
        url: &str,
        kept: Kept,
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
        let causes = Causes::of(chrome.record_mut().a_tab(), &target);
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
            closed: HashSet::new(),
            causes,
            acted: None,
            heard: None,
            hearing: Vec::new(),
            sockets: Vec::new(),
            address: alo_url::parse(url).ok(),
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
        tab.go()?;
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
        self.answer_to(asked)
    }

    /// Go to the page, and wait for its own loader's `load`.
    fn go(&mut self) -> Result<(), Unopened> {
        let asked = self.chrome.send(
            Some(&self.session),
            &Command::Navigate {
                url: self.url.clone(),
            },
        )?;
        let until = Instant::now() + self.chrome.patience();
        let mut going = None;
        let mut loaded = HashSet::new();
        loop {
            if let Some(going) = &going
                && loaded.contains(going)
            {
                return Ok(());
            }
            match self.next(until)? {
                Some(Message::Skipped { id: Some(id), .. }) if id == asked => {
                    return Err(Failed::TooLong.into());
                }
                Some(Message::Reply { id, result, .. }) if id == asked => {
                    let result = result.map_err(Failed::Declined)?;
                    match chrome_protocol::navigated(&result).map_err(Failed::Malformed)? {
                        Ok(by) => going = Some(by),
                        Err(reason) => return Err(Unopened::NotNavigated { reason }),
                    }
                }
                Some(Message::Event { method, params, .. }) if method == "Page.lifecycleEvent" => {
                    let (by, stage) = chrome_protocol::stage(&params).map_err(Failed::Malformed)?;
                    if stage == "load" {
                        loaded.insert(by);
                    }
                }
                _ => {}
            }
        }
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
    pub(crate) fn ask(&mut self, command: &Command) -> Result<Vec<u8>, Failed> {
        let asked = self.chrome.send(Some(&self.session), command)?;
        self.answer_to(asked)
    }

    /// Wait for the result of the command sent as `asked`, answering every
    /// request paused meanwhile.
    fn answer_to(&mut self, asked: u64) -> Result<Vec<u8>, Failed> {
        let until = Instant::now() + self.chrome.patience();
        loop {
            match self.next(until)? {
                Some(Message::Reply { id, result, .. }) if id == asked => {
                    return result.map_err(Failed::Declined);
                }
                Some(Message::Skipped { id: Some(id), .. }) if id == asked => {
                    return Err(Failed::TooLong);
                }
                _ => {}
            }
        }
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
    /// again; Chrome 155 did not stop one held by this tab, idle for forty
    /// seconds, past the thirty after which it stops one nobody holds.
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
    /// before it was — a frame taken out of its page, or a worker that
    /// closed — whether Chrome refused it for that or never answered.
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
            let gone = self.holding.kind(session).is_none();
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
        let held = self.held_by(&message);
        if self.heard_in(&message) {
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
            Message::Event { method, params, .. }
                if is_a_frames(held) && method == "Page.frameNavigated" =>
            {
                self.committed(&params)?;
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
            // A shared worker's end is told of to Chrome's own session.
            Message::Event {
                method,
                session,
                params,
            } if method == "Target.detachedFromTarget" && (session.is_none() || held.is_some()) => {
                let gone = chrome_protocol::detached(&params).map_err(Failed::Malformed)?;
                self.holding.forget(&gone);
                self.hearing.retain(|heard| *heard != gone);
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

    /// Whether `id` is a command this tab sent on its own account, whose
    /// answer it waits for rather than its caller.
    fn awaits(&self, id: u64) -> bool {
        self.holding.awaits(id)
            || self.telling.contains_key(&id)
            || self.answering.contains_key(&id)
    }

    /// Chrome's answer to `id`, a command [`Tab::awaits`].
    fn answered(&mut self, id: u64, result: Result<Vec<u8>, Declined>) -> Result<(), Failed> {
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

    /// A frame of the tab that holds a new document, told of in `params`:
    /// the frame's document from now, and when it is the tab's own, the
    /// tab's. A frame inside another is told of with the frame holding it,
    /// in its own target as in the tab's (measured against Chrome 155).
    fn committed(&mut self, params: &[u8]) -> Result<(), Failed> {
        let committed = chrome_protocol::committed(params).map_err(Failed::Malformed)?;
        let own = committed.frame == self.target;
        if own == committed.inside {
            return Ok(());
        }
        self.causes
            .committed(self.chrome.record_mut(), &committed.frame);
        if own {
            self.address = alo_url::parse(&committed.url).ok();
        }
        Ok(())
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
                let cause = self.causes.of_request(
                    self.chrome.record_mut(),
                    &paused.frame,
                    &paused.url,
                    hop,
                );
                let purpose = purpose_of(&paused.kind);
                match chrome_gate::judge(&paused.url) {
                    Ok(url) => {
                        self.chrome.record_mut().write(
                            url,
                            &paused.method,
                            purpose,
                            cause,
                            Happened::LetGo,
                        );
                        self.went.push(Went {
                            url: paused.url,
                            by,
                        });
                        (Command::Continue { request }, false)
                    }
                    Err(refusal) => {
                        if hop != Hop::None {
                            self.causes.ended(&paused.frame);
                        }
                        self.write_refused(&paused.url, &paused.method, purpose, cause, &refusal);
                        self.refused.push(paused.url);
                        self.refusals.push(refusal);
                        (Command::Refuse { request }, false)
                    }
                }
            }
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
        let held = Some(attached.session.as_str());
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
        // here waits for.
        if self.heard.is_some() && kind.is_heard() {
            let id = self.chrome.send(held, &Command::TellOfScript)?;
            if kind.answers_before_it_runs() {
                asked.push(id);
            }
            self.hearing.push(attached.session.clone());
        }
        if self.is_from_the_network() {
            let id = self.chrome.send(held, &Command::TellOfSockets)?;
            if kind.answers_before_it_runs() {
                asked.push(id);
            } else {
                self.telling.insert(id, kind);
            }
        }
        self.holding.hold(&attached.session, kind, &asked);
        Ok(())
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
