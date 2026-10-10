/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One exchange a page is waiting on, made by whoever holds the session's
//! network, and one line written into its record (ADR 0041 § 1, queue item
//! 351).
//!
//! [`crate::fetch_answering`] decides **which** request is next and delivers
//! what came of it; this is the part in between, which waits on a server.
//! It is a value of its own so that the thread that decides and the thread
//! that waits can be different threads: the window's conductor hands an
//! [`Exchange`] to its network thread, the only holder of the [`Network`],
//! and is handed back an [`Exchanged`] while it goes on serving the window.
//!
//! An [`Exchange`] carries everything it needs and nothing of a tab, so it
//! can cross to another thread. What it remembers of its page — the tab and
//! the document that asked — is only to be delivered by: whether that
//! document is still showing when the answer comes back is decided where the
//! tabs are, as it was before (ADR 0032 § 1).
//!
//! A [`Record`] is a refusal's line in the session's record. A refusal is
//! answered without an exchange, where the tabs are, but its line is written
//! by whoever holds the pool, in its turn, so the record keeps the order
//! things happened in.

use alo_net::cause::DocumentId;
use alo_net::pool::Pool;

use crate::fetch_decide::{self, Fetch};
use crate::fetch_filter;
use crate::fetch_make::{self, Network};
use crate::sheet::SheetAnswer;
use crate::sheet_decide;
use crate::sheet_make;
use crate::tab::TabId;

/// What an exchange is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    /// A fetch a page's script asked for.
    Fetch(Box<Fetch>),
    /// A style sheet a page links.
    Sheet(Box<Fetch>),
    /// A keep-alive fetch a page asked for as it was left: made, and
    /// answered by nobody.
    Outliving(Box<Fetch>),
}

/// One request, decided, to be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    pub(crate) tab: TabId,
    pub(crate) document: DocumentId,
    pub(crate) asked: Asked,
}

/// What making one came to, with the request it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// A fetch's answer, filtered for its page.
    Fetched {
        /// The fetch.
        fetch: Box<Fetch>,
        /// Whether it was made after its page had gone, for nobody.
        outliving: bool,
        /// What came of it.
        made: fetch_make::Made,
    },
    /// A sheet's answer: its bytes only if they are a style sheet.
    Styled {
        /// The sheet.
        sheet: Box<Fetch>,
        /// What came of it.
        made: sheet_make::Made,
    },
}

/// An exchange, made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchanged {
    pub(crate) tab: TabId,
    pub(crate) document: DocumentId,
    pub(crate) outcome: Outcome,
}

impl Exchange {
    /// The tab whose page asked.
    pub fn tab(&self) -> TabId {
        self.tab
    }

    /// The number of the style sheet this asks for, when it is one.
    pub fn sheet(&self) -> Option<u64> {
        match &self.asked {
            Asked::Sheet(sheet) => Some(sheet.number),
            Asked::Fetch(_) | Asked::Outliving(_) => None,
        }
    }

    /// Make it through `network`, as [`fetch_make::make`] or
    /// [`sheet_make::make`] does. This is the part that waits on a server.
    pub fn make(self, network: &mut Network) -> Exchanged {
        let outcome = match self.asked {
            Asked::Fetch(fetch) => Outcome::Fetched {
                made: fetch_make::make(&fetch, network),
                fetch,
                outliving: false,
            },
            Asked::Outliving(fetch) => Outcome::Fetched {
                made: fetch_make::make(&fetch, network),
                fetch,
                outliving: true,
            },
            Asked::Sheet(sheet) => Outcome::Styled {
                made: sheet_make::make(&sheet, network),
                sheet,
            },
        };
        Exchanged {
            tab: self.tab,
            document: self.document,
            outcome,
        }
    }

    /// It, failed without being made — because whatever would have made it
    /// has gone — with `why` said to the person and never to the page.
    pub fn failed(self, why: &str) -> Exchanged {
        let fetch_failed = |fetch: &Fetch| fetch_make::Made {
            fetched: fetch_filter::failed(fetch, why).fetched,
            said: vec![format!(
                "the page's fetch of {} failed: {why}",
                fetch.request.url
            )],
        };
        let outcome = match self.asked {
            Asked::Fetch(fetch) => Outcome::Fetched {
                made: fetch_failed(&fetch),
                fetch,
                outliving: false,
            },
            Asked::Outliving(fetch) => Outcome::Fetched {
                made: fetch_failed(&fetch),
                fetch,
                outliving: true,
            },
            Asked::Sheet(sheet) => Outcome::Styled {
                made: sheet_make::Made {
                    answer: SheetAnswer::failed(sheet.number),
                    said: vec![format!(
                        "the page's style sheet at {} did not arrive: {why}",
                        sheet.request.url
                    )],
                },
                sheet,
            },
        };
        Exchanged {
            tab: self.tab,
            document: self.document,
            outcome,
        }
    }
}

impl Exchanged {
    /// The tab whose page asked.
    pub fn tab(&self) -> TabId {
        self.tab
    }
}

/// A refusal's line in the session's record, to be written by whoever holds
/// the pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A fetch refused.
    Fetch(Box<fetch_decide::Refusal>),
    /// A style sheet refused.
    Sheet(Box<sheet_decide::Refusal>),
}

impl Record {
    /// Write it into `pool`'s record. Whether it was written — not for a URL
    /// that did not parse, which names nothing a line could hold.
    pub fn record(&self, pool: &mut Pool) -> bool {
        match self {
            Record::Fetch(refusal) => refusal.record(pool),
            Record::Sheet(refusal) => refusal.record(pool),
        }
    }
}
