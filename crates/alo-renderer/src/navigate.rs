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
//! A refusal is said ([`Refusal`]'s words) and recorded under ADR 0012 by
//! whoever holds the record ([`Refusal::record`]); the page is told nothing,
//! as in every browser. Loading what was decided — session history, what
//! survives, the request's other headers — is item 85's: until then the
//! decision is handed to whoever drove [`crate::Tabs`].

use alo_net::activity::{Activity, Happened};
use alo_net::cause::Cause;
use alo_net::referrer::{self, Policy};
use alo_net::request::{Purpose, Request};
use alo_url::Url;
use core::fmt;
use std::time::SystemTime;

use crate::ask::{Asked, By};

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
    /// Where to, as this process parsed it.
    pub url: Url,
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
        let request = Request::get(url.clone(), self.cause.clone()).for_purpose(Purpose::Document);
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
/// process's own copy of its URL — under `cause`, which the caller assigned
/// from the message the ask answered.
pub fn decide(asked: &Asked, from: &Url, cause: Cause) -> Decided {
    let refuse = |url: Option<Url>, rule: Rule| {
        Decided::Refused(Refusal {
            asked: asked.url.chars().take(LONGEST_SAID).collect(),
            url,
            rule,
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
    // The link's policy, or the engine's default,
    // `strict-origin-when-cross-origin`.
    let policy: Policy = asked.referrer.unwrap_or_default();
    let referrer = referrer::for_request(policy, from, &url);
    Decided::Go(Navigation {
        url,
        cause,
        by: asked.by,
        referrer,
        replaced: asked.replaced,
    })
}

#[cfg(test)]
mod tests {
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
        }
    }

    fn from(url: &str) -> Url {
        alo_url::parse(url).unwrap()
    }

    fn decided(url: &str, at: &str) -> Decided {
        decide(
            &asking(url),
            &from(at),
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
        let Decided::Go(navigation) = decide(&asked, &from("https://example.com/"), cause.clone())
        else {
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

        let Decided::Refused(unparsed) =
            decide(&asking("::"), &from("https://example.com/"), cause)
        else {
            panic!("nonsense was navigated");
        };
        assert!(!unparsed.record(&mut activity, SystemTime::UNIX_EPOCH));
        assert_eq!(activity.len(), 1);
    }
}
