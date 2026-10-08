/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a page may fetch what it asked for: the browser process deciding
//! one of a renderer's [`FetchAsk`]s (ADR 0032 §§ 2 and 3, queue item 334).
//!
//! The half that cannot be lied to, beside [`crate::navigate`] and for the
//! same reason. Everything in the ask is a claim; **who is asking** is this
//! process's own copy of the document — its URL and the policy its response's
//! headers stated — and **who caused it** is assigned by the caller from which
//! message the ask was answering. The renderer names neither.
//!
//! # In this order, each refusal named
//!
//! 1. The URL is no longer than [`LONGEST_URL`] and parses.
//! 2. Its scheme is `http` or `https`. Fetch answers `about:`, `data:` and
//!    `blob:` without a network, so a page needing one is answered in the
//!    renderer, and an ask naming one here is a network error.
//! 3. The document's own header policy allows it under `connect-src`.
//! 4. It is not insecure content on a secure page ([`alo_net::mixed`]).
//! 5. A `same-origin` ask to another origin is refused before anything is
//!    sent.
//!
//! Then the request is built: `Purpose::Fetch`, the document's origin as the
//! asker, an `Origin` header where Fetch sends one, and the `Referer` worked
//! out here from this process's copy of the document's URL. What is left —
//! the cookies from the jar under the document's top-level site, and whether
//! to ask first under the preflight cache's same partition — is answered by
//! the decided [`Fetch`] for whoever makes the request, at the moment it is
//! made.
//!
//! # And before all of it, the boundary
//!
//! An ask the page's bindings could never have produced — a forbidden header,
//! a forbidden method, `navigate` as a mode, a body on a `GET`, a `no-cors`
//! request a form could not have sent — is **not corrected**. It is refused as
//! a renderer that broke the boundary ([`Broke`]), because only a renderer
//! that was taken over sends one (ADR 0032 § 2).

use alo_net::activity::{Activity, Happened};
use alo_net::cause::Cause;
use alo_net::cookie::Partition;
use alo_net::cors::{self, Credentials, Mode};
use alo_net::csp::Policies;
use alo_net::forbidden;
use alo_net::jar::{How, Jar};
use alo_net::mixed::{self, Verdict};
use alo_net::preflight::Preflights;
use alo_net::redirect;
use alo_net::referrer;
use alo_net::request::{Purpose, Request};
use alo_url::{Origin, Url};
use core::fmt;
use std::time::SystemTime;

use crate::fetch::{FetchAsk, Fetched};
use crate::navigate::{LONGEST_SAID, LONGEST_URL};

/// The document an ask came from, as this process knows it.
#[derive(Debug, Clone, Copy)]
pub struct Asker<'a> {
    /// Where it is: the URL this process loaded it from.
    pub url: &'a Url,
    /// The policies its response's headers stated, enforced ones only.
    pub policies: &'a Policies,
}

/// How an ask broke the boundary: something the page's bindings never send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Broke {
    /// `navigate` as a mode, which is a person going somewhere.
    Navigate,
    /// A method that is not one.
    NotAMethod {
        /// What it said, at most [`LONGEST_SAID`] characters.
        method: String,
    },
    /// `CONNECT`, `TRACE` or `TRACK`.
    ForbiddenMethod {
        /// Which.
        method: String,
    },
    /// A body on a `GET` or a `HEAD`.
    BodyOnARead {
        /// Which.
        method: String,
    },
    /// A header name or value that is not one.
    NotAHeader {
        /// Its name, at most [`LONGEST_SAID`] characters.
        name: String,
    },
    /// A header only the browser sets.
    ForbiddenHeader {
        /// Which.
        name: String,
    },
    /// A `no-cors` method a form could not have used.
    NoCorsMethod {
        /// Which.
        method: String,
    },
    /// `no-cors` headers a form could not have sent.
    NoCorsHeaders {
        /// Which, lowercased.
        names: Vec<String>,
    },
}

