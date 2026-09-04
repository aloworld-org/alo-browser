/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What was asked for, and what happened — the record ADR 0012 keeps.
//!
//! [`crate::cause`] is what caused one request and [`crate::chain`] is the walk
//! back along it. Neither writes anything down: a cause travels with a request
//! and is gone when the request is. This is the file that keeps them, which is
//! ADR 0012 §§ 5 and 6.
//!
//! # Why it is not called `record`
//!
//! [`crate::record`] is already the bytes one cache entry is (ADR 0011). Two
//! files called the same thing is how somebody comes to read the wrong one, and
//! the two are as different as a browser has: one is a response somebody may be
//! served again, and this one is a list of everywhere they went.
//!
//! # What is kept, and what is refused
//!
//! ADR 0012 § 5, and the refusals are the half that matters. An [`Entry`] holds
//! **when**, the [`Cause`], the method, the URL, the [`Purpose`] and what
//! [`Happened`]. It is built in one place, from a [`Request`], reading those
//! fields and no others — so *never a body* and *never a header set* is what
//! the type is rather than what a caller remembers. A record holding `Cookie`
//! and `Authorization` is a file that logs somebody into their own bank.
//!
//! The full URL **is** kept: *"what it read is the whole question, and an
//! origin-only record cannot answer it."*
//!
//! # Walked, never assembled — so an entry keeps a cause and not a chain
//!
//! ADR 0012 § 3 says the chain is walked rather than assembled. A frozen chain
//! copied into every entry would be exactly the side table that section
//! refuses, and the day it disagreed with [`crate::chain::Documents`] the
//! record would still read like evidence. So an entry holds the one link the
//! request carried, and [`Entry::chain`] walks the rest against whatever the
//! browser process remembers.
//!
//! That is also why a chain read out of an old entry can end in
//! [`crate::chain::End::Forgotten`] — the documents are bounded too, and saying
//! *we knew and no longer do* is the honest answer rather than a gap.
//!
//! # For the session, in memory, bounded
//!
//! ADR 0012 § 6: *"Everything, for the session, in memory, bounded. It dies
//! with the process, which is the correct lifetime for a record of a session:
//! the pages are gone, and so is the list of them."*
//!
//! Two bounds, the pair [`crate::disk`] already uses and for the same reason:
//! [`MOST_ENTRIES`] because a session is long, and [`LARGEST_RECORD`] because
//! the size of a line is mostly the length of a URL and a URL is something a
//! page chooses. The oldest go first, and [`Activity::forgotten`] says how many
//! — a record that quietly shortened itself would read as a session in which
//! less happened.
//!
//! **What an agent did, kept until the person deletes it**, is the other half
//! of § 6 and is queue item 202. It is a different lifetime, a different bound
//! (in actions rather than in bytes) and a file on a disk under ADR 0011 § 3,
//! which is why it is not this file.
//!
//! # Who may read it
//!
//! ADR 0012 § 7, and it is kept by where this lives rather than by a check.
//! **The browser process only**: a renderer holds no [`crate::Pool`], nothing
//! crossing the boundary carries an [`Entry`] in either direction, and
//! `alo-agent` does not depend on this crate at all — so there is no type an
//! agent is handed that could carry one. A page has no reach further still.
//!
//! There is no API here that answers *has this been visited*, and there is not
//! going to be one: a record readable by script is a cross-site history oracle
//! handed out by the browser.

use crate::cause::Cause;
use crate::chain::{Chain, Documents};
use crate::request::{Purpose, Request};
use crate::response::Status;
use alo_url::Url;
use core::fmt;
use std::collections::VecDeque;
use std::time::SystemTime;

/// How many requests are remembered at once.
///
/// **A choice rather than a measurement**, in the shape every bound in
/// `alo-net` is made: a limit somebody else chooses is not a limit. Set where a
/// day of ordinary browsing fits inside it — a heavy page is a few hundred
/// requests — and a page fetching in a loop does not grow it without end.
pub const MOST_ENTRIES: usize = 8192;

/// How many bytes of them are held.
///
/// The second bound, because the first one is not enough on its own: what a
/// line costs is mostly the length of its URL, and a URL is something a page
/// chooses. Without this, [`MOST_ENTRIES`] requests for addresses a megabyte
/// long would be a record measured in gigabytes.
pub const LARGEST_RECORD: usize = 4 * 1024 * 1024;

