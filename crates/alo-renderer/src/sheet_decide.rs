/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a page may have the style sheet it links: the browser process
//! deciding one of a renderer's [`SheetAsk`]s (ADR 0035 § 2, queue item 348).
//!
//! Beside [`crate::fetch_decide`] and for its reason: everything in the ask
//! is a claim; **who is asking** is this process's own copy of the document —
//! its URL and the policy its response's headers stated — and **who caused
//! it** is assigned by the caller from which message the ask was answering.
//!
//! # In this order, each refusal named
//!
//! 1. The URL is no longer than [`LONGEST_URL`] and parses.
//! 2. Its mode is one a `<link>` asks in: `no-cors`, or `cors` for one with
//!    `crossorigin`. Anything else is a renderer that broke the boundary.
//! 3. Its scheme is `http` or `https` — or `file`, **only from a `file:`
//!    document** (ADR 0020 § 3's rule for navigation), since `alo` opens local
//!    files and a local page's sheet beside it is the ordinary case, and only
//!    without `crossorigin`, since a file has no origin to agree to be read.
//! 4. The document's own header policy allows it under `style-src`, asked with
//!    the nonce the `<link>` presents.
//! 5. It is not insecure content on a secure page: a style sheet is
//!    blockable, so it is refused and never upgraded ([`alo_net::mixed`]).
//!
//! Then the request is built: `Purpose::Style`, the document's origin as the
//! asker, and the `Referer` from this process's copy of the document's URL
//! under the `<link>`'s policy. CORS for a `cors` ask, the cookies under the
//! document's top-level site when the credentials mode allows them, and each
//! of these again on every hop of a redirect, are [`crate::fetch_make`]'s at
//! the moment the request is made — the same hops as a fetch's
//! ([`crate::sheet_make`]).

use alo_bindings::fetching::Keepalive;
use alo_net::cause::Cause;
use alo_net::cookie::Partition;
use alo_net::cors::Mode;
use alo_net::mixed::{self, Verdict};
use alo_net::pool::Pool;
use alo_net::redirect;
use alo_net::referrer;
use alo_net::request::{Purpose, Request};
use alo_url::{Origin, Url};
use core::fmt;

use crate::fetch_decide::{Asker, Fetch, same_origin};
use crate::navigate::{LONGEST_SAID, LONGEST_URL};
use crate::sheet::{MOST_SHEETS, SheetAnswer, SheetAsk};

/// Which rule refused a sheet.
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
    /// A scheme a style sheet is not asked for over.
    Scheme {
        /// Which.
        scheme: String,
    },
    /// A `file:` sheet linked by a page that is not a file.
    FileFromTheWeb,
    /// A `file:` sheet asked for with `crossorigin`.
    FileAcrossOrigins,
    /// The document's own policy refused it.
    Policy {
        /// The policy's refusal, in its own words.
        said: String,
    },
    /// Insecure content asked for by a secure page.
    Mixed,
    /// More than [`MOST_SHEETS`] asked for by one document.
    TooMany,
    /// A mode no `<link>` asks in: only a renderer that was taken over sends
    /// one.
    Broke {
        /// Which.
        mode: Mode,
    },
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::Unparsed { why } => write!(f, "it is not a URL: {why}"),
            Rule::TooLong { bytes } => write!(
                f,
                "it is {bytes} bytes long, and a page may ask for at most {LONGEST_URL}"
            ),
            Rule::Scheme { scheme } => {
                write!(f, "a style sheet is not asked for over a {scheme}: URL")
            }
            Rule::FileFromTheWeb => f.write_str(
                "a file on this machine is a style sheet only for a page that is itself a file",
            ),
            Rule::FileAcrossOrigins => f.write_str(
                "its link asks for it with crossorigin, and a file has no origin to agree to be \
                 read",
            ),
            Rule::Policy { said } => write!(f, "{said}"),
            Rule::Mixed => f.write_str(
                "a page reached securely may not have a style sheet over an insecure connection, \
                 which anybody in between could read and change",
            ),
            Rule::TooMany => write!(
                f,
                "the page has asked for {MOST_SHEETS} style sheets, the most one page may"
            ),
            Rule::Broke { mode } => write!(
                f,
                "its renderer broke the boundary: it asked for a style sheet in {} mode, which no \
                 link does",
                name_of(*mode)
            ),
        }
    }
}

