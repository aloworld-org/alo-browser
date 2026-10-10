/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page has asked to fetch, and the promises waiting for the answers
//! (ADR 0032 § 1, queue item 335).
//!
//! A script's `fetch` is an **ask**, never a call: it is recorded here, in
//! the document cell, under a number this cell chose, and the promise
//! `fetch` answered waits here under the same number. The renderer takes
//! every ask when the message's work is done ([`take`]) and puts them in its
//! answer, all of them, in the order they were made — a page that fetches
//! three things wants three things. The answer to each comes back later as
//! a message of its own, and [`crate::delivering`] settles the promise
//! waiting under its number.
//!
//! The number is unique for the life of the document, because the cell
//! lives exactly as long as the document does: a new page is a new heap and
//! a new cell, so an answer for an ask the last page made finds nothing
//! waiting, and **a page that goes lets go of every promise it was owed**
//! with the heap that held them.
//!
//! # What it costs, and who counts it
//!
//! Every ask is the page's bytes held outside the heap's slots — its URL,
//! headers and body — so they are counted in the cell's footprint, and a
//! page that fetches without end meets the heap's ceiling rather than this
//! process's memory. What the asks waiting to be taken may add up to is
//! bounded too, by [`MOST_ASKED_BYTES`], because they all cross in one
//! answer and an answer is one message.
//!
//! # What may outlive the page
//!
//! An ask may claim to **outlive its document** ([`Keepalive`], ADR 0040
//! § 2): a beacon, which the browser process makes even after the page has
//! gone. A page may have at most [`MOST_KEPT_ALIVE`] bytes of such bodies in
//! flight (§ 4), Fetch's own number, and this cell counts them from the ask
//! until its answer arrives ([`answered`]), so that `sendBeacon`
//! can answer `false` before it returns. The count is the page's to be told
//! by; the browser process counts again and does not believe this one.

use alo_js::heap::{Barrier, Ref, Tracer};
use alo_js::object::Objects;
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;
use alo_net::referrer::Policy;
use alo_url::Url;

use crate::document_cell::DocumentCell;

/// The most bytes the asks waiting to be taken may hold between them — URLs,
/// methods, headers and bodies.
///
/// They cross in one answer, and an answer is one message of at most 64 MiB
/// (`alo-renderer`'s `LARGEST_MESSAGE`). Half of that leaves the rest of the
/// answer — what the page's script said, the fonts it wanted — room it can
/// never be crowded out of. A fetch past it rejects as a fetch that failed;
/// the next message's answer has room again.
pub const MOST_ASKED_BYTES: usize = 32 * 1024 * 1024;

/// The most bytes of body a document's keep-alive asks may have in flight
/// together: 64 KiB, Fetch's own number (ADR 0040 § 4), which pages are
/// written against. The renderer counts it so a page is told; the browser
/// process counts it again because the renderer's count is a claim.
pub const MOST_KEPT_ALIVE: usize = 64 * 1024;

/// Whether an ask outlives its document, and as what (ADR 0040 §§ 2–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keepalive {
    /// It does not: once its page has gone, it is not made.
    #[default]
    Not,
    /// A fetch with `keepalive` set, which is still a fetch (queue item 375:
    /// no page's bindings ask one yet).
    Fetch,
    /// `navigator.sendBeacon`'s, recorded as a beacon.
    Beacon,
}

impl Keepalive {
    /// Whether it may be made after its page has gone.
    pub const fn outlives(self) -> bool {
        !matches!(self, Keepalive::Not)
    }
}

/// One fetch a page asked for, as its bindings made it: every forbidden
/// header already dropped, every value already one the request steps
/// accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The number the promise waits under.
    pub number: u64,
    /// Where to, resolved against the document's base URL.
    pub url: Url,
    /// The method, normalised as Fetch normalises it.
    pub method: String,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body's bytes; empty for none.
    pub body: Vec<u8>,
    /// Whether the page means to read the answer.
    pub mode: Mode,
    /// Whether the request may carry who the person is.
    pub credentials: Credentials,
    /// What a redirect should do.
    pub redirect: redirect::Mode,
    /// The referrer policy the page asked for, if it asked for one.
    pub referrer: Option<Policy>,
    /// Whether it may outlive its page.
    pub keepalive: Keepalive,
}

