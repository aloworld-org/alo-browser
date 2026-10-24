/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page in Chrome may ask for: the browser process's judgement of a
//! request before Chrome is let send it (ADR 0060 § 2, queue item 633).
//!
//! Chrome's own network carries the bytes of a page opened from the network
//! ([`crate::chrome_tab::Tab::open`]), and every request it would make is
//! paused and brought here first. The address is read as bytes a stranger
//! chose — Chrome holds the page, and the page chose it — parsed again with
//! `alo-url`, bounded, and judged by scheme with the rules ADR 0020 gives the
//! engine's renderer, by the same names ([`Rule`]):
//!
//! | Scheme | Asked for by a page in Chrome |
//! |---|---|
//! | `http`, `https` | let go |
//! | `about:blank` | let go |
//! | `file` | refused: the page is from the network, and a page that is not a file may not reach one |
//! | every other | refused by name |
//!
//! # What reaches this, measured
//!
//! Against Chrome 155, `Fetch` pauses only what Chrome would send over a
//! network or read from a file: a link or a script sending the tab to
//! `data:`, `file:`, `javascript:` or `chrome:` is stopped by Chrome itself
//! and told of nowhere, and a redirect to `file:` or `data:` is Chrome's
//! `ERR_UNSAFE_REDIRECT` before its hop is paused. A link or a script
//! sending the tab to `mailto:`, `blob:`, `ftp:`, `about:srcdoc` or a scheme
//! of nobody's is never paused either, and *is* told of first
//! (`Page.frameRequestedNavigation`): that ask is judged here too, and a
//! refusal is named, though nothing of the browser process's stands between
//! it and Chrome — Chrome went nowhere for any of them, headless, which is
//! how [`crate::chrome::Chrome`] starts it.
//!
//! Nothing here assigns a cause or writes the record: that is item 634's.

use alo_renderer::navigate::{LONGEST_SAID, LONGEST_URL, Rule};
use alo_url::Url;
use core::fmt;

/// A request, or an ask to go somewhere, that the browser process refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// What was asked for, as Chrome spelled it, at most [`LONGEST_SAID`]
    /// characters of it.
    pub asked: String,
    /// Which rule refused it.
    pub rule: Rule,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} was refused: {}", self.asked, self.rule)
    }
}

/// Judge `asked`, an address a page in Chrome asked for: the address, parsed
/// by this process, when Chrome may be let go to it.
///
/// # Errors
///
/// The [`Refusal`], naming the rule, for an address that is too long, does
/// not parse, or has a scheme a page from the network may not reach.
pub fn judge(asked: &str) -> Result<Url, Refusal> {
    let refuse = |rule: Rule| Refusal {
        asked: asked.chars().take(LONGEST_SAID).collect(),
        rule,
    };
    if asked.len() > LONGEST_URL {
        return Err(refuse(Rule::TooLong { bytes: asked.len() }));
    }
    let url = alo_url::parse(asked).map_err(|why| refuse(Rule::Unparsed { why: why.why }))?;
    match url.scheme.as_str() {
        "http" | "https" => Ok(url),
        "about" if url.is_about_blank() => Ok(url),
        // Every page opened this way is from the network, so a file is
        // always asked for from elsewhere.
        "file" => Err(refuse(Rule::FileFromElsewhere)),
        other => Err(refuse(Rule::Scheme {
            scheme: other.to_owned(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(asked: &str) -> Option<Rule> {
        judge(asked).err().map(|refusal| refusal.rule)
    }

    #[test]
    fn the_web_and_the_empty_page_are_let_go() {
        for asked in [
            "http://127.0.0.1:8080/a?b#c",
            "https://example.com/",
            "about:blank",
        ] {
            assert_eq!(
                judge(asked).map(|url| url.to_string()).ok().as_deref(),
                alo_url::parse(asked)
                    .ok()
                    .map(|url| url.to_string())
                    .as_deref(),
                "{asked}"
            );
        }
    }

    #[test]
    fn every_other_scheme_is_refused_by_the_rule_the_engine_names() {
        assert_eq!(
            rule("mailto:someone@example.com"),
            Some(Rule::Scheme {
                scheme: "mailto".to_owned()
            })
        );
        for scheme in ["data", "javascript", "blob", "ftp", "chrome", "foo"] {
            let asked = format!("{scheme}:x");
            assert_eq!(
                rule(&asked),
                Some(Rule::Scheme {
                    scheme: scheme.to_owned()
                }),
                "{asked}"
            );
        }
        assert_eq!(
            rule("about:srcdoc"),
            Some(Rule::Scheme {
                scheme: "about".to_owned()
            })
        );
        assert_eq!(rule("file:///etc/hosts"), Some(Rule::FileFromElsewhere));
    }

    #[test]
    fn what_a_stranger_wrote_is_refused_not_panicked_on() {
        assert!(matches!(rule(""), Some(Rule::Unparsed { .. })));
        assert!(matches!(rule("http://["), Some(Rule::Unparsed { .. })));
        assert!(matches!(rule("\u{0}\u{ffff}"), Some(Rule::Unparsed { .. })));
        let long = format!("https://example.com/{}", "é".repeat(LONGEST_URL));
        let Err(refusal) = judge(&long) else {
            panic!("a URL past the bound was let go");
        };
        assert_eq!(refusal.rule, Rule::TooLong { bytes: long.len() });
        assert_eq!(refusal.asked.chars().count(), LONGEST_SAID, "said, bounded");
    }

    #[test]
    fn a_refusal_reads_as_a_sentence() {
        let Err(refusal) = judge("mailto:someone@example.com") else {
            panic!("let go");
        };
        assert_eq!(
            refusal.to_string(),
            "\"mailto:someone@example.com\" was refused: a page may not send its tab to a \
             mailto: URL, which hands the person to another program"
        );
    }
}
