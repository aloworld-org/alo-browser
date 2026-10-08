/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's fetches, answered one at a time (ADR 0032 § 1, queue item 338).
//!
//! Whoever drives [`Tabs`] — the window's conductor — takes each answer's
//! decided fetches into an [`Answering`] and asks it for one answer at a
//! time: a decided fetch is made ([`crate::fetch_make`]), a refused one is
//! recorded and answered with its network error, and either answer goes
//! back through [`Tabs::fetched`] as a task of its own. What that task asks
//! for in turn joins the end of the queue — **made later, never inside the
//! delivery that asked**, so one page's chain of fetches takes its turn
//! behind everything already waiting rather than in front of it.
//!
//! # One at a time, and why
//!
//! A request is made by the thread that holds the pool, and waits for its
//! answer. The conductor is that thread and is also the one that must hear
//! the window close, so it asks for one answer and then looks at its orders
//! again: a page that fetches for ever costs its own tab's answers, and
//! never the window's ability to close.
//!
//! # An answer for a document that has gone
//!
//! Each queued fetch remembers the document that asked. When that document is
//! no longer the one its tab shows — a new page was loaded, or the tab
//! closed — the fetch is **not made**, and nothing is sent: an answer for a
//! document that has gone is answered by nobody (ADR 0032 § 1), and a request
//! made for it would be a request nobody is waiting on, written into the
//! record as though somebody were.

use std::collections::VecDeque;

use alo_net::cause::DocumentId;

use crate::fetch_decide::Decided;
use crate::fetch_make::{self, Network};
use crate::message::FromRenderer;
use crate::tab::{Lost, Tab, TabId, Tabs};

/// One answer, given.
#[derive(Debug, Clone, PartialEq)]
pub struct Answered {
    /// The tab whose page asked.
    pub tab: TabId,
    /// What its renderer answered the delivery with — [`None`] when the
    /// document was not owed it after all — or why it could not be asked.
    pub delivered: Result<Option<FromRenderer>, Lost>,
    /// What to tell the person: a refusal's rule, a failure's reason, a
    /// cookie that was not kept. Never sent to the page.
    pub said: Vec<String>,
}

/// The fetches waiting to be made, oldest first.
#[derive(Debug, Default)]
pub struct Answering {
    waiting: VecDeque<(TabId, DocumentId, Decided)>,
}

impl Answering {
    /// Nothing waiting.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many fetches are waiting.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Queue every fetch the page in tab `id` has asked for and this process
    /// has decided, behind those already waiting. Call it after every answer
    /// that can carry asks — a `Load`'s, an `Act`'s, a delivery's.
    pub fn take_from(&mut self, tabs: &mut Tabs, id: TabId) {
        let Some(document) = tabs.tab(id).and_then(Tab::document) else {
            // Nothing is showing, so nothing can be owed; whatever was
            // decided is answered by nobody.
            drop(tabs.fetches(id));
            return;
        };
        self.waiting.extend(
            tabs.fetches(id)
                .into_iter()
                .map(|decided| (id, document, decided)),
        );
    }

    /// Answer the oldest fetch whose document is still showing, through
    /// `network`, and queue what its delivery asks for. [`None`] when nothing
    /// is waiting for anybody.
    pub fn answer_next(&mut self, tabs: &mut Tabs, network: &mut Network) -> Option<Answered> {
        loop {
            let (id, document, decided) = self.waiting.pop_front()?;
            if tabs.tab(id).and_then(Tab::document) != Some(document) {
                continue;
            }
            let mut said = Vec::new();
            let fetched = match decided {
                Decided::Refused(refusal) => {
                    refusal.record(&mut network.pool);
                    said.push(refusal.to_string());
                    refusal.answer()
                }
                Decided::Make(fetch) => {
                    let made = fetch_make::make(&fetch, network);
                    said.extend(made.said);
                    made.fetched
                }
            };
            let delivered = tabs.fetched(id, fetched);
            if matches!(delivered, Ok(Some(_))) {
                self.take_from(tabs, id);
            }
            return Some(Answered {
                tab: id,
                delivered,
                said,
            });
        }
    }
}