impl fmt::Display for Broke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Broke::Navigate => f.write_str("it asked to fetch in navigate mode"),
            Broke::NotAMethod { method } => write!(f, "{method:?} is not a method"),
            Broke::ForbiddenMethod { method } => {
                write!(f, "it asked for {method}, which no page may use")
            }
            Broke::BodyOnARead { method } => write!(f, "it sent a body with a {method}"),
            Broke::NotAHeader { name } => write!(f, "its header {name:?} is not a header"),
            Broke::ForbiddenHeader { name } => {
                write!(f, "it set {name}, which only the browser sets")
            }
            Broke::NoCorsMethod { method } => write!(
                f,
                "it asked for {method} in no-cors mode, which a form could not have sent"
            ),
            Broke::NoCorsHeaders { names } => write!(
                f,
                "it set {} in no-cors mode, which a form could not have sent",
                names.join(", ")
            ),
        }
    }
}

/// Which rule refused an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// It is not a URL.
    Unparsed {
        /// Why, in words.
        why: String,
    },
    /// It is longer than [`LONGEST_URL`].
    TooLong {
        /// How long it was, in bytes.
        bytes: usize,
    },
    /// A scheme a page's fetch does not reach over the network.
    Scheme {
        /// Which.
        scheme: String,
    },
    /// The document's own policy refused it.
    Policy {
        /// The policy's refusal, in its own words.
        said: String,
    },
    /// Insecure content asked for by a secure page.
    Mixed,
    /// A `same-origin` ask to another origin.
    NotTheSameOrigin {
        /// The document's origin.
        asker: String,
        /// Where it asked to go.
        target: String,
    },
    /// More asks in one answer than [`crate::fetch_owed::MOST_ASKED_AT_ONCE`].
    TooManyAtOnce,
    /// More fetches waiting for one document than
    /// [`crate::fetch_owed::MOST_IN_FLIGHT`].
    TooManyWaiting,
    /// Something the page's bindings never send.
    Broke(Broke),
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::Unparsed { why } => write!(f, "it is not a URL: {why}"),
            Rule::TooLong { bytes } => write!(
                f,
                "it is {bytes} bytes long, and a page may ask for at most {LONGEST_URL}"
            ),
            Rule::Scheme { scheme } => write!(
                f,
                "a page's fetch of a {scheme}: URL is not made over the network, and this \
                 browser makes it nowhere else yet"
            ),
            Rule::Policy { said } => write!(f, "{said}"),
            Rule::Mixed => f.write_str(
                "a page reached securely may not fetch over an insecure connection, which \
                 anybody in between could read and change",
            ),
            Rule::NotTheSameOrigin { asker, target } => write!(
                f,
                "it asked for the same origin only, and {target} is not {asker}"
            ),
            Rule::TooManyAtOnce => write!(
                f,
                "the page asked for more than {} fetches at once",
                crate::fetch_owed::MOST_ASKED_AT_ONCE
            ),
            Rule::TooManyWaiting => write!(
                f,
                "the page already has {} fetches waiting, the most one page may",
                crate::fetch_owed::MOST_IN_FLIGHT
            ),
            Rule::Broke(broke) => write!(f, "its renderer broke the boundary: {broke}"),
        }
    }
}

/// An ask the browser process refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The ask's number.
    pub number: u64,
    /// What was asked for, at most [`LONGEST_SAID`] characters of it.
    pub asked: String,
    /// It, parsed, when it parsed.
    pub url: Option<Url>,
    /// Which rule refused it.
    pub rule: Rule,
    /// Who caused the ask.
    pub cause: Cause,
}

impl Refusal {
    /// Whether the renderer broke the boundary, which is a renderer to stop
    /// believing rather than a page to tell.
    pub fn broke_the_boundary(&self) -> bool {
        matches!(self.rule, Rule::Broke(_))
    }

    /// What the renderer is sent: a network error, and no reason.
    pub fn answer(&self) -> Fetched {
        Fetched::failed(self.number)
    }

