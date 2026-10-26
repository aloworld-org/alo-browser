/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab in Chrome, as the pages an agent reaches ([`crate::pages`], queue
//! item 630).
//!
//! One tab, because a [`Tab`] borrows its [`crate::chrome::Chrome`] whole:
//! one Chrome holds one tab open at a time, so the tab needs no name of its
//! own beyond [`TheTab`], and is named to an agent by its page's address as
//! every tab is.
//!
//! # What a verb comes to
//!
//! A verb is decided on Chrome's page by the same rules as on this engine's
//! (`chrome_act`), so a refusal is one of `verb.rs`'s and crosses as it
//! does. What only Chrome can come to is said as its own kind:
//!
//! - Chrome could not be asked, or its tree could not be read: the tab
//!   could not answer, [`Unanswered::Gone`].
//! - The verb was decided and Chrome did not carry it out — the node drawn
//!   nowhere, its middle under something else, the page gone elsewhere
//!   while it was decided, an outcome this path does not carry out:
//!   [`Unanswered::NotCarriedOut`], because *nothing was done* is the truth
//!   and *the tab is gone* would not be.
//!
//! # What the page's script said
//!
//! Heard from Chrome while the verb is carried out, and answered as its
//! issues in the engine's words ([`crate::chrome_said`], queue item 639):
//! what the page wrote to its console and what it threw and nobody caught,
//! in the tab's own session. A verb refused before it was carried out
//! answers none, as the engine's does.

use crate::chrome_act::Unacted;
use crate::chrome_tab::Tab;
use crate::pages::{Acted, Pages, RenderedBy, Unanswered};
use alo_agent::{Target, Verb};
use alo_net::cause::{ActionId, DocumentId};
use alo_renderer::Snapshot;
use alo_url::Url;

/// The one tab of a Chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheTab;

/// A tab in Chrome, and which Chrome it is in.
pub struct ChromePages<'c> {
    tab: Tab<'c>,
    rendered_by: RenderedBy,
}

impl<'c> ChromePages<'c> {
    /// `tab`, in the Chrome that says it is `product` — as
    /// `Chrome/155.0.8059.86`, from [`crate::chrome::Chrome::version`].
    pub fn of(tab: Tab<'c>, product: String) -> Self {
        Self {
            tab,
            rendered_by: RenderedBy::Chrome { product },
        }
    }

    /// The tab.
    pub fn tab(&self) -> &Tab<'c> {
        &self.tab
    }

    /// The tab, to be read or closed as a window — never to carry out a
    /// verb, which goes through [`Pages`].
    pub fn tab_mut(&mut self) -> &mut Tab<'c> {
        &mut self.tab
    }
}

impl Pages for ChromePages<'_> {
    type Tab = TheTab;

    fn catch_up(&mut self) {
        // Chrome gone, or its pipe unreadable, is said by the next thing
        // asked of the tab, which fails for it.
        let _ = self.tab.catch_up();
    }

    fn shown(&self) -> Vec<(TheTab, Url)> {
        self.tab
            .address()
            .map(|address| (TheTab, address.clone()))
            .into_iter()
            .collect()
    }

    fn rendered_by(&self, _: TheTab) -> RenderedBy {
        self.rendered_by.clone()
    }

    fn document(&self, _: TheTab) -> Option<DocumentId> {
        self.tab.document()
    }

    fn read(&mut self, _: TheTab) -> Result<Box<Snapshot>, Unanswered> {
        self.tab
            .read()
            .map(Box::new)
            .map_err(|why| Unanswered::Gone(why.to_string()))
    }

    fn act(
        &mut self,
        _: TheTab,
        target: &Target,
        verb: &Verb,
    ) -> Result<(ActionId, Acted), Unanswered> {
        let (action, acted) = self.tab.act_as_action(target, verb);
        match acted {
            Ok((outcome, issues)) => Ok((action, Acted::Done { outcome, issues })),
            Err(Unacted::Refused(refusal)) => Ok((action, Acted::Refused(refusal))),
            Err(why @ (Unacted::Unread(_) | Unacted::Failed(_))) => {
                Err(Unanswered::Gone(why.to_string()))
            }
            Err(
                why @ (Unacted::Undrawn { .. }
                | Unacted::Covered { .. }
                | Unacted::Moved { .. }
                | Unacted::Untypable { .. }
                | Unacted::Closed { .. }
                | Unacted::Uncarried { .. }),
            ) => Err(Unanswered::NotCarriedOut(why.to_string())),
        }
    }
}
