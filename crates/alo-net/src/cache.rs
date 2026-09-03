//! What is kept, and which kept thing answers which request.
//!
//! [`crate::freshness`] answers *may this be used*. This file answers *which
//! one*, and that is where `Vary` lives — the part of caching that, done
//! carelessly, serves one person another person's page.
//!
//! # `Vary`, and why it is a contract rather than a header
//!
//! A response to `Accept-Language: fr` is not an answer to `Accept-Language:
//! de`, and the only thing that says so is a `Vary: Accept-Language` on the
//! response itself. So what is stored is not just the response: it is the
//! response **together with the request header values it was chosen by**. A
//! later request matches only if it would have produced the same choice.
//!
//! `Vary: *` means the server is telling us it cannot promise that for
//! anything, and the only correct reading is to never reuse the response.
//!
//! # Two questions, not one
//!
//! *May this be reused* is above. *May this be written down* is a different
//! question with a different answer for a page behind a password, and **ADR
//! 0011** decides it. Three of its clauses are here and the rest are in
//! [`crate::disk`] and [`crate::record`].
//!
//! **The key carries the top-level site.** The same [`Partition`] the cookie jar
//! uses, and for ADR 0007's reason: a cache shared across sites answers *have
//! you been somewhere that loads this* for any site that thinks to time a load,
//! and an entry only one visitor was ever given is an identifier that survives
//! clearing cookies. There is no method here that does not take one, which is
//! how the promise is kept by the shape rather than by everybody remembering.
//!
//! **[`Cache::keep`] asks the second question before anything is written**, and
//! [`crate::disk::why_it_is_never_written`] answers it. Never written, rather
//! than written and deleted: a file that was deleted was still on the disk.
//!
//! **A cache with no disk is what a session-scoped profile is.**
//! [`Cache::new`] has one nowhere; [`Cache::kept_on`] is the deliberate act of
//! opening one. And that disk belongs to the browser process alone (ADR 0005),
//! so nothing in a renderer opens one.
//!
//! What the disk does **not** change is anything above: a response that went to
//! a disk is served under exactly the freshness and `Vary` rules it would have
//! been served under from memory.

use crate::cookie::Partition;
use crate::directives::{Directives, Flag};
use crate::disk::Disk;
use crate::freshness::{self, Stored, Verdict};
use crate::headers::Headers;
use crate::httpdate;
use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;
use std::time::SystemTime;

/// The most responses this engine keeps in memory at once.
///
/// A bound, not a tuning. Without one, a page that fetches ten thousand small
/// images has made the browser hold ten thousand small images for as long as it
/// runs.
pub const MOST_KEPT: usize = 500;

/// What is kept, keyed by what was asked for.
#[derive(Debug, Default)]
pub struct Cache {
    kept: HashMap<String, Stored>,
    /// Insertion order, so the oldest can go when the bound is reached. A
    /// counter rather than a timestamp: two responses stored in the same
    /// millisecond still have an order, and the clock is not involved in a
    /// decision that has nothing to do with time.
    order: Vec<String>,
    /// Where this survives a restart, when somebody opened one. [`None`] is a
    /// cache that was never written to a disk at all — private browsing, and
    /// any profile that is session-scoped (ADR 0011).
    disk: Option<Disk>,
    hits: usize,
    revalidations: usize,
    misses: usize,
}

/// What the cache says to do about a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Use this. Nothing needs to go out.
    Stored(Box<Response>),
    /// Ask the server whether this is still good, with these headers added.
    Revalidate {
        /// The conditional headers to send.
        conditions: Vec<(String, String)>,
    },
    /// Nothing usable. Fetch it.
    Fetch,
}

impl Cache {
    /// An empty cache, in memory, that outlives nothing.
    ///
    /// This is what a session-scoped profile has, in ADR 0011's words: *"not a
    /// cache that is emptied at the end: a cache that was never opened."*
    pub fn new() -> Self {
        Self::default()
    }

    /// The same cache, with a disk behind it.
    ///
    /// Opening one is deliberate and it is the browser process's to do —
    /// ADR 0005 gives a renderer no filesystem, and ADR 0011 section 5 refuses
    /// the temptation to hand it one, because a compromised renderer with this
    /// directory would have every page that person has read across every site.
    #[must_use]
    pub fn kept_on(mut self, disk: Disk) -> Self {
        self.disk = Some(disk);
        self
    }

    /// The disk behind this, for a caller that wants to look.
    pub fn disk(&self) -> Option<&Disk> {
        self.disk.as_ref()
    }

