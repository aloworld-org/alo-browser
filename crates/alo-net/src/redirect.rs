/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a response says to go instead.
//!
//! # Why this is its own file, and why it is mostly one pure function
//!
//! Following a redirect is three lines of loop. Deciding *what to carry across
//! one* is where the bugs live, and every one of them is a security bug rather
//! than a rendering one:
//!
//! - An `Authorization` header carried to another origin hands that origin the
//!   credentials for this one. The site being redirected to did not have them a
//!   moment ago and must not have them now.
//! - A `POST` replayed at a new URL by a `301` is the thing every browser
//!   learned not to do, in public, decades ago.
//! - A redirect to a scheme this engine does not fetch, followed silently, is a
//!   page that fails with no reason anybody can read.
//! - A chain that never ends is a load that never ends.
//!
//! So the deciding is [`next`] — a function from a request and a response to
//! what to do, with no socket anywhere near it — and the loop that calls it is
//! elsewhere. Everything above can be asserted without a server.

use crate::headers::Headers;
use crate::request::Request;
use crate::response::Response;
use alo_url::{Origin, Url};
use core::fmt;

/// The most redirects this engine will follow for one load.
///
/// Twenty is what browsers settled on. The number matters less than there
/// being one: without it, two servers pointing at each other is a load that
/// never finishes and a tab that cannot be closed politely.
pub const MOST_HOPS: usize = 20;

/// Headers that belong to one origin and must not cross to another.
///
/// `Cookie` is here for completeness — queue item 57 owns cookies and nothing
/// sets this header yet — because the day something does, this list is where a
/// person will look, and a list that was already right is better than a list
/// somebody has to remember to update.
const ONLY_FOR_THE_ORIGIN_THAT_EARNED_THEM: [&str; 3] =
    ["Authorization", "Cookie", "Proxy-Authorization"];

/// Headers that describe a body, and so mean nothing once the body is gone.
const ABOUT_THE_BODY: [&str; 3] = ["Content-Type", "Content-Length", "Content-Encoding"];

/// What to do with a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// This is the answer. Nothing said to go anywhere else.
    Keep,
    /// Ask for this instead.
    Follow(Box<Request>),
}

