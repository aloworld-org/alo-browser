/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Text into a URL.
//!
//! **This is the only file that names `url`.** ADR 0001: a parser that
//! implements a specification and carries none of our value is prior art to
//! take, exactly as `html5ever` and `cssparser` were. WHATWG's URL Standard is
//! a state machine with two decades of interoperability in it, and it drags
//! IDNA in with it — the Unicode specification that decides whether `аpple.com`
//! written in Cyrillic is the same host as `apple.com`. That question is a
//! **security** question, its answer is a table, and writing our own table
//! would be spending effort on the part of a browser nobody would notice us
//! doing well and everybody would notice us doing badly.
//!
//! What comes out is ours: [`crate::Url`], in parts, with the rented type left
//! behind at this door.

use crate::parts::{Host, Url};
use core::fmt;

/// Why some text is not a URL.
///
/// One kind on purpose. A caller does not act differently on "the host is
/// wrong" than on "the scheme is wrong" — it refuses either way — and a
/// catalogue of failures would be a catalogue nobody matched on. The words are
/// for a person reading them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// What was being parsed, so an error in a log says which link.
    pub input: String,
    /// What is wrong with it, in words.
    pub why: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a URL: {}", self.input, self.why)
    }
}

impl std::error::Error for ParseError {}

/// Parse an absolute URL.
///
/// # Errors
///
/// [`ParseError`] for anything that is not one — including a relative
/// reference, which needs [`join`] and a base to mean anything.
pub fn parse(input: &str) -> Result<Url, ParseError> {
    ours(&url::Url::parse(input).map_err(|why| ParseError {
        input: input.to_owned(),
        why: why.to_string(),
    })?)
    .ok_or_else(|| ParseError {
        input: input.to_owned(),
        why: "the host is not one this engine can represent".to_owned(),
    })
}

/// Resolve a reference against a base — what every link on every page needs.
///
/// # Errors
///
/// [`ParseError`] when the result is not a URL. A reference that is itself
/// absolute replaces the base entirely, which is what makes `<a href="https://…">`
/// work on any page.
pub fn join(base: &Url, reference: &str) -> Result<Url, ParseError> {
    let parsed = url::Url::parse(&base.serialised).map_err(|why| ParseError {
        input: base.serialised.clone(),
        why: format!("the base is not a URL: {why}"),
    })?;
    ours(&parsed.join(reference).map_err(|why| ParseError {
        input: reference.to_owned(),
        why: why.to_string(),
    })?)
    .ok_or_else(|| ParseError {
        input: reference.to_owned(),
        why: "the host is not one this engine can represent".to_owned(),
    })
}

/// The rented type, as ours.
///
/// [`None`] only for a host shape that has no home in [`Host`], which the
/// rented parser does not currently produce — kept as an answer rather than an
/// assumption, because an upgrade could add one and a silent `Domain("")` is
/// worse than a refusal.
fn ours(parsed: &url::Url) -> Option<Url> {
    let host = match parsed.host() {
        None => None,
        Some(url::Host::Domain(name)) => Some(Host::Domain(name.to_owned())),
        Some(url::Host::Ipv4(address)) => Some(Host::Ipv4(address)),
        Some(url::Host::Ipv6(address)) => Some(Host::Ipv6(address)),
    };
    if parsed.has_host() && host.is_none() {
        return None;
    }
    Some(Url {
        scheme: parsed.scheme().to_owned(),
        host,
        // `port()` is already "only when written and not the default", which
        // is the distinction `Url::port` documents.
        port: parsed.port(),
        path: parsed.path().to_owned(),
        query: parsed.query().map(str::to_owned),
        fragment: parsed.fragment().map(str::to_owned),
        serialised: parsed.as_str().to_owned(),
    })
}
