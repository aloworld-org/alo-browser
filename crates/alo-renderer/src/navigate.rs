/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a page may send its tab: the browser process deciding a renderer's
//! ask (ADR 0020 §§ 3 and 4, queue item 263).
//!
//! The browser process's half, and the half that cannot be lied to. A
//! renderer is the process that parsed a stranger's page (ADR 0005), so
//! everything in its [`Asked`] is a claim: the URL is parsed again here, as
//! bytes a stranger chose, and judged by scheme from **this process's own
//! copy** of where the document is; the cause is assigned from **which
//! message was answered**, never from anything the renderer said.
//!
//! | Scheme | From a page |
//! |---|---|
//! | `http`, `https` | navigated |
//! | `about:blank` | navigated |
//! | `file` | only from a `file:` document |
//! | `data`, `javascript`, `blob`, every other | refused |
//!
//! # A form's submission is a link's decision and three more checks
//!
//! (ADR 0051 § 5, queue item 470.) Its action passes the table above, and
//! then, in this order:
//!
//! 1. **`form-action`**, from this process's own copy of the document's
//!    header policy ([`alo_net::Policies::allows_form_action`]). A `<meta>`
//!    policy is the renderer's to apply before it asks, and it does not yet:
//!    queue item 486.
//! 2. **The entries written** ([`crate::form_body`]): a `GET`'s query
//!    replacing the action's, a `POST`'s urlencoded body. The 2 MiB bound is
//!    applied again to the URL once its query is in it, and the body is
//!    bounded by [`alo_net::body::LARGEST_BODY`].
//! 3. **Mixed content.** A secure document sending a form to an insecure
//!    address is refused and said. Chrome warns the person and lets them
//!    send it anyway; sending anyway is the person's choice and never an
//!    agent's, and it waits for a window that can ask.
//!
//! The cause is assigned as for a link, and which cookies go is the jar's
//! ([`alo_net::jar::Jar::header_for_navigation`]): a `POST` from another
//! site carries no `SameSite=Lax` cookie. A renderer cannot ask for a method
//! or an enctype no form could make, because the wire's lists of them are
//! closed ([`crate::wire`]), and it cannot name a file at all until item 475.
//!
//! A refusal is said ([`Refusal`]'s words) and recorded under ADR 0012 by
//! whoever holds the record ([`Refusal::record`]); the page is told nothing,
//! as in every browser. Loading what was decided — session history, what
//! survives, the request's other headers — is item 85's: until then the
//! decision is handed to whoever drove [`crate::Tabs`].

use alo_net::activity::{Activity, Happened};
use alo_net::body::LARGEST_BODY;
use alo_net::cause::Cause;
use alo_net::csp::Policies;
use alo_net::mixed::{self, Verdict};
use alo_net::referrer::{self, Policy};
use alo_net::request::{Purpose, Request};
use alo_url::{Origin, Url};
use core::fmt;
use std::time::SystemTime;

use alo_dom::submission::{Enctype, Method};

use crate::ask::{Asked, By};
use crate::form_body::{self, Body, Unwritten};

/// The longest URL a page may ask to go to, in bytes: 2 MiB.
///
/// Chromium's limit for the same check (`url::kMaxURLChars`). A real page has
/// never needed more, and without a bound a page decides how much the browser
/// process holds and parses.
pub const LONGEST_URL: usize = 2 * 1024 * 1024;

/// The most characters of a refused URL that are said, so a refusal quoting
/// a page's two megabytes is a line, not a page.
pub const LONGEST_SAID: usize = 200;

