/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is being asked for.
//!
//! A request carries **who is asking** as well as what for, and that is not
//! decoration: the origin decides whether a response may be read (queue item
//! 61), and queue item 67 asks this crate to say which page and which agent
//! action caused every request. A request that could not say would make both
//! of those impossible to add later, which is the same argument ADR 0005 makes
//! about the process boundary.
//!
//! # What ADR 0012 lands in this file
//!
//! The decision is written and the code is queue item 67's. Four of its clauses
//! are this file's, and they are here rather than only in `docs/decisions/` so
//! that whoever adds the field reads them where the field goes:
//!
//! - A **cause** beside [`Purpose`], with **no default** — a request that
//!   cannot say what caused it does not compile. [`Purpose`] says what kind of
//!   thing is wanted; a cause says who wanted it, and the two are not the same
//!   question.
//! - **Three causes and no fourth**: a person, a document, an agent action.
//!   There is deliberately no `Unknown` — an engine-made request is attributed
//!   to whatever caused the thing it is about, the way a [`Purpose::Report`]
//!   already belongs to the load that violated the policy.
//! - A cause is **a link in a chain**, because *which page* and *which agent
//!   action* are two questions with two true answers, and a document records
//!   what caused its own load.
//! - It is assigned by the **browser process**. A renderer states a
//!   [`Purpose`], which it is the only thing that knows, and never a cause: it
//!   parsed a stranger's page (ADR 0005), so a cause it could state is a cause
//!   it could forge.

use crate::headers::Headers;
use alo_url::{Origin, Url};
use core::fmt;

/// Why something is being fetched.
///
/// Not cosmetic: it decides which rules apply. A stylesheet and a page have
/// different content-type expectations, and a request made *by an agent* is
/// one queue item 67 has to be able to name afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Purpose {
    /// The page itself.
    #[default]
    Document,
    /// A style sheet the page asked for.
    Style,
    /// A picture.
    Image,
    /// A script.
    Script,
    /// A fetch a script made.
    Fetch,
    /// A violation report, which the *engine* sends on a page's behalf rather
    /// than the page asking for it.
    ///
    /// Its own purpose rather than a [`Purpose::Fetch`], and the reason is a
    /// rule instead of a label: a policy governs what the page loads and
    /// deliberately does not govern its own reporting, so a report made with
    /// `Fetch` on a page saying `connect-src 'none'` would be blocked by the
    /// very policy it was about. See [`crate::csp`].
    Report,
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Purpose::Document => "document",
            Purpose::Style => "style",
            Purpose::Image => "image",
            Purpose::Script => "script",
            Purpose::Fetch => "fetch",
            Purpose::Report => "violation report",
        })
    }
}

/// One thing to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// What to fetch.
    pub url: Url,
    /// The method. `GET` until there is a form to submit.
    pub method: String,
    /// What is being asked, and by what.
    pub purpose: Purpose,
    /// Who is asking — the origin of the document that wanted it.
    ///
    /// [`None`] for the first load of a window, which nobody's page asked for.
    /// Every later request has one, and item 61 is where it starts deciding
    /// what comes back.
    pub initiator: Option<Origin>,
    /// The headers to send.
    pub headers: Headers,
    /// The bytes to send with it. Empty for everything a page merely reads.
    ///
    /// Held whole rather than as a stream, and that is a bound as much as a
    /// simplification: a request body is something *this* engine composed — a
    /// form, a `fetch()` — so its size is ours already. A body that is a file
    /// on a disk is item 82's problem, and it will want a reader here rather
    /// than a `Vec`.
    pub body: Vec<u8>,
}

