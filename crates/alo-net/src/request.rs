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
//! Three of its clauses are here, and one is still owed:
//!
//! - A [`Cause`] beside [`Purpose`], with **no default** — a request that
//!   cannot say what caused it does not compile, because neither constructor
//!   will make one without it. [`Purpose`] says what kind of thing is wanted; a
//!   cause says who wanted it, and the two are not the same question.
//! - **Three causes and no fourth**: a person, a document, an agent action.
//!   There is deliberately no `Unknown` — an engine-made request is attributed
//!   to whatever caused the thing it is about, the way a [`Purpose::Report`]
//!   already belongs to the load that violated the policy. That is why a
//!   redirect, a range request and a preflight **clone** a cause rather than
//!   composing one.
//! - It is assigned by the **browser process**. A renderer states a
//!   [`Purpose`], which it is the only thing that knows, and never a cause: it
//!   parsed a stranger's page (ADR 0005), so a cause it could state is a cause
//!   it could forge. Structurally, a renderer has nothing to state one *with* —
//!   only a [`crate::cause::Identities`] mints an id and it lives on this side.
//!
//! The fourth is next door rather than here, which is why this file is only
//! about one request: a cause is *a link in a chain*, because *which page* and
//! *which agent action* are two questions with two true answers. A
//! [`Cause::Document`] names a document, and [`crate::chain`] is what caused
//! **that** document's load — the walk ADR 0012 § 3 describes, over a record
//! the browser process writes as it loads pages.
//!
//! **Owed, and queue item 200's**: nothing writes down the requests
//! themselves. A cause travels with a request and is not kept afterwards.

