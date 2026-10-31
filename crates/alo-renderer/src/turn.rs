/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One turn of the conversation, as the browser process tells the first
//! screen's renderer of it (ADR 0074, ADR 0085, queue item 703).
//!
//! A conversation is every turn of the chat on screen, oldest first: what
//! the person said, then what came back of it. Four things come back today,
//! each a variant here:
//!
//! - **A search's results** ([`Turn::Searched`]), on the tab the browser
//!   process chose for them — All, Sites or Images (queue items 708, 743,
//!   744).
//! - **A page, live** ([`Turn::Page`]), as a card holding Chrome's newest
//!   frame of it (ADR 0076 B). One page is open at a time, so only **the
//!   newest turn** may be one; the wire refuses a card anywhere else.
//! - **A page shown earlier** ([`Turn::Visited`]): a card that a newer turn,
//!   a press on *Close*, or the page going on its own put away, kept as a
//!   line with its address — pressable, to open it again as a new turn.
//!   Not a still of its last frame: every frame of a live card sends the
//!   whole first screen again, and a still per earlier page would send
//!   each of them sixty times a second (ADR 0085 § 2).
//! - **An honest failure** ([`Turn::Failed`]): a page that could not be
//!   opened, and why, in this browser's words.
//!
//! The agent's own words and the steps it took are not here: what they are
//! waits on a model (queue item 707) and on the agent's record given a face
//! (704). Nothing stands in for them.
//!
//! Every string is a stranger's — what a person typed, a site's title, an
//! address, a reason that names one — and enters the document as a text
//! node ([`crate::conversation_document`]).

use crate::card::Card;
use crate::results::Results;
use alo_url::Url;

/// The most turns a first screen is sent: a ceiling the wire reads against,
/// far past any conversation a person keeps on one screen.
pub const MOST_TURNS: usize = 500;

/// One turn of the conversation: what the person said, and what came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turn {
    /// A search, and its results as the All tab draws them; what the person
    /// said is [`Results::said`].
    Searched(Results),
    /// A page open as a card, Chrome's newest frame of it in it; what the
    /// person said is [`Card::said`]. Only ever the newest turn.
    Page(Card),
    /// A page that was shown as a card and is no longer, kept as its
    /// address.
    Visited {
        /// What the person said, which the page answered.
        said: String,
        /// Where the page last was.
        url: Url,
    },
    /// What the person chose that could not be done, and why.
    Failed {
        /// What the person said.
        said: String,
        /// Why not, in this browser's words.
        why: String,
    },
}

impl Turn {
    /// What the person said, which stands as their bubble at the turn's
    /// top.
    pub fn said(&self) -> &str {
        match self {
            Turn::Searched(results) => &results.said,
            Turn::Page(card) => &card.said,
            Turn::Visited { said, .. } | Turn::Failed { said, .. } => said,
        }
    }

    /// The addresses a press on the turn's links opens, in the order they
    /// are drawn: the sites its results show on the tab shown, or the page
    /// it shows as a line. A live card's page is pressed as the page, never
    /// as a link.
    pub fn links(&self) -> Vec<&Url> {
        match self {
            Turn::Searched(results) => results
                .drawn_sites()
                .iter()
                .map(|found| &found.url)
                .collect(),
            Turn::Visited { url, .. } => vec![url],
            Turn::Page(_) | Turn::Failed { .. } => Vec::new(),
        }
    }

    /// The pages a press on the turn's images opens, in the order they are
    /// drawn: the images its results show on the tab shown (queue item 743).
    pub fn pictured(&self) -> Vec<&Url> {
        match self {
            Turn::Searched(results) => results
                .drawn_images()
                .iter()
                .map(|image| &image.page)
                .collect(),
            Turn::Page(_) | Turn::Visited { .. } | Turn::Failed { .. } => Vec::new(),
        }
    }

    /// The page shown as a card, if this turn is one.
    pub fn card(&self) -> Option<&Card> {
        match self {
            Turn::Page(card) => Some(card),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Found, Showing};

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    #[test]
    fn each_turn_says_what_the_person_said() {
        let searched = Turn::Searched(Results::searching("ev chargers"));
        assert_eq!(searched.said(), "ev chargers");
        let page = Turn::Page(Card::new("example.com", url("https://example.com/")));
        assert_eq!(page.said(), "example.com");
        assert!(page.card().is_some() && searched.card().is_none());
        let failed = Turn::Failed {
            said: "a.example".to_owned(),
            why: "no Chrome".to_owned(),
        };
        assert_eq!(failed.said(), "a.example");
    }

    #[test]
    fn a_turns_links_are_its_sites_on_all_or_its_page_and_never_a_live_card() {
        let mut results = Results::searching("x");
        assert!(Turn::Searched(results.clone()).links().is_empty());
        results.showing = Showing::Found(
            [
                "https://a.example/",
                "https://b.example/",
                "https://c.example/",
                "https://d.example/",
            ]
            .map(|address| Found::new("t", url(address), ""))
            .to_vec(),
        );
        assert_eq!(
            Turn::Searched(results).links(),
            vec![
                &url("https://a.example/"),
                &url("https://b.example/"),
                &url("https://c.example/")
            ],
            "the three All shows"
        );
        let visited = Turn::Visited {
            said: "s".to_owned(),
            url: url("https://e.example/p"),
        };
        assert_eq!(visited.links(), vec![&url("https://e.example/p")]);
        assert!(
            Turn::Page(Card::new("s", url("https://e.example/")))
                .links()
                .is_empty()
        );
    }
}
