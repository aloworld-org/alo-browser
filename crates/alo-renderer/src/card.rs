/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page opened from the conversation, shown as a card in it (ADR 0076 B,
//! queue item 717): what the browser process tells the first screen's
//! renderer of it, as data.
//!
//! The design (`docs/design/shell.md`, *A page inside the conversation*)
//! opens a page in a card under what the person said: a bar with *Back*,
//! *Forward*, the page's address — its host in bold, then its path — and
//! *Expand to the whole window* and *Close*; under the bar, the page. Chrome
//! draws the page in a process of its own, so what the card holds is the
//! last frame Chrome streamed of it, a PNG ([`Card::frame`]), which this
//! engine decodes and draws as a picture in the card's page box — the one
//! picture path every `<img>` takes ([`crate::pictures`]).
//!
//! Every string here is a stranger's: what the person typed, and the
//! address the page is at. Each enters the card's document as a text node
//! ([`crate::card_document`]).
//!
//! The card opens as its page's navigation commits, before the page has
//! loaded, and Chrome's frames are drawn from its first paint; until the
//! browser process says the page has loaded, the bar says *Loading* after
//! the address ([`Card::loading`], queue item 751).
//!
//! Not here yet, and not drawn: the design's *1 of 3 sources ‹ ›* and *alo
//! used the highlighted part of this page*, which wait on an answer that
//! has sources. *Back* and *Forward* are drawn, and disabled whenever the
//! browser process says there is nowhere to go ([`Card::back`],
//! [`Card::forward`]).

use alo_url::Url;

/// What the card's picture is named by, as an `<img>`'s `src`: a name no
/// page can write into this document, since none is a page's.
pub const FRAME_SRC: &str = "alo:card-frame";

/// The size of the card's page box, in CSS pixels: the design's 880 column
/// less the card's border on each side, and its 520 of page. What Chrome
/// lays the page out at while it is a card, and what its frames are asked
/// for at, times the window's scale (ADR 0076 B).
pub const PAGE_SIZE: (u32, u32) = (878, 520);

/// A page shown as a card in the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// What the person said, which the card answers: the line above it.
    pub said: String,
    /// The address of the page, as the browser process holds it.
    pub url: Url,
    /// Whether there is a page to go back to.
    pub back: bool,
    /// Whether there is a page to go forward to.
    pub forward: bool,
    /// The last frame Chrome drew of the page: a PNG's bytes, or nothing
    /// before the first has arrived.
    pub frame: Option<Vec<u8>>,
    /// Whether the page is still loading: the newest document of its own
    /// frame has not loaded, as the browser process last heard (queue item
    /// 751).
    pub loading: bool,
}

impl Card {
    /// A card for `url`, answering `said`, with nowhere to go back or
    /// forward to, no frame yet, and its page loading.
    pub fn new(said: impl Into<String>, url: Url) -> Self {
        Self {
            said: said.into(),
            url,
            back: false,
            forward: false,
            frame: None,
            loading: true,
        }
    }

    /// The same card, its page loaded.
    #[must_use]
    pub fn loaded(mut self) -> Self {
        self.loading = false;
        self
    }

    /// The same card holding `frame`.
    #[must_use]
    pub fn holding(mut self, frame: Vec<u8>) -> Self {
        self.frame = Some(frame);
        self
    }

    /// The address as the bar writes it: the host, which is drawn in bold,
    /// and everything after it — the path, the query and the fragment, as
    /// the URL serialises them. An address with no host is all path.
    pub fn address(&self) -> (String, String) {
        let host = alo_url::reading::host(&self.url);
        if host.is_empty() {
            return (String::new(), self.url.to_string());
        }
        let mut rest = self.url.path.clone();
        if let Some(query) = &self.url.query {
            rest.push('?');
            rest.push_str(query);
        }
        if let Some(fragment) = &self.url.fragment {
            rest.push('#');
            rest.push_str(fragment);
        }
        (host, rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(address: &str) -> Card {
        Card::new("a page", alo_url::parse(address).unwrap())
    }

    #[test]
    fn the_address_is_its_host_then_the_rest() {
        assert_eq!(
            card("https://example.com/").address(),
            ("example.com".to_owned(), "/".to_owned())
        );
        assert_eq!(
            card("https://news.example.org/a/b?c=d#e").address(),
            ("news.example.org".to_owned(), "/a/b?c=d#e".to_owned())
        );
        assert_eq!(
            card("http://127.0.0.1:8765/one.html").address(),
            ("127.0.0.1:8765".to_owned(), "/one.html".to_owned())
        );
    }

    #[test]
    fn a_new_card_has_nowhere_to_go_and_no_frame() {
        let new = card("https://example.com/");
        assert!(!new.back && !new.forward);
        assert_eq!(new.frame, None);
        assert_eq!(new.clone().holding(vec![1]).frame, Some(vec![1]));
        // Its page is loading until it is said to have loaded.
        assert!(new.loading);
        assert!(!new.holding(vec![1]).loaded().loading);
    }
}