impl Request {
    /// A plain `GET` for a document, asked by nobody.
    pub fn get(url: Url) -> Self {
        Self {
            url,
            method: "GET".to_owned(),
            purpose: Purpose::Document,
            initiator: None,
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    /// A request that sends something: a form, or what a script handed to
    /// `fetch()`.
    ///
    /// The content type is the caller's to add as a header, because only the
    /// caller knows what the bytes are. The **length** is not: see
    /// [`Request::declared_length`].
    pub fn sending(url: Url, method: &str, body: Vec<u8>) -> Self {
        Self {
            method: method.to_owned(),
            body,
            ..Self::get(url)
        }
    }

    /// The same request, asked for a different reason.
    #[must_use]
    pub fn for_purpose(mut self, purpose: Purpose) -> Self {
        self.purpose = purpose;
        self
    }

    /// The same request, with a page behind it.
    #[must_use]
    pub fn asked_by(mut self, initiator: Origin) -> Self {
        self.initiator = Some(initiator);
        self
    }

    /// Whether sending this twice is the same as sending it once.
    ///
    /// The standard's word is *idempotent*, and the list is the standard's. It
    /// is deliberately not "anything that looks read-only": a `POST` that
    /// failed after the server received it is a payment that has happened, and
    /// sending it again is a payment that has happened twice.
    ///
    /// Here rather than in [`crate::pool`] because two things need it and they
    /// need it to agree — the pool's retry of a connection that was closed
    /// while idle, and [`crate::download::whole_of`], which asks more than once
    /// by definition. Two spellings of this list is one of them being wrong
    /// about a payment.
    pub fn may_be_repeated(&self) -> bool {
        matches!(
            self.method.as_str(),
            "GET" | "HEAD" | "OPTIONS" | "TRACE" | "PUT" | "DELETE"
        )
    }

    /// The length this request states about itself, when it states one.
    ///
    /// **A caller never sets this**, in either protocol: a `Content-Length`
    /// that disagrees with the bytes is the request half of the message that
    /// says two things about where it ends, and that is what request smuggling
    /// is made of. Both `crate::http::write_request` and the HTTP/2 client drop
    /// a caller's and write this instead — here rather than in each of them,
    /// for the reason [`Request::may_be_repeated`] gives: two spellings of a
    /// framing rule is one of them being wrong.
    ///
    /// A method that *anticipates* content says `0` rather than nothing, so
    /// that a `POST` with an empty body is a `POST` sending nothing rather than
    /// a `POST` a server is still waiting on.
    pub fn declared_length(&self) -> Option<usize> {
        if self.body.is_empty() && !matches!(self.method.as_str(), "POST" | "PUT" | "PATCH") {
            return None;
        }
        Some(self.body.len())
    }

    /// An `Expect` this engine will not pretend to honour, said in words.
    ///
    /// **This engine implements no expectation, and refuses rather than
    /// ignores.** An `Expect` is a promise that the sender will *wait*, and the
    /// only bound available for the waiting here is the caller's own socket
    /// timeout — thirty seconds, which would turn every upload to a server that
    /// has never heard of `100-continue` into half a minute of nothing. Sending
    /// the header and then not waiting is worse than either: it tells a server
    /// that does honour it to hold the stream open for a go-ahead we have
    /// already stopped listening for.
    ///
    /// Nothing on the web can reach this. `Expect` is a forbidden request
    /// header in Fetch, so no page and no script may set it; only this engine's
    /// own code could, and this is what it is told when it does. Honouring it
    /// properly needs a bounded wait, which is queue item 187.
    pub fn unmet_expectation(&self) -> Option<String> {
        let asked = self.headers.get("Expect")?;
        Some(format!(
            "an Expect of {asked:?}, which this engine will not claim to honour: \
             an expectation is a promise to wait, and nothing here can bound the waiting"
        ))
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} ({})", self.method, self.url, self.purpose)?;
        if let Some(initiator) = &self.initiator {
            write!(f, " for {initiator}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).expect("a URL")
    }

    #[test]
    fn the_first_load_of_a_window_is_asked_by_nobody() {
        let request = Request::get(url("https://example.com/"));
        assert_eq!(request.method, "GET");
        assert_eq!(request.purpose, Purpose::Document);
        assert_eq!(request.initiator, None, "no page asked for it");
    }

    #[test]
    fn everything_a_page_asks_for_says_which_page_asked() {
        let page = Origin::of(&url("https://example.com/"));
        let request = Request::get(url("https://example.com/a.css"))
            .for_purpose(Purpose::Style)
            .asked_by(page.clone());
        assert_eq!(request.initiator, Some(page));
        assert!(request.to_string().contains("style"));
        assert!(request.to_string().contains("https://example.com"));
    }

    #[test]
    fn what_a_page_reads_carries_nothing_and_declares_nothing() {
        let request = Request::get(url("https://example.com/"));
        assert!(request.body.is_empty());
        assert_eq!(request.declared_length(), None, "a GET said it had a body");
    }

    #[test]
    fn a_request_that_sends_something_declares_what_it_actually_sends() {
        let request = Request::sending(url("https://example.com/"), "POST", b"name=x".to_vec());
        assert_eq!(request.method, "POST");
        assert_eq!(request.declared_length(), Some(6));
        assert!(!request.may_be_repeated(), "a POST is not repeatable");
    }

    /// The difference between a `POST` that sends nothing and a `POST` a server
    /// is still waiting on.
    #[test]
    fn a_method_that_anticipates_content_says_zero_rather_than_nothing() {
        let empty = Request::sending(url("https://example.com/"), "POST", Vec::new());
        assert_eq!(empty.declared_length(), Some(0));
    }

    /// A caller's length is never the one sent. A body and a header disagreeing
    /// about where a message ends is the request half of request smuggling.
    #[test]
    fn a_length_is_the_bodys_rather_than_whatever_a_caller_wrote() {
        let mut request = Request::sending(url("https://example.com/"), "POST", b"1234".to_vec());
        request.headers.add("Content-Length", "99999");
        assert_eq!(request.declared_length(), Some(4));
    }

    #[test]
    fn an_expectation_is_refused_by_name_rather_than_ignored() {
        let plain = Request::sending(url("https://example.com/"), "POST", b"x".to_vec());
        assert_eq!(plain.unmet_expectation(), None);

        let mut expecting = plain.clone();
        expecting.headers.add("expect", "100-continue");
        let why = expecting.unmet_expectation().unwrap_or_default();
        assert!(why.contains("100-continue"), "{why:?}");
        assert!(why.contains("Expect"), "{why:?}");
    }
}
