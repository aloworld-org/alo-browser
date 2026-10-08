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
//! Here, because this thread holds the tabs and the session's [`Network`]
//! (queue item 338, ADR 0032 § 3). Every answer that can carry a page's
//! fetches puts them in an [`Answering`], and between looking at its orders
//! the conductor makes **one** of them and delivers its answer: a request
//! waits on its server, and a page that fetches for ever must cost its own
//! tab's answers rather than the window's ability to close. After a delivery
//! the selected tab is painted again, since the reactions may have changed
//! it, and the reason a fetch failed — which the page is never told — is
//! said to the person.
//!
//! A page's linked style sheets are made the same way, in the same queue and
//! ahead of the fetches its answer asked for (queue item 348, ADR 0035), and
//! the page is painted again with each sheet that arrives. Its first frame is
//! still painted at once, before its sheets have answered: holding it back,
//! within a bound of its own, is ADR 0035 § 5's and queue item 351's.
//!
//! # Why a burst of resizes is one resize
//!
//! Dragging a window's corner sends a resize for nearly every pixel it
//! crosses, and a renderer asked to lay out and paint each one would still be
//! painting sizes the window left a second ago. Every order waiting when the
//! conductor looks is taken at once, and a resize with a later resize behind
//! it is dropped: the size that matters is the one the window is now.

use crate::message::{News, Order};
use alo_layout::Size;
use alo_net::Cause;
use alo_renderer::fetch_answering::{Answered, Answering};
use alo_renderer::fetch_make::Network;
use alo_renderer::{FromRenderer, Lost, Page, TabId, Tabs, ToRenderer};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;

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
    orders: Sender<Order>,
    thread: JoinHandle<()>,
}

impl Conductor {
    /// Start one over `tabs`, making their pages' fetches through `network`,
    /// telling the window what happens with `tell`.
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
        let (orders, inbox) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-conductor".to_owned())
            .spawn(move || conduct(tabs, fonts, network, &inbox, &tell))?;
        Ok(Self { orders, thread })
    }

    /// Where to send it orders.
    pub fn orders(&self) -> Sender<Order> {
        self.orders.clone()
    }

    /// Wait for it to finish, which it does after [`Order::CloseEverything`]
    /// or when every sender of orders has gone.
    ///
    /// Whether it finished rather than panicking — which the lints make
    /// unreachable, and which is said rather than assumed.
    pub fn finish(self) -> bool {
        drop(self.orders);
        self.thread.join().is_ok()
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
    /// A tab opened before the window had a size, with its page — loaded when
    /// the size arrives, because a page is laid out at the window's size and
    /// there was none to lay it out at.
    waiting: Option<(TabId, Page)>,
    /// What every request is made through, for the session.
    network: Network,
    /// The pages' fetches and linked style sheets, decided and waiting to be
    /// made.
    fetches: Answering,
}

fn conduct(
    tabs: Tabs,
    fonts: Fonts,
    network: Network,
    inbox: &Receiver<Order>,
    tell: &dyn Fn(News) -> bool,
) {
    let mut conducting = Conducting {
        tabs,
        fonts,
        selected: None,
        viewport: None,
        waiting: None,
        network,
        fetches: Answering::new(),
    };
    // Every sender gone is the window gone without saying so.
    while let Some(orders) = next_orders(inbox, conducting.fetches.is_empty()) {
        for order in latest_size_only(orders) {
            let news = conducting.carry_out(order);
            if !told(&mut conducting, news, tell) {
                return;
            }
        }
        let news = conducting.answer_a_fetch();
        if !told(&mut conducting, news, tell) {
            return;
        }
    }
    conducting.close_everything();
}