/// The most characters kept of a reason.
///
/// A refusal and a failure are sentences **this engine composes**, and some of
/// them quote what a server sent — a header it could not read, a `Location` it
/// would not follow. So the quoting is bounded here rather than trusted to
/// every place that makes one: a server that could write a thousand lines into
/// a record is a server deciding how much memory this process uses, and one
/// that could bury a real line under its own is worse.
pub const LONGEST_REASON: usize = 200;

/// What became of one request.
///
/// ADR 0012 § 5's *what happened*: **the status, whether it was served from the
/// cache, or which rule refused it, by name, the way `alo-net` already refuses
/// things** — and one more, because *nothing came back at all* is not a status
/// and pretending otherwise would put a number in the record that no server
/// ever said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Happened {
    /// A server answered.
    Answered {
        /// What it said.
        status: Status,
        /// Whether the body arrived whole.
        ///
        /// A body that stopped early is still a server answering, so the status
        /// is what it said and this is the rest of the truth. A download asks
        /// for the remainder (queue item 154) and that ask is its own line.
        whole: bool,
    },
    /// The cache answered, and nothing was sent.
    ///
    /// Its own variant rather than an [`Happened::Answered`] with a flag,
    /// because *what did this page reach the network for* and *what did it
    /// load* are two questions somebody asks separately — and the second
    /// includes this one.
    Served {
        /// What was stored.
        status: Status,
    },
    /// A rule of this engine's refused it, by name.
    ///
    /// The request was made and never sent. That is a line rather than a
    /// silence: *what did this page try to load, and what stopped it* is most
    /// of why somebody opens the record at all.
    Refused {
        /// Which rule, in the words it refuses in.
        rule: String,
    },
    /// Nothing came back at all.
    ///
    /// Not a refusal of ours and not a status: there is no host, the
    /// certificate was not trusted, the connection went away. A record that
    /// filed these as an error status would be inventing an answer.
    Failed {
        /// What went wrong, in words.
        why: String,
    },
}

impl fmt::Display for Happened {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Happened::Answered {
                status,
                whole: true,
            } => write!(f, "answered {status}"),
            Happened::Answered {
                status,
                whole: false,
            } => write!(f, "answered {status}, and the body stopped early"),
            Happened::Served { status } => write!(f, "served from the cache, {status}"),
            Happened::Refused { rule } => write!(f, "refused: {rule}"),
            Happened::Failed { why } => write!(f, "did not happen: {why}"),
        }
    }
}

/// One request, and what became of it.
///
/// Built only by [`Activity::happened`], from a [`Request`], and holding the
/// six things ADR 0012 § 5 lists. The fields are private because the list is
/// the decision: a seventh added here is a diff somebody has to argue for,
/// which is what stops *a header, just this one* from ever being reasonable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    at: SystemTime,
    cause: Cause,
    method: String,
    url: Url,
    purpose: Purpose,
    happened: Happened,
    /// What this line costs, kept rather than recomputed — the same shape as
    /// [`crate::disk::Disk`]'s running total, and for the same reason: a bound
    /// that had to measure everything it held would measure it on every write.
    weighs: usize,
}

impl Entry {
    /// When it was asked for.
    pub fn at(&self) -> SystemTime {
        self.at
    }

    /// What caused it — the one link the request carried.
    pub fn cause(&self) -> &Cause {
        &self.cause
    }

    /// The method.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// What was asked for, whole.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// What kind of thing it was.
    pub fn purpose(&self) -> &Purpose {
        &self.purpose
    }

    /// What became of it.
    pub fn happened(&self) -> &Happened {
        &self.happened
    }

    /// How many bytes this line costs the record.
    pub fn weighs(&self) -> usize {
        self.weighs
    }

    /// Walk back from this request to what caused it, and so on.
    ///
    /// Against the browser process's [`Documents`] rather than against anything
    /// held here, which is ADR 0012 § 3's *walked rather than assembled*: there
    /// is one record of what caused each document's load, and this reads it
    /// instead of keeping a second one that could disagree with it.
    pub fn chain(&self, documents: &Documents) -> Chain {
        documents.chain(&self.cause)
    }
}

impl fmt::Display for Entry {
    /// The line a person reads in queue item 127's interface.
    ///
    /// The moment is not in it: a time is formatted where it is shown, in
    /// whatever the person's own reckoning of one is, and this crate has no
    /// business choosing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({}), caused by {} — {}",
            self.method, self.url, self.purpose, self.cause, self.happened,
        )
    }
}

