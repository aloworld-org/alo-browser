/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the window is called (queue item 958, ADR 0112 § 1): the open
//! card's page's title, as its Chromium last told it
//! ([`Went::Titled`]), or **alo** with no card ([`crate::window_title`]).
//!
//! A title is the live card's: a card opening, closed, failed or put away
//! — a new chat, a search after it — leaves no title of its page behind,
//! so the window is never called what a page no longer shown said.

use super::ConversationWindow;
use crate::conversation_chrome::Went;
use crate::window_title;

/// The live card's page's title, and what the window was last asked to be
/// called.
#[derive(Debug, Default)]
pub(super) struct Titled {
    /// What the live card's page is called, as its Chromium last said.
    page: Option<String>,
    /// What the window was last asked to be called, if it was.
    told: Option<String>,
}

impl ConversationWindow {
    /// What a card's Chromium told of its page, as far as the window's title
    /// goes: its title, or that the card it was of has gone.
    pub(super) fn titled(&mut self, went: &Went) {
        match went {
            Went::Titled(title) => self.title.page = Some(title.clone()),
            Went::Opening(_) | Went::Unopened { .. } | Went::Closed(_) | Went::Shut(_) => {
                self.title.page = None;
            }
            _ => {}
        }
    }

    /// What the window is called now: the live card's page's title, or
    /// **alo**.
    pub fn title(&self) -> String {
        let page = self.conversation.card().and(self.title.page.as_deref());
        window_title::of(page)
    }

    /// What the event loop is to call the window, once each time that
    /// changes — the first time asked included.
    pub fn take_title(&mut self) -> Option<String> {
        let now = self.title();
        if self.title.told.as_deref() == Some(now.as_str()) {
            return None;
        }
        self.title.told = Some(now.clone());
        Some(now)
    }
}