    /// Write the refusal into the record (ADR 0012 § 5), as a request a rule
    /// of ours refused, with its cause. Whether it was written — not for a
    /// URL that did not parse, which names nothing a line could hold.
    pub fn record(&self, activity: &mut Activity, at: SystemTime) -> bool {
        let Some(url) = &self.url else {
            return false;
        };
        let request = Request::get(url.clone(), self.cause.clone()).for_purpose(Purpose::Fetch);
        activity.happened(
            &request,
            at,
            Happened::Refused {
                rule: self.rule.to_string(),
            },
        );
        true
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the page's fetch of {:?} was refused: {}",
            self.asked, self.rule
        )
    }
}

/// A fetch the browser process decided a page may make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetch {
    /// The ask's number, to answer it by.
    pub number: u64,
    /// What to send: `Purpose::Fetch`, the document's origin as the asker,
    /// the cause assigned, the page's headers with `Origin` and `Referer`
    /// added here, and the body.
    pub request: Request,
    /// Whether the page means to read the answer.
    pub mode: Mode,
    /// Whether it may carry who the person is.
    pub credentials: Credentials,
    /// What a redirect should do.
    pub redirect: redirect::Mode,
    /// The document's top-level site, which partitions the jar and the
    /// preflight cache (ADR 0007).
    pub partition: Partition,
}

impl Fetch {
    /// Whether it goes to another origin than the document's.
    pub fn is_cross_origin(&self) -> bool {
        !self
            .request
            .initiator
            .as_ref()
            .is_some_and(|asker| same_origin(asker, &self.request.url))
    }

    /// Whether the person's cookies go with it: always for `include`, to the
    /// document's own origin for `same-origin`, never for `omit`. The same
    /// answer says whether a `Set-Cookie` in the response is kept.
    pub fn sends_credentials(&self) -> bool {
        match self.credentials {
            Credentials::Include => true,
            Credentials::SameOrigin => !self.is_cross_origin(),
            Credentials::Omit => false,
        }
    }

    /// The `Cookie` header to send, from the jar under the document's
    /// top-level site, when its credentials mode allows one.
    pub fn cookies(&self, jar: &Jar, now: SystemTime) -> Option<String> {
        if !self.sends_credentials() {
            return None;
        }
        jar.header_for(&self.request.url, &self.partition, How::Embedded, now)
    }

    /// The `OPTIONS` to send first, when a `cors` request to another origin
    /// is one a form could not have sent and the preflight cache, under the
    /// document's top-level site, does not already cover it.
    pub fn asking_first(&self, preflights: &mut Preflights, now: SystemTime) -> Option<Request> {
        if self.mode != Mode::Cors || !self.is_cross_origin() {
            return None;
        }
        preflights
            .must_ask(&self.request, self.credentials, &self.partition, now)
            .then(|| cors::asking_first(&self.request))
    }
}

/// What the browser process decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// Make it.
    Make(Box<Fetch>),
    /// Do not, and this is why.
    Refused(Box<Refusal>),
}

impl Decided {
    /// The number of the ask decided.
    pub fn number(&self) -> u64 {
        match self {
            Decided::Make(fetch) => fetch.number,
            Decided::Refused(refusal) => refusal.number,
        }
    }
}

/// Whether `url` is of `asker`'s origin. An opaque origin is the same as
/// nothing that can be written down again.
fn same_origin(asker: &Origin, url: &Url) -> bool {
    !asker.is_opaque() && *asker == Origin::of(url)
}

/// What the page's bindings would never have sent, if the ask has any of it.
fn broken(ask: &FetchAsk, method: &str) -> Option<Broke> {
    let said = |text: &str| text.chars().take(LONGEST_SAID).collect::<String>();
    if ask.mode == Mode::Navigate {
        return Some(Broke::Navigate);
    }
    if !forbidden::is_a_method(method) {
        return Some(Broke::NotAMethod {
            method: said(method),
        });
    }
    if forbidden::is_forbidden_method(method) {
        return Some(Broke::ForbiddenMethod {
            method: said(method),
        });
    }
    if !ask.body.is_empty() && matches!(method, "GET" | "HEAD") {
        return Some(Broke::BodyOnARead {
            method: method.to_owned(),
        });
    }
    for (name, value) in &ask.headers {
        if !forbidden::is_a_header_name(name) || !forbidden::is_a_header_value(value) {
            return Some(Broke::NotAHeader { name: said(name) });
        }
        if forbidden::is_forbidden_request_header(name, value) {
            return Some(Broke::ForbiddenHeader { name: said(name) });
        }
    }
    None
}