impl Asked {
    /// The bytes it holds, as [`MOST_ASKED_BYTES`] counts them.
    pub fn bytes(&self) -> usize {
        let headers: usize = self
            .headers
            .iter()
            .map(|(name, value)| name.len().saturating_add(value.len()))
            .sum();
        self.url
            .serialised
            .len()
            .saturating_add(self.method.len())
            .saturating_add(headers)
            .saturating_add(self.body.len())
    }
}

/// The document's fetches: what it has asked and not yet been taken, and
/// what it waits for.
#[derive(Debug, Default)]
pub struct Fetches {
    /// The number the next ask gets.
    next: u64,
    /// Asks made since the renderer last took them, oldest first.
    asked: Vec<Asked>,
    /// What [`Fetches::asked`] holds, in bytes.
    asked_bytes: usize,
    /// The promise waiting for each ask's answer, oldest first.
    waiting: Vec<(u64, Ref)>,
    /// The function a delivery calls, made once by [`crate::fetch::offer`].
    settle: Option<Ref>,
    /// Each keep-alive ask whose answer has not arrived, with its body's
    /// bytes: what [`MOST_KEPT_ALIVE`] counts.
    kept_alive: Vec<(u64, usize)>,
}

impl Fetches {
    /// Whether an ask of `bytes` more would fit beside those already waiting
    /// to be taken.
    pub const fn has_room_for(&self, bytes: usize) -> bool {
        self.asked_bytes.saturating_add(bytes) <= MOST_ASKED_BYTES
    }

    /// The number the next ask will get.
    pub const fn next_number(&self) -> u64 {
        self.next
    }

    /// The bytes of body the document's keep-alive asks have in flight.
    pub fn kept_alive(&self) -> usize {
        self.kept_alive
            .iter()
            .fold(0, |sum, (_, bytes)| sum.saturating_add(*bytes))
    }

    /// Whether a keep-alive body of `bytes` more stays within
    /// [`MOST_KEPT_ALIVE`] beside those in flight.
    pub fn may_keep_alive(&self, bytes: usize) -> bool {
        self.kept_alive().saturating_add(bytes) <= MOST_KEPT_ALIVE
    }

    /// Record `asked`, whose number must be [`Fetches::next_number`]'s, and
    /// count its body if it outlives its page.
    pub(crate) fn ask(&mut self, asked: Asked) {
        self.next = self.next.saturating_add(1);
        self.asked_bytes = self.asked_bytes.saturating_add(asked.bytes());
        if asked.keepalive.outlives() {
            self.kept_alive.push((asked.number, asked.body.len()));
        }
        self.asked.push(asked);
    }

    /// The answer to ask `number` has arrived: take its body off the
    /// keep-alive count if it was counted. Whether it was.
    pub(crate) fn answered(&mut self, number: u64) -> bool {
        let Some(at) = self.kept_alive.iter().position(|(held, _)| *held == number) else {
            return false;
        };
        self.kept_alive.swap_remove(at);
        true
    }

    /// Wait for the answer to ask `number` with `promise`, through the
    /// barrier every store of a reference passes.
    pub(crate) fn wait(&mut self, barrier: &mut Barrier, number: u64, promise: Ref) {
        barrier.stored(None, Some(promise));
        self.waiting.push((number, promise));
    }

    /// Stop waiting for ask `number`: the promise that was, if one was.
    pub(crate) fn stop_waiting(&mut self, number: u64) -> Option<Ref> {
        let at = self.waiting.iter().position(|(held, _)| *held == number)?;
        Some(self.waiting.remove(at).1)
    }

    /// How many answers the page is waiting for.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// Whether anything waits for ask `number`.
    pub fn waits_for(&self, number: u64) -> bool {
        self.waiting.iter().any(|(held, _)| *held == number)
    }

    /// The function a delivery calls, once it is made.
    pub(crate) const fn settle(&self) -> Option<Ref> {
        self.settle
    }

