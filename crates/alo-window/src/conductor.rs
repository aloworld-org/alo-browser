/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conductor: the one thread of the browser process that talks to
//! renderers.
//!
//! ADR 0024 § 2. `winit` runs its event loop on the main thread, and an
//! exchange with a renderer can take up to `alo-renderer`'s
//! `LONGEST_SILENCE` before it is given up on. A window that froze for that
//! long whenever one site stopped answering would make ADR 0005's *"every
//! other tab is untouched"* false at exactly the level a person sees. So the
//! [`Tabs`] — and with them every renderer — live on this thread, which is
//! sent [`Order`]s and answers with [`News`], and the event loop never waits
//! on either.
//!
//! # What it is told, and what it is not
//!
//! It is told what a person did and how big the window is. It is never told
//! a point: hit-testing is item 298, and a person's pointer reaches a
//! renderer through it, not through here.
//!
//! # Where a page's fetches are made
//!
//! Decided here, because this thread holds the tabs (queue item 338, ADR
//! 0032 § 3); **made on the network thread** ([`crate::network`]), the only
//! holder of the session's [`Network`], because a request waits on its server
//! and this thread must not (ADR 0041). Every answer that can carry a page's
//! fetches puts them in an [`Answering`]. The conductor takes its turns one at
//! a time — a refusal answered at once, an exchange handed to the network
//! thread — and takes the next only when the last exchange has come back, so
//! the order a page sees is the queue's. It waits on its [`inbox`], for orders
//! and results alike, and on nothing else: a resize, a visibility change or a
//! close is carried out while a request is in flight, however slowly its
//! server answers. After a delivery the selected tab is painted again, since
//! the reactions may have changed it, and the reason a fetch failed — which
//! the page is never told — is said to the person.
//!
//! A page's linked style sheets are made the same way, in the same queue and
//! ahead of the fetches its answer asked for (queue item 348, ADR 0035), and
//! the page is painted again with each sheet that arrives.
//!
//! # A load waits for its style, only so long
//!
//! ADR 0035 § 5, ADR 0041 § 3. The sheets a load's answer asked for are
//! owed, and the tab is held ([`crate::hold`]): everything is done but
//! nothing is painted, until every owed sheet is answered, or
//! [`crate::hold::LONGEST_HOLD`] passes and the page is painted with what has
//! arrived and said to be shown before its style, or the tab stops showing
//! that document. A sheet that arrives after the bound is applied when it
//! comes, as any sheet after a load is.
//!
//! # Whether a page can be seen
//!
//! ADR 0039 § 1: a tab is `visible` when it is the selected tab of a window
//! that is neither covered nor minimised. The window says which it is, and
//! the conductor tells the selected tab's page — a task that fires
//! `visibilitychange` if that changed it — and loads every page after it in
//! the state the window is in. What the page's listeners ask for is made as
//! a delivery's is, and the page is painted again afterwards, since they may
//! have changed it. Choosing among tabs is item 297's, and tells the tab it
//! leaves before the one it chooses.
//!
//! # A page left
//!
//! ADR 0039 § 2: closing the window closes every tab, and each tab's page is
//! left as it goes — `pagehide`, `hidden`, `unload`, given a second — by
//! [`Tabs::close`]. What such a page asks to fetch without asking to outlive
//! it is refused by name (§ 4) and written into the session's record here,
//! as is what a page replaced by a load asked for. What it asks to outlive
//! it — a beacon — is decided as any fetch is and queued behind the rest,
//! to be made after the page has gone and answered by nobody (ADR 0040
//! § 3). The window waits on none of it once it has asked to close (ADR 0024
//! § 2): this thread finishes leaving behind it, and **makes nothing more**:
//! every beacon still waiting when the browser closes is written into the
//! record as not made, because it closed. The window is told it has closed
//! without waiting for the network thread; the session's record comes back
//! when the conductor finishes, by joining it, which waits for the one
//! exchange in flight if there is one (ADR 0041 § 5).
//!
//! # Why a burst of resizes is one resize
//!
//! Dragging a window's corner sends a resize for nearly every pixel it
//! crosses, and a renderer asked to lay out and paint each one would still be
//! painting sizes the window left a second ago. Everything waiting when the
//! conductor looks is taken at once, and a resize with a later resize behind
//! it is dropped: the size that matters is the one the window is now.