/// Decide `ask`, from the document `asker` as this process knows it, under
/// `cause`, which the caller assigned from the message the ask answered.
pub fn decide(ask: &FetchAsk, asker: &Asker<'_>, cause: &Cause) -> Decided {
    let refuse = |url: Option<Url>, rule: Rule| {
        Decided::Refused(Box::new(Refusal {
            number: ask.number,
            asked: ask.url.chars().take(LONGEST_SAID).collect(),
            url,
            rule,
            cause: cause.clone(),
        }))
    };
    if ask.url.len() > LONGEST_URL {
        return refuse(
            None,
            Rule::TooLong {
                bytes: ask.url.len(),
            },
        );
    }
    let parsed = alo_url::parse(&ask.url);
    let method = forbidden::normalised_method(&ask.method);
    if let Some(broke) = broken(ask, &method) {
        return refuse(parsed.ok(), Rule::Broke(broke));
    }
    let url = match parsed {
        Ok(url) => url,
        Err(why) => return refuse(None, Rule::Unparsed { why: why.why }),
    };
    if !matches!(url.scheme.as_str(), "http" | "https") {
        let scheme = url.scheme.clone();
        return refuse(Some(url), Rule::Scheme { scheme });
    }

    let origin = Origin::of(asker.url);
    let mut request = Request::sending(url.clone(), &method, ask.body.clone(), cause.clone())
        .for_purpose(Purpose::Fetch)
        .asked_by(origin.clone());
    for (name, value) in &ask.headers {
        request.headers.add(name.clone(), value.clone());
    }
    if ask.mode == Mode::NoCors {
        if !matches!(method.as_str(), "GET" | "HEAD" | "POST") {
            return refuse(Some(url), Rule::Broke(Broke::NoCorsMethod { method }));
        }
        let names = cors::names_a_form_could_not_have_sent(&request);
        if !names.is_empty() {
            return refuse(Some(url), Rule::Broke(Broke::NoCorsHeaders { names }));
        }
    }

    if let Err(refusal) = asker.policies.allows(&request, None) {
        return refuse(
            Some(url),
            Rule::Policy {
                said: refusal.to_string(),
            },
        );
    }
    if matches!(mixed::what_to_do(&request), Verdict::Refused { .. }) {
        return refuse(Some(url), Rule::Mixed);
    }
    let cross = !same_origin(&origin, &url);
    if ask.mode == Mode::SameOrigin && cross {
        return refuse(
            Some(url.clone()),
            Rule::NotTheSameOrigin {
                asker: origin.to_string(),
                target: Origin::of(&url).to_string(),
            },
        );
    }

    // Fetch sends `Origin` on a CORS request and on anything that is not a
    // read; it is the asker this process knows, never one the page named.
    if (ask.mode == Mode::Cors && cross) || !matches!(method.as_str(), "GET" | "HEAD") {
        request.headers.add("Origin", origin.to_string());
    }
    // The page's policy, or the engine's default,
    // `strict-origin-when-cross-origin`, from this process's copy of where
    // the document is.
    if let Some(referrer) = referrer::for_request(ask.referrer.unwrap_or_default(), asker.url, &url)
    {
        request.headers.add("Referer", referrer);
    }
    Decided::Make(Box::new(Fetch {
        number: ask.number,
        request,
        mode: ask.mode,
        credentials: ask.credentials,
        redirect: ask.redirect,
        partition: Partition::of(asker.url),
    }))
}

