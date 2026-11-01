/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a tab's page has committed, whether it has loaded, and whether
//! it is loading, as Chrome tells of its own frame's stages
//! (`Page.lifecycleEvent`, queue items 749 and 751) and of its navigations'
//! starts and stops (queue item 770).
//!
//! # The document that counts is the newest
//!
//! A tab's page is the document its own frame holds. `Page.navigate` names
//! the loader of the navigation it began; a page may replace itself before
//! it loads — a script's `location.replace`, as `www.reddit.com`'s challenge
//! does — and the first loader then never loads (queue item 749). So what
//! counts is the newest document the tab's own frame **began** (its `init`
//! stage), or, before any began, the loader `Page.navigate` named.
//!
//! # Committed, and loaded
//!
//! The navigation has **committed** once Chrome has answered `Page.navigate`
//! with a loader and the tab's own frame has begun a document since it was
//! asked: the page is the new one from then, and may draw (queue item 751).
//! It has **loaded** once that newest document's `load` stage has been told
//! of. A document later begun by the frame — a link followed, a script's
//! `location` — is loading again until its own `load`.
//!
//! # Loading, from a navigation's start
//!
//! The page is **loading** from the moment its own frame begins a navigation
//! to another document — a link pressed, a script's `location`, a step back
//! or forward, `Page.navigate` (`Page.frameStartedNavigating`) — until Chrome
//! says the frame has stopped loading (`Page.frameStoppedLoading`): as
//! Chrome's own tab turns its spinner (queue item 770). So a slow server's
//! whole wait, before anything commits, is loading; and a navigation that
//! never commits — a `204`, a download, one stopped, a `mailto:` handed
//! elsewhere — stops being loading when Chrome says so, the page it left
//! still shown. One that fails commits Chrome's error page, and is loading
//! until that has loaded. Measured against Chrome 156 and CEF 156 over the
//! pipe, every request paused as the tab pauses them: the start is told of
//! for each of those before its request is made, and the stop after the
//! newest document's `load`, or at once for a navigation that ends without
//! one. A page stopped after its commit and before its `load` is told
//! stopped with no `load` ever told, and Chrome's spinner stops then — so
//! the stop, not the `load`, ends it.
//!
//! A move within the document — a fragment, an entry the page pushed — is
//! not loading: Chrome turns no spinner for it, and its stop, told a few
//! milliseconds later, would only make the card's word blink. Until Chrome
//! has told of either since the tab was sent somewhere, the page is loading
//! while its newest document has not loaded, as before.
//!
//! Only the tab's own frame is read: a frame inside the page has loaders of
//! its own, and its `load` is not the page's. Only the last document begun
//! and the last loaded are kept, so a page that goes somewhere a million
//! times costs nothing more.

use crate::chrome_protocol::{Stage, Started};

/// What Chrome has told of the tab's own frame's documents since the tab
/// was last sent somewhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loading {
    /// The loader `Page.navigate` answered with, once it has.
    navigated: Option<String>,
    /// The loader of the newest document the tab's own frame began since
    /// it was sent somewhere.
    newest: Option<String>,
    /// The loader of the last of the tab's own frame's documents to load.
    loaded: Option<String>,
    /// Whether Chrome last told of the tab's own frame beginning a
    /// navigation to another document (`true`) or stopping loading
    /// (`false`) — or [`None`], of neither since it was sent somewhere.
    busy: Option<bool>,
}

impl Loading {
    /// The tab is being sent somewhere: nothing told of before counts.
    pub fn navigating(&mut self) {
        *self = Self::default();
    }

    /// Chrome answered `Page.navigate` with `loader`.
    pub fn navigated(&mut self, loader: String) {
        self.navigated = Some(loader);
    }

    /// Chrome told of `stage` in the tab whose own frame is `frame`.
    pub fn told(&mut self, frame: &str, stage: Stage) {
        if stage.frame != frame {
            return;
        }
        match stage.name.as_str() {
            "init" => self.newest = Some(stage.loader),
            "load" => self.loaded = Some(stage.loader),
            _ => {}
        }
    }

    /// Chrome told of the tab whose own frame is `frame` beginning `started`.
    pub fn started(&mut self, frame: &str, started: &Started) {
        if started.frame == frame && !started.is_within_the_document() {
            self.busy = Some(true);
        }
    }

    /// Chrome told of `stopped`, a frame of the tab whose own frame is
    /// `frame`, having stopped loading.
    pub fn stopped(&mut self, frame: &str, stopped: &str) {
        if stopped == frame {
            self.busy = Some(false);
        }
    }

    /// Whether the page is loading: from the start of its own frame's
    /// navigation to another document until Chrome says it stopped — or,
    /// before Chrome has told of either, while its newest document has not
    /// loaded.
    pub fn is_loading(&self) -> bool {
        self.busy.unwrap_or_else(|| !self.loaded())
    }

    /// Whether the navigation has committed: answered, and a document begun
    /// in the tab's own frame since it was asked.
    pub fn committed(&self) -> bool {
        self.navigated.is_some() && self.newest.is_some()
    }