/// A mode's name as Fetch writes it.
const fn name_of(mode: Mode) -> &'static str {
    match mode {
        Mode::Cors => "cors",
        Mode::NoCors => "no-cors",
        Mode::SameOrigin => "same-origin",
        Mode::Navigate => "navigate",
    }
}

/// A sheet the browser process refused.
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
        matches!(self.rule, Rule::Broke { .. })
    }

    /// What the renderer is sent: that it did not arrive, and no reason.
    pub fn answer(&self) -> SheetAnswer {
        SheetAnswer::failed(self.number)
    }

    /// Write the refusal into the session's record (ADR 0012 § 5), as a style
    /// request a rule of ours refused, with its cause. Whether it was written
    /// — not for a URL that did not parse, which names nothing a line could
    /// hold.
    pub fn record(&self, pool: &mut Pool) -> bool {
        let Some(url) = &self.url else {
            return false;
        };
        let request = Request::get(url.clone(), self.cause.clone()).for_purpose(Purpose::Style);
        pool.refused(&request, self.rule.to_string());
        true
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the page's style sheet at {:?} was refused: {}",
            self.asked, self.rule
        )
    }
}

/// What the browser process decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// Make it, as [`crate::sheet_make`] makes a sheet.
    Make(Box<Fetch>),
    /// Do not, and this is why.
    Refused(Box<Refusal>),
}

impl Decided {
    /// The number of the ask decided.
    pub fn number(&self) -> u64 {
        match self {
            Decided::Make(sheet) => sheet.number,
            Decided::Refused(refusal) => refusal.number,
        }
    }
}