    /// How many requests were answered without going out, revalidated, and not
    /// answered at all.
    ///
    /// For tests and for somebody looking at whether this is doing anything.
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.hits, self.revalidations, self.misses)
    }

    /// How many responses are held.
    pub fn len(&self) -> usize {
        self.kept.len()
    }

    /// Whether anything is held.
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// What to do about this request, inside this top-level site.
    ///
    /// `within` is ADR 0011 section 1: there is no version of this that does
    /// not take one, because a cache shared across sites is a history oracle
    /// and an identifier that outlives clearing cookies.
    pub fn answer(&mut self, request: &Request, within: &Partition, now: SystemTime) -> Answer {
        let asked = Directives::of(request.headers.all("Cache-Control"));
        let key = key_of(request, within);
        self.take_from_the_disk(&key);
        let Some(stored) = self.kept.get(&key) else {
            self.misses += 1;
            return Answer::Fetch;
        };
        if !varies_the_same(stored, request) {
            self.misses += 1;
            return Answer::Fetch;
        }
        match freshness::verdict(&asked, stored, now) {
            Verdict::Hit => {
                self.hits += 1;
                Answer::Stored(Box::new(stored.response.clone()))
            }
            Verdict::Revalidate => {
                self.revalidations += 1;
                Answer::Revalidate {
                    conditions: conditions_for(&stored.response.headers),
                }
            }
            Verdict::Miss => {
                self.misses += 1;
                Answer::Fetch
            }
        }
    }

    /// Bring what is on the disk into memory, if there is anything and memory
    /// does not already have it.
    ///
    /// Every way of failing is a miss (ADR 0011 section 4), so nothing here
    /// returns a reason: the caller goes on to answer exactly as it would have
    /// with an empty cache.
    fn take_from_the_disk(&mut self, key: &str) {
        if self.kept.contains_key(key) {
            return;
        }
        let Some(disk) = self.disk.as_mut() else {
            return;
        };
        let Some(stored) = disk.read(key) else {
            return;
        };
        // Into memory without writing it back: it is already there, and a
        // rewrite would give it a new place in the eviction order for having
        // been read.
        self.kept.insert(key.to_owned(), stored);
        self.order.push(key.to_owned());
        self.forget_the_oldest();
    }

    /// Keep a response, if it may be kept.
    ///
    /// Returns whether it was — in memory, which is the question this asks.
    /// Whether it also went to the disk is a second question, and one a caller
    /// does not have to remember to ask: [`crate::disk::why_it_is_never_written`]
    /// is consulted here so that it is answered the same way everywhere.
    pub fn keep(
        &mut self,
        request: &Request,
        within: &Partition,
        response: &Response,
        requested_at: SystemTime,
        received_at: SystemTime,
    ) -> bool {
        if !freshness::may_store(&request.method, response) {
            return false;
        }
        // `Vary: *` is the server saying it cannot promise this response
        // answers any other request. There is no key that would be right, so
        // there is no key.
        let varying = varied_names(&response.headers);
        if varying.iter().any(|name| name == "*") {
            return false;
        }
        let stored = Stored {
            response: response.clone(),
            requested_at,
            received_at,
            varied_on: varying
                .iter()
                .map(|name| (name.clone(), request.headers.get(name).map(str::to_owned)))
                .collect(),
        };
        let key = key_of(request, within);
        self.write_or_never_write(&key, request, &stored);
        if self.kept.insert(key.clone(), stored).is_none() {
            self.order.push(key);
        }
        self.forget_the_oldest();
        true
    }

    /// The disk half of keeping something: ADR 0011 section 2, asked once.
    ///
    /// The `else` is the part that is easy to leave out. A URL that was public
    /// yesterday and sets a session cookie today has an entry on the disk that
    /// is now superseded by something that may never be written — and leaving it
    /// there means a restart serves the older one. So it is removed. That is not
    /// "written and deleted": nothing that the list refuses was ever written.
    fn write_or_never_write(&mut self, key: &str, request: &Request, stored: &Stored) {
        let Some(disk) = self.disk.as_mut() else {
            return;
        };
        if crate::disk::why_it_is_never_written(request, &stored.response).is_none() {
            disk.write(key, stored);
        } else {
            disk.forget(key);
        }
    }

    /// The oldest go when the memory bound is reached.
    ///
    /// The disk is not touched. It has its own bound and its own order, and
    /// evicting from memory is not a decision about what may be kept — a
    /// browser that dropped a disk entry because memory filled up would lose
    /// the thing surviving a restart was for.
    fn forget_the_oldest(&mut self) {
        while self.order.len() > MOST_KEPT {
            if self.order.is_empty() {
                break;
            }
            let oldest = self.order.remove(0);
            self.kept.remove(&oldest);
        }
    }

    /// Take a `304` and update what is stored, returning the response to use.
    ///
    /// # Errors
    ///
    /// Returns `None` when there was nothing stored to refresh, which means the
    /// server answered a conditional request nobody could have sent — a case
    /// worth noticing rather than papering over.
    pub fn refresh(
        &mut self,
        request: &Request,
        within: &Partition,
        not_modified: &Response,
        now: SystemTime,
    ) -> Option<Response> {
        let key = key_of(request, within);
        self.take_from_the_disk(&key);
        let stored = self.kept.get(&key)?;
        let updated = freshness::refreshed(stored, not_modified, now);
        let answer = updated.response.clone();
        // Asked again rather than assumed: a `304` carries headers, and one of
        // them can be a `Set-Cookie`. An entry that was writable when it was
        // stored may not be writable now that it has been refreshed.
        self.write_or_never_write(&key, request, &updated);
        self.kept.insert(key, updated);
        Some(answer)
    }

    /// Forget everything about one URL.
    ///
    /// A `POST`, `PUT` or `DELETE` that succeeds means what is stored for that
    /// URL is now a lie. This is the small half of invalidation; the other half
    /// — invalidating what a `Location` pointed at — waits for something that
    /// actually submits forms.
    pub fn forget(&mut self, request: &Request, within: &Partition) {
        let key = key_of(request, within);
        self.kept.remove(&key);
        self.order.retain(|kept| kept != &key);
        if let Some(disk) = self.disk.as_mut() {
            disk.forget(&key);
        }
    }

    /// Forget everything, on the disk as well as in memory.
    ///
    /// What "clear this browsing data" has to be able to do. ADR 0011: *"a
    /// cache that survives a restart is a browsing record that survives a
    /// restart … what we owe them is that deleting it is real and easy to
    /// reach."*
    pub fn empty(&mut self) {
        self.kept.clear();
        self.order.clear();
        if let Some(disk) = self.disk.as_mut() {
            disk.empty();
        }
    }
}

