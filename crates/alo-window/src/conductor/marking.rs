/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the box an agent's verb acted on is, for the ring the window draws
//! around it (queue item 624).
//!
//! # Asked after the verb, of the frame just painted
//!
//! The verb is decided, carried out, recorded and answered first, and only
//! then is anything about it read for a person: so the ring can change
//! nothing about what the verb did, and no verb gains a coordinate (ADR
//! 0002). The box is the one the verb's outcome names — the renderer's own
//! answer, not the caller's description of it — and its rectangles are read
//! from the page's tree straight after the tab is painted again, with
//! nothing sent to its renderer between, so they are the layout of the frame
//! the window is about to show.
//!
//! Nothing is marked when the verb took the tab to another document — a link
//! followed in place — because the box is not in the page now shown, nor
//! when the box is no longer in the tree, nor when the tab did not paint.
//! A tree that cannot be read is said as a paint that fails is.

use super::{Conducting, said_of, unexpected};
use crate::mark::marked;
use crate::message::News;
use alo_box::BoxId;
use alo_net::cause::DocumentId;
use alo_renderer::{FromRenderer, Screen, Tab, TabId};

impl Conducting {
    /// The selected tab and the document it holds, if any — taken before an
    /// agent's request is served, so that what a verb did can be marked only
    /// on the page it was done to.
    pub(super) fn selected_document(&self) -> Option<(TabId, Option<DocumentId>)> {
        let id = self.tabs.selected()?;
        Some((id, self.tabs.tab(id).and_then(Tab::document)))
    }

    /// After an agent's verb on box `node` in tab `id`, which held document
    /// `before` when the verb was asked and has just told the window
    /// `painted`: the box's rectangles over that frame, if there is anything
    /// to ring.
    pub(super) fn mark(
        &mut self,
        id: TabId,
        node: BoxId,
        before: Option<DocumentId>,
        painted: &[News],
    ) -> Vec<News> {
        let painted = painted.iter().any(|news| matches!(news, News::Painted(_)));
        if !painted || before.is_none() || self.tabs.tab(id).and_then(Tab::document) != before {
            return Vec::new();
        }
        let scale = self.screen.map_or(1.0, Screen::scale);
        match self.tabs.read(id) {
            Ok(FromRenderer::Tree(snapshot)) => snapshot
                .nodes()
                .into_iter()
                .find(|each| each.id == node)
                .map(|box_| marked(&box_.rects, scale))
                .filter(|rings| !rings.is_empty())
                .map(News::Marked)
                .into_iter()
                .collect(),
            Ok(other) => vec![News::Said(unexpected(&other))],
            Err(lost) => vec![said_of(&self.tabs, id, &lost)],
        }
    }
}