/// Why a redirect was not followed.
///
/// Each of these is something to *tell somebody*, which is why they are named
/// rather than collapsed into a string: "this site is redirecting in a circle"
/// and "this site redirected somewhere this browser will not go" want different
/// words on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// More hops than this engine will follow.
    TooManyHops {
        /// How many were followed before it gave up.
        hops: usize,
    },
    /// A URL this load has already been to.
    ACircle {
        /// The one it came back to.
        url: String,
    },
    /// A `Location` that is not a URL, relative to where it came from.
    NotAUrl {
        /// What the header said.
        location: String,
        /// Why it could not be read.
        why: String,
    },
    /// A scheme this engine does not fetch.
    ///
    /// Refused rather than followed, and refused rather than *ignored*: a
    /// `Location: mailto:…` that yielded the redirect's own empty body would be
    /// a blank page with no explanation in it.
    ///
    /// This covers `data:` and `file:`, which this engine fetches when asked
    /// directly and refuses to be *sent* to. A server that could redirect a
    /// load into `file:///` would be reading the disk of whoever opened the
    /// page.
    ASchemeWeDoNotFetch {
        /// Which one.
        scheme: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::TooManyHops { hops } => {
                write!(f, "redirected more than {hops} times without arriving")
            }
            Refusal::ACircle { url } => write!(f, "redirected in a circle, back to {url}"),
            Refusal::NotAUrl { location, why } => {
                write!(f, "redirected to {location:?}, which is not a URL: {why}")
            }
            Refusal::ASchemeWeDoNotFetch { scheme } => {
                write!(
                    f,
                    "redirected to a {scheme}: URL, which this browser does not fetch"
                )
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// What a response says to do next.
///
/// # Errors
///
/// [`Refusal`] when the response points somewhere this engine will not go.
/// Note what is *not* an error: a `3xx` with no `Location` is [`Next::Keep`],
/// because a redirect that does not say where is not a redirect, and the body
/// that came with it is the only thing there is to show.
pub fn next(sent: &Request, got: &Response) -> Result<Next, Refusal> {
    if !got.status.is_redirect() {
        return Ok(Next::Keep);
    }
    let Some(location) = got.headers.get("Location") else {
        return Ok(Next::Keep);
    };
    // Relative to where the response came *from*, which is `got.url` rather
    // than `sent.url`: after one hop those differ, and resolving against the
    // original would send the second hop to the wrong place.
    let target = alo_url::join(&got.url, location.trim()).map_err(|why| Refusal::NotAUrl {
        location: location.to_owned(),
        why: why.why.clone(),
    })?;
    // Only the two that can redirect at all. `data:` and `file:` are refused
    // deliberately even though this engine fetches both directly: a page on the
    // web that could redirect a load into `file:///` would be reading the disk
    // of whoever opened it, and one that could redirect into `data:` would be
    // choosing the bytes *and* inheriting the URL they appear to have come
    // from. Neither is a thing a server gets to decide.
    if !matches!(target.scheme.as_str(), "http" | "https") {
        return Err(Refusal::ASchemeWeDoNotFetch {
            scheme: target.scheme.clone(),
        });
    }

    let method = method_after(&sent.method, got.status.0);
    let lost_its_body = method != sent.method;
    let crossing = Origin::of(&got.url) != Origin::of(&target);

    let mut headers = Headers::new();
    for header in sent.headers.iter() {
        if crossing
            && ONLY_FOR_THE_ORIGIN_THAT_EARNED_THEM
                .iter()
                .any(|guarded| header.name.eq_ignore_ascii_case(guarded))
        {
            continue;
        }
        if lost_its_body
            && ABOUT_THE_BODY
                .iter()
                .any(|about| header.name.eq_ignore_ascii_case(about))
        {
            continue;
        }
        headers.add(header.name.clone(), header.value.clone());
    }

    Ok(Next::Follow(Box::new(Request {
        url: target,
        method,
        purpose: sent.purpose.clone(),
        // Unchanged on purpose: the page that asked is still the page that
        // asked. Item 61 decides what may be *read*, and a redirect must not
        // be able to launder a request into looking self-inflicted.
        initiator: sent.initiator.clone(),
        // Carried for the same reason and a sharper one. ADR 0012 § 2: a hop
        // is attributed to whatever asked for the first hop, because a
        // redirect is not a new intention. A server that could change it would
        // be able to write *the person did that* into the record by answering
        // `302`.
        cause: sent.cause.clone(),
        headers,
        // The same condition that drops `Content-Length` and `Content-Type`
        // drops the bytes they described, and it has to be the same one: a
        // `GET` carrying a body that its headers no longer describe is a
        // message the next server frames by guessing. `307` and `308` keep the
        // method, so they keep the body — which is the whole reason they exist.
        body: if lost_its_body {
            Vec::new()
        } else {
            sent.body.clone()
        },
    })))
}

/// Which method the next hop uses.
///
/// `303` says so outright: see the other thing, with a `GET`.
///
/// `301` and `302` are the interesting ones. The specification says the method
/// is preserved; every browser has turned a redirected `POST` into a `GET`
/// since the nineteen-nineties, because servers were written against that
/// behaviour and because silently re-submitting a form somewhere new is worse
/// than being wrong about an RFC. `307` and `308` exist precisely so a server
/// can ask for the specified behaviour, and those are honoured exactly.
///
/// `HEAD` survives all of them: it is already bodiless and safe, and turning it
/// into a `GET` would fetch a body nobody asked for.
fn method_after(method: &str, status: u16) -> String {
    let changes = match status {
        303 => true,
        301 | 302 => !method.eq_ignore_ascii_case("head"),
        _ => false,
    };
    if changes && !method.eq_ignore_ascii_case("head") {
        "GET".to_owned()
    } else {
        method.to_owned()
    }
}

/// Where a load has been, so it can tell a circle from a chain.
///
/// A plain list rather than a set: twenty is the ceiling, so the scan is
/// twenty comparisons at worst, and a list keeps the order — which is what
/// somebody debugging a redirect chain actually wants to read.
#[derive(Debug, Clone, Default)]
pub struct Trail {
    been: Vec<String>,
}

impl Trail {
    /// A trail that starts where this request does.
    pub fn from(url: &Url) -> Self {
        Self {
            been: vec![url.serialised.clone()],
        }
    }

    /// How many hops have been taken.
    pub fn hops(&self) -> usize {
        self.been.len().saturating_sub(1)
    }

    /// Every URL this load has been to, in order.
    pub fn been(&self) -> impl Iterator<Item = &str> {
        self.been.iter().map(String::as_str)
    }

    /// Record one more hop, or refuse it.
    ///
    /// # Errors
    ///
    /// [`Refusal::ACircle`] for somewhere this load has already been, and
    /// [`Refusal::TooManyHops`] past [`MOST_HOPS`]. The circle is checked first
    /// because it is the more useful thing to say: a chain of twenty distinct
    /// URLs is a misconfiguration, and two URLs pointing at each other is a
    /// specific bug somebody can go and find.
    pub fn and_then(&mut self, url: &Url) -> Result<(), Refusal> {
        if self.been.iter().any(|been| been == &url.serialised) {
            return Err(Refusal::ACircle {
                url: url.serialised.clone(),
            });
        }
        if self.hops() >= MOST_HOPS {
            return Err(Refusal::TooManyHops { hops: MOST_HOPS });
        }
        self.been.push(url.serialised.clone());
        Ok(())
    }
}
