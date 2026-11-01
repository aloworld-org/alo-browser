/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One answer asked of `OpenAI`'s Responses API, and what it comes to (ADR
//! 0081, ADR 0095, queue item 707).
//!
//! # The model
//!
//! [`MODEL`] — the one place it is named. ADR 0081 records why: the
//! builder's choice, pending the owner's confirmation; changing it is
//! changing this line.
//!
//! # What is sent
//!
//! ADR 0095 § 3: a `POST` of [`ENDPOINT`] whose body
//! ([`crate::answer_protocol::request_body`]) is the person's words and the
//! first sites the search found — title, site, address and description —
//! with `store: false`; the key as `Authorization: Bearer …`,
//! `Content-Type: application/json`, `Accept: application/json`, and what
//! `alo-net` sends with every request (`Host`, alo's `User-Agent`,
//! `Accept-Encoding`). No cookie, no `Referer`, no `OpenAI-Organization` or
//! `OpenAI-Project`. It is asked as the person's own (`Cause::Person`),
//! since a person's search caused it.
//!
//! # Where it goes
//!
//! [`Provider::openai`] asks [`ENDPOINT`], always. A test gives another
//! address with [`Provider::at`] — a server on this machine answering
//! hand-written replies in `OpenAI`'s shape. That is a Rust constructor and
//! nothing else: no page, renderer, agent, argument or environment variable
//! can change where an answer is asked, or which model.
//!
//! # What the person is told
//!
//! Where the answer would be, in one sentence ([`unanswered`]): the key
//! refused (401, 403), too many requests or the allowance used up (429),
//! the model not offered (404), `OpenAI` unable to answer (5xx), unreachable,
//! or its answer unreadable or refused. None says the key.

use crate::answer_protocol::{self, Spent};
use crate::search_key::{Held, Key};
use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Request};
use alo_renderer::results::Found;
use alo_renderer::results_answer::Answer;
use alo_url::Url;

/// The model that writes alo's answers (ADR 0081): the builder's choice of
/// 1 November 2026, pending the owner's confirmation.
pub const MODEL: &str = "gpt-6.1-terra";

/// Where an answer is asked: `OpenAI`'s Responses API.
pub const ENDPOINT: &str = "https://api.openai.com/v1/responses";

/// Who answers, in words a person reads.
pub const ANSWERER: &str = "OpenAI";

/// Where answers are asked, of which model, and with what key.
#[derive(Debug, Clone)]
pub struct Provider {
    /// The Responses API's address.
    endpoint: Url,
    /// The model asked.
    model: &'static str,
    /// The key.
    key: Key,
}

impl Provider {
    /// `OpenAI`'s Responses API, asking [`MODEL`] with `key`.
    ///
    /// # Errors
    ///
    /// Only if the URL parser stopped reading [`ENDPOINT`], which a test
    /// pins.
    pub fn openai(key: Key) -> Result<Self, String> {
        Self::at(ENDPOINT, key)
    }

    /// A provider answering at `endpoint` in `OpenAI`'s shape, asking
    /// [`MODEL`] with `key`: for a test's server on this machine.
    ///
    /// # Errors
    ///
    /// When `endpoint` is not an `http` or `https` address.
    pub fn at(endpoint: &str, key: Key) -> Result<Self, String> {
        let endpoint = alo_url::parse(endpoint)
            .ok()
            .filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
            .ok_or_else(|| format!("{endpoint} is not an answer provider's address"))?;
        Ok(Self {
            endpoint,
            model: MODEL,
            key,
        })
    }

    /// The model asked.
    pub fn model(&self) -> &'static str {
        self.model
    }

    /// The request for an answer to `question` from `sources`, made as the
    /// person's.
    pub fn request(
        &self,
        question: &str,
        sources: &[Found],
        identities: &mut Identities,
    ) -> Request {
        let mut request = Request::sending(
            self.endpoint.clone(),
            "POST",
            answer_protocol::request_body(self.model, question, sources),
            Cause::Person {
                tab: identities.a_tab(),
            },
        )
        .reading_at_most(answer_protocol::LARGEST as u64);
        request.headers.add("Content-Type", "application/json");
        request.headers.add("Accept", "application/json");
        request.headers.add(
            "Authorization",
            format!("Bearer {}", self.key.header_value()),
        );
        request
    }
}

/// What came of asking for an answer: the answer, or why not in its
/// [`Answer::Unanswered`]; and what it cost, if `OpenAI` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The answer, or why there is none.
    pub answer: Answer,
    /// What it cost.
    pub spent: Option<Spent>,
}