/// Decide `ask`, from the document `asker` as this process knows it, under
/// `cause`, which the caller assigned from the message the ask answered.
pub fn decide(ask: &SheetAsk, asker: &Asker<'_>, cause: &Cause) -> Decided {
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
    if !matches!(ask.mode, Mode::NoCors | Mode::Cors) {
        return refuse(parsed.ok(), Rule::Broke { mode: ask.mode });
    }
    let url = match parsed {
        Ok(url) => url,
        Err(why) => return refuse(None, Rule::Unparsed { why: why.why }),
    };
    match url.scheme.as_str() {
        "file" if asker.url.scheme != "file" => return refuse(Some(url), Rule::FileFromTheWeb),
        "file" if ask.mode == Mode::Cors => return refuse(Some(url), Rule::FileAcrossOrigins),
        "http" | "https" | "file" => {}
        other => {
            let scheme = other.to_owned();
            return refuse(Some(url), Rule::Scheme { scheme });
        }
    }

    let origin = Origin::of(asker.url);
    let mut request = Request::get(url.clone(), cause.clone())
        .for_purpose(Purpose::Style)
        .asked_by(origin.clone());
    if let Err(refusal) = asker.policies.allows(&request, ask.nonce.as_deref()) {
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
    // A CORS request says who asks; a `no-cors` `GET` says nothing. Each hop
    // works its own out again ([`crate::fetch_make`]), from this same copy.
    if ask.mode == Mode::Cors && !same_origin(&origin, &url) {
        request.headers.add("Origin", origin.to_string());
    }
    let policy = ask.referrer.unwrap_or_default();
    if let Some(referrer) = referrer::for_request(policy, asker.url, &url) {
        request.headers.add("Referer", referrer);
    }
    Decided::Make(Box::new(Fetch {
        number: ask.number,
        request,
        mode: ask.mode,
        credentials: ask.credentials,
        redirect: redirect::Mode::Follow,
        partition: Partition::of(asker.url),
        document: asker.url.clone(),
        referrer: policy,
        policies: asker.policies.clone(),
        nonce: ask.nonce.clone(),
        keepalive: Keepalive::Not,
    }))
}

#[cfg(test)]
mod tests {
    use alo_net::activity::Happened;
    use alo_net::cause::Identities;
    use alo_net::cors::Credentials;
    use alo_net::csp::Policies;
    use alo_net::headers::Headers;

    use super::*;

    const PAGE: &str = "https://shop.example/a/page";

    fn cause() -> Cause {
        Cause::Document {
            document: Identities::default().a_document(),
        }
    }

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    fn ask(to: &str) -> SheetAsk {
        SheetAsk {
            number: 4,
            url: to.to_owned(),
            mode: Mode::NoCors,
            credentials: Credentials::Include,
            referrer: None,
            nonce: None,
        }
    }

    fn decided_with(ask: &SheetAsk, at: &str, policies: &Policies) -> Decided {
        let at = url(at);
        decide(ask, &Asker { url: &at, policies }, &cause())
    }

    fn decided(ask: &SheetAsk) -> Decided {
        decided_with(ask, PAGE, &Policies::none())
    }

    fn rule(decided: &Decided) -> Option<&Rule> {
        match decided {
            Decided::Refused(refusal) => Some(&refusal.rule),
            Decided::Make(_) => None,
        }
    }

    fn made(ask: &SheetAsk) -> Fetch {
        match decided(ask) {
            Decided::Make(sheet) => *sheet,
            Decided::Refused(refusal) => panic!("{refusal}"),
        }
    }

    #[test]
    fn a_sheet_is_a_style_request_as_the_documents_with_its_cause() {
        let sheet = made(&ask("https://cdn.example/site.css"));
        assert_eq!(sheet.number, 4);
        assert_eq!(sheet.request.purpose, Purpose::Style);
        assert_eq!(sheet.request.method, "GET");
        assert_eq!(sheet.request.cause, cause());
        assert_eq!(
            sheet.request.initiator.as_ref().map(ToString::to_string),
            Some("https://shop.example".to_owned())
        );
        assert_eq!(
            sheet.request.headers.get("Origin"),
            None,
            "no-cors says nothing"
        );
        assert_eq!(
            sheet.request.headers.get("Referer"),
            Some("https://shop.example/"),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(sheet.mode, Mode::NoCors);
        assert_eq!(sheet.credentials, Credentials::Include);
        assert_eq!(sheet.redirect, redirect::Mode::Follow);
        assert_eq!(sheet.partition, Partition::of(&url(PAGE)));

        let mut crossing = ask("https://cdn.example/site.css");
        crossing.mode = Mode::Cors;
        crossing.credentials = Credentials::SameOrigin;
        crossing.referrer = Some(referrer::Policy::NoReferrer);
        let sheet = made(&crossing);
        assert_eq!(
            sheet.request.headers.get("Origin"),
            Some("https://shop.example")
        );
        assert_eq!(sheet.request.headers.get("Referer"), None);
    }

    #[test]
    fn style_src_refuses_by_its_own_words_and_a_nonce_lets_in() {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", "style-src 'self' 'nonce-n0'");
        let policies = Policies::stated_by(&headers);
        let refused = decided_with(&ask("https://cdn.example/x.css"), PAGE, &policies);
        let Some(Rule::Policy { said }) = rule(&refused) else {
            panic!("style-src let in another origin: {refused:?}");
        };
        assert!(said.contains("style-src"), "{said}");
        let mut nonced = ask("https://cdn.example/x.css");
        nonced.nonce = Some("n0".to_owned());
        let Decided::Make(sheet) = decided_with(&nonced, PAGE, &policies) else {
            panic!("a nonce the policy names was refused");
        };
        assert_eq!(
            sheet.nonce.as_deref(),
            Some("n0"),
            "asked again on each hop"
        );
        assert!(matches!(
            decided_with(&ask("https://shop.example/own.css"), PAGE, &policies),
            Decided::Make(_)
        ));
        // A connect-src says nothing about a sheet.
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", "connect-src 'none'");
        assert!(matches!(
            decided_with(
                &ask("https://cdn.example/x.css"),
                PAGE,
                &Policies::stated_by(&headers)
            ),
            Decided::Make(_)
        ));
    }

    #[test]
    fn a_secure_page_has_no_insecure_sheet_and_an_insecure_one_may() {
        assert_eq!(
            rule(&decided(&ask("http://cdn.example/x.css"))),
            Some(&Rule::Mixed)
        );
        assert!(matches!(
            decided_with(
                &ask("http://cdn.example/x.css"),
                "http://shop.example/",
                &Policies::none()
            ),
            Decided::Make(_)
        ));
    }

    #[test]
    fn a_file_sheet_only_for_a_file_page_and_never_across_origins() {
        assert_eq!(
            rule(&decided(&ask("file:///etc/passwd"))),
            Some(&Rule::FileFromTheWeb)
        );
        let page = "file:///Users/someone/site/index.html";
        let Decided::Make(sheet) = decided_with(
            &ask("file:///Users/someone/site/site.css"),
            page,
            &Policies::none(),
        ) else {
            panic!("a file page's own sheet was refused");
        };
        assert_eq!(sheet.request.url.scheme, "file");
        let mut crossing = ask("file:///Users/someone/site/site.css");
        crossing.mode = Mode::Cors;
        assert_eq!(
            rule(&decided_with(&crossing, page, &Policies::none())),
            Some(&Rule::FileAcrossOrigins)
        );
        for (to, scheme) in [
            ("data:text/css,p{}", "data"),
            ("blob:https://shop.example/0f", "blob"),
            ("ftp://cdn.example/x.css", "ftp"),
            ("javascript:alert(1)", "javascript"),
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
    fn a_mode_no_link_asks_in_is_a_broken_boundary() {
        for mode in [Mode::SameOrigin, Mode::Navigate] {
            let mut broke = ask("https://cdn.example/x.css");
            broke.mode = mode;
            let Decided::Refused(refusal) = decided(&broke) else {
                panic!("{mode:?} was made");
            };
            assert!(refusal.broke_the_boundary(), "{mode:?}");
            assert!(refusal.to_string().contains(name_of(mode)), "{refusal}");
        }
    }

    #[test]
    fn hostile_urls_are_refused_and_recorded_only_when_they_parsed() {
        for to in ["", "not a url", "https://[::1", "\u{0}"] {
            assert!(
                matches!(rule(&decided(&ask(to))), Some(Rule::Unparsed { .. })),
                "{to:?}"
            );
        }
        let long = format!("https://cdn.example/{}", "a".repeat(LONGEST_URL));
        let Decided::Refused(refusal) = decided(&ask(&long)) else {
            panic!("a URL over the limit was made");
        };
        assert_eq!(refusal.rule, Rule::TooLong { bytes: long.len() });
        assert_eq!(refusal.asked.chars().count(), LONGEST_SAID);

        let mut pool = Pool::with_trust(alo_net::tls::Trust::of(&[]).unwrap());
        assert!(!refusal.record(&mut pool));
        assert!(pool.activity().is_empty());
        let Decided::Refused(mixed) = decided(&ask("http://cdn.example/x.css")) else {
            panic!("mixed content was made");
        };
        assert!(mixed.record(&mut pool));
        let line = pool.activity().latest().unwrap();
        assert_eq!(line.cause(), &cause());
        assert!(matches!(line.happened(), Happened::Refused { rule } if rule.contains("insecure")));
        assert_eq!(mixed.answer(), SheetAnswer::failed(4));
    }
}
