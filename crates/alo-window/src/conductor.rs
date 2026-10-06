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
use alo_renderer::{FromRenderer, Lost, Page, TabId, Tabs, ToRenderer};
use std::sync::mpsc::{Receiver, Sender, channel};
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
    /// Start one over `tabs`, telling the window what happens with `tell`.
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
        tell: impl Fn(News) -> bool + Send + 'static,
    ) -> std::io::Result<Self> {
        let (orders, inbox) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-conductor".to_owned())
            .spawn(move || conduct(tabs, fonts, &inbox, &tell))?;
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
}

fn conduct(tabs: Tabs, fonts: Fonts, inbox: &Receiver<Order>, tell: &dyn Fn(News) -> bool) {
    let mut conducting = Conducting {
        tabs,
        fonts,
        selected: None,
        viewport: None,
        waiting: None,
    };
    while let Ok(first) = inbox.recv() {
        let mut orders = vec![first];
        orders.extend(inbox.try_iter());
        for order in latest_size_only(orders) {
            for news in conducting.carry_out(order) {
                let closed = news == News::Closed;
                let heard = tell(news);
                if closed {
                    return;
                }
                if !heard {
                    conducting.close_everything();
                    return;
                }
            }
        }
    }
    // Every sender has gone, which is the window gone without saying so.
    conducting.close_everything();
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
        match self.tabs.load(id, page, Cause::Person { tab: id }) {
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
