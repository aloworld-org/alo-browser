/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's fetches and linked style sheets, answered one at a time (ADR 0032
//! § 1, queue item 338; ADR 0035 § 4, queue item 348).
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
//! A linked style sheet is answered the same way — made and checked
//! ([`crate::sheet_make`]) or refused — and goes back through
//! [`Tabs::styled`]. An answer's sheets are queued **ahead of its fetches**:
//! a page is drawn without its style until they arrive, and a sheet's task
//! runs no script, so answering it first holds back nothing a fetch's
//! reaction would do.
//!
//! # One at a time, and who waits
//!
//! A request is made by whoever holds the session's [`Network`], and waits
//! for its answer. So answering is in two halves (ADR 0041 § 1):
//! [`Answering::next`] takes the queue's next turn where the tabs are — a
//! refusal answered there and then, with its [`Record`] line to write, or an
//! [`Exchange`] to make — and [`Answering::exchanged`] delivers what making
//! one came to. Between the two the exchange can be made on another thread,
//! which is how the window's conductor never waits on a server: it hands
//! each one to its network thread and goes on serving the window. The next
//! turn is taken only when the last exchange has come back, so the order a
//! page sees is the queue's whichever thread waits.
//!
//! [`Answering::answer_next`] is both halves on one thread, for whoever holds
//! the tabs and the network together.
//!
//! # An answer for a document that has gone
//!
//! Each queued fetch remembers the document that asked. When that document is
//! no longer the one its tab shows — a new page was loaded, or the tab
//! closed — the fetch is **not made**, and nothing is sent: an answer for a
//! document that has gone is answered by nobody (ADR 0032 § 1), and a request
//! made for it would be a request nobody is waiting on, written into the
//! record as though somebody were. One already being made when its document
//! went is delivered to nobody, and what it would have said goes unsaid.
//!
//! **Except a request that asked to outlive its page** (ADR 0040 § 3): a
//! keep-alive fetch is still made once its document has gone — left, or
//! replaced by the next — and its answer is read, filtered and recorded as
//! any is, and sent to nobody. So is each keep-alive fetch a page asked for
//! as it was left ([`Answering::outlive`]), which takes its turn behind
//! everything already waiting, as any fetch does. Each one made is taken off
//! its document's count ([`Tabs::outlived`]).
//!
//! # When the browser closes
//!
//! What is still waiting then is not made ([`Answering::closing`]): a browser
//! the person has told to close does not go on talking to the network for
//! pages they have left. Each keep-alive fetch among it is written into the
//! record as not made, and why; the rest belong to documents that have gone,
//! and are answered by nobody as above.

use std::collections::VecDeque;

use alo_net::cause::DocumentId;
use alo_net::pool::Pool;

use crate::fetch_decide::{self, Fetch, Refusal, Rule};
use crate::fetch_exchange::{Asked, Exchange, Exchanged, Outcome, Record};
use crate::fetch_make::Network;
use crate::message::FromRenderer;
use crate::sheet_decide;
use crate::tab::{Lost, Tab, TabId, Tabs};

/// One answer, given.
#[derive(Debug, Clone, PartialEq)]
pub struct Answered {
    /// The tab whose page asked.
    pub tab: TabId,
    /// The number of the linked style sheet this answered, when it was one:
    /// what a load waiting for its sheets counts (ADR 0041 § 3).
    pub sheet: Option<u64>,
    /// What its renderer answered the delivery with — [`None`] when the
    /// document was not owed it after all — or why it could not be asked.
    pub delivered: Result<Option<FromRenderer>, Lost>,
    /// What to tell the person: a refusal's rule, a failure's reason, a
    /// cookie that was not kept. Never sent to the page.
    pub said: Vec<String>,
}

/// The queue's next turn.
#[derive(Debug, Clone, PartialEq)]
pub enum Turn {
    /// A refusal, answered already with no exchange to wait for, and its
    /// line, to be written into the session's record now.
    Refused {
        /// What came of answering it.
        answered: Box<Answered>,
        /// Its line in the record.
        record: Record,
    },
    /// An exchange to make. What it comes to goes to
    /// [`Answering::exchanged`].
    Exchange(Exchange),
}

/// One request a page is waiting on, decided.
#[derive(Debug)]
enum Owing {
    /// A fetch its script asked for.
    Fetch(fetch_decide::Decided),
    /// A style sheet it links.
    Sheet(sheet_decide::Decided),
    /// A keep-alive fetch its page asked for as it was left: made, and
    /// answered by nobody.
    Outliving(Box<Fetch>),
}

