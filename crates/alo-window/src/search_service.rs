/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The search service: the one interface alo searches through, whichever
//! service it is (ADR 0106 § 1, queue item 904).
//!
//! The box's search ([`crate::conversation_search`]), the agent's
//! ([`crate::conversation_agent`]) and the tabs hold a [`SearchService`]
//! and nothing of whose it is. Behind it, a [`SearchConnector`] — one per
//! service, in a file of its own with its protocol in another: the Brave
//! Search API ([`crate::brave_search`]) and `SearXNG`
//! ([`crate::searxng_search`]). Which one is made is configuration
//! ([`crate::services_made`]), never code.
//!
//! # What a connector searches
//!
//! Each says which of [`Searched`] it can ask. What it cannot is refused
//! here, before anything is asked or paid for, in one sentence a person
//! reads that names no service ([`unsupported`]): a tab the service does
//! not search says so plainly, and a search's All tab draws no row of
//! images it could never fill.
//!
//! # What the threads are told
//!
//! The interface's own types: [`Answered`] for a page of sites, and
//! [`ImageFound`], [`VideoFound`] and [`StoryFound`] for the rest — each
//! with where the service says its picture and its site's mark are, which
//! the browser process alone fetches, and only from the one origin the
//! service names ([`SearchService::pictures`], ADR 0086, ADR 0088).

use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::{Pictured, SEARCH_SERVICE, Showing};
use alo_renderer::results_media::{Story, Video};
use alo_url::Url;
use core::fmt;
use std::sync::Arc;

/// What a search service may be asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Searched {
    /// A web search: the sites.
    Web,
    /// A further page of a web search's sites (queue item 761).
    Pages,
    /// An image search (queue item 743).
    Images,
    /// A video search (queue item 745).
    Videos,
    /// A news search (queue item 746).
    News,
}

impl Searched {
    /// Each, in the order the tabs are drawn.
    pub const ALL: [Self; 5] = [
        Self::Web,
        Self::Pages,
        Self::Images,
        Self::Videos,
        Self::News,
    ];

    /// What it is, as a sentence says it: *videos*.
    pub fn said(self) -> &'static str {
        match self {
            Self::Web => "sites",
            Self::Pages => "further pages of sites",
            Self::Images => "images",
            Self::Videos => "videos",
            Self::News => "news",
        }
    }
}

/// What a person is told when the service chosen does not search for
/// `what`: never its name.
pub fn unsupported(what: Searched) -> String {
    format!(
        "{SEARCH_SERVICE} chosen in Settings does not search for {}.",
        what.said()
    )
}

/// What a web search's answer came to: the sites found, or why not; where
/// the service says each site's mark is, by the site's place (queue item
/// 747); and whether it offers a further page (queue item 761).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// The sites found, or why the search was not answered.
    pub showing: Showing,
    /// Where each site's mark is, by its place.
    pub marks: Vec<Option<Url>>,
    /// Whether the service says more are there to ask for.
    pub more: bool,
}

impl Answered {
    /// Not answered, and `why`.
    pub fn not(why: String) -> Self {
        Self {
            showing: Showing::Unanswered(why),
            marks: Vec::new(),
            more: false,
        }
    }
}

/// One image an image search found, as read: what is drawn of it, and
/// where the service says its thumbnail is — which the browser process
/// alone fetches, and never sends to a renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFound {
    /// The image, with no picture yet.
    pub pictured: Pictured,
    /// Its thumbnail's address, if the service gave one that is a site's.
    pub thumbnail: Option<Url>,
    /// Its site's mark's address, if the service gave one that is a
    /// site's (queue item 768).
    pub mark: Option<Url>,
}

/// One video a video search found, as read, and where the service says its
/// thumbnail is — which the browser process alone fetches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFound {
    /// The video, with no picture yet.
    pub video: Video,
    /// Its thumbnail's address, if the service gave one that is a site's.
    pub thumbnail: Option<Url>,
    /// Its site's mark's address, if the service gave one that is a site's.
    pub mark: Option<Url>,
}

/// One story a news search found, as read, and where the service says its
/// photo (queue item 765) and its site's mark are — which the browser
/// process alone fetches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryFound {
    /// The story, with no photo yet.
    pub story: Story,
    /// Its photo's address, if the service gave one that is a site's.
    pub thumbnail: Option<Url>,
    /// Its site's mark's address, if the service gave one that is a site's.
    pub mark: Option<Url>,
}

/// One search service, asked through the interface. Every method is asked
/// only for what [`SearchConnector::searches`] says it searches;
/// [`SearchService`] refuses the rest first.
pub trait SearchConnector: fmt::Debug + Send + Sync {
    /// Whether it searches for `what`.
    fn searches(&self, what: Searched) -> bool;

    /// The one origin pictures and marks are fetched from — the service's
    /// own proxy — or [`None`] where none is fetched (ADR 0086, ADR 0088).
    fn pictures(&self) -> Option<&Url>;

    /// A web search of `words`: what came of it.
    fn search(&self, pool: &mut Pool, identities: &mut Identities, words: &str) -> Answered;

    /// The further page at `offset` (1 to
    /// [`alo_renderer::results::MOST_PAGES`]) of a web search of `words`.
    fn page(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
        offset: usize,
    ) -> Answered;