/// A navigation the browser process has decided a page may make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigation {
    /// Where to, as this process parsed it — a form sent by `GET` with its
    /// entries as the query.
    pub url: Url,
    /// How: `GET` for a link, and a form's own method.
    pub method: Method,
    /// What to send: a form's body when it is sent by `POST`.
    pub body: Option<Body>,
    /// Who caused it, by ADR 0012 § 4: the agent's action in an `Act`'s
    /// answer, the document otherwise.
    pub cause: Cause,
    /// How the renderer says it arose — kept beside the cause, never instead
    /// of it, so *the agent acted, and the page clicked* both survive.
    pub by: By,
    /// What to send as `Referer`, worked out from this process's copy of the
    /// document's URL under the link's policy, or the default; [`None`] is
    /// send nothing.
    pub referrer: Option<String>,
    /// How many asks this one replaced.
    pub replaced: u32,
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
    /// Its scheme is one a page may not send its tab to.
    Scheme {
        /// Which.
        scheme: String,
    },
    /// A `file:` URL, asked for by a document that is not a file.
    FileFromElsewhere,
    /// A form in an enctype whose writing is not built (queue item 474).
    Enctype {
        /// Which.
        enctype: Enctype,
    },
    /// A form sent to an address HTML gives a form no meaning at.
    FormScheme {
        /// Its scheme.
        scheme: String,
    },
    /// The document's policy does not let its forms go there.
    FormAction {
        /// The policy's refusal, in words.
        said: String,
    },
    /// A form a secure document would send insecurely.
    Insecure,
    /// A form's body larger than any request may send.
    BodyTooLarge {
        /// How large it was, in bytes.
        bytes: usize,
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
            Rule::Scheme { scheme } => match scheme.as_str() {
                "data" => f.write_str(
                    "a page may not send its tab to a data: URL, a document no origin answers for",
                ),
                "javascript" => f.write_str(
                    "a javascript: URL runs script rather than going anywhere, and is not built",
                ),
                "blob" => f.write_str(
                    "a blob: URL's bytes are the renderer's, and a page may not send its tab to \
                     one",
                ),
                "about" => f.write_str("the only about: URL a page may go to is about:blank"),
                other => write!(
                    f,
                    "a page may not send its tab to a {other}: URL, which hands the person to \
                     another program"
                ),
            },
            Rule::FileFromElsewhere => {
                f.write_str("a page that is not a file may not send its tab to a file")
            }
            Rule::Enctype { enctype } => {
                write!(f, "{}, so nothing was sent", Unwritten::Enctype(*enctype))
            }
            Rule::FormScheme { scheme } => write!(
                f,
                "{}, so nothing was sent",
                Unwritten::Scheme(scheme.clone())
            ),
            Rule::FormAction { said } => f.write_str(said),
            Rule::Insecure => f.write_str(
                "a page reached securely may not send a form where anybody on the network can \
                 read it, and sending it anyway is the person's choice, which needs a window \
                 to ask in",
            ),
            Rule::BodyTooLarge { bytes } => write!(
                f,
                "the form's body is {bytes} bytes, and a request may send at most {LARGEST_BODY}"
            ),
        }
    }
}

/// An ask the browser process refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// What was asked for, at most [`LONGEST_SAID`] characters of it.
    pub asked: String,
    /// It, parsed, when it parsed.
    pub url: Option<Url>,
    /// Which rule refused it.
    pub rule: Rule,
    /// How it was to go: `GET` for a link, and a form's own method.
    pub method: Method,
    /// Who caused the ask, as for a [`Navigation`].
    pub cause: Cause,
    /// How the renderer says it arose.
    pub by: By,
}

impl Refusal {
    /// Write the refusal into the record (ADR 0012 § 5): a request a rule of
    /// ours refused is a line naming the rule, with its cause. Whether it
    /// was written — not for a URL that did not parse or was too long to,
    /// which names nothing a line could hold and is only said.
    pub fn record(&self, activity: &mut Activity, at: SystemTime) -> bool {
        let Some(url) = &self.url else {
            return false;
        };
        let mut request =
            Request::get(url.clone(), self.cause.clone()).for_purpose(Purpose::Document);
        self.method.name().clone_into(&mut request.method);
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
            "the page asked, by {}, to go to {:?}, and was refused: {}",
            self.by, self.asked, self.rule
        )
    }
}

/// What the browser process decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// Go.
    Go(Navigation),
    /// Do not, and this is why.
    Refused(Refusal),
}

impl fmt::Display for Decided {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decided::Go(navigation) if navigation.body.is_some() => write!(
                f,
                "the page asked, by {}, to send a form by {} to {}",
                navigation.by,
                navigation.method.name(),
                navigation.url
            ),
            Decided::Go(navigation) => write!(
                f,
                "the page asked, by {}, to go to {}",
                navigation.by, navigation.url
            ),
            Decided::Refused(refusal) => write!(f, "{refusal}"),
        }
    }
}

