/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's main button, on the selected tab's page (ADR 0024 § 5, queue
//! item 535).
//!
//! The window has decided from its own composition that the point is over
//! the page and sent it in the page's CSS pixels ([`crate::aim`]). The
//! conductor sends it to the selected tab's renderer as a person's pointer —
//! the one message that carries a point, sent for a person and never for
//! an agent — whose hit test finds the element, and paints the tab again,
//! since what the page heard may have changed it.
//!
//! # A press, and its release
//!
//! A press goes to the selected tab, if it is showing a document and not
//! held for its style: a held tab shows the last frame of something else,
//! and a person pressing it is pressing that. Its release goes to **the tab
//! the press went to**, wherever the pointer has got to — and to no tab, if
//! that one is no longer selected, or there was no press.
//!
//! # A link followed
//!
//! Decided as the answer passes, as the person's (`alo-renderer`'s
//! `Tabs::point`), and said to the person: going there is session history's
//! (item 85), and a link a person clicked that silently went nowhere would
//! be a page that looks broken.

use super::{Conducting, said_of, unexpected};
use crate::message::News;
use alo_renderer::navigate::Decided;
use alo_renderer::pointer::{Pointer, Went};
use alo_renderer::{FromRenderer, Tab, TabId};

/// What is said after the browser process decided a person's click goes
/// somewhere it does not yet go.
pub const NOT_GONE: &str = "and going to another page is not built yet (queue item 85)";

impl Conducting {
    /// Send a person's main button to the page it is aimed at, and paint
    /// the page again.
    pub(super) fn pointed(&mut self, pointer: Pointer) -> Vec<News> {
        let Some(id) = self.aimed(pointer.went) else {
            return Vec::new();
        };
        let answered = self.tabs.point(id, pointer);
        self.fetches.take_from(&mut self.tabs, id);
        let mut news = match answered {
            Ok(FromRenderer::Delivered { .. }) => self.paint(id),
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        };
        // After the paint, which clears what was said.
        if let Some(decided) = self.tabs.navigation(id) {
            news.push(News::Said(going(&decided)));
        }
        news
    }

    /// The tab a button that `went` goes to, if any.
    fn aimed(&mut self, went: Went) -> Option<TabId> {
        let shows = |conducting: &Self, id: TabId| {
            conducting.tabs.selected() == Some(id)
                && conducting.tabs.tab(id).and_then(Tab::document).is_some()
        };
        match went {
            Went::Down => {
                self.pressed = self
                    .tabs
                    .selected()
                    .filter(|id| shows(self, *id) && !self.is_held(*id));
                self.pressed
            }
            Went::Up => self.pressed.take().filter(|id| shows(self, *id)),
        }
    }
}

/// What a person is told of where their click was decided to go.
fn going(decided: &Decided) -> String {
    match decided {
        Decided::Go(navigation) => format!("this link goes to {}, {NOT_GONE}", navigation.url),
        Decided::Refused(refusal) => refusal.to_string(),
    }
}
