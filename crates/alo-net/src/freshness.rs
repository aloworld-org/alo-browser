//! Whether a stored response may be used, and how old it is.
//!
//! `ROADMAP.md` says of the cache: *"subtly wrong here is invisible for months
//! and then serves somebody a stale bank page."* This file is where that would
//! happen, so two things are true of it on purpose.
//!
//! **Time is a parameter.** Nothing here reads the clock. Every function takes
//! `now`, which is the only way to test the answers that are *only wrong an
//! hour later* — and those are the ones nobody finds by using the browser.
//!
//! **Age is not "how long we have had it".** A response can arrive already old,
//! because something between here and the server had it first and said so in an
//! `Age` header. A cache that started counting from arrival serves a response
//! for its full lifetime *after* somebody else already served it for theirs,
//! which is how one `max-age=3600` becomes six hours of staleness across a
//! chain of caches.

use crate::directives::{Directives, Flag};
use crate::headers::Headers;
use crate::httpdate;
use crate::response::{Response, Status};
use std::time::{Duration, SystemTime};

/// The longest a response with no explicit expiry is guessed to be fresh.
///
/// Heuristic freshness is a guess, and this is the bound on how wrong the guess
/// may be. A day is what browsers settled on: long enough to help, short enough
/// that a page nobody has configured is never a week out of date.
pub const LONGEST_GUESS: Duration = Duration::from_secs(24 * 60 * 60);

/// A response that was kept, and the two moments that decide how old it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// What came back.
    pub response: Response,
    /// When the request that produced it went out.
    pub requested_at: SystemTime,
    /// When it arrived.
    ///
    /// Both, rather than one: the gap between them is time the response spent
    /// in transit, during which it was already ageing. On a slow connection
    /// that gap is seconds, and a `max-age=5` response that took two seconds to
    /// arrive is fresh for three.
    pub received_at: SystemTime,
    /// The request header values this response was chosen by — the `Vary`
    /// contract, kept so a later request can be compared against it.
    pub varied_on: Vec<(String, Option<String>)>,
}

/// What to do with a stored response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Use it. Nothing needs to be asked.
    Hit,
    /// It might still be good, but the server has to say so.
    Revalidate,
    /// It cannot be used. Fetch normally.
    Miss,
}

/// How old a stored response is now.
///
/// The specification's arithmetic, and every term in it is there because of a
/// way of being wrong:
///
/// - **apparent age** — arrival minus the server's `Date`. Wrong if the clocks
///   disagree, which is why it is a floor rather than the answer.
/// - **corrected age** — the `Age` header plus how long the response spent in
///   transit. This is the term that stops a chain of caches each granting a
///   response a fresh lifetime.
/// - **resident time** — how long it has been here.
pub fn age(stored: &Stored, now: SystemTime) -> Duration {
    let arrival = stored.received_at;
    let apparent = header_date(&stored.response.headers, "Date")
        .and_then(|sent| arrival.duration_since(sent).ok())
        .unwrap_or_default();

    let in_transit = arrival
        .duration_since(stored.requested_at)
        .unwrap_or_default();
    let claimed = stored
        .response
        .headers
        .get("Age")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(Duration::ZERO, Duration::from_secs);
    let corrected = claimed.saturating_add(in_transit);

    let when_it_arrived = apparent.max(corrected);
    let resident = now.duration_since(arrival).unwrap_or_default();
    when_it_arrived.saturating_add(resident)
}

/// How long a response is fresh for, if it says.
///
/// `s-maxage` is deliberately not consulted: it is for shared caches, this is a
/// private one, and reading it here would make a response meant to live five
/// seconds in a CDN live five seconds in somebody's browser too.
pub fn lifetime(headers: &Headers) -> Option<Duration> {
    let said = Directives::of(headers.all("Cache-Control"));
    if let Some(seconds) = said.max_age {
        return Some(Duration::from_secs(seconds));
    }
    // `Expires` is a moment, so it only means anything against the `Date` the
    // same response carried. Against *our* clock it would be wrong by however
    // much the two machines disagree.
    let expires = header_date(headers, "Expires")?;
    let date = header_date(headers, "Date")?;
    Some(expires.duration_since(date).unwrap_or(Duration::ZERO))
}

/// How long to guess a response is fresh for, when it did not say.
///
/// A tenth of the time since it last changed, capped at [`LONGEST_GUESS`].
/// Something edited a year ago is unlikely to change in the next hour; something
/// edited a minute ago probably will. It is a heuristic and it is allowed to be
/// wrong, which is why it is capped and why `no-store` and `no-cache` are
/// checked long before anything reaches here.
pub fn guessed_lifetime(headers: &Headers) -> Option<Duration> {
    let changed = header_date(headers, "Last-Modified")?;
    let date = header_date(headers, "Date")?;
    let since = date.duration_since(changed).ok()?;
    Some((since / 10).min(LONGEST_GUESS))
}

