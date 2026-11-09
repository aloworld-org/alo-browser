/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What alo's agent found by searching (queue item 821, ADR 0103): the
//! sites of each of a run's searches — asked through the box's own search
//! ([`crate::brave_search::search`]) — kept by the number the model was
//! told each by, and written as the words the model is told.
//!
//! # Numbered across the run
//!
//! A run's searches are numbered on from each other — the first's sites 1
//! to 8, the next's from 9 — so a number the model gives `open` names one
//! site whichever search it came from. At most [`MOST_SHOWN`] sites of a
//! search are told, and at most [`MOST_KEPT`] are kept for a run; past
//! that a search's sites are told as found and not kept, and said so.
//!
//! # What the model is told
//!
//! Each site's number, title, address and description — Brave's words, a
//! stranger's, which the instructions say are data — each cut to a line,
//! and nothing else: no mark, no picture.

use crate::agent_protocol::kept;
use alo_renderer::results::{Found, Showing};
use core::fmt::Write as _;

/// The most sites of one search the model is told.
pub const MOST_SHOWN: usize = 8;

/// The most sites a run keeps to be opened by their numbers.
pub const MOST_KEPT: usize = 48;

/// The longest title the model is told, in characters.
const MOST_TITLE: usize = 160;

/// The longest description the model is told, in characters.
const MOST_DESCRIPTION: usize = 300;

/// The sites a run found, by the numbers it was told them by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoundSites {
    sites: Vec<Found>,
}

impl FoundSites {
    /// The site numbered `number`, counting from 1, if one is.
    pub fn numbered(&self, number: usize) -> Option<&Found> {
        number.checked_sub(1).and_then(|at| self.sites.get(at))
    }

    /// Whether `url` is the address of a site found.
    pub fn holds(&self, url: &alo_url::Url) -> bool {
        self.sites.iter().any(|found| found.url == *url)
    }

    /// What a search for `words` came to, `showing`, kept and written as
    /// the step's outcome the model is told.
    pub fn searched(&mut self, words: &str, showing: Showing) -> String {
        match showing {
            Showing::Searching => {
                format!("The search for \u{201c}{words}\u{201d} got no answer.")
            }
            Showing::Unanswered(why) => {
                format!("The search for \u{201c}{words}\u{201d} got no answer: {why}")
            }
            Showing::Found(found) if found.is_empty() => {
                format!("The search found no sites for \u{201c}{words}\u{201d}.")
            }
            Showing::Found(found) => {
                let mut said = format!(
                    "The search found these sites for \u{201c}{words}\u{201d}; open one with \
                     its number:"
                );
                let mut unkept = false;
                for site in found.into_iter().take(MOST_SHOWN) {
                    let number = if self.sites.len() < MOST_KEPT {
                        self.sites.push(site.clone());
                        Some(self.sites.len())
                    } else {
                        unkept = true;
                        None
                    };
                    let title = kept(&site.title, MOST_TITLE);
                    let description = kept(&site.description, MOST_DESCRIPTION);
                    let _ = match number {
                        Some(number) => {
                            write!(said, "\n{number}. {title} \u{2014} {}", site.url.serialised)
                        }
                        None => write!(
                            said,
                            "\n(not numbered) {title} \u{2014} {}",
                            site.url.serialised
                        ),
                    };
                    if !description.is_empty() {
                        let _ = write!(said, "\n   {description}");
                    }
                }
                if unkept {
                    let _ = write!(
                        said,
                        "\nalo keeps at most {MOST_KEPT} sites for one task: those not \
                         numbered can be opened by their address."
                    );
                }
                said
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(title: &str, address: &str, description: &str) -> Found {
        Found {
            title: title.to_owned(),
            url: alo_url::parse(address).unwrap(),
            description: description.to_owned(),
            name: String::new(),
            mark: Some(vec![1, 2, 3]),
        }
    }

    #[test]
    fn a_searchs_sites_are_told_numbered_on_across_the_run_and_kept_by_number() {
        let mut found = FoundSites::default();
        let said = found.searched(
            "Uganda Museum hours",
            Showing::Found(vec![
                site(
                    "Uganda Museum",
                    "https://ugandamuseum.go.ug/",
                    "Open daily\nfrom 10.",
                ),
                site("Visit", "https://visit.example/museum", ""),
            ]),
        );
        assert_eq!(
            said,
            "The search found these sites for \u{201c}Uganda Museum hours\u{201d}; open \
             one with its number:\n1. Uganda Museum \u{2014} https://ugandamuseum.go.ug/\n   Open \
             daily from 10.\n2. Visit \u{2014} https://visit.example/museum"
        );
        let more = found.searched(
            "Kampala",
            Showing::Found(vec![site("Kampala", "https://k.example/", "")]),
        );
        assert!(more.contains("\n3. Kampala"), "{more}");
        assert_eq!(
            found.numbered(2).map(|site| site.url.serialised.as_str()),
            Some("https://visit.example/museum")
        );
        assert_eq!(found.numbered(0), None);
        assert_eq!(found.numbered(4), None);
        assert!(found.holds(&alo_url::parse("https://k.example/").unwrap()));
        // No picture, no mark, is told.
        assert!(!said.contains('\u{1}'));
    }

    #[test]
    fn none_found_or_unanswered_is_said_and_past_the_most_not_kept() {
        let mut found = FoundSites::default();
        assert_eq!(
            found.searched("x", Showing::Found(Vec::new())),
            "The search found no sites for \u{201c}x\u{201d}."
        );
        assert!(
            found
                .searched("x", Showing::Unanswered("the key was refused".to_owned()))
                .ends_with("the key was refused")
        );
        let many: Vec<Found> = (0..MOST_SHOWN + 4)
            .map(|at| {
                site(
                    &format!("Site {at}"),
                    &format!("https://s{at}.example/"),
                    "",
                )
            })
            .collect();
        for _ in 0..(MOST_KEPT / MOST_SHOWN) {
            let _ = found.searched("many", Showing::Found(many.clone()));
        }
        assert!(found.numbered(MOST_KEPT).is_some());
        let past = found.searched("many", Showing::Found(many));
        assert!(past.contains("(not numbered)"), "{past}");
        assert!(found.numbered(MOST_KEPT + 1).is_none());
    }

    #[test]
    fn what_the_model_is_told_of_a_search_names_no_provider() {
        // Queue item 900: the agent repeats what it is told to the person,
        // so the provider is never named in it.
        let mut found = FoundSites::default();
        for showing in [
            Showing::Searching,
            Showing::Unanswered("not answered".to_owned()),
            Showing::Found(Vec::new()),
            Showing::Found(vec![site("One", "https://one.example/", "")]),
        ] {
            let said = found.searched("x", showing);
            assert!(said.starts_with("The search"), "{said}");
            assert!(!said.to_lowercase().contains("brave"), "{said}");
        }
    }
}