impl Owing {
    /// The keep-alive fetch to be made, if this is one.
    fn outliving(&self) -> Option<&Fetch> {
        match self {
            Owing::Outliving(fetch) => Some(fetch),
            Owing::Fetch(fetch_decide::Decided::Make(fetch)) if fetch.outlives() => Some(fetch),
            Owing::Fetch(_) | Owing::Sheet(_) => None,
        }
    }
}

/// The fetches and sheets waiting to be made, oldest first.
#[derive(Debug, Default)]
pub struct Answering {
    waiting: VecDeque<(TabId, DocumentId, Owing)>,
}

impl Answering {
    /// Nothing waiting.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many fetches and sheets are waiting.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Queue every sheet and then every fetch the page in tab `id` has asked
    /// for and this process has decided, behind those already waiting. Call
    /// it after every answer that can carry asks — a `Load`'s, an `Act`'s, a
    /// delivery's.
    pub fn take_from(&mut self, tabs: &mut Tabs, id: TabId) {
        let Some(document) = tabs.tab(id).and_then(Tab::document) else {
            // Nothing is showing, so nothing can be owed; whatever was
            // decided is answered by nobody.
            drop(tabs.sheets(id));
            drop(tabs.fetches(id));
            return;
        };
        let sheets = tabs.sheets(id).into_iter().map(Owing::Sheet);
        let fetches = tabs.fetches(id).into_iter().map(Owing::Fetch);
        self.waiting
            .extend(sheets.chain(fetches).map(|owing| (id, document, owing)));
    }

    /// The numbers of the style sheets `document`, shown by tab `id`, has
    /// waiting, in the order they will be answered: what its load waits for,
    /// asked straight after the load's answer is taken (ADR 0041 § 3).
    pub fn sheets_waiting(&self, id: TabId, document: DocumentId) -> Vec<u64> {
        self.waiting
            .iter()
            .filter(|(tab, asker, _)| *tab == id && *asker == document)
            .filter_map(|(_, _, owing)| match owing {
                Owing::Sheet(decided) => Some(decided.number()),
                Owing::Fetch(_) | Owing::Outliving(_) => None,
            })
            .collect()
    }

    /// Queue the keep-alive fetches `document`, shown by tab `id`, asked
    /// for as it was left, behind those already waiting: each is made in its
    /// turn and answered by nobody.
    pub fn outlive(&mut self, id: TabId, document: DocumentId, fetches: Vec<Fetch>) {
        self.waiting.extend(
            fetches
                .into_iter()
                .map(|fetch| (id, document, Owing::Outliving(Box::new(fetch)))),
        );
    }

    /// Answer the oldest fetch or sheet whose document is still showing, or
    /// make the oldest keep-alive fetch whatever became of its page, through
    /// `network`, and queue what its delivery asks for. [`None`] when
    /// nothing is waiting for anybody.
    ///
    /// [`Answering::next`] and [`Answering::exchanged`] on one thread, which
    /// waits on the server.
    pub fn answer_next(&mut self, tabs: &mut Tabs, network: &mut Network) -> Option<Answered> {
        match self.next(tabs)? {
            Turn::Refused { answered, record } => {
                record.record(&mut network.pool);
                Some(*answered)
            }
            Turn::Exchange(exchange) => Some(self.exchanged(tabs, exchange.make(network))),
        }
    }