    /// Whether the newest document the tab's own frame began — or, before
    /// any, the one `Page.navigate` named — has loaded.
    pub fn loaded(&self) -> bool {
        match (
            self.newest.as_ref().or(self.navigated.as_ref()),
            &self.loaded,
        ) {
            (Some(counts), Some(loaded)) => counts == loaded,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAB: &str = "TAB";

    fn stage(frame: &str, loader: &str, name: &str) -> Stage {
        Stage {
            frame: frame.to_owned(),
            loader: loader.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn committed_once_answered_and_begun_and_loaded_at_its_load() {
        let mut loading = Loading::default();
        loading.navigating();
        assert!(!loading.committed() && !loading.loaded());
        loading.told(TAB, stage(TAB, "A", "init"));
        assert!(!loading.committed(), "not answered yet");
        loading.navigated("A".to_owned());
        assert!(loading.committed());
        assert!(!loading.loaded());
        loading.told(TAB, stage(TAB, "A", "DOMContentLoaded"));
        assert!(!loading.loaded());
        loading.told(TAB, stage(TAB, "A", "load"));
        assert!(loading.loaded());
    }

    #[test]
    fn answered_alone_is_not_committed() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        assert!(!loading.committed(), "no document begun");
        // Its load told before its beginning is still its load.
        loading.told(TAB, stage(TAB, "A", "load"));
        assert!(loading.loaded());
    }

    #[test]
    fn a_page_that_replaces_itself_is_loaded_at_the_newest_documents_load() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        loading.told(TAB, stage(TAB, "A", "init"));
        loading.told(TAB, stage(TAB, "B", "init"));
        assert!(loading.committed());
        loading.told(TAB, stage(TAB, "A", "load"));
        assert!(
            !loading.loaded(),
            "the first document's load is not the page's"
        );
        loading.told(TAB, stage(TAB, "B", "load"));
        assert!(loading.loaded());
        // A link followed later is loading again until its own load.
        loading.told(TAB, stage(TAB, "C", "init"));
        assert!(!loading.loaded());
        loading.told(TAB, stage(TAB, "C", "load"));
        assert!(loading.loaded());
    }

    #[test]
    fn a_frame_inside_the_page_is_not_the_page() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        loading.told(TAB, stage("INNER", "F", "init"));
        assert!(!loading.committed());
        loading.told(TAB, stage(TAB, "A", "init"));
        loading.told(TAB, stage("INNER", "A", "load"));
        assert!(!loading.loaded());
    }

    fn begun(frame: &str, kind: &str) -> Started {
        Started {
            frame: frame.to_owned(),
            kind: kind.to_owned(),
        }
    }

    /// The tab's page, opened and loaded, as Chrome tells of it.
    fn loaded_page() -> Loading {
        let mut loading = Loading::default();
        loading.navigating();
        loading.started(TAB, &begun(TAB, "differentDocument"));
        loading.navigated("A".to_owned());
        loading.told(TAB, stage(TAB, "A", "init"));
        assert!(loading.committed() && loading.is_loading());
        loading.told(TAB, stage(TAB, "A", "load"));
        assert!(loading.is_loading(), "loading until Chrome says stopped");
        loading.stopped(TAB, TAB);
        assert!(!loading.is_loading());
        loading
    }

    #[test]
    fn loading_from_a_links_start_through_a_slow_servers_wait_to_its_stop() {
        let mut loading = loaded_page();
        // Pressed: nothing committed, the server not yet answered.
        loading.started(TAB, &begun(TAB, "differentDocument"));
        assert!(loading.is_loading(), "said from the start");
        loading.told(TAB, stage(TAB, "B", "init"));
        loading.told(TAB, stage(TAB, "B", "load"));
        assert!(loading.is_loading());
        loading.stopped(TAB, TAB);
        assert!(!loading.is_loading());
        // Back, a reload: loading from their starts too.
        for kind in ["historyDifferentDocument", "reload"] {
            loading.started(TAB, &begun(TAB, kind));
            assert!(loading.is_loading(), "{kind}");
            loading.stopped(TAB, TAB);
            assert!(!loading.is_loading(), "{kind}");
        }
    }

    #[test]
    fn a_navigation_that_never_commits_stops_loading_where_chrome_stops() {
        // A 204, a download, a stop, a mailto: — begun, and stopped, with
        // no document begun: the page before it is still the page.
        let mut loading = loaded_page();
        loading.started(TAB, &begun(TAB, "differentDocument"));
        assert!(loading.is_loading());
        loading.stopped(TAB, TAB);
        assert!(!loading.is_loading());
        assert!(loading.loaded(), "the page left is loaded still");
    }

    #[test]
    fn a_page_stopped_before_its_load_is_not_loading() {
        // Measured: Page.stopLoading after the commit, a picture held — the
        // stop is told, no load ever is, and Chrome's spinner stops.
        let mut loading = loaded_page();
        loading.started(TAB, &begun(TAB, "differentDocument"));
        loading.told(TAB, stage(TAB, "B", "init"));
        loading.stopped(TAB, TAB);
        assert!(!loading.loaded());
        assert!(!loading.is_loading());
    }

    #[test]
    fn a_move_within_the_document_or_a_frame_inside_is_not_the_pages_loading() {
        let mut loading = loaded_page();
        loading.started(TAB, &begun(TAB, "sameDocument"));
        loading.started(TAB, &begun(TAB, "historySameDocument"));
        assert!(!loading.is_loading());
        loading.started(TAB, &begun("INNER", "differentDocument"));
        assert!(!loading.is_loading());
        // A frame inside stopping is not the page stopping.
        loading.started(TAB, &begun(TAB, "differentDocument"));
        loading.stopped(TAB, "INNER");
        assert!(loading.is_loading());
    }

    #[test]
    fn before_a_start_or_a_stop_is_told_the_newest_documents_load_decides() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        loading.told(TAB, stage(TAB, "A", "init"));
        assert!(loading.is_loading());
        loading.told(TAB, stage(TAB, "A", "load"));
        assert!(!loading.is_loading());
    }

    #[test]
    fn sent_somewhere_again_forgets_what_was_told() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        loading.told(TAB, stage(TAB, "A", "init"));
        loading.told(TAB, stage(TAB, "A", "load"));
        loading.stopped(TAB, TAB);
        loading.navigating();
        assert_eq!(loading, Loading::default());
        assert!(!loading.committed() && !loading.loaded());
    }
}