/// What a response is stored under.
///
/// Three parts, and each is a way of serving somebody the wrong page.
///
/// The **top-level site** is ADR 0011 section 1 and ADR 0007's argument: one
/// cache across every site joins a person's activity on one to their activity
/// on another, answers *have you loaded this before* to anybody who times a
/// load, and carries an identifier that survives clearing cookies.
///
/// The **method** is there because a `HEAD` and a `GET` for one URL are
/// different responses — the first has no body — and answering a `GET` from a
/// stored `HEAD` would be a blank page.
///
/// A space separates them because a host cannot contain one and a method cannot
/// either, so no two different keys can be spelled the same way.
fn key_of(request: &Request, within: &Partition) -> String {
    format!(
        "{} {} {}",
        within.site(),
        request.method.to_ascii_uppercase(),
        request.url.serialised
    )
}

/// The header names a response varies by, lowercased.
fn varied_names(headers: &Headers) -> Vec<String> {
    headers
        .all("Vary")
        .flat_map(|value| value.split(','))
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Whether a request would have been answered the same way as the one that
/// produced what is stored.
///
/// An absent header and an empty one are different, which is why the stored
/// side is an `Option` rather than a string: a request with no
/// `Accept-Language` at all did not ask for the same thing as one that sent an
/// empty `Accept-Language`, and a server may well answer them differently.
fn varies_the_same(stored: &Stored, request: &Request) -> bool {
    stored.varied_on.iter().all(|(name, was)| {
        let now = request.headers.get(name).map(str::to_owned);
        &now == was
    })
}

/// The conditional headers that ask "is this still what you have?".
///
/// `ETag` first, and both when both exist. An `ETag` is exact; a
/// `Last-Modified` has one-second resolution, so a file changed twice in the
/// same second is a file a date cannot tell apart.
fn conditions_for(headers: &Headers) -> Vec<(String, String)> {
    let mut conditions = Vec::new();
    if let Some(tag) = headers.get("ETag") {
        conditions.push(("If-None-Match".to_owned(), tag.to_owned()));
    }
    if let Some(changed) = headers.get("Last-Modified") {
        conditions.push(("If-Modified-Since".to_owned(), changed.to_owned()));
    }
    conditions
}

/// A request with the conditions added, ready to send.
pub fn asking_whether_it_changed(request: &Request, conditions: &[(String, String)]) -> Request {
    let mut asking = request.clone();
    for (name, value) in conditions {
        asking.headers.replace(name, value);
    }
    asking
}

/// Whether a response says not to keep anything about this exchange.
///
/// Separate from [`freshness::may_store`] because a request may also say it,
/// and a request's `no-store` binds even a response that would have been
/// perfectly cacheable.
pub fn nobody_wants_this_kept(request: &Request, response: &Response) -> bool {
    Directives::of(request.headers.all("Cache-Control")).says(Flag::NoStore)
        || Directives::of(response.headers.all("Cache-Control")).says(Flag::NoStore)
}

/// The `Date` a response should be treated as having.
///
/// A response with no `Date` is given the moment it arrived. Without this,
/// every age calculation on such a response falls back to zero — which reads as
/// "brand new" and is the optimistic direction to be wrong in.
pub fn dated(response: &mut Response, received_at: SystemTime) {
    if response.headers.get("Date").is_none() {
        response
            .headers
            .replace("Date", &httpdate::format(received_at));
    }
}
