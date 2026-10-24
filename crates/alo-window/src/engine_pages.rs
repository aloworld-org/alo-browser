/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! This engine's tabs, as the pages an agent reaches ([`crate::pages`]).
//!
//! Each is asked over the message boundary that already carries a verb to
//! its renderer (ADR 0005, ADR 0055 § 2), on the conductor's thread, which
//! holds them; nothing new crosses into a renderer.

use crate::pages::{Acted, Pages, RenderedBy, Unanswered};
use alo_agent::{Target, Verb};
use alo_net::cause::{ActionId, DocumentId};
use alo_renderer::{FromRenderer, Lost, Snapshot, Tab, TabId, Tabs};
use alo_url::Url;

impl Pages for Tabs {
    type Tab = TabId;

    /// Nothing: a page here goes elsewhere only as its renderer answers,
    /// and every answer is taken as it comes.
    fn catch_up(&mut self) {}

    fn shown(&self) -> Vec<(TabId, Url)> {
        self.all()
            .iter()
            .map(|tab| (tab.id(), tab.address().unwrap_or_else(|| tab.url()).clone()))
            .collect()
    }

    fn rendered_by(&self, _: TabId) -> RenderedBy {
        RenderedBy::ThisEngine
    }

    fn document(&self, id: TabId) -> Option<DocumentId> {
        self.tab(id).and_then(Tab::document)
    }

    fn read(&mut self, id: TabId) -> Result<Box<Snapshot>, Unanswered> {
        match Tabs::read(self, id) {
            Ok(FromRenderer::Tree(snapshot)) => Ok(snapshot),
            answered => Err(gone(self, id, answered)),
        }
    }

    fn act(
        &mut self,
        id: TabId,
        target: &Target,
        verb: &Verb,
    ) -> Result<(ActionId, Acted), Unanswered> {
        let (action, answered) = Tabs::act(self, id, target.clone(), verb.clone())
            .map_err(|lost| gone(self, id, Err(lost)))?;
        match answered {
            FromRenderer::Acted {
                outcome, issues, ..
            } => Ok((action, Acted::Done { outcome, issues })),
            FromRenderer::Refused(refusal) => Ok((action, Acted::Refused(refusal))),
            other => Err(gone(self, id, Ok(other))),
        }
    }
}

/// Why tab `id` could not answer, for an answer that was not the one asked
/// for: in the tab's own words when it has them.
fn gone(tabs: &Tabs, id: TabId, answered: Result<FromRenderer, Lost>) -> Unanswered {
    Unanswered::Gone(match answered {
        Ok(FromRenderer::Failed(failure)) => failure.to_string(),
        Ok(_) => "its renderer answered something it was not asked".to_owned(),
        Err(lost) => tabs
            .tab(id)
            .and_then(Tab::what_happened)
            .unwrap_or_else(|| lost.to_string()),
    })
}
