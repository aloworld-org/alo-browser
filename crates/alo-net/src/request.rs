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
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Purpose::Document => "document",
            Purpose::Style => "style",
            Purpose::Image => "image",
            Purpose::Script => "script",
            Purpose::Fetch => "fetch",
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
}
