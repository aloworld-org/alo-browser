/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Not asking the same question twice.
//!
//! [`crate::cors`] decides whether a cross-origin request must be asked about
//! before it is sent, and whether the answer allowed it. Asking is an `OPTIONS`
//! and a whole round trip, so a page that sends ten `DELETE`s to one endpoint
//! would spend ten of them on the same question. This is where the answer is
//! remembered, and `Access-Control-Max-Age` is the server saying for how long.
//!
//! # A record of answers, never a licence
//!
//! Everything here is one rule applied four times: **what is remembered is what
//! a server actually said about a request that was actually made.** A cache
//! that stored anything wider than that would be a way of getting a permission
//! the server never gave, which is the only interesting way this file can be
//! wrong.
//!
//! - A `*` in `Access-Control-Allow-Methods` or `Access-Control-Allow-Headers`
//!   is remembered as **the method and the headers this request asked for**,
//!   never as a wildcard. `*` means "and anything else you care to ask", which
//!   is a sentence about the request in front of the server rather than a
//!   standing offer, and a later request of a different shape asks again.
//! - So the rule that `*` never covers `Authorization`
//!   ([`crate::cors::asking_first_allowed`]) needs no restatement here. A
//!   header is remembered only when the server named it, and a server that
//!   named `authorization` has done the thing the rule asks for.
//! - An answer to a request **without** credentials does not cover one with
//!   them. The other direction does: a server that agreed to be read by this
//!   origin with cookies has agreed to the stricter case as well.
//! - Nothing is remembered for an answer that refused. [`Preflights::allowed`]
//!   is the only way in, and it checks before it stores — so a caller cannot
//!   remember a permission it never received by forgetting which order to call
//!   two functions in.
//!
//! # Whose question it was
//!
//! The key carries the **top-level site**, the same [`Partition`] the cookie jar
//! and the HTTP cache take, for ADR 0011 section 1's reason: an entry that makes
//! one site's request faster because another site already asked is a timing
//! answer to *have you been there*, and it survives clearing cookies. There is
//! no method here that does not take one.
//!
//! It carries the **asking origin** too, which is finer, and an **opaque origin
//! is never a key**. `null` is what every opaque origin serialises to, so a key
//! containing one would be shared between two pages that are by definition not
//! each other (`alo_url`, queue item 50). Such a request asks every time.
//!
//! # The clock is the caller's
//!
//! Nothing here reads a clock. Every method takes the moment, exactly as
//! [`crate::cache`] does, because a table of expiries is only honest when the
//! test names the moment rather than sleeping through it.
//!
//! # And it is memory only
//!
//! There is no disk. A preflight answer is worth a round trip, not a file: it
//! expires in minutes, and ADR 0011's whole argument about what may be written
//! where other programs can read it applies to this as much as to a page.

use crate::cookie::Partition;
use crate::cors::{self, Credentials, Refusal};
use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// How long an answer holds when the server did not say.
///
/// Five seconds, which is what Fetch specifies. Long enough for the burst of
/// requests one interaction makes, short enough that a server which has not
/// thought about caching has not accidentally granted anything.
pub const WHEN_NOBODY_SAYS: Duration = Duration::from_secs(5);

/// The longest an answer is remembered, however long the server asked for.
///
/// Two hours. A preflight answer is a **permission**, and the cap is what makes
/// revoking one possible: a server that changes its mind about who may `DELETE`
/// should not have to wait out a `max-age` of a year it wrote once. The same
/// argument as [`crate::cookie::LONGEST_LIFE`], on a much shorter scale,
/// because nothing here is a preference somebody chose.
pub const LONGEST_MEMORY: Duration = Duration::from_secs(2 * 60 * 60);

/// The most answers held at once.
///
/// A bound, not a tuning, and the same reason [`crate::cache::MOST_KEPT`] gives:
/// without one, a page that preflights two thousand URLs has made the browser
/// hold two thousand entries for as long as it runs.
pub const MOST_KEPT: usize = 200;

/// The most method or header names remembered from one answer.
///
/// A server naming more than sixty-four methods is not describing an API. What
/// is dropped is dropped from the *end*, and dropping a permission only ever
/// means asking again — the direction that costs a round trip rather than the
/// direction that skips a question.
pub const MOST_NAMES: usize = 64;

