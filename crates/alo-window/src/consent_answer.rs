/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's page's cookie banner answered with its most private choice
//! (ADR 0108 §§ 2–4, queue item 920), by the thread holding the card's
//! tab, through the verb an agent uses.
//!
//! # When it looks
//!
//! Once each document of the tab's own frame has loaded — and, since a
//! consent platform's script often draws its banner a moment after the
//! page's `load`, again [`AFTER`] each look that found nothing, at most
//! [`LOOKS`] times in all. A banner found, answered or left, is the end of
//! it for that document; another document (the page went somewhere, or
//! was loaded again) is looked at anew. *Settings › Privacy*'s *Leave them
//! to me* looks at nothing ([`Banners::Leave`]).
//!
//! # How it answers
//!
//! The page is read as the tree an agent reads ([`Tab::read`]), the banner
//! and its refusal are found by name ([`crate::consent_banner`]), and the
//! refusal is pressed by **the agent's own verb**: `activate` on the node
//! it was read as ([`Target::Node`], [`Verb::Activate`]) — decided by
//! `alo-agent` against the page as it is then, carried out as a person's
//! click at the middle of that node after Chromium is asked what a press
//! there would press ([`crate::chrome_act`]). Never a coordinate of ours,
//! never script in the page, and never a consent: a name that is one is
//! never pressed, and a banner offering only consent is left to the
//! person ([`Answered::LeftToYou`]).
//!
//! # What it says
//!
//! What it did, as [`Answered`], which the card's bar shows and the
//! terminal says; the press itself is an action in the tab's record, as
//! every verb is (ADR 0012 § 4).

use crate::chrome_tab::Tab;
use crate::consent_banner::{self, Found};
use alo_agent::verb::{Target, Verb};
use alo_net::cause::DocumentId;
use alo_renderer::card::Consent;
use alo_renderer::settings_privacy::Banners;
use std::time::{Duration, Instant};

/// How many times one document is looked at for a banner.
pub const LOOKS: u8 = 4;

/// How long after a look that found nothing the next is: the first at the
/// page's load, then a second, two and four after.
pub const AFTER: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// What was done with a page's cookie banner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// Its refusal, named so, was pressed.
    Refused {
        /// The refusal's name, as the page wrote it.
        name: String,
    },
    /// It offered no refusal alo would press: left to the person.
    LeftToYou {
        /// The consent it offered, as the page wrote its name.
        offered: String,
    },
    /// Its refusal could not be pressed, and why: it is the person's.
    NotPressed {
        /// The refusal's name.
        name: String,
        /// Why, in this browser's words.
        why: String,
    },
}

impl Answered {
    /// What the card's bar says of it: a refusal pressed, or the banner
    /// left to the person.
    pub fn consent(&self) -> Consent {
        match self {
            Self::Refused { .. } => Consent::Refused,
            Self::LeftToYou { .. } | Self::NotPressed { .. } => Consent::LeftToYou,
        }
    }

    /// What the terminal says of it, in plain words.
    pub fn said(&self) -> String {
        match self {
            Self::Refused { name } => format!(
                "alo answered the card's page's cookie banner with its most private choice, \
                 \u{201c}{name}\u{201d} (ADR 0108)"
            ),
            Self::LeftToYou { offered } => format!(
                "the card's page's cookie banner offers no refusal, only \u{201c}{offered}\u{201d}, \
                 so it is left to you (ADR 0108)"
            ),
            Self::NotPressed { name, why } => format!(
                "alo could not press \u{201c}{name}\u{201d} on the card's page's cookie banner: \
                 {why}; it is left to you (ADR 0108)"
            ),
        }
    }
}

/// Where the looking for one tab's banner stands.
#[derive(Debug, Default)]
pub struct Answering {
    /// The document last looked at.
    document: Option<DocumentId>,
    /// How many times it was looked at.
    looked: u8,
    /// When it is next looked at.
    next: Option<Instant>,
    /// Whether it is done with: a banner found, or looked at enough.
    done: bool,
}

impl Answering {
    /// Whether the document last looked at is done with: its banner
    /// answered or left, looked at [`LOOKS`] times, or banners left to the
    /// person.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Look at `tab`'s page if it is time to, under the choice `banners`:
    /// what was done with a banner on it, if one was found.
    pub fn pass(&mut self, tab: &mut Tab<'_>, banners: Banners) -> Option<Answered> {
        if tab.is_loading() {
            return None;
        }
        let document = tab.document()?;
        if self.document != Some(document) {
            *self = Self {
                document: Some(document),
                ..Self::default()
            };
        }
        if banners == Banners::Leave {
            self.done = true;
        }
        if self.done {
            return None;
        }
        let now = Instant::now();
        if self.next.is_some_and(|next| now < next) {
            return None;
        }
        let found = tab
            .read()
            .map_or(Found::Nothing, |snapshot| consent_banner::find(&snapshot));
        match found {
            Found::Nothing => {
                let after = AFTER.get(usize::from(self.looked)).copied();
                self.looked = self.looked.saturating_add(1);
                match after {
                    Some(after) if self.looked < LOOKS => self.next = Some(now + after),
                    _ => self.done = true,
                }
                None
            }
            Found::NoRefusal { offered } => {
                self.done = true;
                Some(Answered::LeftToYou { offered })
            }
            Found::Refusal { node, name } => {
                self.done = true;
                Some(match tab.act(&Target::Node(node), &Verb::Activate) {
                    Ok(_) => Answered::Refused { name },
                    Err(why) => Answered::NotPressed {
                        name,
                        why: why.to_string(),
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_answer_is_said_with_the_pages_own_words_quoted() {
        assert!(
            Answered::Refused {
                name: "Reject all".into()
            }
            .said()
            .contains("\u{201c}Reject all\u{201d}")
        );
        assert!(
            Answered::LeftToYou {
                offered: "AGREE".into()
            }
            .said()
            .contains("left to you")
        );
        assert_eq!(LOOKS as usize, AFTER.len() + 1);
    }
}
