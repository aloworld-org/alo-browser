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
//! Opening a page from the network, through Chrome's own requests and under
//! the grants a person gave, is not built: what the browser process lets
//! Chrome fetch is a question of its own (queue item 629).
//!
//! # Waiting for the page, not the blank one before it
//!
//! A tab begins at `about:blank`, whose loading may still be told of after
//! the tab is attached. So the page is loaded when its **own** loader — the
//! one `Page.navigate` names — reaches its `load` stage, and a `load` told
//! of any other is passed over.

use crate::chrome::{Chrome, Failed};
use crate::chrome_protocol::{self, AxNode, Command, Malformed, Message, Requested};
use crate::chrome_tree::{self, Unread};
use alo_layout::Rect;
use alo_renderer::Snapshot;
use core::fmt;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// A page in Chrome.
pub struct Tab<'c> {
    chrome: &'c mut Chrome,
    target: String,
    session: String,
    url: String,
    page: Vec<u8>,
    refused: Vec<String>,
    /// The requests this tab answered and awaits Chrome's word on, and
    /// whether each was the page.
    answering: HashMap<u64, bool>,
    /// The navigations the page's own frame asked for, told of since they
    /// were last taken ([`Tab::take_requested`]).
    requested: Vec<Requested>,
    /// How many tabs the page opened, each closed before it loaded anything.
    closed: usize,
}

/// Why a page was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unopened {
    /// Asking Chrome failed.
    Failed(Failed),
    /// Chrome did not go to the page, and said why.
    NotNavigated {
        /// Chrome's own words, as `net::ERR_ABORTED`.
        reason: String,
    },
}

impl fmt::Display for Unopened {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(why) => why.fmt(f),
            Self::NotNavigated { reason } => write!(f, "Chrome did not open the page: {reason}"),
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
            page,
            refused: Vec::new(),
            answering: HashMap::new(),
            requested: Vec::new(),
            closed: 0,
        };
        // Only now, so that the tab itself is not held.
        tab.chrome.ask(None, &Command::HoldNewTabs { hold: true })?;
        for command in [
            Command::PauseEveryRequest,
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

    /// How many tabs the page opened — a link with `target=_blank`, a
    /// `window.open` — each closed before it loaded anything.
    pub fn closed(&self) -> usize {
        self.closed
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

    /// The next message for this tab that is not about a paused request —
    /// those are answered here — or [`None`] for one that was.
    fn next(&mut self, until: Instant) -> Result<Option<Message>, Failed> {
        let message = self.chrome.receive(until)?;
        match message {
            Message::Event {
                method,
                session: Some(session),
                params,
            } if session == self.session && method == "Fetch.requestPaused" => {
                let paused = chrome_protocol::paused(&params).map_err(Failed::Malformed)?;
                let page = paused.url == self.url;
                let answer = if page {
                    Command::Fulfil {
                        request: paused.request,
                        body: self.page.clone(),
                    }
                } else {
                    self.refused.push(paused.url);
                    Command::Refuse {
                        request: paused.request,
                    }
                };
                let id = self.chrome.send(Some(&self.session), &answer)?;
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
                    self.requested.push(requested);
                }
                Ok(None)
            }
            // A tab the page opened, held before it loads anything: every
            // request it would make is paused, it is let go — held, it may
            // hold the page that opened it, whose renderer it can share —
            // and it is closed with its requests unanswered.
            Message::Event {
                method,
                session: None,
                params,
            } if method == "Target.attachedToTarget" => {
                let attached = chrome_protocol::attached(&params).map_err(Failed::Malformed)?;
                if attached.opener.as_deref() == Some(self.target.as_str()) {
                    let held = Some(attached.session.as_str());
                    self.chrome.send(held, &Command::PauseEveryRequest)?;
                    self.chrome.send(held, &Command::LetGo)?;
                    self.chrome.send(
                        None,
                        &Command::CloseTarget {
                            target: attached.target,
                        },
                    )?;
                    self.closed = self.closed.saturating_add(1);
                }
                Ok(None)
            }
            // Chrome's word on a request this tab answered. The page not taken
            // is a page not shown; a refusal not taken is a request that had
            // already gone, which is what a refusal wanted.
            Message::Reply { id, result, .. } if self.answering.contains_key(&id) => {
                if self.answering.remove(&id) == Some(true) {
                    result.map_err(Failed::Declined)?;
                }
                Ok(None)
            }
            Message::Event {
                session: Some(ref session),
                ..
            }
            | Message::Reply {
                session: Some(ref session),
                ..
            } if *session != self.session => Ok(None),
            other => Ok(Some(other)),
        }
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