use crate::hold::{Holds, SHOWN_BEFORE_STYLE};
use crate::inbox::{self, Arrival, Orders, arrivals};
use crate::message::{News, Order};
use crate::network::{Job, Networking};
use alo_layout::Size;
use alo_net::Cause;
use alo_renderer::fetch_answering::{Answered, Answering, Turn};
use alo_renderer::fetch_exchange::{Exchange, Exchanged, Record};
use alo_renderer::fetch_make::Network;
use alo_renderer::{FromRenderer, Lost, Page, Tab, TabId, Tabs, ToRenderer, Visibility};
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use std::time::Instant;

/// What the person is told when the network thread has gone, and with it
/// every request this window could make.
const NETWORK_GONE: &str =
    "this browser's network has stopped, and nothing more can be fetched until it is started again";

/// Whether a renderer that asks for a font by name is sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonts {
    /// Yes, from this machine's fonts (queue item 170): the page is drawn as
    /// it asked to be.
    AsAsked,
    /// No: the renderers have the fonts they were started with and nothing
    /// else, so a frozen page draws exactly as its committed render does.
    AsStarted,
}

/// A conductor running on its own thread.
#[derive(Debug)]
pub struct Conductor {
    orders: Orders,
    /// The thread, which hands back the session's network as it ends.
    thread: JoinHandle<Option<Network>>,
}

impl Conductor {
    /// Start one over `tabs`, making their pages' fetches through `network`
    /// on a network thread of its own, telling the window what happens with
    /// `tell`.
    ///
    /// `tell` answers whether anybody heard: `false` means the window has gone,
    /// and the conductor closes every tab and stops rather than painting for
    /// nobody.
    ///
    /// # Errors
    ///
    /// What the operating system said when it would not start a thread.
    pub fn start(
        tabs: Tabs,
        fonts: Fonts,
        network: Network,
        tell: impl Fn(News) -> bool + Send + 'static,
    ) -> std::io::Result<Self> {
        let (orders, results, inbox) = inbox::inbox();
        let networking = Networking::start(network, results)?;
        let thread = std::thread::Builder::new()
            .name("alo-conductor".to_owned())
            .spawn(move || conduct(tabs, fonts, networking, &inbox, &tell))?;
        Ok(Self { orders, thread })
    }

    /// Where to send it orders.
    pub fn orders(&self) -> Orders {
        self.orders.clone()
    }

    /// Wait for it to finish, which it does after [`Order::CloseEverything`]
    /// or when every sender of orders has gone.
    ///
    /// Whether it finished, and its network thread with it, rather than
    /// panicking — which the lints make unreachable, and which is said rather
    /// than assumed.
    pub fn finish(self) -> bool {
        self.hand_back().is_some()
    }

    /// Wait for it to finish, as [`Conductor::finish`] does, and take back
    /// the session's network: its record of every request made or refused,
    /// which outlives the window (ADR 0012 § 5) — including what a page asked
    /// for as the window closed and it was left (ADR 0039 § 4). [`None`] if
    /// it or its network thread panicked.
    pub fn hand_back(self) -> Option<Network> {
        drop(self.orders);
        self.thread.join().ok().flatten()
    }
}