use crate::cause::Cause;
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
    /// What caused it: a person, a document, or an agent action (ADR 0012).
    ///
    /// **Not the same question as [`Request::initiator`]**, which is an
    /// *origin* and is what queue item 61 decides against. Two documents of the
    /// same origin have one initiator and two causes, and that difference is
    /// the whole of *which page did this*.
    ///
    /// There is no builder for it and no default: it is an argument to
    /// [`Request::get`] and [`Request::sending`] because ADR 0012 § 1 asks for
    /// a guarantee rather than a habit.
    pub cause: Cause,
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
    /// A plain `GET` for a document, asked by nobody, caused by this.
    ///
    /// The cause is an argument rather than something added afterwards, and
    /// that is the whole of ADR 0012 § 1:
    ///
    /// ```
    /// # fn made(url: alo_url::Url, cause: alo_net::Cause) {
    /// let request = alo_net::Request::get(url, cause);
    /// # let _ = request;
    /// # }
    /// ```
    ///
    /// There is no way to leave it out, which is a property of this signature
    /// rather than of anybody's care:
    ///
    /// ```compile_fail
    /// # fn made(url: alo_url::Url) {
    /// let request = alo_net::Request::get(url);
    /// # let _ = request;
    /// # }
    /// ```
    pub fn get(url: Url, cause: Cause) -> Self {
        Self {
            url,
            method: "GET".to_owned(),
            purpose: Purpose::Document,
            cause,
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
    /// [`Request::declared_length`]. The cause is not the caller's to omit: see
    /// [`Request::get`].
    pub fn sending(url: Url, method: &str, body: Vec<u8>, cause: Cause) -> Self {
        Self {
            method: method.to_owned(),
            body,
            ..Self::get(url, cause)
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
        // Last, and never omitted: a request that printed without saying what
        // caused it would be the shape of a record with the answer missing,
        // which is what ADR 0012 § 1 refuses in the type.
        write!(f, ", caused by {}", self.cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cause::Identities;

    fn url(text: &str) -> Url {
        alo_url::parse(text).expect("a URL")
    }

    /// What every request in this file was caused by: a person, in one tab.
    ///
    /// The same tab each time, which is what these tests mean — they are about
    /// one person doing one thing.
    fn a_person() -> Cause {
        Cause::Person {
            tab: Identities::default().a_tab(),
        }
    }

    #[test]
    fn the_first_load_of_a_window_is_asked_by_nobody() {
        let request = Request::get(url("https://example.com/"), a_person());
        assert_eq!(request.method, "GET");
        assert_eq!(request.purpose, Purpose::Document);
        assert_eq!(request.initiator, None, "no page asked for it");
    }

    /// The two questions [`Request`] answers separately: *which origin may read
    /// this* and *what caused it*.
    #[test]
    fn a_request_says_what_caused_it_as_well_as_who_may_read_it() {
        let page = Origin::of(&url("https://example.com/"));
        let mut minting = Identities::default();
        let document = minting.a_document();
        let request = Request::get(
            url("https://example.com/a.css"),
            Cause::Document { document },
        )
        .for_purpose(Purpose::Style)
        .asked_by(page.clone());

        assert_eq!(request.initiator, Some(page));
        assert_eq!(request.cause, Cause::Document { document });
        let said = request.to_string();
        assert!(said.contains("style"), "{said:?}");
        assert!(said.contains("https://example.com"), "{said:?}");
        assert!(said.contains("caused by document#0"), "{said:?}");
    }

    /// A person's own navigation and a page's fetch are different lines in the
    /// record even when they are the same URL for the same origin.
    #[test]
    fn a_person_and_a_page_asking_for_the_same_thing_are_two_causes() {
        let mut minting = Identities::default();
        let typed = Request::get(
            url("https://example.com/a"),
            Cause::Person {
                tab: minting.a_tab(),
            },
        );
        let fetched = Request::get(
            url("https://example.com/a"),
            Cause::Document {
                document: minting.a_document(),
            },
        );
        assert_ne!(typed.cause, fetched.cause);
        assert_ne!(typed, fetched, "two causes are two requests");
    }

    #[test]
    fn what_a_page_reads_carries_nothing_and_declares_nothing() {
        let request = Request::get(url("https://example.com/"), a_person());
        assert!(request.body.is_empty());
        assert_eq!(request.declared_length(), None, "a GET said it had a body");
    }

    #[test]
    fn a_request_that_sends_something_declares_what_it_actually_sends() {
        let request = Request::sending(
            url("https://example.com/"),
            "POST",
            b"name=x".to_vec(),
            a_person(),
        );
        assert_eq!(request.method, "POST");
        assert_eq!(request.declared_length(), Some(6));
        assert!(!request.may_be_repeated(), "a POST is not repeatable");
    }

    /// The difference between a `POST` that sends nothing and a `POST` a server
    /// is still waiting on.
    #[test]
    fn a_method_that_anticipates_content_says_zero_rather_than_nothing() {
        let empty = Request::sending(url("https://example.com/"), "POST", Vec::new(), a_person());
        assert_eq!(empty.declared_length(), Some(0));
    }

    /// A caller's length is never the one sent. A body and a header disagreeing
    /// about where a message ends is the request half of request smuggling.
    #[test]
    fn a_length_is_the_bodys_rather_than_whatever_a_caller_wrote() {
        let mut request = Request::sending(
            url("https://example.com/"),
            "POST",
            b"1234".to_vec(),
            a_person(),
        );
        request.headers.add("Content-Length", "99999");
        assert_eq!(request.declared_length(), Some(4));
    }

    #[test]
    fn an_expectation_is_refused_by_name_rather_than_ignored() {
        let plain = Request::sending(
            url("https://example.com/"),
            "POST",
            b"x".to_vec(),
            a_person(),
        );
        assert_eq!(plain.unmet_expectation(), None);

        let mut expecting = plain.clone();
        expecting.headers.add("expect", "100-continue");
        let why = expecting.unmet_expectation().unwrap_or_default();
        assert!(why.contains("100-continue"), "{why:?}");
        assert!(why.contains("Expect"), "{why:?}");
    }
}