/// Decide `asked`, a renderer's ask from the document at `from` — this
/// process's own copy of its URL, and of the policies its response's headers
/// stated — under `cause`, which the caller assigned from the message the
/// ask answered.
pub fn decide(asked: &Asked, from: &Url, policies: &Policies, cause: Cause) -> Decided {
    let method = asked
        .submission
        .as_ref()
        .map_or(Method::Get, |submission| submission.method);
    let refuse = |url: Option<Url>, rule: Rule| {
        Decided::Refused(Refusal {
            asked: asked.url.chars().take(LONGEST_SAID).collect(),
            url,
            rule,
            method,
            cause: cause.clone(),
            by: asked.by,
        })
    };
    if asked.url.len() > LONGEST_URL {
        return refuse(
            None,
            Rule::TooLong {
                bytes: asked.url.len(),
            },
        );
    }
    let url = match alo_url::parse(&asked.url) {
        Ok(url) => url,
        Err(why) => return refuse(None, Rule::Unparsed { why: why.why }),
    };
    let allowed = match url.scheme.as_str() {
        "http" | "https" => Ok(()),
        "about" if url.is_about_blank() => Ok(()),
        "file" if from.scheme == "file" => Ok(()),
        "file" => Err(Rule::FileFromElsewhere),
        other => Err(Rule::Scheme {
            scheme: other.to_owned(),
        }),
    };
    if let Err(rule) = allowed {
        return refuse(Some(url), rule);
    }
    let (to, body) = match &asked.submission {
        None => (url, None),
        Some(submission) => {
            let page = Origin::of(from);
            if let Err(refusal) = policies.allows_form_action(&url, &page) {
                return refuse(
                    Some(url),
                    Rule::FormAction {
                        said: refusal.to_string(),
                    },
                );
            }
            let written = match form_body::write(&url, submission) {
                Ok(written) => written,
                Err(unwritten) => {
                    let rule = match unwritten {
                        Unwritten::Enctype(enctype) => Rule::Enctype { enctype },
                        Unwritten::Scheme(scheme) => Rule::FormScheme { scheme },
                        Unwritten::Unparsed(why) => Rule::Unparsed { why },
                    };
                    return refuse(Some(url), rule);
                }
            };
            let bytes = written.url.serialised.len();
            if bytes > LONGEST_URL {
                return refuse(Some(url), Rule::TooLong { bytes });
            }
            let sent = written.body.as_ref().map_or(0, |body| body.bytes.len());
            if !u64::try_from(sent).is_ok_and(|sent| sent <= LARGEST_BODY) {
                return refuse(Some(url), Rule::BodyTooLarge { bytes: sent });
            }
            let request = Request::get(written.url.clone(), cause.clone())
                .for_purpose(Purpose::Document)
                .asked_by(page);
            if matches!(mixed::what_to_do(&request), Verdict::Refused { .. }) {
                return refuse(Some(url), Rule::Insecure);
            }
            (written.url, written.body)
        }
    };
    // The link's policy, or the engine's default,
    // `strict-origin-when-cross-origin`.
    let policy: Policy = asked.referrer.unwrap_or_default();
    let referrer = referrer::for_request(policy, from, &to);
    Decided::Go(Navigation {
        url: to,
        method,
        body,
        cause,
        by: asked.by,
        referrer,
        replaced: asked.replaced,
    })
}

#[cfg(test)]
mod tests {
    use alo_bindings::navigating::Submission;
    use alo_dom::submission::{Entry, Value};
    use alo_net::cause::{DocumentId, Identities};

    use super::*;

    fn document() -> DocumentId {
        Identities::default().a_document()
    }

    fn asking(url: &str) -> Asked {
        Asked {
            url: url.to_owned(),
            by: By::Script,
            referrer: None,
            replaced: 0,
            submission: None,
        }
    }

    fn from(url: &str) -> Url {
        alo_url::parse(url).unwrap()
    }

    fn decided(url: &str, at: &str) -> Decided {
        decide(
            &asking(url),
            &from(at),
            &Policies::none(),
            Cause::Document {
                document: document(),
            },
        )
    }

    fn refused_by(decided: &Decided) -> Option<&Rule> {
        match decided {
            Decided::Refused(refusal) => Some(&refusal.rule),
            Decided::Go(_) => None,
        }
    }

    #[test]
    fn the_web_and_about_blank_are_navigated() {
        for url in [
            "https://example.com/next",
            "http://example.com/",
            "about:blank",
        ] {
            let decided = decided(url, "https://example.com/");
            assert!(matches!(decided, Decided::Go(_)), "{url}: {decided}");
        }
    }