/// What one server said, and until when.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answered {
    /// The moment after which this is not used. Computed from the caller's
    /// clock when it was stored, never read from one here.
    expires_at: SystemTime,
    /// Whether the request this answered carried credentials.
    with_credentials: bool,
    /// The methods the server named, uppercased.
    methods: Vec<String>,
    /// The header names the server named, lowercased.
    headers: Vec<String>,
}

impl Answered {
    /// Whether this answer covers the request in front of us.
    fn covers(&self, request: &Request, credentials: Credentials) -> bool {
        // An answer given to a request without credentials says nothing about
        // one that carries them: the server was never shown the harder
        // question.
        if credentials == Credentials::Include && !self.with_credentials {
            return false;
        }
        let method = request.method.to_ascii_uppercase();
        // The same three [`cors::asking_first_allowed`] allows once a preflight
        // has succeeded at all, because they are what a form could have sent.
        if !matches!(method.as_str(), "GET" | "HEAD" | "POST") && !self.methods.contains(&method) {
            return false;
        }
        // An exact name, never a pattern. Nothing here holds a `*` to begin
        // with, and this is where a later change that introduced one would have
        // to say so out loud.
        cors::names_a_form_could_not_have_sent(request)
            .iter()
            .all(|name| self.headers.contains(name))
    }
}

/// The answers to questions already asked.
#[derive(Debug, Default)]
pub struct Preflights {
    kept: HashMap<String, Answered>,
    /// Insertion order, so the oldest can go when the bound is reached. A
    /// counter of position rather than a timestamp, for [`crate::cache`]'s
    /// reason: two answers stored in the same millisecond still have an order.
    order: Vec<String>,
    asked: usize,
    spared: usize,
}

impl Preflights {
    /// Nothing remembered yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many questions were asked, and how many were not asked again.
    ///
    /// For tests, and for somebody looking at whether this is doing anything.
    pub fn counts(&self) -> (usize, usize) {
        (self.asked, self.spared)
    }

    /// How many answers are held.
    pub fn len(&self) -> usize {
        self.kept.len()
    }

    /// Whether anything is held.
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// Whether an `OPTIONS` has to go out before this request does.
    ///
    /// **The only way to consult this cache**, and deliberately so: it asks
    /// [`cors::needs_asking_first`] itself rather than trusting a caller to ask
    /// in the right order. A caller that looked in the cache first would skip a
    /// preflight for a request that needed one whenever some earlier request to
    /// the same URL had happened to need one and been allowed.
    ///
    /// A request that needs no preflight at all is counted as neither asked nor
    /// spared. Nothing was ever going to go out for it.
    pub fn must_ask(
        &mut self,
        request: &Request,
        credentials: Credentials,
        within: &Partition,
        now: SystemTime,
    ) -> bool {
        if !cors::needs_asking_first(request) {
            return false;
        }
        let Some(key) = key_of(request, within) else {
            self.asked += 1;
            return true;
        };
        self.forget_if_it_has_expired(&key, now);
        if self
            .kept
            .get(&key)
            .is_some_and(|answered| answered.covers(request, credentials))
        {
            self.spared += 1;
            return false;
        }
        self.asked += 1;
        true
    }

    /// Whether a preflight answer allows the request — and, when it does,
    /// remember it.
    ///
    /// One call rather than two, so that remembering a permission the server
    /// refused is not a thing a caller can do by getting the order wrong.
    ///
    /// # Errors
    ///
    /// [`Refusal`], exactly as [`cors::asking_first_allowed`] gives it, and
    /// nothing is stored.
    pub fn allowed(
        &mut self,
        request: &Request,
        credentials: Credentials,
        within: &Partition,
        answer: &Response,
        now: SystemTime,
    ) -> Result<(), Refusal> {
        cors::asking_first_allowed(request, credentials, answer)?;
        let Some(key) = key_of(request, within) else {
            return Ok(());
        };
        let Some(lifetime) = how_long(answer) else {
            return Ok(());
        };
        // A clock so near the end of representable time that two hours does not
        // fit in it is not one this engine will do arithmetic on. Not
        // remembering is always a correct answer.
        let Some(expires_at) = now.checked_add(lifetime) else {
            return Ok(());
        };
        let entry = Answered {
            expires_at,
            with_credentials: credentials == Credentials::Include,
            methods: named(
                answer,
                "Access-Control-Allow-Methods",
                &[request.method.to_ascii_uppercase()],
            )
            .into_iter()
            .map(|one| one.to_ascii_uppercase())
            .collect(),
            headers: named(
                answer,
                "Access-Control-Allow-Headers",
                &cors::names_a_form_could_not_have_sent(request),
            ),
        };
        if self.kept.insert(key.clone(), entry).is_none() {
            self.order.push(key);
        }
        self.forget_the_oldest();
        Ok(())
    }

