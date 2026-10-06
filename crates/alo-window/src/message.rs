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

use alo_layout::Size;
use alo_renderer::{Frame, Page};
use alo_url::Url;

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
    /// The window was closed: close every tab, and say [`News::Closed`] when
    /// they are.
    CloseEverything,
}

/// What the conductor tells the event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum News {
    /// The selected tab painted this.
    Painted(Frame),
    /// The selected tab has something to tell a person, in words — that its
    /// renderer is gone, or that what it was given could not be shown.
    Said(String),
    /// Every tab is closed, and every renderer under them stopped.
    Closed,
}
