/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What of a response a page may read, decided before it leaves the browser
//! process (ADR 0032 § 4, queue item 334).
//!
//! One function, [`filter`], from a decided [`Fetch`] and the response it got
//! to the [`Fetched`] a renderer is sent — Fetch's filtered responses, built
//! from `alo-net`'s [`cors`] rules:
//!
//! | | status, URL, redirected | headers | body |
//! |---|---|---|---|
//! | `basic`, the document's origin | yes | all but `Set-Cookie` | yes |
//! | `cors`, another origin that agreed | yes | the safelisted and exposed | yes |
//! | `opaque`, another origin in `no-cors` | 0, none, no | none | **none** |
//! | `opaqueredirect`, a redirect asked to stop at | 0, none, no | none | **none** |
//!
//! **The bytes of an opaque body are not sent**, so they are not in the
//! renderer's memory at all — the property ADR 0005 pays a process per site
//! for. Filtering in the renderer would have made the same-origin policy a
//! rule script is asked to keep.
//!
//! A failure crosses as a network error with **no reason in it**, and the
//! reason comes back beside it ([`Filtered::why`]) for whoever shows the
//! person and keeps the record. The same is true of a body too large to cross
//! in one message, which is a failure rather than a truncation.

use alo_net::cors::{self, Mode};
use alo_net::redirect;
use alo_net::response::Response;

use crate::fetch::{Answer, Fetched, Kind, Readable};
use crate::fetch_decide::Fetch;
use crate::wire::LARGEST_MESSAGE;

/// The response headers a page may never read, under any arrangement.
const NEVER_READ: [&str; 2] = ["set-cookie", "set-cookie2"];

/// What crosses, and why it is a failure when it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    /// What the renderer is sent.
    pub fetched: Fetched,
    /// Why it failed, for the person and the record — never for the page.
    pub why: Option<String>,
}

/// A failure of `fetch`, with the reason kept on this side.
pub fn failed(fetch: &Fetch, why: impl Into<String>) -> Filtered {
    Filtered {
        fetched: Fetched::failed(fetch.number),
        why: Some(why.into()),
    }
}

/// The response `fetch` got, as the page that asked may see it.
///
/// `status_text` is the reason phrase the server sent, which `alo-net`'s
/// [`Response`] does not keep; `redirected` is whether a redirect was
/// followed on the way to it.
pub fn filter(fetch: &Fetch, response: &Response, status_text: &str, redirected: bool) -> Filtered {
    if response.status.is_redirect() {
        match fetch.redirect {
            redirect::Mode::Manual => return crossing(fetch, nothing(Kind::OpaqueRedirect)),
            redirect::Mode::Error => {
                return failed(
                    fetch,
                    "the page asked not to be redirected, and the answer was a redirect",
                );
            }
            redirect::Mode::Follow => {}
        }
    }
    let readable = |kind: Kind, headers: &alo_net::headers::Headers| Readable {
        kind,
        status: response.status.0,
        status_text: status_text.to_owned(),
        url: Some(response.url.serialised.clone()),
        redirected,
        headers: headers
            .iter()
            .filter(|header| !NEVER_READ.contains(&header.name.to_ascii_lowercase().as_str()))
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect(),
        body: response.body.clone(),
    };
    if cors::is_same_origin(fetch.request.initiator.as_ref(), response) {
        return crossing(fetch, readable(Kind::Basic, &response.headers));
    }
    match fetch.mode {
        Mode::NoCors => crossing(fetch, nothing(Kind::Opaque)),
        Mode::Cors => match cors::may_read(&fetch.request, fetch.credentials, response) {
            Ok(()) => crossing(
                fetch,
                readable(Kind::Cors, &cors::readable(&fetch.request, response)),
            ),
            Err(refusal) => failed(fetch, refusal.to_string()),
        },
        // Refused before it was sent unless a redirect took it elsewhere.
        Mode::SameOrigin => failed(
            fetch,
            "the page asked for the same origin only, and was redirected to another",
        ),
        // Refused before it was sent, always.
        Mode::Navigate => failed(fetch, "a page's fetch is never a navigation"),
    }
}

