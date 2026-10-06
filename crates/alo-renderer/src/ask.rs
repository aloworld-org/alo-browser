/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a page asked to go, as it crosses the boundary (ADR 0020 §§ 1, 2
//! and 5, queue item 263).
//!
//! A renderer **asks** to navigate, as a claim in the answer to the message
//! whose work made it — [`crate::FromRenderer::Loaded`] or
//! [`crate::FromRenderer::Acted`] — and never learns what became of it.
//! The ask is what the page's ongoing navigation was when the work ended
//! (`alo-bindings`' `navigating.rs`): the last one, and how many it replaced.
//!
//! **What it carries is what only the renderer knows**: the URL, resolved
//! against the document's base, which the browser process cannot see without
//! reading the page's markup; how the click arose; and the link's referrer
//! policy. **Never who** — a cause, a tab or a document — which the browser
//! process assigns from which message it was answering
//! ([`crate::navigate`]), and never anything it must believe: the URL is
//! parsed again there, as bytes a stranger chose.

use alo_bindings::navigating::Ongoing;
use alo_net::referrer::Policy;

use crate::said;

pub use alo_bindings::navigating::By;

/// The navigation a page asked for, as a renderer says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Where to, resolved and serialised by the renderer. A claim: the
    /// browser process parses it again.
    pub url: String,
    /// How it arose.
    pub by: By,
    /// The referrer policy the link asked for — its `referrerpolicy`, or
    /// `no-referrer` for `rel="noreferrer"` — if any it asked for is one this
    /// engine knows.
    pub referrer: Option<Policy>,
    /// How many navigations the same work asked for before this one, which
    /// this one replaced (§ 5).
    pub replaced: u32,
}

/// What the page asked for since the renderer last answered, as an answer
/// carries it: the ask, if there is one, and the lines it says about it —
/// the links it did not follow (at most `alo-bindings`'
/// [`MOST_NOT_FOLLOWED`](alo_bindings::navigating::MOST_NOT_FOLLOWED) and a
/// count of the rest), and how many asks the last one replaced.
pub(crate) fn answer(ongoing: &Ongoing) -> (Option<Asked>, Vec<String>) {
    let mut lines: Vec<String> = ongoing
        .not_followed()
        .iter()
        .map(|why| said::line(why))
        .collect();
    if ongoing.more_not_followed() > 0 {
        lines.push(format!(
            "{} more links were not followed and are not said",
            ongoing.more_not_followed()
        ));
    }
    let asked = ongoing.last().map(|navigation| Asked {
        url: navigation.url.serialised.clone(),
        by: navigation.by,
        referrer: if navigation.no_referrer {
            Some(Policy::NoReferrer)
        } else {
            navigation
                .referrer_policy
                .as_deref()
                .and_then(Policy::named)
        },
        replaced: ongoing.replaced(),
    });
    if let Some(asked) = &asked
        && asked.replaced > 0
    {
        lines.push(said::line(&format_args!(
            "the page asked to go somewhere {} times before it asked for {:?}, and only the \
             last is asked for",
            asked.replaced, asked.url
        )));
    }
    (asked, lines)
}

#[cfg(test)]
mod tests {
    use alo_bindings::navigating::{Navigation, NotFollowed};

    use super::*;

    fn going(to: &str, by: By) -> Navigation {
        Navigation {
            url: alo_url::parse(to).unwrap(),
            by,
            referrer_policy: None,
            no_referrer: false,
        }
    }

    #[test]
    fn nothing_asked_is_nothing_said() {
        assert_eq!(answer(&Ongoing::default()), (None, Vec::new()));
    }

    #[test]
    fn the_last_ask_crosses_with_how_many_it_replaced_and_says_so() {
        let mut ongoing = Ongoing::default();
        ongoing.start(Ok(going("https://example.com/one", By::Browser)));
        ongoing.start(Err(NotFollowed::Download {
            href: "/f".to_owned(),
        }));
        ongoing.start(Ok(going("https://example.com/two", By::Script)));
        let (asked, lines) = answer(&ongoing);
        assert_eq!(
            asked,
            Some(Asked {
                url: "https://example.com/two".to_owned(),
                by: By::Script,
                referrer: None,
                replaced: 1,
            })
        );
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines.iter().any(|line| line.contains("download")));
        assert!(lines.iter().any(|line| line.contains("1 times")));
    }

    #[test]
    fn a_referrer_policy_the_engine_knows_crosses_and_noreferrer_wins() {
        let mut navigation = going("https://example.com/", By::Script);
        navigation.referrer_policy = Some("Origin".to_owned());
        let mut ongoing = Ongoing::default();
        ongoing.start(Ok(navigation.clone()));
        assert_eq!(
            answer(&ongoing).0.and_then(|asked| asked.referrer),
            Some(Policy::Origin)
        );

        navigation.referrer_policy = Some("whatever-you-like".to_owned());
        let mut ongoing = Ongoing::default();
        ongoing.start(Ok(navigation.clone()));
        assert_eq!(answer(&ongoing).0.and_then(|asked| asked.referrer), None);

        navigation.no_referrer = true;
        navigation.referrer_policy = Some("unsafe-url".to_owned());
        let mut ongoing = Ongoing::default();
        ongoing.start(Ok(navigation));
        assert_eq!(
            answer(&ongoing).0.and_then(|asked| asked.referrer),
            Some(Policy::NoReferrer)
        );
    }
}