#[cfg(test)]
mod tests {
    use alo_net::cause::{DocumentId, Identities};
    use alo_net::cookie::Cookie;
    use alo_net::headers::Headers;

    use super::*;

    const PAGE: &str = "https://shop.example/a/things?q=1";

    fn document() -> DocumentId {
        Identities::default().a_document()
    }

    fn cause() -> Cause {
        Cause::Document {
            document: document(),
        }
    }

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    fn ask(to: &str) -> FetchAsk {
        FetchAsk {
            number: 7,
            url: to.to_owned(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
            mode: Mode::Cors,
            credentials: Credentials::SameOrigin,
            redirect: redirect::Mode::Follow,
            referrer: None,
        }
    }

    fn decided_with(ask: &FetchAsk, at: &str, policies: &Policies) -> Decided {
        let at = url(at);
        decide(ask, &Asker { url: &at, policies }, &cause())
    }

    fn decided(ask: &FetchAsk) -> Decided {
        decided_with(ask, PAGE, &Policies::none())
    }

    fn made(ask: &FetchAsk) -> Fetch {
        match decided(ask) {
            Decided::Make(fetch) => *fetch,
            Decided::Refused(refusal) => panic!("{refusal}"),
        }
    }

    fn rule(decided: &Decided) -> Option<&Rule> {
        match decided {
            Decided::Refused(refusal) => Some(&refusal.rule),
            Decided::Make(_) => None,
        }
    }

    #[test]
    fn a_same_origin_read_is_made_as_the_documents_with_its_cause() {
        let fetch = made(&ask("https://shop.example/download/app.dmg"));
        assert_eq!(fetch.number, 7);
        assert_eq!(fetch.request.purpose, Purpose::Fetch);
        assert_eq!(fetch.request.method, "GET");
        assert_eq!(fetch.request.cause, cause());
        assert_eq!(
            fetch.request.initiator.as_ref().map(ToString::to_string),
            Some("https://shop.example".to_owned())
        );
        assert!(!fetch.is_cross_origin());
        assert!(fetch.sends_credentials());
        assert_eq!(
            fetch.request.headers.get("Origin"),
            None,
            "a same-origin GET"
        );
        assert_eq!(
            fetch.request.headers.get("Referer"),
            Some("https://shop.example/a/things?q=1")
        );
        assert_eq!(fetch.partition, Partition::of(&url(PAGE)));
    }

    #[test]
    fn a_cross_origin_cors_ask_says_its_origin_and_sends_no_cookies_unless_included() {
        let mut asked = ask("https://api.example/v1/things");
        asked.method = "post".to_owned();
        asked.body = b"{}".to_vec();
        let fetch = made(&asked);
        assert_eq!(fetch.request.method, "POST", "normalised here as well");
        assert_eq!(
            fetch.request.headers.get("Origin"),
            Some("https://shop.example")
        );
        assert_eq!(
            fetch.request.headers.get("Referer"),
            Some("https://shop.example/"),
            "strict-origin-when-cross-origin"
        );
        assert!(fetch.is_cross_origin());
        assert!(!fetch.sends_credentials());

        let mut jar = Jar::new();
        let within = Partition::of(&url(PAGE));
        let cookie = Cookie::parse(
            "id=1; SameSite=None; Secure",
            &url("https://api.example/"),
            &within,
        )
        .unwrap();
        jar.keep(cookie, SystemTime::UNIX_EPOCH);
        assert_eq!(fetch.cookies(&jar, SystemTime::UNIX_EPOCH), None);
        asked.credentials = Credentials::Include;
        let included = made(&asked);
        assert_eq!(
            included.cookies(&jar, SystemTime::UNIX_EPOCH).as_deref(),
            Some("id=1"),
            "the jar under the document's top-level site"
        );
        asked.credentials = Credentials::Omit;
        assert_eq!(made(&asked).cookies(&jar, SystemTime::UNIX_EPOCH), None);
    }

    #[test]
    fn a_request_a_form_could_not_have_sent_asks_first_and_the_cache_spares_the_second() {
        let mut asked = ask("https://api.example/v1/things");
        asked.method = "PUT".to_owned();
        asked.headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
        let fetch = made(&asked);
        let mut preflights = Preflights::new();
        let now = SystemTime::UNIX_EPOCH;
        let Some(first) = fetch.asking_first(&mut preflights, now) else {
            panic!("a cross-origin PUT was not asked about first");
        };
        assert_eq!(first.method, "OPTIONS");
        assert_eq!(
            first.headers.get("Access-Control-Request-Method"),
            Some("PUT")
        );
        let mut answer =
            alo_net::response::Response::ok(url("https://api.example/v1/things"), Vec::new());
        answer.headers = Headers::new();
        answer
            .headers
            .add("Access-Control-Allow-Origin", "https://shop.example");
        answer.headers.add("Access-Control-Allow-Methods", "PUT");
        answer
            .headers
            .add("Access-Control-Allow-Headers", "content-type");
        answer.headers.add("Access-Control-Max-Age", "60");
        assert!(
            preflights
                .allowed(
                    &fetch.request,
                    fetch.credentials,
                    &fetch.partition,
                    &answer,
                    now
                )
                .is_ok()
        );
        assert_eq!(fetch.asking_first(&mut preflights, now), None, "remembered");

        let same = made(&ask("https://shop.example/x"));
        assert_eq!(same.asking_first(&mut Preflights::new(), now), None);
        let mut simple = ask("https://api.example/v1/things");
        simple.mode = Mode::NoCors;
        assert_eq!(
            made(&simple).asking_first(&mut Preflights::new(), now),
            None
        );
    }

    #[test]
    fn connect_src_none_refuses_by_the_policys_own_words() {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", "connect-src 'none'");
        let policies = Policies::stated_by(&headers);
        let decided = decided_with(&ask("https://shop.example/x"), PAGE, &policies);
        let Some(Rule::Policy { said }) = rule(&decided) else {
            panic!("connect-src 'none' allowed a fetch: {decided:?}");
        };
        assert!(said.contains("connect-src"), "{said}");
        assert!(matches!(
            decided_with(&ask("https://shop.example/x"), PAGE, &Policies::none()),
            Decided::Make(_)
        ));
    }

    #[test]
    fn a_secure_page_asking_for_http_is_refused_as_mixed_content() {
        assert_eq!(
            rule(&decided(&ask("http://shop.example/x"))),
            Some(&Rule::Mixed)
        );
        assert!(matches!(
            decided_with(
                &ask("http://other.example/x"),
                "http://shop.example/",
                &Policies::none()
            ),
            Decided::Make(_)
        ));
    }

    #[test]
    fn a_same_origin_ask_to_another_origin_is_refused_before_anything_is_sent() {
        let mut asked = ask("https://api.example/x");
        asked.mode = Mode::SameOrigin;
        assert_eq!(
            rule(&decided(&asked)),
            Some(&Rule::NotTheSameOrigin {
                asker: "https://shop.example".to_owned(),
                target: "https://api.example".to_owned(),
            })
        );
        asked.url = "https://shop.example/x".to_owned();
        assert!(matches!(decided(&asked), Decided::Make(_)));
    }

    #[test]
    fn only_the_web_is_fetched_over_the_network() {
        for (to, scheme) in [
            ("data:text/plain,hi", "data"),
            ("blob:https://shop.example/0f6c", "blob"),
            ("about:blank", "about"),
            ("file:///etc/passwd", "file"),
            ("ftp://shop.example/x", "ftp"),
        ] {
            assert_eq!(
                rule(&decided(&ask(to))),
                Some(&Rule::Scheme {
                    scheme: scheme.to_owned()
                }),
                "{to}"
            );
        }
    }

    #[test]
    fn what_the_bindings_never_send_is_a_broken_boundary_and_not_corrected() {
        let broke = |change: &dyn Fn(&mut FetchAsk)| {
            let mut asked = ask("https://shop.example/x");
            change(&mut asked);
            match decided(&asked) {
                Decided::Refused(refusal) if refusal.broke_the_boundary() => refusal.rule,
                other => panic!("not refused as a broken boundary: {other:?}"),
            }
        };
        assert_eq!(
            broke(&|asked| asked.headers = vec![("Cookie".to_owned(), "id=forged".to_owned())]),
            Rule::Broke(Broke::ForbiddenHeader {
                name: "Cookie".to_owned()
            })
        );
        assert_eq!(
            broke(&|asked| asked.headers =
                vec![("Origin".to_owned(), "https://bank.example".to_owned())]),
            Rule::Broke(Broke::ForbiddenHeader {
                name: "Origin".to_owned()
            })
        );
        assert_eq!(
            broke(&|asked| asked.headers = vec![("X-A".to_owned(), "a\r\nHost: evil".to_owned())]),
            Rule::Broke(Broke::NotAHeader {
                name: "X-A".to_owned()
            })
        );
        assert_eq!(
            broke(&|asked| asked.mode = Mode::Navigate),
            Rule::Broke(Broke::Navigate)
        );
        assert_eq!(
            broke(&|asked| asked.body = b"x".to_vec()),
            Rule::Broke(Broke::BodyOnARead {
                method: "GET".to_owned()
            })
        );
        assert_eq!(
            broke(&|asked| asked.method = "trace".to_owned()),
            Rule::Broke(Broke::ForbiddenMethod {
                method: "trace".to_owned()
            })
        );
        assert!(matches!(
            broke(&|asked| asked.method = "GET /x HTTP/1.1".to_owned()),
            Rule::Broke(Broke::NotAMethod { .. })
        ));
        assert!(matches!(
            broke(&|asked| {
                asked.mode = Mode::NoCors;
                asked.method = "PUT".to_owned();
            }),
            Rule::Broke(Broke::NoCorsMethod { .. })
        ));
        assert_eq!(
            broke(&|asked| {
                asked.mode = Mode::NoCors;
                asked.headers = vec![("X-Token".to_owned(), "1".to_owned())];
            }),
            Rule::Broke(Broke::NoCorsHeaders {
                names: vec!["x-token".to_owned()]
            })
        );
    }

    #[test]
    fn hostile_urls_are_refused_and_a_refusal_is_recorded_only_when_it_parsed() {
        for to in ["", "not a url", "https://[::1", "\u{0}"] {
            assert!(
                matches!(rule(&decided(&ask(to))), Some(Rule::Unparsed { .. })),
                "{to:?}"
            );
        }
        let long = format!("https://shop.example/{}", "a".repeat(LONGEST_URL));
        let Decided::Refused(refusal) = decided(&ask(&long)) else {
            panic!("a URL over the limit was fetched");
        };
        assert_eq!(refusal.rule, Rule::TooLong { bytes: long.len() });
        assert_eq!(refusal.asked.chars().count(), LONGEST_SAID);

        let mut activity = Activity::new();
        assert!(!refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
        let Decided::Refused(scheme) = decided(&ask("data:text/plain,hi")) else {
            panic!("a data: URL was fetched over the network");
        };
        assert!(scheme.record(&mut activity, SystemTime::UNIX_EPOCH));
        let line = activity.latest().unwrap();
        assert_eq!(line.cause(), &cause());
        assert!(matches!(line.happened(), Happened::Refused { rule } if rule.contains("data:")));
        assert_eq!(scheme.answer(), Fetched::failed(7));
    }

    #[test]
    fn the_referrer_follows_the_asks_policy_from_this_processs_copy() {
        let mut asked = ask("https://shop.example/x");
        asked.referrer = Some(referrer::Policy::NoReferrer);
        assert_eq!(made(&asked).request.headers.get("Referer"), None);
        asked.referrer = Some(referrer::Policy::Origin);
        assert_eq!(
            made(&asked).request.headers.get("Referer"),
            Some("https://shop.example/")
        );
    }
}
