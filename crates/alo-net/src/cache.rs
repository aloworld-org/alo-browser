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
//! # What this is not, yet
//!
//! Memory, for one process, gone when it exits. Persisting it is queue item
//! 155, and it is a separate item because *what may be written to a disk that
//! other programs can read* is a different question from *what may be reused*,
//! with a different answer for a page behind a password.

use crate::directives::{Directives, Flag};
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
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
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

    /// What to do about this request.
    pub fn answer(&mut self, request: &Request, now: SystemTime) -> Answer {
        let asked = Directives::of(request.headers.all("Cache-Control"));
        let Some(stored) = self.kept.get(&key_of(request)) else {
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

    /// Keep a response, if it may be kept.
    ///
    /// Returns whether it was. A caller does not have to check first — the
    /// decision is here so that it is made the same way everywhere.
    pub fn keep(
        &mut self,
        request: &Request,
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
        let key = key_of(request);
        if self.kept.insert(key.clone(), stored).is_none() {
            self.order.push(key);
        }
        while self.order.len() > MOST_KEPT {
            if self.order.is_empty() {
                break;
            }
            let oldest = self.order.remove(0);
            self.kept.remove(&oldest);
        }
        true
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
        not_modified: &Response,
        now: SystemTime,
    ) -> Option<Response> {
        let key = key_of(request);
        let stored = self.kept.get(&key)?;
        let updated = freshness::refreshed(stored, not_modified, now);
        let answer = updated.response.clone();
        self.kept.insert(key, updated);
        Some(answer)
    }

    /// Forget everything about one URL.
    ///
    /// A `POST`, `PUT` or `DELETE` that succeeds means what is stored for that
    /// URL is now a lie. This is the small half of invalidation; the other half
    /// — invalidating what a `Location` pointed at — waits for something that
    /// actually submits forms.
    pub fn forget(&mut self, request: &Request) {
        let key = key_of(request);
        self.kept.remove(&key);
        self.order.retain(|kept| kept != &key);
    }
}

/// What a response is stored under.
///
/// The method is part of it because a `HEAD` and a `GET` for one URL are
/// different responses — the first has no body — and answering a `GET` from a
/// stored `HEAD` would be a blank page.
fn key_of(request: &Request) -> String {
    format!(
        "{} {}",
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
