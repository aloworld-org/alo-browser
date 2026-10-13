/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Choosing a tab: what a person's keys ask of the tabs, and what the pages
//! in them are told (queue item 529).
//!
//! # A tab is chosen four ways
//!
//! By opening one, which selects it (ADR 0024 § 6); by the next and previous
//! keys; and by closing the selected tab, which [`alo_renderer::Tabs::close`]
//! answers by selecting its neighbour. Each ends the same way: the window is
//! told [`News::Chosen`] with the tab's last frame, so nothing of the tab left
//! is shown under the one chosen (ADR 0024 § 2), and the tab chosen is laid
//! out at the window's size and painted — it may have been behind while the
//! window changed size.
//!
//! # The tab left is told before the tab chosen
//!
//! ADR 0039 § 1: *"Selecting a tab tells the one left `hidden` before it
//! tells the one chosen `visible`, so no instant has two visible tabs in one
//! window."* Both only when the window can be seen: under a covered window
//! every tab is already hidden, and the window's being seen again tells the
//! selected one. A tab closed is not told anything here, because closing it
//! left its page, and leaving a page is `hidden` among its steps (§ 2).
//!
//! What either page's listeners ask to fetch is the document's, made as a
//! delivery's is. The tab left is not painted and nothing about it is said:
//! it is not what the person is looking at.
//!
//! # Closing the last tab closes the window
//!
//! § 6. The conductor closes everything, as when the window itself is
//! closed, and says [`News::Closed`].

use super::{Conducting, said_of, unexpected};
use crate::message::{News, TabOrder};
use alo_renderer::{FromRenderer, Tab, TabId, Visibility};
use alo_url::Url;

impl Conducting {
    /// Do what a person's key asked of the tabs.
    pub(super) fn tab_order(&mut self, order: TabOrder) -> Vec<News> {
        match order {
            TabOrder::New => {
                // Empty until the address bar exists (item 119): nothing is
                // loaded, so no renderer is started for it.
                let id = self.tabs.open(Url::about_blank());
                self.choose(id)
            }
            TabOrder::CloseSelected => self.close_selected(),
            TabOrder::Next => self.tabs.next().map_or_else(Vec::new, |id| self.choose(id)),
            TabOrder::Previous => self
                .tabs
                .previous()
                .map_or_else(Vec::new, |id| self.choose(id)),
        }
    }

    /// Select tab `id`, telling the tab left that it is hidden before the tab
    /// chosen that it is visible, and show it. Nothing, when it is already
    /// the selected tab or is not open.
    pub(super) fn choose(&mut self, id: TabId) -> Vec<News> {
        let left = self.tabs.selected();
        if left == Some(id) || self.tabs.tab(id).is_none() {
            return Vec::new();
        }
        if let Some(left) = left {
            self.hide(left);
        }
        self.tabs.select(id);
        self.show_chosen(id)
    }

    /// Close the selected tab, leaving its page, and show the tab selected in
    /// its place — or close the window, when it was the last.
    fn close_selected(&mut self) -> Vec<News> {
        let Some(id) = self.tabs.selected() else {
            return Vec::new();
        };
        if self.tabs.len() == 1 {
            self.close_everything();
            return vec![News::Closed];
        }
        self.tabs.close(id);
        if self
            .waiting
            .as_ref()
            .is_some_and(|(waiting, _)| *waiting == id)
        {
            self.waiting = None;
        }
        self.record_left();
        match self.tabs.selected() {
            Some(chosen) => self.show_chosen(chosen),
            None => Vec::new(),
        }
    }

    /// Tell the page in tab `left` that it is hidden, if the window can be
    /// seen and the tab holds a page; take what its listeners asked to fetch.
    ///
    /// Nothing is said about how that went: the tab is no longer shown. A
    /// renderer that has gone is recorded on the tab by the asking, and said
    /// when the tab is chosen again.
    fn hide(&mut self, left: TabId) {
        if self.visibility != Visibility::Visible
            || self.tabs.tab(left).and_then(Tab::document).is_none()
        {
            return;
        }
        let _ = self.tabs.visibility(left, Visibility::Hidden);
        self.take_asks(left);
    }

    /// Show tab `id`, the one now selected: its last frame at once, then —
    /// if it holds a page — the page told it is visible, when the window can
    /// be seen, and laid out at the window's size and painted.
    fn show_chosen(&mut self, id: TabId) -> Vec<News> {
        let frame = self.tabs.tab(id).and_then(Tab::frame).cloned();
        let mut news = vec![News::Chosen(frame)];
        if self.tabs.tab(id).and_then(Tab::document).is_none() {
            return news;
        }
        if self.visibility == Visibility::Visible {
            let told = self.tabs.visibility(id, Visibility::Visible);
            self.take_asks(id);
            match told {
                Ok(FromRenderer::Delivered { .. }) => {}
                Ok(other) => {
                    news.push(News::Said(unexpected(&other)));
                    return news;
                }
                Err(lost) => {
                    news.push(said_of(&self.tabs, id, &lost));
                    return news;
                }
            }
        }
        match self.viewport {
            Some(viewport) => news.extend(self.redraw(id, viewport)),
            None => news.extend(self.paint(id)),
        }
        news
    }
}