/// What this browser asked for this session, and what happened.
///
/// One of these per browser process. It dies with the process, which ADR 0012
/// § 6 says is the correct lifetime for a record of a session.
#[derive(Debug)]
pub struct Activity {
    /// Oldest first, so the oldest is the one that goes.
    entries: VecDeque<Entry>,
    /// The total of every [`Entry::weighs`], kept rather than recomputed.
    bytes: usize,
    /// The bounds. Values rather than constants because a bound *is* a value —
    /// and because a test wants one it can reach without making eight thousand
    /// requests to find out.
    most: usize,
    largest: usize,
    /// How many lines have been dropped under the bounds.
    ///
    /// Counted rather than left implicit, for [`crate::chain::End::Forgotten`]'s
    /// reason: a record that quietly shortened itself would read as a session in
    /// which less happened.
    forgotten: usize,
}

impl Default for Activity {
    fn default() -> Self {
        Self::new()
    }
}

impl Activity {
    /// An empty record, bounded as this file's constants say.
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            most: MOST_ENTRIES,
            largest: LARGEST_RECORD,
            forgotten: 0,
        }
    }

    /// The same record, holding at most this many lines and this many bytes.
    ///
    /// A bound below what is already held is applied at once rather than argued
    /// with, the same as [`crate::disk::Disk::bounded_to`].
    #[must_use]
    pub fn bounded_to(mut self, entries: usize, bytes: usize) -> Self {
        self.most = entries;
        self.largest = bytes;
        self.stay_within_bounds();
        self
    }

    /// Write down that this request was made, and what became of it.
    ///
    /// **The only way a line gets into the record**, and it takes a [`Request`]
    /// rather than the pieces of one so that what is copied out of it is
    /// decided here — once — instead of at every call site. Six fields, and a
    /// body and a header set are not among them.
    ///
    /// `at` is when it was asked, and it is the caller's: nothing here reads a
    /// clock, which is the shape [`crate::cache`] already has and which is what
    /// makes a test of this able to name a moment.
    ///
    /// # One line per request, written when the answer is known
    ///
    /// This engine fetches synchronously, so there is no moment at which a
    /// request is outstanding and somebody could be reading — which is why the
    /// line is written once, with its outcome, rather than opened and closed.
    /// Concurrent loads would need the second half of that, and it is written
    /// here rather than discovered later.
    pub fn happened(&mut self, request: &Request, at: SystemTime, happened: Happened) {
        // Everything this record ever holds is chosen on these six lines. What
        // is *not* here is the decision: `request.headers` and `request.body`
        // are in scope and are not read.
        let happened = match happened {
            Happened::Refused { rule } => Happened::Refused {
                rule: shortened(&rule),
            },
            Happened::Failed { why } => Happened::Failed {
                why: shortened(&why),
            },
            answered => answered,
        };
        let entry = Entry {
            at,
            cause: request.cause.clone(),
            method: request.method.clone(),
            url: request.url.clone(),
            purpose: request.purpose.clone(),
            weighs: weight_of(&request.method, &request.url, &happened),
            happened,
        };
        self.bytes = self.bytes.saturating_add(entry.weighs);
        self.entries.push_back(entry);
        self.stay_within_bounds();
    }

    /// Every line, oldest first.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &Entry> + ExactSizeIterator {
        self.entries.iter()
    }

    /// The most recent line, if there is one.
    pub fn latest(&self) -> Option<&Entry> {
        self.entries.back()
    }

    /// How many lines are held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many bytes they cost.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// How many lines have been dropped under the bounds this session.
    pub fn forgotten(&self) -> usize {
        self.forgotten
    }

    /// Forget everything.
    ///
    /// What "clear this browsing data" has to be able to do, and what a person
    /// who wants the record gone reaches for. The count of what was dropped
    /// goes with it: a record somebody deleted must not keep saying how much
    /// there used to be.
    pub fn empty(&mut self) {
        self.entries.clear();
        self.bytes = 0;
        self.forgotten = 0;
    }

    /// Drop the oldest until both bounds are met.
    fn stay_within_bounds(&mut self) {
        while self.entries.len() > self.most || (self.bytes > self.largest && !self.is_empty()) {
            let Some(oldest) = self.entries.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(oldest.weighs);
            self.forgotten = self.forgotten.saturating_add(1);
        }
    }
}