/// An answer of an opaque kind: nothing at all.
fn nothing(kind: Kind) -> Readable {
    Readable {
        kind,
        status: 0,
        status_text: String::new(),
        url: None,
        redirected: false,
        headers: Vec::new(),
        body: Vec::new(),
    }
}

/// `readable` as the answer to `fetch`, when it fits in one message.
fn crossing(fetch: &Fetch, readable: Readable) -> Filtered {
    let fetched = Fetched {
        number: fetch.number,
        answer: Answer::Response(Box::new(readable)),
    };
    let size = crate::wire::fetched_size(&fetched);
    if size > LARGEST_MESSAGE {
        return failed(
            fetch,
            format!(
                "the response is {size} bytes as a message, and one message may carry at most \
                 {LARGEST_MESSAGE}"
            ),
        );
    }
    Filtered { fetched, why: None }
}

#[cfg(test)]
mod tests {
    use alo_net::Status;
    use alo_net::cause::{Cause, Identities};
    use alo_net::cors::Credentials;
    use alo_net::csp::Policies;
    use alo_url::Url;

    use super::*;
    use crate::fetch::FetchAsk;
    use crate::fetch_decide::{Asker, Decided, decide};

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    fn fetch(to: &str, mode: Mode, redirect: redirect::Mode) -> Fetch {
        let ask = FetchAsk {
            number: 2,
            url: to.to_owned(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
            mode,
            credentials: Credentials::SameOrigin,
            redirect,
            referrer: None,
        };
        let page = url("https://shop.example/");
        let policies = Policies::none();
        let cause = Cause::Document {
            document: Identities::default().a_document(),
        };
        match decide(
            &ask,
            &Asker {
                url: &page,
                policies: &policies,
            },
            &cause,
        ) {
            Decided::Make(fetch) => *fetch,
            Decided::Refused(refusal) => panic!("{refusal}"),
        }
    }

    fn response(at: &str, headers: &[(&str, &str)]) -> Response {
        let mut response = Response::ok(url(at), b"the secret".to_vec());
        for (name, value) in headers {
            response.headers.add(*name, *value);
        }
        response
    }

    fn readable(filtered: &Filtered) -> &Readable {
        match &filtered.fetched.answer {
            Answer::Response(readable) => readable,
            Answer::NetworkError => panic!("a network error: {:?}", filtered.why),
        }
    }

    #[test]
    fn the_documents_own_origin_reads_everything_but_set_cookie() {
        let asked = fetch("https://shop.example/a", Mode::Cors, redirect::Mode::Follow);
        let got = response(
            "https://shop.example/a",
            &[
                ("Content-Type", "text/plain"),
                ("Set-Cookie", "id=1"),
                ("set-cookie2", "id=2"),
                ("X-Mine", "yes"),
            ],
        );
        let filtered = filter(&asked, &got, "OK", true);
        let readable = readable(&filtered);
        assert_eq!(filtered.fetched.number, 2);
        assert_eq!(readable.kind, Kind::Basic);
        assert_eq!(readable.status, 200);
        assert_eq!(readable.status_text, "OK");
        assert_eq!(readable.url.as_deref(), Some("https://shop.example/a"));
        assert!(readable.redirected);
        assert_eq!(
            readable.headers,
            vec![
                ("Content-Type".to_owned(), "text/plain".to_owned()),
                ("X-Mine".to_owned(), "yes".to_owned()),
            ]
        );
        assert_eq!(readable.body, b"the secret");
    }

    #[test]
    fn an_origin_that_agreed_is_read_with_only_what_it_exposed_and_never_set_cookie() {
        let asked = fetch("https://api.example/a", Mode::Cors, redirect::Mode::Follow);
        let got = response(
            "https://api.example/a",
            &[
                ("Access-Control-Allow-Origin", "https://shop.example"),
                ("Access-Control-Expose-Headers", "X-Count, Set-Cookie"),
                ("Content-Type", "application/json"),
                ("X-Count", "3"),
                ("X-Hidden", "no"),
                ("Set-Cookie", "id=1"),
            ],
        );
        let filtered = filter(&asked, &got, "", false);
        let readable = readable(&filtered);
        assert_eq!(readable.kind, Kind::Cors);
        let names: Vec<&str> = readable
            .headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(
            names.contains(&"Content-Type") && names.contains(&"X-Count"),
            "{names:?}"
        );
        assert!(!names.contains(&"X-Hidden"), "{names:?}");
        assert!(
            !names.contains(&"Set-Cookie"),
            "even when exposed by name: {names:?}"
        );
        assert_eq!(readable.body, b"the secret");
    }

    #[test]
    fn a_cors_failure_crosses_with_no_reason_and_the_reason_stays_here() {
        let asked = fetch("https://api.example/a", Mode::Cors, redirect::Mode::Follow);
        let filtered = filter(&asked, &response("https://api.example/a", &[]), "OK", false);
        assert_eq!(filtered.fetched, Fetched::failed(2));
        assert!(
            filtered
                .why
                .as_deref()
                .is_some_and(|why| why.contains("Access-Control-Allow-Origin")),
            "{:?}",
            filtered.why
        );
    }

    #[test]
    fn a_no_cors_answer_from_elsewhere_crosses_with_no_bytes_at_all() {
        let asked = fetch(
            "https://api.example/a",
            Mode::NoCors,
            redirect::Mode::Follow,
        );
        let got = response("https://api.example/a", &[("Content-Type", "text/plain")]);
        let filtered = filter(&asked, &got, "OK", true);
        assert_eq!(*readable(&filtered), nothing(Kind::Opaque));
        assert_eq!(filtered.why, None);
        let bytes =
            crate::wire::write_to_renderer(&crate::ToRenderer::Fetched(Box::new(filtered.fetched)));
        assert!(
            !bytes
                .windows(b"the secret".len())
                .any(|run| run == b"the secret"),
            "an opaque body crossed the boundary"
        );
    }

    #[test]
    fn a_redirect_is_stopped_at_failed_or_followed_as_the_page_asked() {
        let mut redirect_answer = response("https://shop.example/a", &[("Location", "/b")]);
        redirect_answer.status = Status(302);
        let manual = fetch("https://shop.example/a", Mode::Cors, redirect::Mode::Manual);
        assert_eq!(
            *readable(&filter(&manual, &redirect_answer, "Found", false)),
            nothing(Kind::OpaqueRedirect)
        );
        let error = fetch("https://shop.example/a", Mode::Cors, redirect::Mode::Error);
        let filtered = filter(&error, &redirect_answer, "Found", false);
        assert_eq!(filtered.fetched, Fetched::failed(2));
        assert!(filtered.why.is_some());
        let follow = fetch("https://shop.example/a", Mode::Cors, redirect::Mode::Follow);
        assert_eq!(
            readable(&filter(&follow, &redirect_answer, "Found", false)).status,
            302
        );
    }

    #[test]
    fn a_same_origin_fetch_redirected_elsewhere_is_a_failure() {
        let asked = fetch(
            "https://shop.example/a",
            Mode::SameOrigin,
            redirect::Mode::Follow,
        );
        let filtered = filter(&asked, &response("https://api.example/b", &[]), "OK", true);
        assert_eq!(filtered.fetched, Fetched::failed(2));
    }

    #[test]
    fn a_body_too_large_for_one_message_is_a_failure_rather_than_a_truncation() {
        let asked = fetch("https://shop.example/a", Mode::Cors, redirect::Mode::Follow);
        let mut got = response("https://shop.example/a", &[]);
        got.body = vec![0; LARGEST_MESSAGE];
        let filtered = filter(&asked, &got, "OK", false);
        assert_eq!(filtered.fetched, Fetched::failed(2));
        assert!(
            filtered
                .why
                .as_deref()
                .is_some_and(|why| why.contains("one message"))
        );
    }
}
