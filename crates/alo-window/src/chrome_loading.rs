/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a tab's page has committed and whether it has loaded, as Chrome
//! tells of its own frame's stages (`Page.lifecycleEvent`, queue items 749
//! and 751).
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
//! Only the tab's own frame is read: a frame inside the page has loaders of
//! its own, and its `load` is not the page's. Only the last document begun
//! and the last loaded are kept, so a page that goes somewhere a million
//! times costs nothing more.

use crate::chrome_protocol::Stage;

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

    #[test]
    fn sent_somewhere_again_forgets_what_was_told() {
        let mut loading = Loading::default();
        loading.navigated("A".to_owned());
        loading.told(TAB, stage(TAB, "A", "init"));
        loading.told(TAB, stage(TAB, "A", "load"));
        loading.navigating();
        assert_eq!(loading, Loading::default());
        assert!(!loading.committed() && !loading.loaded());
    }
}
