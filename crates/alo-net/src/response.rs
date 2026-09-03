/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What came back.
//!
//! Bytes, not text. What encoding they are in is a question with a
//! [`crate::encoding`]-shaped answer that needs the headers *and* the bytes,
//! so a response that decoded eagerly would have to decide before it had
//! everything — and would have to decide again when it was wrong.

use crate::headers::Headers;
use crate::media_type::MediaType;
use alo_url::Url;
use core::fmt;

/// What a server said about how it went.
///
/// A number, because that is what it is on the wire and because a browser has
/// to carry one it has never heard of without flattening it to "an error".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Status(pub u16);

impl Status {
    /// `200`.
    pub const OK: Status = Status(200);

    /// Whether this is a success — the two hundreds.
    pub fn is_ok(self) -> bool {
        (200..300).contains(&self.0)
    }

    /// How many interim responses this engine will read before the real one.
    ///
    /// A bound rather than a limit somebody hit: an interim response is a head
    /// with no body, so a server can send them for ever at almost no cost to
    /// itself, and a client that read them for ever would be a tab that never
    /// finishes loading. One number, read by both protocols, because a bound
    /// that differed between them would be one of them being wrong.
    pub const MOST_INTERIM: usize = 8;

    /// Whether this is something said *before* the answer — the one hundreds.
    ///
    /// An interim response is not the response. It carries no body, it does not
    /// end the message, and another one may follow it. A client that treated
    /// one as the answer would show a blank page for every server that sends
    /// `103 Early Hints`, which is a great many of them — and every client has
    /// to read them whether or not it asked for one, because a server may send
    /// one unprompted.
    pub fn is_interim(self) -> bool {
        (100..200).contains(&self.0)
    }

    /// Whether this points somewhere else.
    pub fn is_redirect(self) -> bool {
        matches!(self.0, 301 | 302 | 303 | 307 | 308)
    }

    /// Whether the asking was wrong, or the answering was.
    pub fn is_error(self) -> bool {
        self.0 >= 400
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What a fetch produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// Where it actually came from — **after redirects**, which is what an
    /// origin has to be taken from rather than what was asked for.
    pub url: Url,
    /// How it went.
    pub status: Status,
    /// What came with it.
    pub headers: Headers,
    /// The bytes. Undecoded, deliberately: see this file's own note.
    pub body: Vec<u8>,
}

impl Response {
    /// A plain success.
    pub fn ok(url: Url, body: Vec<u8>) -> Self {
        Self {
            url,
            status: Status::OK,
            headers: Headers::new(),
            body,
        }
    }

    /// What the `Content-Type` says these bytes are, if it says anything this
    /// engine can read.
    pub fn media_type(&self) -> Option<MediaType> {
        MediaType::parse(self.headers.get("Content-Type")?)
    }

    /// The body as text, with the encoding decided the way HTML says.
    pub fn text(&self) -> crate::encoding::Decoded {
        crate::encoding::decode(&self.body, self.media_type().as_ref())
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({} bytes)",
            self.status,
            self.url,
            self.body.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_says_which_kind_it_is_without_a_table_of_every_one() {
        assert!(Status::OK.is_ok());
        assert!(Status(204).is_ok());
        assert!(Status(301).is_redirect());
        assert!(Status(404).is_error());
        assert!(Status(500).is_error());
        // One nobody has heard of is carried rather than flattened.
        assert!(!Status(299).is_error());
        assert!(Status(299).is_ok());
        assert_eq!(Status(418).to_string(), "418");
    }

    #[test]
    fn the_body_is_bytes_until_something_asks_for_text() {
        let url = alo_url::parse("https://example.com/").expect("a URL");
        let mut response = Response::ok(url, b"<p>hello</p>".to_vec());
        assert_eq!(response.media_type(), None, "nobody said what it is");
        response
            .headers
            .add("Content-Type", "text/html; charset=utf-8");
        assert!(response.media_type().is_some_and(|held| held.is_html()));
        assert_eq!(response.text().text, "<p>hello</p>");
    }
}