/// What one line costs, near enough for a bound.
///
/// The fixed part of an [`Entry`] plus the text hanging off it. Not exact —
/// `SystemTime` and [`Cause`] are counted by their type and a [`Url`] holds its
/// pieces separately — and it does not need to be: a bound is a promise that
/// this cannot grow without end, not an accounting of the allocator.
fn weight_of(method: &str, url: &Url, happened: &Happened) -> usize {
    let said = match happened {
        Happened::Refused { rule } => rule.len(),
        Happened::Failed { why } => why.len(),
        Happened::Answered { .. } | Happened::Served { .. } => 0,
    };
    size_of::<Entry>()
        .saturating_add(method.len())
        .saturating_add(url.to_string().len())
        .saturating_add(said)
}

/// The first [`LONGEST_REASON`] characters of a sentence, and a mark when there
/// were more.
///
/// By characters rather than by bytes, so that a reason quoting a name in a
/// script other than this one is cut between letters instead of through one.
fn shortened(text: &str) -> String {
    if text.char_indices().nth(LONGEST_REASON).is_none() {
        return text.to_owned();
    }
    let mut kept: String = text.chars().take(LONGEST_REASON).collect();
    kept.push('…');
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cause::{Identities, TabId};
    use crate::headers::Headers;
    use std::time::Duration;

    fn url(text: &str) -> Url {
        match alo_url::parse(text) {
            Ok(url) => url,
            Err(why) => panic!("{text} is not a URL: {why}"),
        }
    }

    /// A moment, named rather than read off a clock. Nothing here reads one.
    fn moment(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn a_tab() -> TabId {
        Identities::default().a_tab()
    }

    fn asked_for(text: &str, cause: Cause) -> Request {
        Request::get(url(text), cause)
    }

    fn answered() -> Happened {
        Happened::Answered {
            status: Status::OK,
            whole: true,
        }
    }

    // --- What is recorded, and what is not (ADR 0012 § 5) --------------------

    #[test]
    fn a_line_holds_the_six_things_the_decision_lists() {
        let mut minting = Identities::default();
        let document = minting.a_document();
        let request = asked_for("https://example.com/a.css", Cause::Document { document })
            .for_purpose(Purpose::Style);
        let mut record = Activity::new();

        record.happened(&request, moment(1_700_000_000), answered());

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        assert_eq!(line.at(), moment(1_700_000_000));
        assert_eq!(line.cause(), &Cause::Document { document });
        assert_eq!(line.method(), "GET");
        assert_eq!(line.url(), &url("https://example.com/a.css"));
        assert_eq!(line.purpose(), &Purpose::Style);
        assert_eq!(line.happened(), &answered());
        assert_eq!(record.len(), 1);
        assert!(!record.is_empty());
    }

    /// The half of § 5 that matters. A record holding `Cookie` and
    /// `Authorization` is a file that logs somebody into their own bank, and
    /// one holding a body holds whatever they typed into a form.
    ///
    /// Asserted against the **whole** of what an entry can be made to say —
    /// its own words and its `Debug` — rather than against the fields it
    /// happens to have, because a field added later would pass a test that only
    /// checked the fields.
    #[test]
    fn nothing_a_page_or_a_person_put_in_a_request_reaches_the_record() {
        let mut sending = Request::sending(
            url("https://bank.example/transfer"),
            "POST",
            b"amount=500&to=someone".to_vec(),
            Cause::Person { tab: a_tab() },
        );
        sending.headers = Headers::new();
        sending.headers.add("Cookie", "session=hunter2");
        sending
            .headers
            .add("Authorization", "Bearer sk-live-secret");
        let mut record = Activity::new();

        record.happened(&sending, moment(1), answered());

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        for held in [line.to_string(), format!("{line:?}")] {
            assert!(
                !held.contains("hunter2"),
                "a cookie reached the record: {held}"
            );
            assert!(!held.contains("sk-live-secret"), "a credential: {held}");
            assert!(!held.contains("amount=500"), "a body: {held}");
            assert!(!held.contains("Bearer"), "a credential: {held}");
        }
        assert!(
            line.to_string().contains("bank.example/transfer"),
            "the URL is kept whole, which is the whole question: {line}",
        );
    }

    /// A cache hit is a line too: *what did this page load* includes what it
    /// did not go to the network for.
    #[test]
    fn what_the_cache_answered_is_recorded_as_well_as_what_a_server_did() {
        let mut record = Activity::new();
        let request = asked_for(
            "https://example.com/logo.png",
            Cause::Person { tab: a_tab() },
        )
        .for_purpose(Purpose::Image);

        record.happened(&request, moment(1), answered());
        record.happened(&request, moment(2), Happened::Served { status: Status::OK });

        let said: Vec<String> = record.entries().map(ToString::to_string).collect();
        assert_eq!(said.len(), 2);
        assert!(
            said.first()
                .is_some_and(|line| line.ends_with("answered 200"))
        );
        assert!(
            said.last()
                .is_some_and(|line| line.ends_with("served from the cache, 200")),
        );
    }

    /// Four things that can become of a request, each said in words a person
    /// reads rather than a code they look up.
    #[test]
    fn every_outcome_says_what_it_was_in_words() {
        assert_eq!(answered().to_string(), "answered 200");
        assert_eq!(
            Happened::Answered {
                status: Status(206),
                whole: false,
            }
            .to_string(),
            "answered 206, and the body stopped early",
        );
        assert_eq!(
            Happened::Served {
                status: Status(304),
            }
            .to_string(),
            "served from the cache, 304",
        );
        assert_eq!(
            Happened::Refused {
                rule: "redirected in a circle, back to https://example.com/".to_owned(),
            }
            .to_string(),
            "refused: redirected in a circle, back to https://example.com/",
        );
        assert_eq!(
            Happened::Failed {
                why: "there is no such host".to_owned(),
            }
            .to_string(),
            "did not happen: there is no such host",
        );
    }

    #[test]
    fn a_line_reads_as_a_sentence() {
        let mut minting = Identities::default();
        let action = minting.an_action();
        let document = minting.a_document();
        let mut record = Activity::new();
        record.happened(
            &asked_for(
                "https://example.com/next",
                Cause::Agent { action, document },
            )
            .for_purpose(Purpose::Fetch),
            moment(1),
            answered(),
        );

        assert_eq!(
            record.latest().map(ToString::to_string),
            Some(
                "GET https://example.com/next (fetch), caused by action#0, in document#0 \
                 — answered 200"
                    .to_owned()
            ),
        );
    }

    // --- The chain, walked rather than kept (ADR 0012 § 3) -------------------

    /// The question the record exists to answer: *did this follow from what the
    /// agent did*. The entry holds one link; the walk is against the browser
    /// process's own [`Documents`], so there is no second copy to disagree.
    #[test]
    fn a_line_walks_back_to_the_agent_action_that_caused_it() {
        let mut minting = Identities::default();
        let mut documents = Documents::default();
        let tab = minting.a_tab();
        let read = documents.opened(&mut minting, Cause::Person { tab });
        let action = minting.an_action();
        let opened = documents.opened(
            &mut minting,
            Cause::Agent {
                action,
                document: read,
            },
        );
        let mut record = Activity::new();
        record.happened(
            &asked_for(
                "https://example.com/app.js",
                Cause::Document { document: opened },
            )
            .for_purpose(Purpose::Script),
            moment(1),
            answered(),
        );

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        let chain = line.chain(&documents);
        assert_eq!(chain.action(), Some(action), "{chain}");
        assert!(chain.followed_from(action));
        assert_eq!(chain.person(), Some(tab));
        assert!(chain.is_whole());
    }

    /// And the other direction, which is what makes the first worth anything: a
    /// page a person opened themselves reaches no action at all.
    #[test]
    fn a_line_from_a_persons_own_browsing_reaches_no_action() {
        let mut minting = Identities::default();
        let mut documents = Documents::default();
        let tab = minting.a_tab();
        let opened = documents.opened(&mut minting, Cause::Person { tab });
        let mut record = Activity::new();
        record.happened(
            &asked_for(
                "https://example.com/style.css",
                Cause::Document { document: opened },
            ),
            moment(1),
            answered(),
        );

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        assert_eq!(line.chain(&documents).action(), None);
        assert_eq!(line.chain(&documents).person(), Some(tab));
    }

    // --- Bounded, and honest about it (ADR 0012 § 6) -------------------------

    #[test]
    fn the_oldest_lines_go_first_and_the_record_says_how_many() {
        let mut record = Activity::new().bounded_to(4, LARGEST_RECORD);
        for asked in 0..10u64 {
            record.happened(
                &asked_for(
                    &format!("https://example.com/{asked}"),
                    Cause::Person { tab: a_tab() },
                ),
                moment(asked),
                answered(),
            );
        }

        assert_eq!(record.len(), 4);
        assert_eq!(record.forgotten(), 6);
        assert_eq!(
            record.entries().next().map(Entry::at),
            Some(moment(6)),
            "the oldest kept is not the one after the last dropped",
        );
        assert_eq!(record.latest().map(Entry::at), Some(moment(9)));
    }

    /// The bound the count alone does not give: a line costs what its URL is
    /// long, and a page chooses that.
    #[test]
    fn a_page_with_enormous_addresses_cannot_grow_the_record_without_end() {
        let mut record = Activity::new().bounded_to(MOST_ENTRIES, 16 * 1024);
        let enormous = format!("https://example.com/{}", "a".repeat(4096));
        for asked in 0..64u64 {
            record.happened(
                &asked_for(&enormous, Cause::Person { tab: a_tab() }),
                moment(asked),
                answered(),
            );
        }

        assert!(record.len() < 8, "{} lines were kept", record.len());
        assert!(record.bytes() <= 16 * 1024, "{} bytes", record.bytes());
        assert!(record.forgotten() >= 56);
        assert!(
            !record.is_empty(),
            "the bound emptied it rather than trimmed it"
        );
    }

    /// A bound smaller than one line is honoured rather than argued with, the
    /// same as [`crate::disk::Disk::bounded_to`]: it holds nothing.
    #[test]
    fn a_record_bounded_below_one_line_holds_none() {
        let mut record = Activity::new().bounded_to(MOST_ENTRIES, 1);
        record.happened(
            &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
            moment(1),
            answered(),
        );
        assert!(record.is_empty());
        assert_eq!(record.bytes(), 0);
        assert_eq!(record.forgotten(), 1);
    }

    /// A narrower bound applied to a record that is already full takes effect
    /// at once, rather than at the next request.
    #[test]
    fn a_bound_narrowed_afterwards_is_applied_at_once() {
        let mut record = Activity::new();
        for asked in 0..8u64 {
            record.happened(
                &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
                moment(asked),
                answered(),
            );
        }
        let record = record.bounded_to(2, LARGEST_RECORD);
        assert_eq!(record.len(), 2);
        assert_eq!(record.forgotten(), 6);
    }

    /// A server that could write a thousand lines into the record would be
    /// deciding how much memory this process uses — and could bury a real line
    /// under its own.
    #[test]
    fn a_reason_a_server_had_a_hand_in_is_bounded() {
        let mut record = Activity::new();
        let shouting = "x".repeat(100_000);
        record.happened(
            &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
            moment(1),
            Happened::Failed {
                why: format!("the header said {shouting}"),
            },
        );

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        let said = line.to_string();
        assert!(
            said.chars().count() < 400,
            "{} characters",
            said.chars().count()
        );
        assert!(said.contains('…'), "it was cut without saying so: {said}");
        assert!(line.weighs() < 1024, "{} bytes for one line", line.weighs());
    }

    /// A reason short enough to keep is kept exactly, mark and all: a sentence
    /// that always ended in an ellipsis would be one nobody could trust the end
    /// of.
    #[test]
    fn a_reason_that_fits_is_not_marked_as_cut() {
        let mut record = Activity::new();
        record.happened(
            &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
            moment(1),
            Happened::Refused {
                rule: "redirected more than 20 times without arriving".to_owned(),
            },
        );
        assert_eq!(
            record.latest().map(Entry::happened),
            Some(&Happened::Refused {
                rule: "redirected more than 20 times without arriving".to_owned(),
            }),
        );
    }

    /// Cut between letters rather than through one, which is what makes this
    /// worth doing by character.
    #[test]
    fn a_reason_in_another_script_is_cut_between_letters() {
        let mut record = Activity::new();
        record.happened(
            &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
            moment(1),
            Happened::Failed {
                why: "日".repeat(1000),
            },
        );
        let Some(Happened::Failed { why }) = record.latest().map(Entry::happened) else {
            panic!("a failure was not recorded as one");
        };
        assert_eq!(why.chars().count(), LONGEST_REASON + 1, "{why}");
        assert!(why.starts_with('日'));
    }

    // --- What a person can do with it ----------------------------------------

    /// Deleting is real, and it takes the count of what was dropped with it: a
    /// record somebody emptied must not keep saying how much there used to be.
    #[test]
    fn emptying_it_leaves_nothing_and_says_nothing_about_what_was_there() {
        let mut record = Activity::new().bounded_to(1, LARGEST_RECORD);
        for asked in 0..4u64 {
            record.happened(
                &asked_for("https://example.com/", Cause::Person { tab: a_tab() }),
                moment(asked),
                answered(),
            );
        }
        assert_eq!(record.forgotten(), 3);

        record.empty();
        assert!(record.is_empty());
        assert_eq!(record.len(), 0);
        assert_eq!(record.bytes(), 0);
        assert_eq!(record.forgotten(), 0);
        assert_eq!(record.latest(), None);
    }
}