/// The conductor's state: the tabs, which one is selected, and the size a
/// page is laid out at.
struct Conducting {
    tabs: Tabs,
    fonts: Fonts,
    selected: Option<TabId>,
    /// The window's size in CSS pixels, once it has said one.
    viewport: Option<Size>,
    /// Whether the window can be seen: `visible` until it says otherwise,
    /// as a window that has just opened can be.
    visibility: Visibility,
    /// A tab opened before the window had a size, with its page — loaded when
    /// the size arrives, because a page is laid out at the window's size and
    /// there was none to lay it out at.
    waiting: Option<(TabId, Page)>,
    /// The thread every request is made on, until it has gone.
    networking: Option<Networking>,
    /// The exchange it is making, if any: the next is handed out only when
    /// this one has come back.
    in_flight: Option<Exchange>,
    /// The pages' fetches and linked style sheets, decided and waiting to be
    /// made.
    fetches: Answering,
    /// The tabs whose first frame waits for their load's sheets.
    holds: Holds,
}

fn conduct(
    tabs: Tabs,
    fonts: Fonts,
    networking: Networking,
    inbox: &Receiver<Arrival>,
    tell: &dyn Fn(News) -> bool,
) -> Option<Network> {
    let mut conducting = Conducting {
        tabs,
        fonts,
        selected: None,
        viewport: None,
        visibility: Visibility::Visible,
        waiting: None,
        networking: Some(networking),
        in_flight: None,
        fetches: Answering::new(),
        holds: Holds::new(),
    };
    conducting.conduct(inbox, tell);
    // After the window has been told it closed: this waits for the exchange
    // in flight, if any (ADR 0041 § 5).
    conducting.networking.and_then(Networking::finish)
}

/// Tell the window `news`, in order. Whether to go on: not once every tab is
/// closed, and not when nobody heard, after closing every tab.
fn told(conducting: &mut Conducting, news: Vec<News>, tell: &dyn Fn(News) -> bool) -> bool {
    for news in news {
        let closed = news == News::Closed;
        let heard = tell(news);
        if closed {
            return false;
        }
        if !heard {
            conducting.close_everything();
            return false;
        }
    }
    true
}

/// What arrived, with every resize that has a later resize behind it
/// dropped.
///
/// Order is otherwise kept: an `Open` between two resizes still happens
/// between them, at the size the window was then — which the later resize
/// then corrects.
fn latest_size_only(arrived: Vec<Arrival>) -> Vec<Arrival> {
    let is_resize = |arrival: &Arrival| matches!(arrival, Arrival::Order(Order::Resize(_)));
    let last_resize = arrived.iter().rposition(is_resize);
    arrived
        .into_iter()
        .enumerate()
        .filter(|(at, arrival)| !is_resize(arrival) || Some(*at) == last_resize)
        .map(|(_, arrival)| arrival)
        .collect()
}

impl Conducting {
    /// Serve the window until it closes or goes.
    fn conduct(&mut self, inbox: &Receiver<Arrival>, tell: &dyn Fn(News) -> bool) {
        loop {
            // A turn waiting to be taken is not waited past; otherwise the
            // nearest bound is as long as anything is waited for.
            let until = if self.in_flight.is_none() && !self.fetches.is_empty() {
                Some(Instant::now())
            } else {
                self.holds.until()
            };
            // Every sender gone is the window gone without saying so.
            let Some(arrived) = arrivals(inbox, until) else {
                self.close_everything();
                return;
            };
            for arrival in latest_size_only(arrived) {
                let news = match arrival {
                    Arrival::Order(order) => self.carry_out(order),
                    Arrival::Exchanged(exchanged) => self.exchanged(*exchanged),
                    Arrival::NetworkGone => self.network_gone(),
                    Arrival::Abandoned => {
                        self.close_everything();
                        return;
                    }
                };
                if !told(self, news, tell) {
                    return;
                }
            }
            let news = self.bounds_passed(Instant::now());
            if !told(self, news, tell) {
                return;
            }
            let news = self.take_a_turn();
            if !told(self, news, tell) {
                return;
            }
        }
    }

