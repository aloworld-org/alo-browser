/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tab strip's state: what the browser process tells the strip's
//! renderer, as data.
//!
//! ADR 0024 § 4: the tab strip is **a document of our own, built from the
//! browser's state as data and rendered by the engine in a sandboxed renderer
//! of its own**. This is the data. It is a typed message rather than markup
//! because a tab's title is a stranger's string — a page chose it — and the
//! first title containing `<` would be the first injection into the
//! browser's own interface if anybody assembled markup from it. So nothing
//! here is markup: the strip's renderer builds its document from these
//! values with `alo-dom`'s operations ([`crate::strip_document`]), and a
//! title goes in as a text node.
//!
//! # Where the strip is
//!
//! At [`site`], a site no page can name: its origin is opaque and minted
//! fresh, so no URL a page writes is ever the same site, and the strip's
//! renderer holds nobody's page but the strip (§ 4: *"at an internal site no
//! page can name or navigate to"*). It is confined as every renderer is.

use crate::site::Site;
use alo_layout::Size;
use alo_url::Url;

/// What the strip's document is said to be at. A scheme this engine
/// registers nothing for, so its origin is opaque and its [`site`] is a
/// process of its own.
pub const ADDRESS: &str = "alo:tab-strip";

/// The strip's address, as a URL.
///
/// [`None`] only if the URL parser stopped accepting a scheme it accepts
/// today, which a test pins.
pub fn address() -> Option<Url> {
    alo_url::parse(ADDRESS).ok()
}

/// A site for the strip's renderer that no page can name.
///
/// Minted on every call, as every opaque origin is (`crate::site`'s *"a site
/// is decided once"*): a browser process asks once and keeps the answer for
/// as long as it keeps the strip. `about:blank`'s opaque origin stands in if
/// the address ever stopped parsing; it is just as unnameable.
pub fn site() -> Site {
    Site::of(&address().unwrap_or_else(Url::about_blank))
}

/// The most characters of a title the strip draws.
///
/// A title is a page's to choose and costs the strip's renderer its length in
/// shaping on every drawing: a hundred thousand characters were fifteen
/// seconds of a debug build's time for one tab of 120 pixels. No tab shows
/// more than a few dozen, so the strip keeps this many and the stylesheet
/// cuts the rest to the tab's width with an ellipsis, as it would anyway. A
/// bound of ours, not measured against any other browser.
pub const LONGEST_TITLE: usize = 1024;

/// The strip, as the browser process sees it at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Strip {
    /// Every open tab, in the order the browser process keeps them.
    pub tabs: Vec<StripTab>,
    /// How big the strip is drawn, in CSS pixels.
    pub viewport: Size,
    /// The scale of the screen the window is on, which the strip is laid
    /// out and drawn at, as a page is (ADR 0057 § 3): any finite factor
    /// above zero, and anything else is one.
    pub scale: f32,
}

/// One tab in the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripTab {
    /// The page's title, as the page wrote it. A stranger's string: never
    /// markup, carried whole, drawn to [`LONGEST_TITLE`] characters, and cut
    /// by the stylesheet where it does not fit.
    pub title: String,
    /// Where the tab is, as the browser process keeps it: what the strip
    /// shows when the page gave no title.
    pub address: String,
    /// Whether this is the selected tab.
    pub selected: bool,
    /// What is happening to it.
    pub condition: Condition,
}

/// What is happening to a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// Showing its page.
    Shown,
    /// Loading, and what it shows may be stale.
    Loading,
    /// Its renderer has gone: it keeps its last frame and says so (ADR 0005).
    Gone,
}

impl Default for Strip {
    /// No tabs, at no size, on a screen of one.
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            viewport: Size::default(),
            scale: 1.0,
        }
    }
}

impl Strip {
    /// The same strip, drawn for a screen of `scale` device pixels to the
    /// CSS pixel. Anything that is not a finite factor above zero is one.
    #[must_use]
    pub fn at_scale(mut self, scale: f32) -> Self {
        self.scale = alo_value::device_scale(scale);
        self
    }
}

impl StripTab {
    /// A tab showing its page, not selected.
    pub fn new(title: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            address: address.into(),
            selected: false,
            condition: Condition::Shown,
        }
    }

    /// The same tab, selected.
    #[must_use]
    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }

    /// The same tab, in `condition`.
    #[must_use]
    pub fn in_condition(mut self, condition: Condition) -> Self {
        self.condition = condition;
        self
    }

    /// What the strip shows for this tab: its title, or its address when the
    /// title is nothing but white space, as a page with no `<title>` has —
    /// at most [`LONGEST_TITLE`] characters of either, cut at a character.
    pub fn label(&self) -> &str {
        let whole = if self.title.trim().is_empty() {
            &self.address
        } else {
            &self.title
        };
        longest(whole)
    }
}

/// At most [`LONGEST_TITLE`] characters of `text`, cut at a character.
///
/// What a renderer sends of a page's title, what the browser process keeps of
/// one, and what the strip draws: the same bound at every step, so a title
/// is never cut in two different places.
pub fn longest(text: &str) -> &str {
    match text.char_indices().nth(LONGEST_TITLE) {
        Some((at, _)) => text.get(..at).unwrap_or(text),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_parses_and_its_site_is_a_process_of_its_own() {
        assert!(address().is_some());
        let first = site();
        assert!(first.is_alone());
        assert_eq!(first.scheme(), "alo");
        // Minted, so two asks are two sites, and no page's URL is either.
        assert_ne!(first, site());
        let a_page = Site::of(&alo_url::parse(ADDRESS).unwrap_or_else(|_| Url::about_blank()));
        assert_ne!(first, a_page);
    }

    #[test]
    fn a_tab_with_no_title_shows_its_address() {
        let tab = StripTab::new(" \n\t", "https://example.com/");
        assert_eq!(tab.label(), "https://example.com/");
        let titled = StripTab::new("Invoices", "https://example.com/");
        assert_eq!(titled.label(), "Invoices");
    }

    #[test]
    fn a_long_title_is_kept_to_the_longest_at_a_character() {
        let tab = StripTab::new("é".repeat(LONGEST_TITLE + 5), "about:blank");
        assert_eq!(tab.label().chars().count(), LONGEST_TITLE);
        assert!(tab.label().chars().all(|c| c == 'é'));
        let address = StripTab::new("", "a".repeat(LONGEST_TITLE * 2));
        assert_eq!(address.label().len(), LONGEST_TITLE);
        let short = StripTab::new("é".repeat(LONGEST_TITLE), "about:blank");
        assert_eq!(short.label(), short.title);
    }

    #[test]
    fn a_tab_is_shown_and_unselected_until_it_is_said_otherwise() {
        let tab = StripTab::new("a", "b");
        assert!(!tab.selected);
        assert_eq!(tab.condition, Condition::Shown);
        let tab = tab.selected().in_condition(Condition::Gone);
        assert!(tab.selected);
        assert_eq!(tab.condition, Condition::Gone);
    }
}
