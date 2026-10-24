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
use crate::chrome_protocol::{self, Command, Malformed, Message};
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
        };
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

    /// The page, read as the tree an agent reads.
    ///
    /// # Errors
    ///
    /// [`Unread`], when Chrome does not answer, or its tree is not a tree.
    pub fn read(&mut self) -> Result<Snapshot, Unread> {
        let nodes =
            chrome_protocol::ax_nodes(&self.ask(&Command::ReadTree)?).map_err(Failed::Malformed)?;
        let window =
            chrome_protocol::window(&self.ask(&Command::Window)?).map_err(Failed::Malformed)?;
        chrome_tree::snapshot(&nodes, window, &mut |node| self.drawn(node))
    }

    /// Where Chrome drew the DOM node `node`: nowhere, when it declines to
    /// say, which it does for a node with no box.
    fn drawn(&mut self, node: u64) -> Result<Vec<Rect>, Failed> {
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
    fn ask(&mut self, command: &Command) -> Result<Vec<u8>, Failed> {
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
        // Not waited for: Chrome closes the tab, and the reply is passed over
        // by whatever asks next.
        let _ = self.chrome.send(
            None,
            &Command::CloseTarget {
                target: self.target.clone(),
            },
        );
    }
}