    #[test]
    fn every_other_scheme_is_refused_by_name() {
        for (url, scheme) in [
            ("data:text/html,<form>", "data"),
            ("javascript:alert(1)", "javascript"),
            ("blob:https://example.com/0f6c", "blob"),
            ("mailto:someone@example.com", "mailto"),
            ("about:srcdoc", "about"),
            ("alo://settings", "alo"),
        ] {
            let decided = decided(url, "https://example.com/");
            assert_eq!(
                refused_by(&decided),
                Some(&Rule::Scheme {
                    scheme: scheme.to_owned()
                }),
                "{url}"
            );
            assert!(decided.to_string().contains("refused"), "{decided}");
        }
    }

    #[test]
    fn a_file_is_only_for_a_file() {
        assert_eq!(
            refused_by(&decided("file:///etc/passwd", "https://example.com/")),
            Some(&Rule::FileFromElsewhere),
        );
        assert_eq!(
            refused_by(&decided("file:///etc/passwd", "about:blank")),
            Some(&Rule::FileFromElsewhere),
        );
        assert!(matches!(
            decided("file:///home/me/two.html", "file:///home/me/one.html"),
            Decided::Go(_)
        ));
    }

    #[test]
    fn hostile_urls_are_refused_rather_than_trusted() {
        for url in [
            "",
            "not a url",
            "https://[::1",
            "http://exa mple.com/",
            "\u{0}",
        ] {
            assert!(
                matches!(
                    refused_by(&decided(url, "https://example.com/")),
                    Some(Rule::Unparsed { .. })
                ),
                "{url:?}"
            );
        }
        let long = format!("https://example.com/{}", "a".repeat(LONGEST_URL));
        let decided = decided(&long, "https://example.com/");
        assert_eq!(
            refused_by(&decided),
            Some(&Rule::TooLong { bytes: long.len() })
        );
        let Decided::Refused(refusal) = decided else {
            panic!("a URL over the limit was navigated");
        };
        assert_eq!(refusal.asked.chars().count(), LONGEST_SAID);
        assert!(refusal.url.is_none());
        let exactly = format!(
            "https://example.com/{}",
            "a".repeat(LONGEST_URL - "https://example.com/".len())
        );
        assert_eq!(exactly.len(), LONGEST_URL);
        assert!(matches!(
            decide(
                &asking(&exactly),
                &from("https://example.com/"),
                &Policies::none(),
                Cause::Document {
                    document: document()
                }
            ),
            Decided::Go(_)
        ));
    }

    #[test]
    fn the_cause_is_the_one_assigned_whatever_the_renderer_claims() {
        let mut identities = Identities::default();
        let document = identities.a_document();
        let action = identities.an_action();
        let mut asked = asking("https://example.com/next");
        asked.by = By::Browser;
        let cause = Cause::Agent { action, document };
        let Decided::Go(navigation) = decide(
            &asked,
            &from("https://example.com/"),
            &Policies::none(),
            cause.clone(),
        ) else {
            panic!("an https navigation was refused");
        };
        assert_eq!(navigation.cause, cause);
        assert_eq!(navigation.by, By::Browser);
    }

    #[test]
    fn the_referrer_is_worked_out_here_from_this_processs_copy_of_the_document() {
        let go = |policy: Option<Policy>, to: &str| {
            let mut asked = asking(to);
            asked.referrer = policy;
            match decide(
                &asked,
                &from("https://example.com/private/path?q=1"),
                &Policies::none(),
                Cause::Document {
                    document: document(),
                },
            ) {
                Decided::Go(navigation) => navigation.referrer,
                Decided::Refused(refusal) => panic!("{refusal}"),
            }
        };
        assert_eq!(
            go(None, "https://other.example/").as_deref(),
            Some("https://example.com/"),
            "the default is strict-origin-when-cross-origin",
        );
        assert_eq!(
            go(None, "https://example.com/next").as_deref(),
            Some("https://example.com/private/path?q=1"),
        );
        assert_eq!(go(Some(Policy::NoReferrer), "https://example.com/"), None);
        assert_eq!(go(None, "http://example.com/"), None, "a downgrade");
    }