    /// Keep the function a delivery calls.
    pub(crate) fn set_settle(&mut self, barrier: &mut Barrier, settle: Ref) {
        barrier.stored(self.settle, Some(settle));
        self.settle = Some(settle);
    }

    /// Every promise waiting, and the delivery's function, as strong edges
    /// of the cell: a promise a page is owed an answer for lives until the
    /// answer comes or the page goes.
    pub(crate) fn trace(&self, tracer: &mut Tracer) {
        for (_, promise) in &self.waiting {
            tracer.edge(*promise);
        }
        if let Some(settle) = self.settle {
            tracer.edge(settle);
        }
    }

    /// What it owns beyond the cell's slot.
    pub(crate) fn footprint(&self) -> usize {
        self.asked_bytes
            .saturating_add(self.asked.capacity().saturating_mul(size_of::<Asked>()))
            .saturating_add(
                self.waiting
                    .capacity()
                    .saturating_mul(size_of::<(u64, Ref)>()),
            )
            .saturating_add(
                self.kept_alive
                    .capacity()
                    .saturating_mul(size_of::<(u64, usize)>()),
            )
    }
}

/// The answer to ask `number` has arrived at the document `cell` holds:
/// take its body off the keep-alive count if it was a keep-alive ask (ADR
/// 0040 § 4). Whether it was — so that an answer nothing waits for, which a
/// beacon's always is, is known to be expected — or [`None`] if `cell` is
/// not a document cell.
pub fn answered(objects: &mut Objects, cell: Ref, number: u64) -> Option<bool> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| held.fetches.answered(number))
}

/// Take every ask the page has made since this was last called, oldest
/// first, leaving none — [`None`] if `cell` is not a document cell.
pub fn take(objects: &mut Objects, cell: Ref) -> Option<Vec<Asked>> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| {
        held.fetches.asked_bytes = 0;
        core::mem::take(&mut held.fetches.asked)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(number: u64, body: usize) -> Asked {
        Asked {
            number,
            url: alo_url::parse("https://example.com/a").unwrap(),
            method: "GET".to_owned(),
            headers: vec![("accept".to_owned(), "text/plain".to_owned())],
            body: vec![b'x'; body],
            mode: Mode::Cors,
            credentials: Credentials::SameOrigin,
            redirect: redirect::Mode::Follow,
            referrer: None,
            keepalive: Keepalive::Not,
        }
    }

    #[test]
    fn an_ask_counts_every_byte_it_holds() {
        assert_eq!(asked(0, 5).bytes(), 21 + 3 + 16 + 5);
    }

    #[test]
    fn numbers_go_up_and_the_bytes_waiting_are_bounded() {
        let mut fetches = Fetches::default();
        assert_eq!(fetches.next_number(), 0);
        fetches.ask(asked(0, 10));
        assert_eq!(fetches.next_number(), 1);
        assert!(fetches.has_room_for(MOST_ASKED_BYTES - 50));
        assert!(!fetches.has_room_for(MOST_ASKED_BYTES));
    }

    #[test]
    fn a_keep_alive_body_is_counted_until_its_answer_arrives() {
        let mut fetches = Fetches::default();
        let mut beacon = asked(0, MOST_KEPT_ALIVE - 1);
        beacon.keepalive = Keepalive::Beacon;
        assert!(fetches.may_keep_alive(MOST_KEPT_ALIVE));
        fetches.ask(beacon);
        // An ordinary fetch is not counted, whatever its body.
        fetches.ask(asked(1, 10));
        assert_eq!(fetches.kept_alive(), MOST_KEPT_ALIVE - 1);
        assert!(fetches.may_keep_alive(1));
        assert!(!fetches.may_keep_alive(2));
        assert!(!fetches.answered(1), "not a keep-alive ask");
        assert!(fetches.answered(0));
        assert!(!fetches.answered(0), "answered once");
        assert_eq!(fetches.kept_alive(), 0);
        assert!(fetches.may_keep_alive(MOST_KEPT_ALIVE));
        assert!(!fetches.may_keep_alive(MOST_KEPT_ALIVE + 1));
    }
}
