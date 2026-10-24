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
//! [`Tab::went`] or [`Tab::refusals`]. A navigation the page asks for that
//! Chrome never pauses — a link to `mailto:` — is judged as it is told of, and
//! a refusal named. [`Tab::open_for_agent`] is the same, after the agent's
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
//! same way, its script paused in its own target and refused. A target that
//! cannot be held is [`Failed::Unheld`], and the tab is closed.
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
use crate::chrome_protocol::{self, AxNode, Command, Malformed, Message, Requested};
use crate::chrome_tree::{self, Unread};
use alo_layout::Rect;
use alo_renderer::Snapshot;
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
    /// The navigations the page's own frame asked for, told of since they
    /// were last taken ([`Tab::take_requested`]).
    requested: Vec<Requested>,
    /// The tabs the page opened, by id, each closed before it loaded
    /// anything.
    closed: HashSet<String>,
}

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
        let blank = Command::CreateTarget {
            url: "about:blank".to_owned(),
        };
        let target =
            chrome_protocol::target(&chrome.ask(None, &blank)?).map_err(Failed::Malformed)?;
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
            requested: Vec::new(),
            closed: HashSet::new(),
        };
        // Only now, so that the tab itself is not held.
        tab.chrome.ask(None, &Command::HoldNewTabs { hold: true })?;
        for command in [
            Command::PauseEveryRequest,
            Command::HoldNewTabs { hold: true },
            Command::TellOfLoading,
            Command::TellOfStages,
            Command::LayOutAt { width, height },
        ] {
            tab.ask(&command)?;
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

    /// Send `command` through this tab's session and wait for its result,
    /// answering every request paused meanwhile.
    pub(crate) fn ask(&mut self, command: &Command) -> Result<Vec<u8>, Failed> {
        let asked = self.chrome.send(Some(&self.session), command)?;
        let until = Instant::now() + self.chrome.patience();
        loop {
            if let Some(Message::Reply { id, result, .. }) = self.next(until)?
                && id == asked
            {
                return result.map_err(Failed::Declined);
            }
        }
    }

    /// The next message for this tab that is not about a paused request or a
    /// held target — those are answered here — or [`None`] for one that was.
    fn next(&mut self, until: Instant) -> Result<Option<Message>, Failed> {
        let message = self.chrome.receive(until)?;
        // Through the tab's own session, or one of a target of it held.
        let held = match &message {
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
        };
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
            } if session == self.session && method == "Page.frameRequestedNavigation" => {
                let requested = chrome_protocol::requested(&params).map_err(Failed::Malformed)?;
                if requested.frame == self.target {
                    // Never paused when it is a scheme Chrome hands
                    // elsewhere, so judged as it is told of.
                    if matches!(self.kept, Kept::Network)
                        && let Err(refusal) = chrome_gate::judge(&requested.url)
                    {
                        self.refused.push(requested.url.clone());
                        self.refusals.push(refusal);
                    }
                    self.requested.push(requested);
                }
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
            Message::Event { method, params, .. }
                if method == "Target.detachedFromTarget" && held.is_some() =>
            {
                let gone = chrome_protocol::detached(&params).map_err(Failed::Malformed)?;
                self.holding.forget(&gone);
                Ok(None)
            }
            // Chrome's word on holding a target: once it has answered all
            // the target was asked, the target is let go.
            Message::Reply { id, result, .. } if self.holding.awaits(id) => {
                let answer = result.map(|_| ());
                if let Some(ready) = self
                    .holding
                    .answered(id, answer)
                    .map_err(|why| Failed::Unheld(Box::new(why)))?
                {
                    self.chrome.send(Some(&ready), &Command::LetGo)?;
                }
                Ok(None)
            }
            // Chrome's word on a request this tab answered. The page not taken
            // is a page not shown; a refusal not taken is a request that had
            // already gone, which is what a refusal wanted; a request let go
            // and not taken was one Chrome had already given up.
            Message::Reply { id, result, .. } if self.answering.contains_key(&id) => {
                if self.answering.remove(&id) == Some(true) {
                    result.map_err(Failed::Declined)?;
                }
                Ok(None)
            }
            Message::Event {
                session: Some(_), ..
            }
            | Message::Reply {
                session: Some(_), ..
            } if held.is_none() => Ok(None),
            other => Ok(Some(other)),
        }
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
            Kept::Network => match chrome_gate::judge(&paused.url) {
                Ok(_) => {
                    self.went.push(Went {
                        url: paused.url,
                        by,
                    });
                    (Command::Continue { request }, false)
                }
                Err(refusal) => {
                    self.refused.push(paused.url);
                    self.refusals.push(refusal);
                    (Command::Refuse { request }, false)
                }
            },
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
        if by_chrome {
            // A frame or worker Chrome tells of to itself is told of to the
            // tab that holds it too, and is held there.
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