    #[test]
    fn a_refusal_whose_url_parsed_is_recorded_with_its_rule_and_cause() {
        let cause = Cause::Document {
            document: document(),
        };
        let Decided::Refused(refusal) = decide(
            &asking("data:text/html,hi"),
            &from("https://example.com/"),
            &Policies::none(),
            cause.clone(),
        ) else {
            panic!("a data: URL was navigated");
        };
        let mut activity = Activity::new();
        assert!(refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
        let line = activity.latest().unwrap();
        assert_eq!(line.cause(), &cause);
        assert_eq!(line.url().serialised, "data:text/html,hi");
        assert!(matches!(line.happened(), Happened::Refused { rule } if rule.contains("data:")));

        let Decided::Refused(unparsed) = decide(
            &asking("::"),
            &from("https://example.com/"),
            &Policies::none(),
            cause,
        ) else {
            panic!("nonsense was navigated");
        };
        assert!(!unparsed.record(&mut activity, SystemTime::UNIX_EPOCH));
        assert_eq!(activity.len(), 1);
    }

    fn submitting(to: &str, method: Method, enctype: Enctype, entries: &[(&str, &str)]) -> Asked {
        let mut asked = asking(to);
        asked.by = By::Browser;
        asked.submission = Some(Submission {
            method,
            enctype,
            entries: entries
                .iter()
                .map(|(name, value)| Entry {
                    name: (*name).to_owned(),
                    value: Value::Text((*value).to_owned()),
                })
                .collect(),
        });
        asked
    }

    fn sent(asked: &Asked, at: &str, policies: &Policies) -> Decided {
        decide(
            asked,
            &from(at),
            policies,
            Cause::Document {
                document: document(),
            },
        )
    }

    fn header_policy(text: &str) -> Policies {
        let mut headers = alo_net::headers::Headers::new();
        headers.add("Content-Security-Policy", text);
        Policies::stated_by(&headers)
    }

    #[test]
    fn a_form_sent_by_post_carries_chromes_body() {
        let asked = submitting(
            "https://httpbin.org/post",
            Method::Post,
            Enctype::UrlEncoded,
            &[("custname", "Ada Lovelace"), ("custtel", "")],
        );
        let Decided::Go(navigation) =
            sent(&asked, "https://httpbin.org/forms/post", &Policies::none())
        else {
            panic!("the form was refused");
        };
        assert_eq!(navigation.url.serialised, "https://httpbin.org/post");
        assert_eq!(navigation.method, Method::Post);
        let body = navigation.body.as_ref().expect("a body");
        assert_eq!(body.content_type, "application/x-www-form-urlencoded");
        assert_eq!(body.bytes, b"custname=Ada+Lovelace&custtel=");
        assert_eq!(
            navigation.referrer.as_deref(),
            Some("https://httpbin.org/forms/post")
        );
        assert!(
            Decided::Go(navigation)
                .to_string()
                .contains("to send a form by POST to https://httpbin.org/post")
        );
    }

    #[test]
    fn a_form_sent_by_get_goes_to_its_query_and_sends_nothing() {
        let asked = submitting(
            "https://example.test/get?q=9",
            Method::Get,
            Enctype::Multipart,
            &[("a", "1")],
        );
        let Decided::Go(navigation) = sent(&asked, "https://example.test/", &Policies::none())
        else {
            panic!("the form was refused");
        };
        assert_eq!(navigation.url.serialised, "https://example.test/get?a=1");
        assert_eq!(navigation.method, Method::Get);
        assert_eq!(navigation.body, None);
    }

    #[test]
    fn what_is_not_written_yet_is_refused_by_name_and_recorded_under_its_method() {
        for (enctype, words) in [
            (Enctype::Multipart, "multipart/form-data"),
            (Enctype::TextPlain, "text/plain"),
        ] {
            let asked = submitting("https://httpbin.org/post", Method::Post, enctype, &[]);
            let Decided::Refused(refusal) =
                sent(&asked, "https://httpbin.org/forms/post", &Policies::none())
            else {
                panic!("{words} was sent");
            };
            assert_eq!(refusal.rule, Rule::Enctype { enctype });
            let said = refusal.to_string();
            assert!(
                said.contains(words) && said.contains("queue item 474"),
                "{said}"
            );
            let mut activity = Activity::new();
            assert!(refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
            let line = activity.latest().unwrap();
            assert_eq!(line.method(), "POST");
            assert_eq!(line.url().serialised, "https://httpbin.org/post");
        }
        let asked = submitting("about:blank", Method::Get, Enctype::UrlEncoded, &[]);
        assert_eq!(
            refused_by(&sent(&asked, "https://example.com/", &Policies::none())),
            Some(&Rule::FormScheme {
                scheme: "about".to_owned()
            })
        );
    }

    #[test]
    fn a_form_goes_only_where_the_header_policy_lets_it() {
        let policies = header_policy("default-src 'none'; form-action 'self'");
        let away = submitting(
            "https://evil.example/steal",
            Method::Post,
            Enctype::UrlEncoded,
            &[("card", "4111")],
        );
        let decided = sent(&away, "https://shop.example/pay", &policies);
        let Some(Rule::FormAction { said }) = refused_by(&decided) else {
            panic!("the form went where form-action forbids: {decided}");
        };
        assert!(said.contains("form-action allows 'self'"), "{said}");
        let home = submitting(
            "https://shop.example/pay",
            Method::Post,
            Enctype::UrlEncoded,
            &[],
        );
        assert!(matches!(
            sent(&home, "https://shop.example/pay", &policies),
            Decided::Go(_)
        ));
        // A link is not a form, and `form-action` is not about it.
        assert!(matches!(
            sent(
                &asking("https://evil.example/"),
                "https://shop.example/",
                &policies
            ),
            Decided::Go(_)
        ));
    }

    #[test]
    fn a_secure_page_does_not_send_a_form_in_the_clear() {
        let asked = submitting(
            "http://example.com/post",
            Method::Post,
            Enctype::UrlEncoded,
            &[("password", "hunter2")],
        );
        let decided = sent(&asked, "https://example.com/", &Policies::none());
        assert_eq!(refused_by(&decided), Some(&Rule::Insecure));
        let Decided::Refused(refusal) = decided else {
            panic!("sent in the clear");
        };
        let mut activity = Activity::new();
        assert!(refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
        assert_eq!(activity.latest().unwrap().method(), "POST");
        // An insecure page's form is its own business, and nothing travels
        // in the clear from localhost.
        assert!(matches!(
            sent(&asked, "http://example.com/", &Policies::none()),
            Decided::Go(_)
        ));
        let local = submitting(
            "http://localhost:8080/post",
            Method::Post,
            Enctype::UrlEncoded,
            &[],
        );
        assert!(matches!(
            sent(&local, "https://example.com/", &Policies::none()),
            Decided::Go(_)
        ));
    }

    #[test]
    fn a_query_that_makes_the_url_too_long_is_refused() {
        let long = "a".repeat(LONGEST_URL);
        let asked = submitting(
            "https://example.com/get",
            Method::Get,
            Enctype::UrlEncoded,
            &[("q", long.as_str())],
        );
        let decided = sent(&asked, "https://example.com/", &Policies::none());
        assert!(
            matches!(refused_by(&decided), Some(Rule::TooLong { bytes }) if *bytes > LONGEST_URL),
            "{decided}"
        );
        // The same entries in a body are not a URL.
        let posted = submitting(
            "https://example.com/post",
            Method::Post,
            Enctype::UrlEncoded,
            &[("q", long.as_str())],
        );
        assert!(matches!(
            sent(&posted, "https://example.com/", &Policies::none()),
            Decided::Go(_)
        ));
    }

    #[test]
    fn a_cross_site_post_carries_no_lax_cookie() {
        use alo_net::cookie::{Cookie, Partition};
        use alo_net::jar::Jar;

        let bank = from("https://bank.example/");
        let mut jar = Jar::new();
        for header in [
            "lax=1; Secure; SameSite=Lax",
            "none=1; Secure; SameSite=None",
        ] {
            jar.keep(
                Cookie::parse(header, &bank, &Partition::of(&bank)).unwrap(),
                SystemTime::UNIX_EPOCH,
            );
        }
        let at = "https://evil.example/";
        for (method, carried) in [(Method::Post, "none=1"), (Method::Get, "lax=1; none=1")] {
            let asked = submitting(
                "https://bank.example/transfer",
                method,
                Enctype::UrlEncoded,
                &[("to", "evil")],
            );
            let Decided::Go(navigation) = sent(&asked, at, &Policies::none()) else {
                panic!("the form was refused");
            };
            assert_eq!(
                jar.header_for_navigation(
                    &navigation.url,
                    Some(&from(at)),
                    navigation.method.name(),
                    SystemTime::UNIX_EPOCH
                )
                .as_deref(),
                Some(carried),
                "{}",
                navigation.method.name()
            );
        }
    }

    #[test]
    fn the_scheme_table_comes_before_the_form() {
        let asked = submitting("data:text/html,hi", Method::Get, Enctype::UrlEncoded, &[]);
        let decided = sent(
            &asked,
            "https://example.com/",
            &header_policy("form-action 'none'"),
        );
        assert!(matches!(refused_by(&decided), Some(Rule::Scheme { .. })));
    }
}