/// Every order waiting — after waiting for one when there is nothing else
/// to do, and without waiting when a fetch is. [`None`] when every sender
/// has gone.
fn next_orders(inbox: &Receiver<Order>, idle: bool) -> Option<Vec<Order>> {
    let mut orders = Vec::new();
    if idle {
        orders.push(inbox.recv().ok()?);
    }
    loop {
        match inbox.try_recv() {
            Ok(order) => orders.push(order),
            Err(TryRecvError::Disconnected) if orders.is_empty() => return None,
            // Nothing more for now — or the last sender has gone, and what
            // arrived before it went is still carried out; the next look
            // finds nobody.
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Some(orders),
        }
    }
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

/// The orders, with every resize that has a later resize behind it dropped.
///
/// Order is otherwise kept: an `Open` between two resizes still happens
/// between them, at the size the window was then — which the later resize
/// then corrects.
fn latest_size_only(orders: Vec<Order>) -> Vec<Order> {
    let last_resize = orders
        .iter()
        .rposition(|order| matches!(order, Order::Resize(_)));
    orders
        .into_iter()
        .enumerate()
        .filter(|(at, order)| !matches!(order, Order::Resize(_)) || Some(*at) == last_resize)
        .map(|(_, order)| order)
        .collect()
}

impl Conducting {
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
            Order::CloseEverything => {
                self.close_everything();
                vec![News::Closed]
            }
        }
    }

    /// Load `page` into tab `id` at `viewport`, and paint it.
    fn load(&mut self, id: TabId, mut page: Page, viewport: Size) -> Vec<News> {
        page.viewport = viewport;
        let loaded = self.tabs.load(id, page, Cause::Person { tab: id });
        self.fetches.take_from(&mut self.tabs, id);
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
        if self
            .tabs
            .tab(id)
            .and_then(alo_renderer::Tab::document)
            .is_none()
        {
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

    /// Paint a tab.
    ///
    /// A page laid out at no size is not painted: a window can be that small
    /// for a moment, and a renderer would only answer that nothing could be.
    fn paint(&mut self, id: TabId) -> Vec<News> {
        if self
            .viewport
            .is_some_and(|size| size.width < 1.0 || size.height < 1.0)
        {
            return Vec::new();
        }
        match self.tabs.paint(id) {
            Ok(FromRenderer::Painted(frame)) => vec![News::Painted(frame)],
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }

    /// Make the oldest fetch or style sheet a page is waiting on, deliver its
    /// answer, and say what came of it for the selected tab: the page drawn
    /// again, and why it failed when it did.
    fn answer_a_fetch(&mut self) -> Vec<News> {
        let Some(Answered {
            tab,
            delivered,
            said,
        }) = self.fetches.answer_next(&mut self.tabs, &mut self.network)
        else {
            return Vec::new();
        };
        if self.selected != Some(tab) {
            return Vec::new();
        }
        let mut news = match delivered {
            Ok(Some(FromRenderer::Delivered { .. })) => self.paint(tab),
            Ok(Some(other)) => vec![News::Said(unexpected(&other))],
            Ok(None) => Vec::new(),
            Err(lost) => vec![said_of(&self.tabs, tab, &lost)],
        };
        // After the paint, which clears what was said: the reason stays on
        // the page the person is looking at until it is drawn again.
        news.extend(said.into_iter().map(News::Said));
        news
    }

    /// Close every tab, which stops every renderer (item 64's lifecycle).
    fn close_everything(&mut self) {
        let open: Vec<TabId> = self.tabs.all().iter().map(alo_renderer::Tab::id).collect();
        for id in open {
            self.tabs.close(id);
        }
        self.selected = None;
        self.waiting = None;
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

    fn size(width: f32) -> Order {
        Order::Resize(Size::new(width, 10.0))
    }

    fn widths(orders: &[Order]) -> Vec<Option<f32>> {
        orders
            .iter()
            .map(|order| match order {
                Order::Resize(size) => Some(size.width),
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
        let kept = latest_size_only(vec![
            size(1.0),
            Order::CloseEverything,
            size(2.0),
            size(3.0),
        ]);
        assert_eq!(widths(&kept), vec![None, Some(3.0)]);
        assert!(matches!(kept.first(), Some(Order::CloseEverything)));
    }

    #[test]
    fn no_resize_at_all_changes_nothing() {
        let kept = latest_size_only(vec![Order::CloseEverything]);
        assert_eq!(kept.len(), 1);
    }
}
