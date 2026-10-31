/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results, shown in the conversation as the design's **All**
//! tab draws them (`docs/design/shell.md`, *What appears when a person
//! types*; ADR 0082 § 0, ADR 0083; queue item 708): what the browser process
//! tells the first screen's renderer of them, as data.
//!
//! The browser process asks the search provider — the Brave Search API —
//! on a thread of its own and reads its answer (`alo-window`'s
//! `conversation_search`); this is what it then holds: what the person
//! said, and whether the search is still under way, found sites, or was
//! not answered and why. Each site is its title, its address, its two lines
//! of description and the name the provider gives it — every one a
//! stranger's string, which enters the first screen's document as a text
//! node ([`crate::results_document`]).
//!
//! # What All shows
//!
//! The tab row, *All* chosen; the top [`ON_ALL`] sites, each its mark — the
//! first letter of its name — its name and address, its title as a link and
//! two lines of its description; then *More sites ›*. The short answer the
//! design puts first waits on the model (queue item 707) and is not drawn,
//! nor anything in its place. The row of images (743), the Sites tab and
//! *More sites ›* (744), Videos (745), News (746) and a site's own mark
//! (747) are queued: the tabs and *More sites ›* are drawn disabled, and
//! claim nothing.

use alo_url::Url;

/// How many sites the All tab shows, so that the column, the box and what
/// the box offers fit the design's window, 820 tall, without scrolling —
/// which the first screen does not do.
pub const ON_ALL: usize = 3;

/// The most sites a search's results are kept with: Brave's own most for
/// one page of web results.
pub const MOST_FOUND: usize = 20;

/// The tabs of a search's results, in the design's order.
pub const TABS: [&str; 6] = ["All", "Answer", "Sites", "Images", "Videos", "News"];

/// What the search provider is called, where the person reads it.
pub const PROVIDER: &str = "Brave Search";

/// Whether searching can be done, as the box's offer says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetUp {
    /// It can: searches go to [`PROVIDER`].
    Ready,
    /// It cannot, and why, in words a person reads under the offer.
    Not(String),
}

/// A search's results, as the conversation shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Results {
    /// What the person said: the line above the results.
    pub said: String,
    /// Where the search is.
    pub showing: Showing,
}

/// Where a search is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Showing {
    /// Asked, and not yet answered.
    Searching,
    /// Answered: the sites found, best first — perhaps none.
    Found(Vec<Found>),
    /// Not answered, and why, in a sentence a person reads.
    Unanswered(String),
}

/// One site a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The page's title, as plain text.
    pub title: String,
    /// The page's address: an `http` or `https` one.
    pub url: Url,
    /// What the provider says of the page, as plain text, perhaps nothing.
    pub description: String,
    /// The site's name, as the provider gives it, perhaps nothing.
    pub name: String,
}

impl Results {
    /// A search for `said`, under way.
    pub fn searching(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            showing: Showing::Searching,
        }
    }

    /// The sites All shows, best first: at most [`ON_ALL`].
    pub fn on_all(&self) -> &[Found] {
        match &self.showing {
            Showing::Found(found) => found.get(..ON_ALL.min(found.len())).unwrap_or_default(),
            Showing::Searching | Showing::Unanswered(_) => &[],
        }
    }

    /// What is said where the sites would be, when there are none to show:
    /// that it is searching, that nothing was found, or why it was not
    /// answered.
    pub fn note(&self) -> Option<String> {
        match &self.showing {
            Showing::Searching => Some(format!(
                "Searching {PROVIDER} for \u{201c}{}\u{201d}\u{2026}",
                self.said
            )),
            Showing::Found(found) if found.is_empty() => Some(format!(
                "{PROVIDER} found no sites for \u{201c}{}\u{201d}.",
                self.said
            )),
            Showing::Found(_) => None,
            Showing::Unanswered(why) => Some(why.clone()),
        }
    }
}

impl Found {
    /// A site found at `url`, called `title`, said of as `description`.
    pub fn new(title: impl Into<String>, url: Url, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url,
            description: description.into(),
            name: String::new(),
        }
    }

    /// The same site, named `name` by the provider.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// What the site is called in bold: its name, or its host when the
    /// provider gave none.
    pub fn site(&self) -> String {
        if self.name.trim().is_empty() {
            alo_url::reading::host(&self.url)
        } else {
            self.name.trim().to_owned()
        }
    }

    /// Its address as the design writes it: the host, then each part of the
    /// path after a `›` — `evhomeguide.org › installing-a-charger`.
    pub fn address(&self) -> String {
        let mut written = alo_url::reading::host(&self.url);
        for part in self.url.path.split('/').filter(|part| !part.is_empty()) {
            written.push_str(" \u{203a} ");
            written.push_str(part);
        }
        written
    }

    /// The site's mark: the first letter or digit of what it is called, as
    /// a capital.
    pub fn letter(&self) -> Option<String> {
        self.site()
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|first| first.to_uppercase().collect())
    }

    /// Which of the mark's four colours the site is drawn in, the same for
    /// a host every time: a number below four.
    pub fn tint(&self) -> usize {
        let host = alo_url::reading::host(&self.url);
        let sum = host
            .bytes()
            .fold(0usize, |sum, byte| sum.wrapping_add(usize::from(byte)));
        sum % 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(address: &str) -> Found {
        Found::new("A page", alo_url::parse(address).unwrap(), "Said of it.")
    }

    #[test]
    fn a_site_is_named_by_the_provider_or_by_its_host() {
        let plain = found("https://evhomeguide.org/installing-a-charger/");
        assert_eq!(plain.site(), "evhomeguide.org");
        assert_eq!(plain.letter().as_deref(), Some("E"));
        let named = plain.clone().named("  EV Home Guide ");
        assert_eq!(named.site(), "EV Home Guide");
        assert_eq!(
            named.address(),
            "evhomeguide.org \u{203a} installing-a-charger"
        );
        assert_eq!(found("https://example.com/").address(), "example.com");
        assert!(plain.tint() < 4);
        assert_eq!(plain.tint(), found("https://evhomeguide.org/other").tint());
    }

    #[test]
    fn all_shows_the_top_three_and_says_why_when_there_are_none() {
        let mut results = Results::searching("rust borrow checker");
        assert!(results.on_all().is_empty());
        assert_eq!(
            results.note().as_deref(),
            Some("Searching Brave Search for \u{201c}rust borrow checker\u{201d}\u{2026}")
        );
        results.showing = Showing::Found(vec![found("https://a.example/"); 5]);
        assert_eq!(results.on_all().len(), ON_ALL);
        assert_eq!(results.note(), None);
        results.showing = Showing::Found(vec![found("https://a.example/")]);
        assert_eq!(results.on_all().len(), 1);
        results.showing = Showing::Found(Vec::new());
        assert_eq!(
            results.note().as_deref(),
            Some("Brave Search found no sites for \u{201c}rust borrow checker\u{201d}.")
        );
        results.showing =
            Showing::Unanswered("Brave Search could not answer just now (500).".into());
        assert!(results.on_all().is_empty());
        assert_eq!(
            results.note().as_deref(),
            Some("Brave Search could not answer just now (500).")
        );
    }
}