    /// Forget everything, which is what "clear this browsing data" has to be
    /// able to do here as much as anywhere else.
    pub fn empty(&mut self) {
        self.kept.clear();
        self.order.clear();
    }

    /// Drop an answer whose time is up, rather than leaving it to be skipped
    /// over on every later request.
    fn forget_if_it_has_expired(&mut self, key: &str, now: SystemTime) {
        // `>=` rather than `>`: an answer good for five seconds is not good at
        // the fifth second, which is the pair of moments either side of an
        // expiry that a test can name.
        if self
            .kept
            .get(key)
            .is_some_and(|answered| now >= answered.expires_at)
        {
            self.kept.remove(key);
            self.order.retain(|held| held != key);
        }
    }

    /// The oldest go when the bound is reached.
    fn forget_the_oldest(&mut self) {
        while self.order.len() > MOST_KEPT {
            let oldest = self.order.remove(0);
            self.kept.remove(&oldest);
        }
    }
}

/// What an answer is remembered under, or [`None`] for one that must not be.
///
/// Three parts, and each is a way of skipping a question somebody should have
/// been asked. The **site** is ADR 0011 section 1; the **origin** is who the
/// server was told was asking, and an answer given about one page's origin says
/// nothing about another's; the **URL** is what was asked about, because
/// `Access-Control-Allow-Methods` is a statement about an endpoint rather than
/// about a host.
///
/// A space separates them because none of the three may contain one.
///
/// [`None`] when nobody asked, or when whoever asked has an **opaque** origin:
/// every opaque origin serialises to `null`, so such a key would be shared
/// between pages that are by definition not each other.
fn key_of(request: &Request, within: &Partition) -> Option<String> {
    let asker = request.initiator.as_ref()?;
    if asker.is_opaque() {
        return None;
    }
    Some(format!(
        "{} {} {}",
        within.site(),
        asker,
        request.url.serialised
    ))
}

/// The names a server gave in one of the two allow headers.
///
/// A `*` is replaced by `asked_for` rather than kept: see this module's
/// documentation. The result is bounded, deduplicated, and never grows with
/// what a server chose to send.
fn named(answer: &Response, header: &str, asked_for: &[String]) -> Vec<String> {
    let listed = cors::list(&answer.headers, header);
    let mut names = if listed.iter().any(|one| one == "*") {
        asked_for.to_vec()
    } else {
        listed
    };
    names.sort();
    names.dedup();
    names.truncate(MOST_NAMES);
    names
}

/// How long a server asked to be remembered for, or [`None`] for not at all.
///
/// Every reading of `Access-Control-Max-Age` is a reading of bytes a stranger
/// sent, so each way of it not being a number has an answer rather than a
/// panic:
///
/// - absent, or something that is not a number at all — the default. A server
///   that said nothing intelligible has said nothing.
/// - zero or negative — **not at all**. `-1` is how a server spells "do not
///   remember this", and zero says the same thing arithmetically.
/// - larger than an `i64` — every such value is above the cap by an enormous
///   margin, so it is the cap. Reading `10000000000000000000` as five seconds
///   would be technically defensible and would surprise the only kind of
///   person who writes it.
/// - anything above the cap — the cap, [`LONGEST_MEMORY`].
fn how_long(answer: &Response) -> Option<Duration> {
    let Some(said) = answer.headers.get("Access-Control-Max-Age") else {
        return Some(WHEN_NOBODY_SAYS);
    };
    let said = said.trim();
    let Ok(seconds) = said.parse::<i64>() else {
        let enormous = !said.is_empty() && said.bytes().all(|byte| byte.is_ascii_digit());
        return Some(if enormous {
            LONGEST_MEMORY
        } else {
            WHEN_NOBODY_SAYS
        });
    };
    if seconds <= 0 {
        return None;
    }
    // `unsigned_abs` rather than a fallible conversion: the value is positive
    // here, so there is nothing left to fail.
    Some(Duration::from_secs(seconds.unsigned_abs()).min(LONGEST_MEMORY))
}
