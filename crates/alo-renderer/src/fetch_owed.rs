/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a document is owed, and how much it may be owed at once (ADR 0032
//! §§ 1 and 3, queue item 334).
//!
//! Every ask a renderer sends is answered — made and filtered, or refused —
//! and the answer goes back under the ask's number. [`Owed`] is the browser
//! process's list of the numbers it still owes one document an answer for, so
//! that an answer for a document that has gone is answered by nobody (§ 1): a
//! new load starts a new list, and an answer whose number is not on it is not
//! sent.
//!
//! # The two bounds
//!
//! A page decides how many asks it makes, and every one it is allowed is
//! memory and a socket in the process that may not crash. So the asks in one
//! answer and the fetches waiting for one document are each bounded **here**,
//! whatever the renderer says, and an ask past either is a network error said
//! among the issues. Refused asks are owed an answer too — the page's promise
//! is waiting for one — and cost a number each until it is given.
//!
//! An ask that claims to outlive its page is bounded once more, after it is
//! decided, by the bytes of body the document's keep-alive requests may
//! have in flight ([`KeptAlive`], ADR 0040 § 4). The two bounds above cover
//! it too, empty bodies included, so a page cannot leave a flood of empty
//! beacons behind it.

use alo_net::cause::Cause;

use crate::fetch::FetchAsk;
use crate::fetch_decide::{self, Asker, Decided, Refusal, Rule};
use crate::fetch_kept::KeptAlive;
use crate::navigate::LONGEST_SAID;
use crate::said;

/// The most fetches one document may have waiting at once: 64.
///
/// The most connections `alo-net`'s pool keeps idle for every site together,
/// so a document cannot have more requests in flight than this process keeps
/// sockets for all of them. The only frozen page that fetches,
/// `alo-downloads`, asks for two at once. A real page that needs more opens an
/// item to raise this, with the page frozen beside it.
pub const MOST_IN_FLIGHT: usize = 64;

/// The most asks one answer may carry that are decided: the same 64, since an
/// answer asking for more than may be in flight is refusing its own excess.
pub const MOST_ASKED_AT_ONCE: usize = MOST_IN_FLIGHT;

/// The numbers one document is still owed an answer for.
#[derive(Debug, Clone, Default)]
pub struct Owed {
    /// Asks made, waiting for their response.
    made: Vec<u64>,
    /// Asks refused, waiting for their network error.
    refused: Vec<u64>,
}

impl Owed {
    /// How many fetches are waiting for a response.
    pub fn in_flight(&self) -> usize {
        self.made.len()
    }

    /// Whether `number` is owed an answer.
    pub fn owes(&self, number: u64) -> bool {
        self.made.contains(&number) || self.refused.contains(&number)
    }

    /// Strike one answer for `number` off the list. Whether it was on it.
    ///
    /// One, not every: a renderer that reused a number while its first ask
    /// was waiting is owed two answers and confuses only its own page.
    pub fn settle(&mut self, number: u64) -> bool {
        for list in [&mut self.made, &mut self.refused] {
            if let Some(at) = list.iter().position(|owed| *owed == number) {
                list.swap_remove(at);
                return true;
            }
        }
        false
    }

    /// Decide every ask in one answer, in order, under the bounds — each
    /// from `asker` and under `cause`, a keep-alive one counted in `kept`
    /// under `cause`'s document — and owe each an answer. What comes back is
    /// the decisions, in the asks' order, and the lines to say among the
    /// answer's issues.
    pub fn decide(
        &mut self,
        asks: &[FetchAsk],
        asker: &Asker<'_>,
        cause: &Cause,
        kept: &mut KeptAlive,
    ) -> (Vec<Decided>, Vec<String>) {
        let mut decisions = Vec::new();
        let mut lines = Vec::new();
        let (mut at_once, mut waiting) = (0_usize, 0_usize);
        for (place, ask) in asks.iter().enumerate() {
            let bounded = if place >= MOST_ASKED_AT_ONCE {
                at_once += 1;
                Some(Rule::TooManyAtOnce)
            } else if self.made.len() >= MOST_IN_FLIGHT {
                waiting += 1;
                Some(Rule::TooManyWaiting)
            } else {
                None
            };
            let decided = if let Some(rule) = bounded {
                Decided::Refused(Box::new(Refusal {
                    number: ask.number,
                    asked: ask.url.chars().take(LONGEST_SAID).collect(),
                    // Not parsed: an ask past a bound is not looked at, which
                    // is the bound's whole point.
                    url: None,
                    rule,
                    cause: cause.clone(),
                    purpose: fetch_decide::purpose(ask),
                }))
            } else {
                let decided = kept_within(fetch_decide::decide(ask, asker, cause), cause, kept);
                if let Decided::Refused(refusal) = &decided {
                    lines.push(said::line(refusal));
                }
                decided
            };
            match &decided {
                Decided::Make(fetch) => self.made.push(fetch.number),
                Decided::Refused(refusal) => self.refused.push(refusal.number),
            }
            decisions.push(decided);
        }
        if at_once > 0 {
            lines.push(said::line(&format_args!(
                "{at_once} of the page's fetches were refused: {}",
                Rule::TooManyAtOnce
            )));
        }
        if waiting > 0 {
            lines.push(said::line(&format_args!(
                "{waiting} of the page's fetches were refused: {}",
                Rule::TooManyWaiting
            )));
        }
        (decisions, lines)
    }
}

/// `decided`, counted in `kept` under `cause`'s document if it is a
/// keep-alive fetch to be made, or refused by [`Rule::KeptAlive`] if
/// counting it would pass the bound. Every cause a page's ask is given names
/// its document; one that named none would have nobody to count for.
fn kept_within(decided: Decided, cause: &Cause, kept: &mut KeptAlive) -> Decided {
    let Decided::Make(fetch) = decided else {
        return decided;
    };
    let Some(document) = cause.in_document().filter(|_| fetch.outlives()) else {
        return Decided::Make(fetch);
    };
    let (bytes, held) = (fetch.request.body.len(), kept.of(document));
    if kept.claim(document, bytes) {
        return Decided::Make(fetch);
    }
    let url = fetch.request.url;
    Decided::Refused(Box::new(Refusal {
        number: fetch.number,
        asked: url.serialised.chars().take(LONGEST_SAID).collect(),
        url: Some(url),
        rule: Rule::KeptAlive { bytes, held },
        cause: cause.clone(),
        purpose: fetch.request.purpose,
    }))
}

#[cfg(test)]
mod tests {
    use alo_bindings::fetching::Keepalive;
    use alo_net::cause::Identities;
    use alo_net::cors::{Credentials, Mode};
    use alo_net::csp::Policies;
    use alo_net::redirect;
    use alo_url::Url;

    use super::*;

    fn ask(number: u64) -> FetchAsk {
        FetchAsk {
            number,
            url: format!("https://shop.example/{number}"),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
            mode: Mode::Cors,
            credentials: Credentials::SameOrigin,
            redirect: redirect::Mode::Follow,
            referrer: None,
            keepalive: Keepalive::Not,
        }
    }

    /// A beacon of `bytes`, as `sendBeacon` asks one.
    fn beacon(number: u64, bytes: usize) -> FetchAsk {
        FetchAsk {
            method: "POST".to_owned(),
            headers: vec![(
                "Content-Type".to_owned(),
                "text/plain;charset=UTF-8".to_owned(),
            )],
            body: vec![b'x'; bytes],
            mode: Mode::NoCors,
            credentials: Credentials::Include,
            keepalive: Keepalive::Beacon,
            ..ask(number)
        }
    }

    fn cause() -> Cause {
        Cause::Document {
            document: Identities::default().a_document(),
        }
    }

    fn decide_kept(
        owed: &mut Owed,
        asks: &[FetchAsk],
        kept: &mut KeptAlive,
    ) -> (Vec<Decided>, Vec<String>) {
        let page: Url = alo_url::parse("https://shop.example/").unwrap();
        let policies = Policies::none();
        owed.decide(
            asks,
            &Asker {
                url: &page,
                policies: &policies,
            },
            &cause(),
            kept,
        )
    }

    fn decide(owed: &mut Owed, asks: &[FetchAsk]) -> (Vec<Decided>, Vec<String>) {
        decide_kept(owed, asks, &mut KeptAlive::default())
    }

    fn refused_by(decided: &Decided) -> Option<&Rule> {
        match decided {
            Decided::Refused(refusal) => Some(&refusal.rule),
            Decided::Make(_) => None,
        }
    }

    #[test]
    fn every_ask_is_decided_in_order_and_owed_its_answer_once() {
        let mut owed = Owed::default();
        let (decided, lines) = decide(&mut owed, &[ask(1), ask(2)]);
        assert_eq!(
            decided.iter().map(Decided::number).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(owed.in_flight(), 2);
        assert!(owed.settle(1));
        assert!(!owed.settle(1), "answered twice");
        assert!(!owed.owes(1) && owed.owes(2));
        assert!(!owed.settle(99), "never asked for");
    }

    #[test]
    fn a_refusal_is_owed_its_network_error_and_said() {
        let mut owed = Owed::default();
        let mut refused = ask(5);
        refused.url = "data:text/plain,hi".to_owned();
        let (decided, lines) = decide(&mut owed, &[refused]);
        assert!(matches!(refused_by(&decided[0]), Some(Rule::Scheme { .. })));
        assert_eq!(owed.in_flight(), 0);
        assert!(owed.owes(5));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("data:"), "{lines:?}");
    }

    #[test]
    fn an_answer_asking_past_the_bound_has_the_rest_refused_and_said_once() {
        let mut owed = Owed::default();
        let asks: Vec<FetchAsk> = (0..MOST_ASKED_AT_ONCE as u64 + 3).map(ask).collect();
        let (decided, lines) = decide(&mut owed, &asks);
        assert_eq!(decided.len(), asks.len(), "every ask is answered");
        assert!(
            decided[..MOST_ASKED_AT_ONCE]
                .iter()
                .all(|one| matches!(one, Decided::Make(_)))
        );
        assert!(
            decided[MOST_ASKED_AT_ONCE..]
                .iter()
                .all(|one| refused_by(one) == Some(&Rule::TooManyAtOnce))
        );
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("3 of the page's fetches were refused"),
            "{lines:?}"
        );
        assert_eq!(owed.in_flight(), MOST_ASKED_AT_ONCE);
    }

    #[test]
    fn a_document_with_the_most_waiting_has_its_next_ask_refused_until_one_is_answered() {
        let mut owed = Owed::default();
        let asks: Vec<FetchAsk> = (0..MOST_IN_FLIGHT as u64).map(ask).collect();
        let _ = decide(&mut owed, &asks);
        let (decided, lines) = decide(&mut owed, &[ask(1000)]);
        assert_eq!(refused_by(&decided[0]), Some(&Rule::TooManyWaiting));
        assert!(
            lines[0].contains("already has 64 fetches waiting"),
            "{lines:?}"
        );
        assert!(owed.settle(0));
        assert!(owed.settle(1000), "the refusal is owed its answer");
        let (decided, _) = decide(&mut owed, &[ask(1001)]);
        assert!(matches!(decided[0], Decided::Make(_)));
    }

    #[test]
    fn a_beacon_past_the_documents_keep_alive_bytes_is_refused_by_its_own_rule() {
        use crate::fetch_kept::MOST_KEPT_ALIVE;
        let mut owed = Owed::default();
        let mut kept = KeptAlive::default();
        // A whole 64 KiB, then one byte: the second is refused by this
        // process's own count, whatever the renderer's said, and owed its
        // answer like any refusal.
        let (decided, lines) = decide_kept(
            &mut owed,
            &[beacon(1, MOST_KEPT_ALIVE), beacon(2, 1), ask(3)],
            &mut kept,
        );
        let Decided::Make(made) = &decided[0] else {
            panic!("{:?}", decided[0]);
        };
        assert_eq!(made.request.purpose, alo_net::Purpose::Beacon);
        assert!(made.outlives());
        assert_eq!(
            refused_by(&decided[1]),
            Some(&Rule::KeptAlive {
                bytes: 1,
                held: MOST_KEPT_ALIVE
            })
        );
        assert!(
            matches!(decided[2], Decided::Make(_)),
            "a fetch is not counted"
        );
        assert!(owed.owes(2));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("65536"), "{lines:?}");
        let Decided::Refused(refusal) = &decided[1] else {
            panic!("not refused");
        };
        assert_eq!(refusal.purpose, alo_net::Purpose::Beacon);

        // Made, it is taken off, and the next fits.
        let document = cause().in_document().unwrap();
        kept.release(document, MOST_KEPT_ALIVE);
        let (decided, _) = decide_kept(&mut owed, &[beacon(4, 1)], &mut kept);
        assert!(matches!(decided[0], Decided::Make(_)));
        assert_eq!(kept.of(document), 1);
    }

    #[test]
    fn empty_beacons_are_bounded_by_the_bounds_every_ask_is() {
        let mut owed = Owed::default();
        let mut kept = KeptAlive::default();
        let asks: Vec<FetchAsk> = (0..=MOST_ASKED_AT_ONCE as u64)
            .map(|number| beacon(number, 0))
            .collect();
        let (decided, _) = decide_kept(&mut owed, &asks, &mut kept);
        assert_eq!(
            refused_by(&decided[MOST_ASKED_AT_ONCE]),
            Some(&Rule::TooManyAtOnce)
        );
        let (decided, _) = decide_kept(&mut owed, &[beacon(1000, 0)], &mut kept);
        assert_eq!(refused_by(&decided[0]), Some(&Rule::TooManyWaiting));
    }
}
