/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's fetch, as it crosses the boundary in each direction (ADR 0032
//! §§ 1, 2 and 4, queue item 334).
//!
//! **Out**, a [`FetchAsk`]: a claim in the answer to the message whose work
//! made it — [`crate::FromRenderer::Loaded`], [`crate::FromRenderer::Acted`]
//! or [`crate::FromRenderer::Delivered`] — every ask the work made, in order.
//! It carries what only the renderer knows: the URL as it resolved it, the
//! method and headers the page set, the body, and the page's wishes as
//! `alo-net`'s own enums. **Never an origin, a cause, a cookie, a tab or a
//! document**: the browser process knows each of those without being told
//! ([`crate::fetch_decide`]).
//!
//! **Back**, a [`Fetched`]: a message of its own, which is a task of its own
//! (ADR 0016 § 2), naming the ask by the number the renderer chose and
//! carrying **only what the page may read**. The filtering is the browser
//! process's ([`crate::fetch_filter`]) and happens before the bytes leave it,
//! so an opaque response's body is never in a renderer's memory at all; and a
//! failure carries no reason, so that the page cannot tell a refused
//! connection from a refused read.

use alo_bindings::fetching::{Asked, Keepalive};
use alo_bindings::{Responded, response};
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;
use alo_net::referrer::Policy;
use core::fmt;

/// A fetch a page asked for, as a renderer says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchAsk {
    /// The renderer's number for it, unique for the life of its document.
    /// The browser process echoes it back and never interprets it.
    pub number: u64,
    /// Where to, resolved and serialised by the renderer. A claim: the
    /// browser process parses it again.
    pub url: String,
    /// The method, normalised as Fetch normalises it.
    pub method: String,
    /// The headers the page set, in order, every forbidden one already
    /// dropped by the page's bindings.
    pub headers: Vec<(String, String)>,
    /// The body, as bytes the renderer serialised. Empty for none.
    pub body: Vec<u8>,
    /// Whether the page means to read the answer.
    pub mode: Mode,
    /// Whether the request may carry who the person is.
    pub credentials: Credentials,
    /// What a redirect should do.
    pub redirect: redirect::Mode,
    /// The referrer policy the page asked for, if it asked for one this
    /// engine knows.
    pub referrer: Option<Policy>,
    /// Whether the page asked for it to outlive its document, and as what
    /// (ADR 0040 § 2). A claim like the rest: it gains a page nothing the
    /// browser process does not bound by its own count
    /// ([`crate::fetch_kept`]).
    pub keepalive: Keepalive,
}

impl From<Asked> for FetchAsk {
    /// An ask as the page's bindings recorded it, as it crosses: the URL
    /// serialised, everything else as it was.
    fn from(asked: Asked) -> Self {
        Self {
            number: asked.number,
            url: asked.url.serialised,
            method: asked.method,
            headers: asked.headers,
            body: asked.body,
            mode: asked.mode,
            credentials: asked.credentials,
            redirect: asked.redirect,
            referrer: asked.referrer,
            keepalive: asked.keepalive,
        }
    }
}

/// Which of Fetch's filtered responses an answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The document's own origin: everything but `Set-Cookie`.
    Basic,
    /// Another origin that agreed to be read: only the headers it exposed.
    Cors,
    /// Another origin, not readable: nothing at all.
    Opaque,
    /// A redirect the page asked to stop at: nothing at all.
    OpaqueRedirect,
}

impl Kind {
    /// The name `Response.type` gives it.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Basic => "basic",
            Kind::Cors => "cors",
            Kind::Opaque => "opaque",
            Kind::OpaqueRedirect => "opaqueredirect",
        }
    }

    /// The same kind, as the page's bindings name it.
    const fn as_seen(self) -> response::Kind {
        match self {
            Kind::Basic => response::Kind::Basic,
            Kind::Cors => response::Kind::Cors,
            Kind::Opaque => response::Kind::Opaque,
            Kind::OpaqueRedirect => response::Kind::OpaqueRedirect,
        }
    }

    /// Whether a page may see anything of an answer of this kind.
    pub const fn is_opaque(self) -> bool {
        matches!(self, Kind::Opaque | Kind::OpaqueRedirect)
    }
}

/// A response, as the page may see it.
///
/// For an opaque kind every field but the kind is empty — status 0, no URL,
/// no headers, no body — and the decoder refuses one that is not
/// ([`crate::wire`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readable {
    /// Which filtered response this is.
    pub kind: Kind,
    /// The status, or 0.
    pub status: u16,
    /// The status text, as the server said it.
    pub status_text: String,
    /// Where it came from, after redirects, as the browser process parsed it.
    pub url: Option<String>,
    /// Whether a redirect was followed on the way.
    pub redirected: bool,
    /// The headers the page may read, in order.
    pub headers: Vec<(String, String)>,
    /// The body, whole.
    pub body: Vec<u8>,
}

/// What became of an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// A response the page may see as much of as [`Readable`] holds.
    Response(Box<Readable>),
    /// A network error. **No reason**, on purpose: the page's promise
    /// rejects with one `TypeError` whatever happened, and the reason is
    /// written where the person can see it instead (ADR 0032 § 4).
    NetworkError,
}

impl Answer {
    /// What the page's promise is settled with: the response as the page's
    /// bindings hold one, or [`None`] for a network error.
    pub fn responded(&self) -> Option<Responded> {
        match self {
            Answer::Response(readable) => Some(Responded {
                kind: readable.kind.as_seen(),
                status: readable.status,
                status_text: readable.status_text.clone(),
                url: readable.url.clone(),
                redirected: readable.redirected,
                headers: readable.headers.clone(),
                body: readable.body.clone(),
            }),
            Answer::NetworkError => None,
        }
    }
}

/// The answer to one ask, sent to the renderer that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The ask's number, as the renderer chose it.
    pub number: u64,
    /// What became of it.
    pub answer: Answer,
}

impl Fetched {
    /// A network error for ask `number`.
    pub const fn failed(number: u64) -> Self {
        Self {
            number,
            answer: Answer::NetworkError,
        }
    }
}

impl fmt::Display for Fetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.answer {
            Answer::Response(readable) => write!(
                f,
                "the answer to fetch {}: {} {}, {} bytes",
                self.number,
                readable.kind.name(),
                readable.status,
                readable.body.len()
            ),
            Answer::NetworkError => {
                write!(f, "the answer to fetch {}: a network error", self.number)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_says_what_it_is_and_a_failure_says_no_more() {
        let readable = Readable {
            kind: Kind::Basic,
            status: 200,
            status_text: "OK".to_owned(),
            url: Some("https://example.com/a".to_owned()),
            redirected: false,
            headers: Vec::new(),
            body: b"hello".to_vec(),
        };
        let fetched = Fetched {
            number: 3,
            answer: Answer::Response(Box::new(readable)),
        };
        assert_eq!(
            fetched.to_string(),
            "the answer to fetch 3: basic 200, 5 bytes"
        );
        assert_eq!(
            Fetched::failed(4).to_string(),
            "the answer to fetch 4: a network error"
        );
    }

    #[test]
    fn the_kinds_are_named_as_a_page_reads_them() {
        assert_eq!(Kind::OpaqueRedirect.name(), "opaqueredirect");
        assert!(Kind::Opaque.is_opaque() && Kind::OpaqueRedirect.is_opaque());
        assert!(!Kind::Cors.is_opaque() && !Kind::Basic.is_opaque());
    }
}