    /// Do what one order says, and say what came of it.
    fn carry_out(&mut self, order: Order) -> Vec<News> {
        match order {
            Order::Open { url, page } => {
                let id = self.tabs.open(url);
                self.selected = Some(id);
                match (page, self.viewport) {
                    (Some(page), Some(viewport)) => self.load(id, *page, viewport),
                    (Some(page), None) => {
                        self.waiting = Some((id, *page));
                        Vec::new()
                    }
                    (None, _) => Vec::new(),
                }
            }
            Order::Resize(viewport) => {
                self.viewport = Some(viewport);
                if let Some((id, page)) = self.waiting.take() {
                    return self.load(id, page, viewport);
                }
                self.resize(viewport)
            }
            Order::Visibility(to) => {
                self.visibility = to;
                self.shown(to)
            }
            Order::CloseEverything => {
                self.close_everything();
                vec![News::Closed]
            }
        }
    }

    /// Tell the selected tab's page whether it can be seen, if it is showing
    /// anything, and paint it again: its listeners may have changed it.
    fn shown(&mut self, to: Visibility) -> Vec<News> {
        let Some(id) = self.selected else {
            return Vec::new();
        };
        if self.tabs.tab(id).and_then(Tab::document).is_none() {
            // Nothing loaded, so nobody to tell; the page it loads will start
            // as the window is.
            return Vec::new();
        }
        let told = self.tabs.visibility(id, to);
        self.fetches.take_from(&mut self.tabs, id);
        match told {
            Ok(FromRenderer::Delivered { .. }) => self.paint(id),
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }

    /// Load `page` into tab `id` at `viewport`, and paint it — once the
    /// sheets its answer asked for have answered, or the bound has passed.
    fn load(&mut self, id: TabId, mut page: Page, viewport: Size) -> Vec<News> {
        page.viewport = viewport;
        // The tab loading is the selected one, so it is as visible as the
        // window.
        page.visibility = self.visibility;
        let loaded = self.tabs.load(id, page, Cause::Person { tab: id });
        self.fetches.take_from(&mut self.tabs, id);
        if matches!(loaded, Ok(FromRenderer::Loaded { .. }))
            && let Some(document) = self.tabs.tab(id).and_then(Tab::document)
        {
            // The bound runs from now, the moment the load's answer arrived.
            let owed = self.fetches.sheets_waiting(id, document);
            self.holds.loaded(id, document, owed, Instant::now());
        }
        self.record_left();
        match loaded {
            Ok(FromRenderer::Loaded { wanted, .. }) => {
                if !wanted.is_empty() && self.fonts == Fonts::AsAsked {
                    // Sent, then drawn again: a `Resize` lays the document out
                    // again with what it now has, running none of its script.
                    if let Err(lost) = self.tabs.supply(id, &wanted) {
                        return vec![said_of(&self.tabs, id, &lost)];
                    }
                    return self.redraw(id, viewport);
                }
                self.paint(id)
            }
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }

    /// The selected tab at a new size, if it is showing anything.
    fn resize(&mut self, viewport: Size) -> Vec<News> {
        let Some(id) = self.selected else {
            return Vec::new();
        };
        if self.tabs.tab(id).and_then(Tab::document).is_none() {
            // Nothing loaded, so nothing to lay out again — and asking would
            // start a renderer to say so.
            return Vec::new();
        }
        self.redraw(id, viewport)
    }

    /// Lay a tab's document out again at `viewport` and paint it.
    fn redraw(&mut self, id: TabId, viewport: Size) -> Vec<News> {
        match self.tabs.ask(id, &ToRenderer::Resize(viewport)) {
            Ok(FromRenderer::Loaded { .. }) => self.paint(id),
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }

    /// Whether tab `id` is held for the document it is showing.
    fn is_held(&self, id: TabId) -> bool {
        self.holds
            .is_held(id, self.tabs.tab(id).and_then(Tab::document))
    }

    /// Paint a tab.
    ///
    /// A page laid out at no size is not painted: a window can be that small
    /// for a moment, and a renderer would only answer that nothing could be.
    /// Nor is a held one: the window shows what it was last sent until the
    /// hold ends.
    fn paint(&mut self, id: TabId) -> Vec<News> {
        if self
            .viewport
            .is_some_and(|size| size.width < 1.0 || size.height < 1.0)
            || self.is_held(id)
        {
            return Vec::new();
        }
        match self.tabs.paint(id) {
            Ok(FromRenderer::Painted(frame)) => vec![News::Painted(frame)],
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }

    /// Take the queue's next turn, unless an exchange is in flight: a
    /// refusal answered now and its line sent to be recorded, or an exchange
    /// handed to the network thread — or failed here, if that has gone.
    fn take_a_turn(&mut self) -> Vec<News> {
        if self.in_flight.is_some() {
            return Vec::new();
        }
        match self.fetches.next(&mut self.tabs) {
            None => Vec::new(),
            Some(Turn::Refused { answered, record }) => {
                self.send(Job::Record(record));
                self.answered(*answered)
            }
            Some(Turn::Exchange(exchange)) => {
                let handed = self.networking.as_ref().is_some_and(|networking| {
                    networking.send(Job::Make(Box::new(exchange.clone())))
                });
                if handed {
                    self.in_flight = Some(exchange);
                    return Vec::new();
                }
                let failed = self
                    .fetches
                    .exchanged(&mut self.tabs, exchange.failed(NETWORK_GONE));
                let mut news = self.answered(failed);
                news.extend(self.network_gone());
                news
            }
        }
    }

    /// Hand the network thread `job`, if it is still there. A line that
    /// cannot be recorded goes with the record it would have been written
    /// into, and the thread's going is said when it arrives.
    fn send(&self, job: Job) {
        if let Some(networking) = &self.networking {
            let _ = networking.send(job);
        }
    }

    /// Deliver what the exchange in flight came to.
    fn exchanged(&mut self, exchanged: Exchanged) -> Vec<News> {
        self.in_flight = None;
        let answered = self.fetches.exchanged(&mut self.tabs, exchanged);
        self.answered(answered)
    }

    /// The network thread has gone: say so, once, and answer the exchange it
    /// was making as failed. Everything after it is failed as its turn comes.
    fn network_gone(&mut self) -> Vec<News> {
        if self.networking.take().is_none() {
            return Vec::new();
        }
        let mut news = match self.in_flight.take() {
            Some(exchange) => {
                let failed = self
                    .fetches
                    .exchanged(&mut self.tabs, exchange.failed(NETWORK_GONE));
                self.answered(failed)
            }
            None => Vec::new(),
        };
        news.push(News::Said(NETWORK_GONE.to_owned()));
        news
    }

    /// Say what came of one answer for the selected tab: the page drawn
    /// again, and why it failed when it did. A sheet a load was held for is
    /// counted, and the last of them ends the hold; while a tab is still
    /// held, what would be said waits to be said with its first frame.
    fn answered(&mut self, answered: Answered) -> Vec<News> {
        let Answered {
            tab,
            sheet,
            delivered,
            said,
        } = answered;
        let released = sheet.and_then(|sheet| self.holds.answered(tab, sheet));
        if self.selected != Some(tab) {
            return Vec::new();
        }
        let mut news = match delivered {
            Ok(Some(FromRenderer::Delivered { .. })) => self.paint(tab),
            Ok(Some(other)) => vec![News::Said(unexpected(&other))],
            Ok(None) if released.is_some() => self.paint(tab),
            Ok(None) => Vec::new(),
            Err(lost) => vec![said_of(&self.tabs, tab, &lost)],
        };
        if self.is_held(tab) {
            self.holds.defer(tab, said);
            return news;
        }
        // After the paint, which clears what was said: the reason stays on
        // the page the person is looking at until it is drawn again.
        news.extend(released.unwrap_or_default().into_iter().map(News::Said));
        news.extend(said.into_iter().map(News::Said));
        news
    }

    /// Paint every held tab whose bound has passed by `now`, with what has
    /// arrived, and say it was shown before its style. A hold whose tab no
    /// longer shows its document is dropped first, painting nothing.
    fn bounds_passed(&mut self, now: Instant) -> Vec<News> {
        let tabs = &self.tabs;
        self.holds
            .keep_showing(|id| tabs.tab(id).and_then(Tab::document));
        let mut news = Vec::new();
        for (id, deferred) in self.holds.passed(now) {
            if self.selected != Some(id) {
                continue;
            }
            news.extend(self.paint(id));
            news.extend(deferred.into_iter().map(News::Said));
            news.push(News::Said(SHOWN_BEFORE_STYLE.to_owned()));
        }
        news
    }

    /// Close every tab, which stops every renderer (item 64's lifecycle),
    /// and make nothing more: what is still waiting is not made, and each
    /// beacon among it is recorded so (ADR 0040 § 3).
    fn close_everything(&mut self) {
        let open: Vec<TabId> = self.tabs.all().iter().map(Tab::id).collect();
        for id in open {
            self.tabs.close(id);
        }
        self.record_left();
        for record in self.fetches.closing(&mut self.tabs) {
            self.send(Job::Record(record));
        }
        self.holds.clear();
        self.selected = None;
        self.waiting = None;
    }

    /// Write every fetch a page asked for as it was left, and was refused,
    /// into the session's record (ADR 0039 § 4), and queue every one it
    /// asked to outlive it, to be made after it has gone and answered by
    /// nobody (ADR 0040 § 3).
    fn record_left(&mut self) {
        for leaving in self.tabs.left() {
            for refusal in leaving.refused {
                self.send(Job::Record(Record::Fetch(Box::new(refusal))));
            }
            if let Some(document) = leaving.document {
                self.fetches
                    .outlive(leaving.tab, document, leaving.outliving);
            }
        }
    }
}

/// What to tell a person when a tab could not be answered: the tab's own
/// sentence when it has one, which is the words `tab.rs` chose for it.
fn said_of(tabs: &Tabs, id: TabId, lost: &Lost) -> News {
    News::Said(
        tabs.tab(id)
            .and_then(alo_renderer::Tab::what_happened)
            .unwrap_or_else(|| lost.to_string()),
    )
}

/// An answer that was not the one asked for, in words.
fn unexpected(answer: &FromRenderer) -> String {
    match answer {
        FromRenderer::Failed(failure) => format!("this page could not be shown: {failure}"),
        _ => "this page's renderer answered something it was not asked".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: f32) -> Arrival {
        Arrival::Order(Order::Resize(Size::new(width, 10.0)))
    }

    fn widths(arrived: &[Arrival]) -> Vec<Option<f32>> {
        arrived
            .iter()
            .map(|arrival| match arrival {
                Arrival::Order(Order::Resize(size)) => Some(size.width),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_burst_of_resizes_is_the_last_of_them() {
        let kept = latest_size_only(vec![size(1.0), size(2.0), size(3.0)]);
        assert_eq!(widths(&kept), vec![Some(3.0)]);
    }

    #[test]
    fn what_is_not_a_resize_keeps_its_place() {
        let kept = latest_size_only(vec![size(1.0), Arrival::NetworkGone, size(2.0), size(3.0)]);
        assert_eq!(widths(&kept), vec![None, Some(3.0)]);
        assert!(matches!(kept.first(), Some(Arrival::NetworkGone)));
    }

    #[test]
    fn no_resize_at_all_changes_nothing() {
        let kept = latest_size_only(vec![Arrival::Order(Order::CloseEverything)]);
        assert_eq!(kept.len(), 1);
    }
}