/// Whether a response may be kept at all.
///
/// Being *stored* and being *used* are different questions, and this is the
/// first. A `no-cache` response is stored and always revalidated; a `no-store`
/// response is never written down in the first place.
pub fn may_store(method: &str, response: &Response) -> bool {
    if !matches!(method.to_ascii_uppercase().as_str(), "GET" | "HEAD") {
        return false;
    }
    let said = Directives::of(response.headers.all("Cache-Control"));
    if said.says(Flag::NoStore) {
        return false;
    }
    // A status this engine has no rule for is not stored. The list is what may
    // be reused without the server saying so — anything else is either an
    // error worth re-asking about or something we have not thought about, and
    // "have not thought about" must not default to keeping it.
    let reusable = matches!(
        response.status.0,
        200 | 203 | 204 | 300 | 301 | 308 | 404 | 405 | 410 | 414 | 501
    );
    reusable && (lifetime(&response.headers).is_some() || can_be_revalidated(&response.headers))
}

/// Whether a stored response carries something to revalidate it with.
///
/// Without one, a stale response is simply gone: there is nothing to ask the
/// server that is cheaper than asking for the whole thing.
pub fn can_be_revalidated(headers: &Headers) -> bool {
    headers.get("ETag").is_some() || headers.get("Last-Modified").is_some()
}

/// What to do with a stored response, given what the request asked for.
///
/// The order of the checks is the point. A request's `no-store` beats
/// everything; a response's `no-cache` beats freshness; freshness beats a
/// request's willingness to take something stale. Getting that order wrong is
/// how a bank page gets served an hour late.
pub fn verdict(asked: &Directives, stored: &Stored, now: SystemTime) -> Verdict {
    let said = Directives::of(stored.response.headers.all("Cache-Control"));

    // Somebody pressed reload, or a script asked not to use a cache.
    if asked.says(Flag::NoStore) || asked.says(Flag::NoCache) {
        return if can_be_revalidated(&stored.response.headers) {
            Verdict::Revalidate
        } else {
            Verdict::Miss
        };
    }

    // The server said: keep it, but never serve it without asking.
    if said.says(Flag::NoCache) {
        return if can_be_revalidated(&stored.response.headers) {
            Verdict::Revalidate
        } else {
            Verdict::Miss
        };
    }

    let age = age(stored, now);
    let fresh_for = lifetime(&stored.response.headers)
        .or_else(|| guessed_lifetime(&stored.response.headers))
        .unwrap_or_default();

    // `immutable` is a promise that it will not change while it is fresh, which
    // is what makes a reload of a fingerprinted asset free. It does not survive
    // the response actually expiring.
    if age < fresh_for {
        if let Some(wanted) = asked.min_fresh {
            // A request that needs it to stay fresh a while longer.
            if fresh_for.saturating_sub(age) < Duration::from_secs(wanted) {
                return revalidate_or_miss(stored);
            }
        }
        return Verdict::Hit;
    }

    // Past its life. A request may still be willing — unless the response said
    // it must not be, which is exactly what `must-revalidate` is for.
    if let Some(tolerance) = asked.max_stale {
        if !said.says(Flag::MustRevalidate) {
            let staleness = age.saturating_sub(fresh_for);
            let allowed = tolerance.map_or(Duration::MAX, Duration::from_secs);
            if staleness <= allowed {
                return Verdict::Hit;
            }
        }
    }

    revalidate_or_miss(stored)
}

fn revalidate_or_miss(stored: &Stored) -> Verdict {
    if can_be_revalidated(&stored.response.headers) {
        Verdict::Revalidate
    } else {
        Verdict::Miss
    }
}

/// A date header, or `None` — and `None` is never permission.
fn header_date(headers: &Headers, name: &str) -> Option<SystemTime> {
    httpdate::parse(headers.get(name)?)
}

/// What a `304` says to change about what is already stored.
///
/// The stored body is kept; the headers are updated, because a `304` is the
/// server saying "still good, and here is what is different about it now" —
/// most often a new `Date`, `Expires` or `Cache-Control`. Headers that describe
/// the *body* are not touched, since the body did not come with it.
pub fn refreshed(stored: &Stored, from: &Response, now: SystemTime) -> Stored {
    let mut response = stored.response.clone();
    for header in from.headers.iter() {
        // A `304` carrying `Content-Length` is describing a body it did not
        // send. Believing it would make the stored body's length a lie.
        if matches!(
            header.name.to_ascii_lowercase().as_str(),
            "content-length" | "content-encoding" | "content-range" | "transfer-encoding"
        ) {
            continue;
        }
        response.headers.replace(&header.name, &header.value);
    }
    Stored {
        response,
        requested_at: now,
        received_at: now,
        varied_on: stored.varied_on.clone(),
    }
}

/// Whether a status is one that says "use what you have".
pub fn is_not_modified(status: Status) -> bool {
    status.0 == 304
}
