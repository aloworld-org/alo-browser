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
//! # One connector of the AI service (ADR 0106)
//!
//! [`Provider`] is an [`AiConnector`]: the answer's thread and the agent
//! hold it as an [`AiService`] and never name it. Its wire formats are its
//! own — an answer's ([`crate::answer_protocol`]) and the agent's step
//! ([`crate::agent_protocol`]).
//!
//! # What the person is told
//!
//! Where the answer would be, in one sentence ([`unanswered`]): the key
//! refused (401, 403), too many requests or the allowance used up (429),
//! the model not offered (404), the service unable to answer (5xx),
//! unreachable, or its answer unreadable or refused. None says the key, and
//! none names `OpenAI`: each calls it [`AI_SERVICE`] (ADR 0106 § 5).

use crate::agent_protocol::{self, Asking};
pub use crate::ai_service::Asked;
use crate::ai_service::{AI_SERVICE, AiConnector, AiService};
use crate::answer_protocol;
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

/// Who answers, for the terminal alone: never on screen (ADR 0106 § 5).
pub const NAME: &str = "OpenAI";

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

    /// Where its key is held: carried by the application, or in this Mac's
    /// keychain — what a refusal of it says.
    pub fn held(&self) -> Held {
        self.key.held()
    }

    /// The request for an answer to `question` from `sources`, made as the
    /// person's.
    pub fn request(
        &self,
        question: &str,
        sources: &[Found],
        identities: &mut Identities,
    ) -> Request {
        self.post(
            answer_protocol::request_body(self.model, question, sources),
            answer_protocol::LARGEST,
            identities,
        )
    }

    /// The request posting `body` to the Responses API, made as the
    /// person's, its answer read to at most `largest` bytes: an answer's
    /// ([`Self::request`]) or the agent's step (queue item 704,
    /// [`crate::agent_protocol`]), with the same headers and nothing more.
    pub fn post(&self, body: Vec<u8>, largest: usize, identities: &mut Identities) -> Request {
        let mut request = Request::sending(
            self.endpoint.clone(),
            "POST",
            body,
            Cause::Person {
                tab: identities.a_tab(),
            },
        )
        .reading_at_most(largest as u64);
        request.headers.add("Content-Type", "application/json");
        request.headers.add("Accept", "application/json");
        request.headers.add(
            "Authorization",
            format!("Bearer {}", self.key.header_value()),
        );
        request
    }
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
                    "alo could not write an answer: {AI_SERVICE}'s answer could not be read: \
                     {why}."
                )),
            }
        }
        Ok(response) => not(unanswered(response.status.0, provider.key.held())),
        Err(why) => not(format!(
            "alo could not write an answer: {AI_SERVICE} could not be reached: {why}"
        )),
    }
}

/// The agent's next step for `asking`, asked of `provider` through `pool`
/// (queue item 704, ADR 0096).
///
/// # Errors
///
/// Why there is none, in the words said after *alo could not go on:*.
pub fn step(
    provider: &Provider,
    pool: &mut Pool,
    identities: &mut Identities,
    asking: &Asking<'_>,
) -> Result<agent_protocol::Read, String> {
    let body = agent_protocol::request_body(provider.model(), asking);
    let request = provider.post(body, agent_protocol::LARGEST, identities);
    let response = pool
        .fetch(&request)
        .map_err(|why| format!("{AI_SERVICE} could not be reached: {why}"))?;
    if response.status.0 != 200 {
        return Err(why_unanswered(response.status.0, provider.held()));
    }
    agent_protocol::read(&response.body)
        .map_err(|why| format!("{AI_SERVICE}'s step could not be read: {why}."))
}

impl AiConnector for Provider {
    fn named(&self) -> String {
        format!("{NAME}'s {}", self.model)
    }

    fn answer(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        question: &str,
        sources: &[Found],
    ) -> Asked {
        ask(self, pool, identities, question, sources)
    }

    fn step(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        asking: &Asking<'_>,
    ) -> Result<agent_protocol::Read, String> {
        step(self, pool, identities, asking)
    }
}

impl From<Provider> for AiService {
    fn from(provider: Provider) -> Self {
        Self::of(provider)
    }
}

/// What a person is told when the AI service's answer has the status `status`, not
/// 200, to a request asked with a key `held` there.
pub fn unanswered(status: u16, held: Held) -> String {
    format!(
        "alo could not write an answer: {}",
        why_unanswered(status, held)
    )
}

/// Why the AI service did not answer with the status `status`, not 200, to a
/// request asked with a key `held` there: the sentence an answer and the
/// agent (queue item 704) both say after their own words.
pub fn why_unanswered(status: u16, held: Held) -> String {
    match (status, held) {
        (401 | 403, Held::Keychain) => format!(
            "{AI_SERVICE} refused alo's key ({status}). The key in this Mac's keychain may be \
             wrong or revoked."
        ),
        (401 | 403, Held::Carried) => format!(
            "{AI_SERVICE} refused alo's key ({status}). The key this application carries may \
             have been revoked: a newer alo carries a new one."
        ),
        (401 | 403, Held::Settings) => format!(
            "{AI_SERVICE} refused alo's key ({status}). The key you gave in Settings may be \
             wrong or revoked."
        ),
        (404, _) => format!("{AI_SERVICE} does not offer the model alo asks for to its key (404)."),
        (429, _) => format!(
            "{AI_SERVICE} says alo has asked too often, or its allowance is used up ({status}). \
             Try again later."
        ),
        (500..=599, _) => format!("{AI_SERVICE} could not answer just now ({status})."),
        _ => format!("{AI_SERVICE} answered {status}, which alo does not read."),
    }
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
                "does not offer the model alo asks for to its key (404)",
            ),
            (429, "allowance is used up (429)"),
            (500, "could not answer just now (500)"),
            (503, "could not answer just now (503)"),
            (400, "answered 400, which alo does not read"),
        ] {
            for held in [Held::Keychain, Held::Carried, Held::Settings] {
                let said = unanswered(status, held);
                assert!(said.contains(says), "{status}: {said}");
                assert!(
                    said.starts_with("alo could not write an answer: the AI service"),
                    "{said}"
                );
                assert!(!said.to_lowercase().contains("openai"), "{said}");
                assert!(!said.contains("test-key"), "{said}");
            }
        }
        assert!(unanswered(401, Held::Keychain).ends_with("keychain may be wrong or revoked."));
        assert!(unanswered(403, Held::Carried).ends_with("a newer alo carries a new one."));
        assert!(unanswered(401, Held::Settings).ends_with("Settings may be wrong or revoked."));
    }
}
