/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the event loop and the conductor say to each other.
//!
//! ADR 0024 § 2: the event loop never calls a renderer. It sends the
//! conductor what happened as an [`Order`] and carries on; the conductor,
//! which owns the tabs and so the renderers, does the slow part on a thread of
//! its own and posts what it learnt back as [`News`]. Neither ever waits for an
//! answer from the other, which is what keeps one silent site from freezing
//! the window.
//!
//! An agent outside the browser reaches the conductor the same way, as an
//! [`Order::Agent`] sent by the thread that holds its connection
//! ([`crate::agents`]), and is answered on a channel of its own — never in
//! [`News`], which is the window's.

use crate::agent_record::Caller;
use crate::asking::{Answer, Ask, Refused};
use alo_layout::Size;
use alo_renderer::pointer::Pointer;
use alo_renderer::{Frame, Page, Visibility};
use alo_url::Url;
use std::sync::mpsc::Sender;

/// What the event loop asks of the conductor.
#[derive(Debug, Clone)]
pub enum Order {
    /// Open a tab at `url` and select it, loading `page` into it if there is
    /// one. A tab opened with no page is `about:blank` until the address bar
    /// exists (ADR 0024 § 6, item 119).
    Open {
        /// Where the tab is.
        url: Url,
        /// What to load, stated by the browser process — a page read from a
        /// file a person named, never one a renderer described.
        page: Option<Box<Page>>,
    },
    /// The window is now this many CSS pixels.
    Resize(Size),
    /// The window can now be seen, or can no longer — covered or minimised
    /// (ADR 0039 § 1). What the selected tab's page is told, and what the
    /// next page loaded starts as.
    Visibility(Visibility),
    /// A person asked for a tab with one of the browser's own keys (queue item
    /// 529, ADR 0024 § 5): read by the window before any page could see them.
    Tabs(TabOrder),
    /// A person's main button went down or up over the selected tab's page,
    /// at a point in its CSS pixels, as the window decided from its own
    /// composition (ADR 0024 § 5, queue item 535). Only the window sends
    /// one, for a person: nothing an agent sends becomes a point.
    Pointer(Pointer),
    /// The window was closed: close every tab, and say [`News::Closed`] when
    /// they are.
    CloseEverything,
    /// An agent outside the browser asked something (ADR 0055): answer it on
    /// `reply`, and record it, whatever it came to.
    Agent {
        /// Which connection asked.
        caller: Caller,
        /// What it asked, or why what it sent was not a request.
        ask: Result<Ask, Refused>,
        /// Where the answer goes.
        reply: Sender<Answer>,
    },
}

/// What a person asked of the tabs (ADR 0024 §§ 5, 6).
///
/// Each is about the selected tab or the order the tabs are in, never about a
/// tab by name: a person's key acts on what they are looking at, so nothing
/// that carries one can single out a tab the person cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabOrder {
    /// Open a new tab, empty (`about:blank`, until the address bar exists —
    /// item 119), and select it.
    New,
    /// Close the selected tab. Closing the last closes the window (§ 6).
    CloseSelected,
    /// Select the tab after the selected one, or the first after the last.
    Next,
    /// Select the tab before the selected one, or the last before the first.
    Previous,
}

/// What the conductor tells the event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum News {
    /// The selected tab painted this.
    Painted(Frame),
    /// Another tab is selected, and replaces the one shown: this is the last
    /// frame it painted, if it has painted one, and nothing is said about it
    /// until it is (queue item 529). What was shown of the tab left is not
    /// shown under the one chosen.
    Chosen(Option<Frame>),
    /// The tab strip painted this (ADR 0024 § 4, queue item 528).
    Strip(Frame),
    /// The tab strip has something to tell a person, in words — that its
    /// renderer is gone, or that it could not be drawn. It keeps the last
    /// frame it painted.
    StripSaid(String),
    /// The selected tab has something to tell a person, in words — that its
    /// renderer is gone, or that what it was given could not be shown.
    Said(String),
    /// Every tab is closed, and every renderer under them stopped.
    Closed,
}

impl News {
    /// Whether this is about the tab strip rather than the selected tab: for
    /// whoever listens to only one of them.
    pub fn is_strip(&self) -> bool {
        matches!(self, News::Strip(_) | News::StripSaid(_))
    }
}