    /// An image search of `words`.
    ///
    /// # Errors
    ///
    /// Why none were found, in a sentence a person reads.
    fn images(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<ImageFound>, String>;

    /// A video search of `words`.
    ///
    /// # Errors
    ///
    /// Why none were found, in a sentence a person reads.
    fn videos(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<VideoFound>, String>;

    /// A news search of `words`.
    ///
    /// # Errors
    ///
    /// Why none were found, in a sentence a person reads.
    fn news(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<StoryFound>, String>;
}

/// The search service alo asks: any connector, held so every thread that
/// searches may hold it too.
#[derive(Debug, Clone)]
pub struct SearchService(Arc<dyn SearchConnector>);

impl SearchService {
    /// The service `connector` answers for.
    pub fn of(connector: impl SearchConnector + 'static) -> Self {
        Self(Arc::new(connector))
    }

    /// Whether it searches for `what`.
    pub fn searches(&self, what: Searched) -> bool {
        self.0.searches(what)
    }

    /// The one origin its pictures and marks are fetched from, if any.
    pub fn pictures(&self) -> Option<&Url> {
        self.0.pictures()
    }

    /// Whether images are asked of it: it searches for them, and names
    /// where their pictures come from.
    pub fn asks_images(&self) -> bool {
        self.searches(Searched::Images) && self.pictures().is_some()
    }

    /// A web search of `words` through `pool`.
    pub fn search(&self, pool: &mut Pool, identities: &mut Identities, words: &str) -> Answered {
        if !self.searches(Searched::Web) {
            return Answered::not(unsupported(Searched::Web));
        }
        self.0.search(pool, identities, words)
    }

    /// The further page at `offset` of a web search of `words`.
    pub fn page(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
        offset: usize,
    ) -> Answered {
        if !self.searches(Searched::Pages) {
            return Answered::not(unsupported(Searched::Pages));
        }
        self.0.page(pool, identities, words, offset)
    }

    /// An image search of `words`.
    ///
    /// # Errors
    ///
    /// [`unsupported`] where it searches for none; otherwise the
    /// connector's sentence.
    pub fn images(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<ImageFound>, String> {
        self.refused(Searched::Images)?;
        self.0.images(pool, identities, words)
    }

    /// A video search of `words`.
    ///
    /// # Errors
    ///
    /// As [`SearchService::images`].
    pub fn videos(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<VideoFound>, String> {
        self.refused(Searched::Videos)?;
        self.0.videos(pool, identities, words)
    }

    /// A news search of `words`.
    ///
    /// # Errors
    ///
    /// As [`SearchService::images`].
    pub fn news(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        words: &str,
    ) -> Result<Vec<StoryFound>, String> {
        self.refused(Searched::News)?;
        self.0.news(pool, identities, words)
    }

    /// [`unsupported`], where it does not search for `what`.
    fn refused(&self, what: Searched) -> Result<(), String> {
        if self.searches(what) {
            Ok(())
        } else {
            Err(unsupported(what))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connector searching the web alone, that must never be asked for
    /// anything else.
    #[derive(Debug)]
    struct WebAlone;

    impl SearchConnector for WebAlone {
        fn searches(&self, what: Searched) -> bool {
            what == Searched::Web
        }
        fn pictures(&self) -> Option<&Url> {
            None
        }
        fn search(&self, _: &mut Pool, _: &mut Identities, words: &str) -> Answered {
            Answered::not(format!("asked for {words}"))
        }
        fn page(&self, _: &mut Pool, _: &mut Identities, _: &str, _: usize) -> Answered {
            Answered::not("a page was asked".to_owned())
        }
        fn images(
            &self,
            _: &mut Pool,
            _: &mut Identities,
            _: &str,
        ) -> Result<Vec<ImageFound>, String> {
            Err("images were asked".to_owned())
        }
        fn videos(
            &self,
            _: &mut Pool,
            _: &mut Identities,
            _: &str,
        ) -> Result<Vec<VideoFound>, String> {
            Err("videos were asked".to_owned())
        }
        fn news(
            &self,
            _: &mut Pool,
            _: &mut Identities,
            _: &str,
        ) -> Result<Vec<StoryFound>, String> {
            Err("news was asked".to_owned())
        }
    }

    #[test]
    fn what_a_connector_does_not_search_is_refused_before_it_is_asked_naming_no_service() {
        let service = SearchService::of(WebAlone);
        let mut pool = Pool::with_trust(alo_net::tls::Trust::of(&[]).unwrap());
        let mut identities = Identities::default();
        assert_eq!(
            service.search(&mut pool, &mut identities, "ev").showing,
            Showing::Unanswered("asked for ev".to_owned()),
            "the web is asked of it"
        );
        assert!(!service.asks_images());
        assert_eq!(
            service.page(&mut pool, &mut identities, "ev", 1).showing,
            Showing::Unanswered(
                "The search service chosen in Settings does not search for further pages of \
                 sites."
                    .to_owned()
            )
        );
        assert_eq!(
            service.images(&mut pool, &mut identities, "ev").err(),
            Some("The search service chosen in Settings does not search for images.".to_owned())
        );
        assert_eq!(
            service.videos(&mut pool, &mut identities, "ev").err(),
            Some("The search service chosen in Settings does not search for videos.".to_owned())
        );
        assert_eq!(
            service.news(&mut pool, &mut identities, "ev").err(),
            Some("The search service chosen in Settings does not search for news.".to_owned())
        );
        for what in Searched::ALL {
            let said = unsupported(what).to_lowercase();
            assert!(
                !said.contains("brave") && !said.contains("searxng"),
                "{said}"
            );
        }
    }
}
