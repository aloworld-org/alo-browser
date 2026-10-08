/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a document is owed of its linked style sheets, and how many it may
//! ask for (ADR 0035 §§ 1 and 4, queue item 348).
//!
//! As [`crate::fetch_owed`] is for a page's fetches: every sheet a renderer
//! asks for is answered — made and checked, or refused — under the ask's
//! number, and [`Owed`] is the browser process's list of the numbers it still
//! owes one document, so that an answer for a document that has gone is
//! answered by nobody. A new load starts a new list.
//!
//! # The bound
//!
//! [`MOST_SHEETS`] over the document's life, **counting every ask** —
//! refused ones too — whatever the renderer says: an honest renderer asks
//! each URL once and stops at the same number ([`crate::linked`]), so an ask
//! past it is either a renderer that was taken over or a page that will not
//! stop adding links, and each would otherwise be a request in the process
//! that may not crash. An ask past the bound is refused, owed its answer, and
//! counted in one line among the answer's issues.

use alo_net::cause::Cause;

use crate::fetch_decide::Asker;
use crate::navigate::LONGEST_SAID;
use crate::said;
use crate::sheet::{MOST_SHEETS, SheetAsk};
use crate::sheet_decide::{self, Decided, Refusal, Rule};

/// The sheets one document is still owed an answer for, and how many it has
/// asked for.
#[derive(Debug, Clone, Default)]
pub struct Owed {
    /// Every ask the document has made, refused ones included.
    asked: usize,
    /// The numbers owed an answer.
    owed: Vec<u64>,
}

impl Owed {
    /// How many sheets are waiting for an answer.
    pub fn waiting(&self) -> usize {
        self.owed.len()
    }

    /// Whether `number` is owed an answer.
    pub fn owes(&self, number: u64) -> bool {
        self.owed.contains(&number)
    }

    /// Strike one answer for `number` off the list. Whether it was on it.
    pub fn settle(&mut self, number: u64) -> bool {
        if let Some(at) = self.owed.iter().position(|owed| *owed == number) {
            self.owed.swap_remove(at);
            return true;
        }
        false
    }

    /// Decide every sheet in one answer, in order, under the bound — each
    /// from `asker` and under `cause` — and owe each an answer. What comes
    /// back is the decisions, in the asks' order, and the lines to say among
    /// the answer's issues.
    pub fn decide(
        &mut self,
        asks: &[SheetAsk],
        asker: &Asker<'_>,
        cause: &Cause,
    ) -> (Vec<Decided>, Vec<String>) {
        let mut decisions = Vec::new();
        let mut lines = Vec::new();
        let mut past = 0_usize;
        for ask in asks {
            self.asked = self.asked.saturating_add(1);
            let decided = if self.asked > MOST_SHEETS {
                past += 1;
                Decided::Refused(Box::new(Refusal {
                    number: ask.number,
                    asked: ask.url.chars().take(LONGEST_SAID).collect(),
                    // Not parsed: an ask past the bound is not looked at,
                    // which is the bound's whole point.
                    url: None,
                    rule: Rule::TooMany,
                    cause: cause.clone(),
                }))
            } else {
                let decided = sheet_decide::decide(ask, asker, cause);
                if let Decided::Refused(refusal) = &decided {
                    lines.push(said::line(refusal));
                }
                decided
            };
            self.owed.push(decided.number());
            decisions.push(decided);
        }
        if past > 0 {
            lines.push(said::line(&format_args!(
                "{past} of the page's style sheets were refused: {}",
                Rule::TooMany
            )));
        }
        (decisions, lines)
    }
}

#[cfg(test)]
mod tests {
    use alo_net::cause::Identities;
    use alo_net::cors::{Credentials, Mode};
    use alo_net::csp::Policies;
    use alo_url::Url;

    use super::*;

    fn ask(number: u64) -> SheetAsk {
        SheetAsk {
            number,
            url: format!("https://shop.example/{number}.css"),
            mode: Mode::NoCors,
            credentials: Credentials::Include,
            referrer: None,
            nonce: None,
        }
    }

    fn decide(owed: &mut Owed, asks: &[SheetAsk]) -> (Vec<Decided>, Vec<String>) {
        let page: Url = alo_url::parse("https://shop.example/").unwrap();
        let policies = Policies::none();
        let cause = Cause::Document {
            document: Identities::default().a_document(),
        };
        owed.decide(
            asks,
            &Asker {
                url: &page,
                policies: &policies,
            },
            &cause,
        )
    }

    #[test]
    fn every_sheet_is_decided_in_order_and_owed_its_answer_once() {
        let mut owed = Owed::default();
        let mut refused = ask(2);
        refused.url = "ftp://shop.example/x.css".to_owned();
        let (decided, lines) = decide(&mut owed, &[ask(1), refused]);
        assert_eq!(
            decided.iter().map(Decided::number).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(matches!(decided[0], Decided::Make(_)));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("ftp:"), "{lines:?}");
        assert_eq!(owed.waiting(), 2, "a refusal is owed its answer too");
        assert!(owed.settle(2));
        assert!(!owed.settle(2), "answered twice");
        assert!(owed.owes(1) && !owed.owes(2));
        assert!(!owed.settle(99), "never asked for");
    }

    #[test]
    fn past_the_bound_every_ask_is_refused_unread_and_counted_once_an_answer() {
        let mut owed = Owed::default();
        let asks: Vec<SheetAsk> = (0..MOST_SHEETS as u64 - 1).map(ask).collect();
        let (decided, lines) = decide(&mut owed, &asks);
        assert!(decided.iter().all(|one| matches!(one, Decided::Make(_))));
        assert!(lines.is_empty());
        // The last allowed, then three past it, in one answer.
        let more: Vec<SheetAsk> = (1000..1004).map(ask).collect();
        let (decided, lines) = decide(&mut owed, &more);
        assert!(matches!(decided[0], Decided::Make(_)));
        for past in &decided[1..] {
            let Decided::Refused(refusal) = past else {
                panic!("made past the bound");
            };
            assert_eq!(refusal.rule, Rule::TooMany);
            assert_eq!(refusal.url, None, "not looked at");
        }
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("3 of the page's style sheets"),
            "{lines:?}"
        );
        assert_eq!(owed.waiting(), MOST_SHEETS + 3);
        // Settling does not give a document more to ask for.
        for number in 0..MOST_SHEETS as u64 - 1 {
            assert!(owed.settle(number));
        }
        let (decided, _) = decide(&mut owed, &[ask(2000)]);
        assert!(matches!(&decided[0], Decided::Refused(refusal) if refusal.rule == Rule::TooMany));
    }
}
