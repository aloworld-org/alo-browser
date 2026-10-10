/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's fetch on the wire: the asks in an answer, and the answer to one
//! (ADR 0032, queue item 334).
//!
//! The asks come from a renderer, so every field is read as a stranger's:
//! lengths checked against what is left, enums by a closed list of tags, the
//! URL as text the browser process parses itself, and whether it outlives its
//! page as one of three tags. **How many** is not bounded
//! here: an ask past the browser process's bounds is a network error the page
//! is answered with ([`crate::fetch_owed`]), not a message nobody can read.
//!
//! The answer goes to a renderer, and is read with the same suspicion — and
//! one check of its own: an opaque answer that carries anything is refused,
//! because the one promise an opaque answer makes is that it carries nothing.

use alo_bindings::fetching::Keepalive;
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;

use super::{POLICIES, Reader, Unreadable, Writer, unreadable};
use crate::fetch::{Answer, FetchAsk, Fetched, Kind, Readable};

pub(super) const MODES: [Mode; 4] = [Mode::Cors, Mode::NoCors, Mode::SameOrigin, Mode::Navigate];
pub(super) const CREDENTIALS: [Credentials; 3] = [
    Credentials::Omit,
    Credentials::SameOrigin,
    Credentials::Include,
];
const REDIRECTS: [redirect::Mode; 3] = [
    redirect::Mode::Follow,
    redirect::Mode::Error,
    redirect::Mode::Manual,
];
const KINDS: [Kind; 4] = [Kind::Basic, Kind::Cors, Kind::Opaque, Kind::OpaqueRedirect];
const KEEPALIVES: [Keepalive; 3] = [Keepalive::Not, Keepalive::Fetch, Keepalive::Beacon];

/// The tag of `value` in `list`. Every list here names every variant, so the
/// fallback is never written.
pub(super) fn tag_of<T: PartialEq>(list: &[T], value: &T) -> u8 {
    list.iter()
        .position(|known| known == value)
        .and_then(|at| u8::try_from(at).ok())
        .unwrap_or(u8::MAX)
}

/// The value tagged `tag` in `list`.
pub(super) fn tagged<T: Copy>(list: &[T], tag: u8, what: &str) -> Result<T, Unreadable> {
    list.get(usize::from(tag))
        .copied()
        .ok_or_else(|| unreadable(format!("{what} tagged {tag}")))
}

impl Writer {
    fn pairs(&mut self, pairs: &[(String, String)]) {
        self.number(pairs.len() as u64);
        for (name, value) in pairs {
            self.text(name);
            self.text(value);
        }
    }

    /// Every ask in an answer, in order.
    pub(super) fn fetches(&mut self, asks: &[FetchAsk]) {
        self.number(asks.len() as u64);
        for ask in asks {
            self.number(ask.number);
            self.text(&ask.url);
            self.text(&ask.method);
            self.pairs(&ask.headers);
            self.bytes(&ask.body);
            self.tag(tag_of(&MODES, &ask.mode));
            self.tag(tag_of(&CREDENTIALS, &ask.credentials));
            self.tag(tag_of(&REDIRECTS, &ask.redirect));
            let policy = ask
                .referrer
                .map_or(0, |policy| tag_of(&POLICIES, &policy).saturating_add(1));
            self.tag(policy);
            self.tag(tag_of(&KEEPALIVES, &ask.keepalive));
        }
    }

    /// The answer to one ask.
    pub(super) fn fetched(&mut self, fetched: &Fetched) {
        self.number(fetched.number);
        match &fetched.answer {
            Answer::NetworkError => self.tag(0),
            Answer::Response(readable) => {
                self.tag(1);
                self.tag(tag_of(&KINDS, &readable.kind));
                self.number(u64::from(readable.status));
                self.text(&readable.status_text);
                self.maybe_text(readable.url.as_deref());
                self.bool(readable.redirected);
                self.pairs(&readable.headers);
                self.bytes(&readable.body);
            }
        }
    }
}

impl Reader<'_> {
    fn pairs(&mut self) -> Result<Vec<(String, String)>, Unreadable> {
        let how_many = self.count()?;
        let mut pairs = Vec::new();
        for _ in 0..how_many {
            let name = self.text()?;
            pairs.push((name, self.text()?));
        }
        Ok(pairs)
    }

    /// Every ask in an answer: each part a claim.
    pub(super) fn fetches(&mut self) -> Result<Vec<FetchAsk>, Unreadable> {
        let how_many = self.count()?;
        let mut asks = Vec::new();
        for _ in 0..how_many {
            let number = self.number()?;
            let url = self.text()?;
            let method = self.text()?;
            let headers = self.pairs()?;
            let body = self.bytes()?;
            let mode = tagged(&MODES, self.tag()?, "a fetch's mode")?;
            let credentials = tagged(&CREDENTIALS, self.tag()?, "a fetch's credentials")?;
            let redirect = tagged(&REDIRECTS, self.tag()?, "a fetch's redirect mode")?;
            let referrer = match self.tag()? {
                0 => None,
                tag => Some(tagged(&POLICIES, tag - 1, "a fetch's referrer policy")?),
            };
            let keepalive = tagged(&KEEPALIVES, self.tag()?, "a fetch's keepalive")?;
            asks.push(FetchAsk {
                number,
                url,
                method,
                headers,
                body,
                mode,
                credentials,
                redirect,
                referrer,
                keepalive,
            });
        }
        Ok(asks)
    }

    /// The answer to one ask.
    pub(super) fn fetched(&mut self) -> Result<Fetched, Unreadable> {
        let number = self.number()?;
        let answer = match self.tag()? {
            0 => Answer::NetworkError,
            1 => {
                let kind = tagged(&KINDS, self.tag()?, "a response's kind")?;
                let status = u16::try_from(self.number()?)
                    .map_err(|_| unreadable("a status larger than any status"))?;
                let readable = Readable {
                    kind,
                    status,
                    status_text: self.text()?,
                    url: self.maybe_text()?,
                    redirected: self.bool()?,
                    headers: self.pairs()?,
                    body: self.bytes()?,
                };
                if kind.is_opaque()
                    && (readable.status != 0
                        || !readable.status_text.is_empty()
                        || readable.url.is_some()
                        || readable.redirected
                        || !readable.headers.is_empty()
                        || !readable.body.is_empty())
                {
                    return Err(unreadable(format!(
                        "an {} response carrying something a page may not read",
                        kind.name()
                    )));
                }
                Answer::Response(Box::new(readable))
            }
            other => return Err(unreadable(format!("a fetch's answer tagged {other}"))),
        };
        Ok(Fetched { number, answer })
    }
}

/// How many bytes `fetched` is as a message, without writing it: what
/// [`crate::fetch_filter`] asks before it sends a body, so that a body too
/// large for one message is a failure rather than a message nobody reads.
pub fn fetched_size(fetched: &Fetched) -> usize {
    const NUMBER: usize = 8;
    let text = |text: &str| NUMBER.saturating_add(text.len());
    // The message's tag, the ask's number and the answer's tag.
    let mut size = 1 + NUMBER + 1;
    if let Answer::Response(readable) = &fetched.answer {
        size = size
            .saturating_add(1 + NUMBER)
            .saturating_add(text(&readable.status_text))
            .saturating_add(1 + readable.url.as_deref().map_or(0, text))
            .saturating_add(1 + NUMBER)
            .saturating_add(NUMBER.saturating_add(readable.body.len()));
        for (name, value) in &readable.headers {
            size = size.saturating_add(text(name)).saturating_add(text(value));
        }
    }
    size
}