/// Ask `provider` through `pool` to answer `question` from `sources`: what
/// came of it, an answer not had saying why in a sentence a person reads.
pub fn ask(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    question: &str,
    sources: &[Found],
) -> Asked {
    let request = provider.request(question, sources, identities);
    let not = |why: String| Asked {
        answer: Answer::Unanswered(why),
        spent: None,
    };
    match pool.fetch(&request) {
        Ok(response) if response.status.0 == 200 => {
            match answer_protocol::read(&response.body, sources.len()) {
                Ok(read) => Asked {
                    answer: read.answer,
                    spent: read.spent,
                },
                Err(why) => not(format!(
                    "alo could not write an answer: {ANSWERER}'s answer could not be read: {why}."
                )),
            }
        }
        Ok(response) => not(unanswered(response.status.0, provider.key.held())),
        Err(why) => not(format!(
            "alo could not write an answer: {ANSWERER} could not be reached: {why}"
        )),
    }
}

/// What a person is told when `OpenAI`'s answer has the status `status`, not
/// 200, to a request asked with a key `held` there.
pub fn unanswered(status: u16, held: Held) -> String {
    let why = match (status, held) {
        (401 | 403, Held::Keychain) => format!(
            "{ANSWERER} refused alo's key ({status}). The key in this Mac's keychain may be \
             wrong or revoked."
        ),
        (401 | 403, Held::Carried) => format!(
            "{ANSWERER} refused alo's key ({status}). The key this application carries may \
             have been revoked: a newer alo carries a new one."
        ),
        (404, _) => format!("{ANSWERER} does not offer alo's model, {MODEL}, to its key (404)."),
        (429, _) => format!(
            "{ANSWERER} says alo has asked too often, or its allowance is used up ({status}). \
             Try again later."
        ),
        (500..=599, _) => format!("{ANSWERER} could not answer just now ({status})."),
        _ => format!("{ANSWERER} answered {status}, which alo does not read."),
    };
    format!("alo could not write an answer: {why}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key::of("test-key-not-real").unwrap()
    }

    #[test]
    fn the_endpoint_is_openais_and_only_https_or_http_is_a_provider() {
        let openai = Provider::openai(key()).unwrap();
        assert_eq!(openai.endpoint.to_string(), ENDPOINT);
        assert_eq!(openai.model(), MODEL);
        assert!(Provider::at("http://127.0.0.1:9/v1/responses", key()).is_ok());
        assert!(Provider::at("file:///etc/passwd", key()).is_err());
        assert!(Provider::at("not an address", key()).is_err());
    }

    #[test]
    fn the_request_is_a_post_carrying_the_key_in_its_one_header() {
        let openai = Provider::openai(key()).unwrap();
        let sources = [Found::new(
            "A",
            alo_url::parse("https://a.example/").unwrap(),
            "B",
        )];
        let request = openai.request("q", &sources, &mut Identities::default());
        assert_eq!(request.method, "POST");
        assert_eq!(request.url.to_string(), ENDPOINT);
        assert!(matches!(request.cause, Cause::Person { .. }));
        assert!(request.initiator.is_none());
        let headers: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        assert_eq!(
            headers,
            vec![
                ("Content-Type".to_owned(), "application/json".to_owned()),
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "Authorization".to_owned(),
                    "Bearer test-key-not-real".to_owned()
                ),
            ]
        );
        assert_eq!(
            request.body,
            answer_protocol::request_body(MODEL, "q", &sources)
        );
        let body = String::from_utf8_lossy(&request.body);
        assert!(!body.contains("test-key"), "the key is in the body");
        assert!(body.contains("\"store\":false"), "{body}");
    }

    #[test]
    fn every_status_is_said_in_words_and_none_says_the_key() {
        for (status, says) in [
            (401, "refused alo's key (401)"),
            (403, "refused alo's key (403)"),
            (
                404,
                "does not offer alo's model, gpt-6.1-terra, to its key (404)",
            ),
            (429, "allowance is used up (429)"),
            (500, "could not answer just now (500)"),
            (503, "could not answer just now (503)"),
            (400, "answered 400, which alo does not read"),
        ] {
            for held in [Held::Keychain, Held::Carried] {
                let said = unanswered(status, held);
                assert!(said.contains(says), "{status}: {said}");
                assert!(
                    said.starts_with("alo could not write an answer: OpenAI"),
                    "{said}"
                );
                assert!(!said.contains("test-key"), "{said}");
            }
        }
        assert!(unanswered(401, Held::Keychain).ends_with("keychain may be wrong or revoked."));
        assert!(unanswered(403, Held::Carried).ends_with("a newer alo carries a new one."));
    }
}
