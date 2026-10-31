/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One web search asked of the Brave Search API, and what its answer comes
//! to (ADR 0082 § 0, ADR 0083, queue item 708).
//!
//! # What is sent
//!
//! ADR 0083 § 3: a `GET` of [`ENDPOINT`] with **the words typed, trimmed, as
//! `q`, and nothing else in the address** — no count, no country, no
//! language, no partner code — and the key as `X-Subscription-Token`,
//! `Accept: application/json`, and what `alo-net` sends with every request
//! (`Host`, alo's `User-Agent`, `Accept-Encoding`). No cookie, no `Referer`.
//! It is asked as the person's own (`Cause::Person`), since a person chose
//! it.
//!
//! # Where it goes
//!
//! [`Provider::brave`] asks [`ENDPOINT`], always. A test gives another
//! address with [`Provider::at`] — a server on this machine answering
//! hand-written replies in Brave's shape. That is a Rust constructor and
//! nothing else: no page, renderer, agent, argument or environment variable
//! can change where a search goes.
//!
//! # What the person is told
//!
//! ADR 0083 § 5's sentences, by status ([`unanswered`]). None says the key.

use crate::search_key::Key;
use crate::search_protocol;
use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Request};
use alo_renderer::results::{PROVIDER, Showing};
use alo_url::Url;

/// Where a web search is asked.
pub const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Where searches go and with what key.
#[derive(Debug, Clone)]
pub struct Provider {
    /// The web search's address, with no query.
    endpoint: Url,
    /// The key.
    key: Key,
}

impl Provider {
    /// The Brave Search API, asked with `key`.
    ///
    /// # Errors
    ///
    /// Only if the URL parser stopped reading [`ENDPOINT`], which a test pins.
    pub fn brave(key: Key) -> Result<Self, String> {
        Self::at(ENDPOINT, key)
    }

    /// A provider answering at `endpoint` in Brave's shape, asked with
    /// `key`: for a test's server on this machine.
    ///
    /// # Errors
    ///
    /// When `endpoint` is not an `http` or `https` address.
    pub fn at(endpoint: &str, key: Key) -> Result<Self, String> {
        let endpoint = alo_url::parse(endpoint)
            .ok()
            .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
            .ok_or_else(|| format!("{endpoint} is not a search provider's address"))?;
        Ok(Self { endpoint, key })
    }

    /// The request for a search of `words`, made as the person's in `tab`.
    ///
    /// # Errors
    ///
    /// When the words make no address, which percent-encoding rules out.
    pub fn request(&self, words: &str, identities: &mut Identities) -> Result<Request, String> {
        let query = alo_url::form::urlencoded([("q", words.trim())]);
        let url = alo_url::with_query(&self.endpoint, &query)
            .map_err(|why| format!("the search could not be addressed: {why}"))?;
        let mut request = Request::get(
            url,
            Cause::Person {
                tab: identities.a_tab(),
            },
        );
        request.headers.add("Accept", "application/json");
        request
            .headers
            .add("X-Subscription-Token", self.key.header_value());
        Ok(request)
    }
}

/// Ask `provider` for `words` through `pool`: the sites found, or why the
/// search was not answered, in a sentence a person reads.
pub fn search(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    words: &str,
) -> Showing {
    let request = match provider.request(words, identities) {
        Ok(request) => request,
        Err(why) => return Showing::Unanswered(why),
    };
    match pool.fetch(&request) {
        Ok(response) if response.status.0 == 200 => answered(&response.body),
        Ok(response) => Showing::Unanswered(unanswered(response.status.0)),
        Err(why) => Showing::Unanswered(format!("{PROVIDER} could not be reached: {why}")),
    }
}

/// What an answer of 200 with `body` comes to.
pub fn answered(body: &[u8]) -> Showing {
    match search_protocol::read(body) {
        Ok(read) => Showing::Found(read.found),
        Err(why) => Showing::Unanswered(format!("{PROVIDER}'s answer could not be read: {why}")),
    }
}

/// What a person is told when the answer's status is `status`, not 200.
pub fn unanswered(status: u16) -> String {
    match status {
        401 | 403 => format!(
            "{PROVIDER} refused alo's key ({status}). The key in this Mac's keychain may be wrong \
             or revoked."
        ),
        429 => format!(
            "{PROVIDER} says alo has searched too often, or its allowance is used up ({status}). \
             Try again later."
        ),
        500..=599 => format!("{PROVIDER} could not answer just now ({status})."),
        _ => format!("{PROVIDER} answered {status}, which alo does not read."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key::of("test-key-not-real").unwrap()
    }

    #[test]
    fn the_endpoint_is_brave_and_only_https_or_http_is_a_provider() {
        let brave = Provider::brave(key()).unwrap();
        assert_eq!(brave.endpoint.to_string(), ENDPOINT);
        assert!(Provider::at("http://127.0.0.1:9/res/v1/web/search", key()).is_ok());
        assert!(Provider::at("file:///etc/passwd", key()).is_err());
        assert!(Provider::at("not an address", key()).is_err());
    }

    #[test]
    fn the_request_carries_the_words_alone_and_the_key_in_its_header() {
        let brave = Provider::brave(key()).unwrap();
        let request = brave
            .request("  rust borrow & checker?  ", &mut Identities::default())
            .unwrap();
        assert_eq!(
            request.url.to_string(),
            "https://api.search.brave.com/res/v1/web/search?q=rust+borrow+%26+checker%3F"
        );
        assert_eq!(request.method, "GET");
        assert!(matches!(request.cause, Cause::Person { .. }));
        let headers: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        assert_eq!(
            headers,
            vec![
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "X-Subscription-Token".to_owned(),
                    "test-key-not-real".to_owned()
                ),
            ]
        );
        assert!(request.body.is_empty());
        assert!(request.initiator.is_none());
    }

    #[test]
    fn every_status_is_said_in_words_and_none_says_the_key() {
        for (status, says) in [
            (401, "refused alo's key (401)"),
            (403, "refused alo's key (403)"),
            (429, "allowance is used up (429)"),
            (500, "could not answer just now (500)"),
            (503, "could not answer just now (503)"),
            (422, "answered 422, which alo does not read"),
            (302, "answered 302"),
        ] {
            let said = unanswered(status);
            assert!(said.contains(says), "{status}: {said}");
            assert!(said.starts_with("Brave Search"), "{said}");
            assert!(!said.contains("test-key"), "{said}");
        }
    }

    #[test]
    fn an_answer_is_sites_or_says_why_it_could_not_be_read() {
        assert_eq!(
            answered(br#"{"type":"search"}"#),
            Showing::Found(Vec::new())
        );
        assert_eq!(
            answered(b"<html>"),
            Showing::Unanswered(
                "Brave Search's answer could not be read: it is not JSON (expected value at \
                 line 1 column 1)"
                    .to_owned()
            )
        );
    }
}