    /// Take the queue's next turn: the oldest fetch or sheet whose document
    /// is still showing, or the oldest keep-alive fetch whatever became of
    /// its page. A refusal is answered here, and what its delivery asks for
    /// queued; anything to be made is handed back to be made. [`None`] when
    /// nothing is waiting for anybody.
    pub fn next(&mut self, tabs: &mut Tabs) -> Option<Turn> {
        loop {
            let (id, document, owing) = self.waiting.pop_front()?;
            let showing = tabs.tab(id).and_then(Tab::document) == Some(document);
            let exchange = |asked| {
                Some(Turn::Exchange(Exchange {
                    tab: id,
                    document,
                    asked,
                }))
            };
            match owing {
                Owing::Outliving(fetch) => return exchange(Asked::Outliving(fetch)),
                Owing::Fetch(fetch_decide::Decided::Make(fetch))
                    if fetch.outlives() && !showing =>
                {
                    return exchange(Asked::Outliving(fetch));
                }
                _ if !showing => {}
                Owing::Fetch(fetch_decide::Decided::Make(fetch)) => {
                    return exchange(Asked::Fetch(fetch));
                }
                Owing::Sheet(sheet_decide::Decided::Make(sheet)) => {
                    return exchange(Asked::Sheet(sheet));
                }
                Owing::Fetch(fetch_decide::Decided::Refused(refusal)) => {
                    let delivered = tabs.fetched(id, refusal.answer());
                    let said = vec![refusal.to_string()];
                    return Some(Turn::Refused {
                        answered: Box::new(self.delivered(tabs, id, None, delivered, said)),
                        record: Record::Fetch(refusal),
                    });
                }
                Owing::Sheet(sheet_decide::Decided::Refused(refusal)) => {
                    let delivered = tabs.styled(id, refusal.answer());
                    let said = vec![refusal.to_string()];
                    return Some(Turn::Refused {
                        answered: Box::new(self.delivered(
                            tabs,
                            id,
                            Some(refusal.number),
                            delivered,
                            said,
                        )),
                        record: Record::Sheet(refusal),
                    });
                }
            }
        }
    }

    /// Deliver what making an exchange [`Answering::next`] handed out came
    /// to, and queue what that delivery asks for.
    ///
    /// When its document is no longer the one its tab shows, it is delivered
    /// to nobody and nothing is said, unless it asked to outlive its page:
    /// then nobody is sent it, and what it says is still said.
    pub fn exchanged(&mut self, tabs: &mut Tabs, exchanged: Exchanged) -> Answered {
        let Exchanged {
            tab: id,
            document,
            outcome,
        } = exchanged;
        let showing = tabs.tab(id).and_then(Tab::document) == Some(document);
        let nobody = |said| Answered {
            tab: id,
            sheet: None,
            delivered: Ok(None),
            said,
        };
        match outcome {
            Outcome::Fetched {
                fetch,
                outliving,
                made,
            } => {
                tabs.outlived(document, &fetch);
                if outliving {
                    return nobody(made.said);
                }
                if !showing {
                    return nobody(if fetch.outlives() {
                        made.said
                    } else {
                        Vec::new()
                    });
                }
                let delivered = tabs.fetched(id, made.fetched);
                self.delivered(tabs, id, None, delivered, made.said)
            }
            Outcome::Styled { sheet, made } => {
                if !showing {
                    return nobody(Vec::new());
                }
                let delivered = tabs.styled(id, made.answer);
                self.delivered(tabs, id, Some(sheet.number), delivered, made.said)
            }
        }
    }

    /// An answer delivered to tab `id`, with what its delivery asked for
    /// queued.
    fn delivered(
        &mut self,
        tabs: &mut Tabs,
        id: TabId,
        sheet: Option<u64>,
        delivered: Result<Option<FromRenderer>, Lost>,
        said: Vec<String>,
    ) -> Answered {
        if matches!(delivered, Ok(Some(_))) {
            self.take_from(tabs, id);
        }
        Answered {
            tab: id,
            sheet,
            delivered,
            said,
        }
    }

    /// The browser is closing: make nothing more. Each keep-alive fetch
    /// still waiting is taken off its document's count and handed back as
    /// a line to write into the session's record — not made, because the
    /// browser closed (ADR 0040 § 3); everything else waiting belongs to a
    /// document that has gone and is answered by nobody.
    pub fn closing(&mut self, tabs: &mut Tabs) -> Vec<Record> {
        let mut records = Vec::new();
        for (_, document, owing) in core::mem::take(&mut self.waiting) {
            let Some(fetch) = owing.outliving() else {
                continue;
            };
            tabs.outlived(document, fetch);
            records.push(Record::Fetch(Box::new(Refusal {
                number: fetch.number,
                asked: fetch.request.url.serialised.clone(),
                url: Some(fetch.request.url.clone()),
                rule: Rule::Closed,
                cause: fetch.request.cause.clone(),
                purpose: fetch.request.purpose.clone(),
            })));
        }
        records
    }

    /// [`Answering::closing`], with each line written into `pool`'s record
    /// here. How many were recorded.
    pub fn close(&mut self, tabs: &mut Tabs, pool: &mut Pool) -> usize {
        self.closing(tabs)
            .iter()
            .filter(|record| record.record(pool))
            .count()
    }
}
