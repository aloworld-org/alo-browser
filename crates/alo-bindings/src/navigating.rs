/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a page has asked to go: HTML's *ongoing navigation*, held in the
//! document cell until the renderer answers (ADR 0020 §§ 1, 2 and 5, queue
//! item 263).
//!
//! A click nobody cancelled on a link follows it, and following it is the
//! page navigating itself — which is the browser process's to decide, never
//! the renderer's (ADR 0005). So a renderer only **asks**, in the answer to
//! the message whose work made the ask, and this is where the ask is kept
//! meanwhile: a script's `click()` is a native in the middle of the page's
//! script, and the document cell is the one place it can reach.
//!
//! # What following a link is, here
//!
//! HTML's *follow the hyperlink*, as far as a renderer's half of it goes:
//!
//! 1. a link with a `download` attribute asks for a file rather than a page
//!    — ADR 0020 § 6, queue item 264 — and is not followed;
//! 2. a link whose target names **another window** — its `target`, or the
//!    first `<base target>` — is not followed: a window is a popup policy
//!    and a tab strip (item 118), and putting it in this tab instead would
//!    be a different thing happening. `_self`, `_parent` and `_top` are all
//!    this tab, since there are no frames (item 86);
//! 3. the `href` is resolved against the document's **base URL** — the
//!    first `<base href>` resolved against the document's own URL, or the
//!    document's URL — and a link that does not resolve goes nowhere;
//! 4. otherwise the page's navigation **starts**, and replaces any the same
//!    task started before it (§ 5): one ask, the last, and a count.
//!
//! What the ask carries is *what*, never *who* (ADR 0020 § 2): the URL, how
//! the click arose ([`By`]), and the link's own referrer policy. The cause
//! is the browser process's, from which message it was answering.

use alo_dom::{Document, NodeId};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_url::Url;
use core::fmt;

use crate::document_cell::DocumentCell;

/// The most links one answer says it did not follow, by name; the rest are
/// counted.
///
/// A page can click a thousand links that go nowhere in one task, and each
/// sentence crosses the boundary — so the number is the renderer's to bound
/// rather than the page's to choose. Sixteen is more than any page that is
/// not probing has a reason to reach.
pub const MOST_NOT_FOLLOWED: usize = 16;

/// How a navigation arose. A claim the renderer makes, beside the cause the
/// browser process assigns (ADR 0020 §§ 2 and 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// The browser's own click — an agent's `Activate`.
    Browser,
    /// A page's script calling `click()`.
    Script,
}

impl fmt::Display for By {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            By::Browser => "the browser's click",
            By::Script => "a script's click",
        })
    }
}

/// A navigation a page has started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigation {
    /// Where to, resolved against the document's base URL.
    pub url: Url,
    /// How it arose.
    pub by: By,
    /// The link's `referrerpolicy`, as written. The browser process reads
    /// it; one it does not know leaves the default, as for the header.
    pub referrer_policy: Option<String>,
    /// Whether the link's `rel` says `noreferrer`.
    pub no_referrer: bool,
}

/// Why a link was not followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotFollowed {
    /// It asks for a file: `<a download>` (ADR 0020 § 6).
    Download {
        /// Its `href`, as written.
        href: String,
    },
    /// Its target names another window.
    AnotherWindow {
        /// Its `href`, as written.
        href: String,
        /// The target it named.
        target: String,
    },
    /// Its `href` does not resolve against the document's base URL.
    Unresolved {
        /// Its `href`, as written.
        href: String,
        /// Why, in words.
        why: String,
    },
    /// The node is not a link any more: not an element, or no `href`.
    NotALink,
}

impl fmt::Display for NotFollowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotFollowed::Download { href } => write!(
                f,
                "a link to {href:?} asks for a download, which is queue item 264, and was not \
                 followed"
            ),
            NotFollowed::AnotherWindow { href, target } => write!(
                f,
                "a link to {href:?} names another window ({target:?}), and opening one is queue \
                 item 118, so it was not followed"
            ),
            NotFollowed::Unresolved { href, why } => {
                write!(f, "a link to {href:?} goes nowhere: {why}")
            }
            NotFollowed::NotALink => f.write_str("what was clicked is no longer a link"),
        }
    }
}

/// The page's ongoing navigation, and what did not become one, since the
/// renderer last answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ongoing {
    last: Option<Navigation>,
    replaced: u32,
    not_followed: Vec<NotFollowed>,
    more_not_followed: u32,
}

impl Ongoing {
    /// A link was followed or was not: keep the navigation, replacing the
    /// one before it (ADR 0020 § 5), or keep why not. Whether a navigation
    /// started.
    ///
    /// A link that was not followed replaces nothing: HTML aborts an
    /// ongoing navigation only when another one starts.
    pub fn start(&mut self, followed: Result<Navigation, NotFollowed>) -> bool {
        match followed {
            Ok(navigation) => {
                if self.last.replace(navigation).is_some() {
                    self.replaced = self.replaced.saturating_add(1);
                }
                true
            }
            Err(why) => {
                if self.not_followed.len() < MOST_NOT_FOLLOWED {
                    self.not_followed.push(why);
                } else {
                    self.more_not_followed = self.more_not_followed.saturating_add(1);
                }
                false
            }
        }
    }

    /// The navigation the page asked for last, if it asked for one.
    pub const fn last(&self) -> Option<&Navigation> {
        self.last.as_ref()
    }

    /// How many navigations the last one replaced.
    pub const fn replaced(&self) -> u32 {
        self.replaced
    }

    /// Why links were not followed, at most [`MOST_NOT_FOLLOWED`], in the
    /// order they were clicked.
    pub fn not_followed(&self) -> &[NotFollowed] {
        &self.not_followed
    }

    /// How many more links were not followed than are said.
    pub const fn more_not_followed(&self) -> u32 {
        self.more_not_followed
    }

    /// Whether nothing has been asked and nothing refused.
    pub fn is_empty(&self) -> bool {
        self.last.is_none() && self.not_followed.is_empty() && self.more_not_followed == 0
    }
}

/// Follow the link `link` in `document`, whose own URL is `url`: the
/// navigation it starts, or why it starts none.
///
/// # Errors
///
/// [`NotFollowed`], for the reasons the module lists.
pub fn follow(
    document: &Document,
    url: &Url,
    link: NodeId,
    by: By,
) -> Result<Navigation, NotFollowed> {
    let element = document.element(link).ok_or(NotFollowed::NotALink)?;
    let href = element.attr("href").ok_or(NotFollowed::NotALink)?;
    if element.attr("download").is_some() {
        return Err(NotFollowed::Download {
            href: href.to_owned(),
        });
    }
    let target = element
        .attr("target")
        .or_else(|| first_base(document, "target"));
    if let Some(target) = target
        && !is_this_tab(target)
    {
        return Err(NotFollowed::AnotherWindow {
            href: href.to_owned(),
            target: target.to_owned(),
        });
    }
    let url = alo_url::join(&base(document, url), href).map_err(|why| NotFollowed::Unresolved {
        href: href.to_owned(),
        why: why.why,
    })?;
    let no_referrer = element.attr("rel").is_some_and(|rel| {
        rel.split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case("noreferrer"))
    });
    Ok(Navigation {
        url,
        by,
        referrer_policy: element.attr("referrerpolicy").map(ToOwned::to_owned),
        no_referrer,
    })
}

/// The document's base URL: its first `<base href>`, resolved against its
/// own URL, or its own URL when there is none or it does not resolve.
pub fn base(document: &Document, url: &Url) -> Url {
    first_base(document, "href")
        .and_then(|href| alo_url::join(url, href).ok())
        .unwrap_or_else(|| url.clone())
}

/// The attribute `name` of the first `base` element in the document that has
/// one — HTML takes `href` and `target` each from the first that says it.
fn first_base<'a>(document: &'a Document, name: &str) -> Option<&'a str> {
    document.descendants(document.root()).find_map(|node| {
        document
            .element(node)
            .filter(|element| element.name.is_html("base"))
            .and_then(|element| element.attr(name))
    })
}

/// Whether a target names this tab: nothing, `_self`, `_parent` or `_top`,
/// which with no frames (item 86) are all the page's own tab.
fn is_this_tab(target: &str) -> bool {
    target.is_empty()
        || ["_self", "_parent", "_top"]
            .iter()
            .any(|own| target.eq_ignore_ascii_case(own))
}

/// Say that the document `cell` holds is at `url` — stated by the browser
/// process, before any of the page's script runs. Whether `cell` is a
/// document cell.
pub fn locate(objects: &mut Objects, cell: Ref, url: Url) -> bool {
    objects
        .write_embedded::<DocumentCell, _>(cell, |held, _| held.url = url)
        .is_some()
}

/// Follow `link` in the document `cell` holds and keep what came of it in
/// the cell: whether a navigation started, or [`None`] if `cell` is not a
/// document cell.
pub fn start(objects: &mut Objects, cell: Ref, link: NodeId, by: By) -> Option<bool> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| {
        let followed = follow(&held.document, &held.url, link, by);
        held.ongoing.start(followed)
    })
}

/// Take what the page has asked since this was last taken, leaving nothing
/// — [`None`] if `cell` is not a document cell.
pub fn take(objects: &mut Objects, cell: Ref) -> Option<Ongoing> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| core::mem::take(&mut held.ongoing))
}

#[cfg(test)]
mod tests {
    use alo_dom::parse_document;

    use super::*;

    fn at(url: &str) -> Url {
        alo_url::parse(url).unwrap()
    }

    fn link(document: &Document) -> NodeId {
        document
            .descendants(document.root())
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("a"))
            })
            .unwrap()
    }

    fn followed(markup: &str, from: &str) -> Result<Navigation, NotFollowed> {
        let document = parse_document(markup);
        follow(&document, &at(from), link(&document), By::Script)
    }

    #[test]
    fn a_link_resolves_against_the_documents_own_url() {
        let gone = followed("<a href=../two?x#y>go</a>", "https://example.com/a/b/one").unwrap();
        assert_eq!(gone.url.serialised, "https://example.com/a/two?x#y");
        assert_eq!(gone.by, By::Script);
        assert_eq!(gone.referrer_policy, None);
        assert!(!gone.no_referrer);
    }

    #[test]
    fn the_first_base_href_is_the_base_and_one_that_does_not_resolve_is_not() {
        let gone = followed(
            "<base href=/docs/><base href=/other/><a href=page>go</a>",
            "https://example.com/a/b",
        )
        .unwrap();
        assert_eq!(gone.url.serialised, "https://example.com/docs/page");
        let gone = followed(
            "<base href='http://[::'><a href=page>go</a>",
            "https://example.com/a/b",
        )
        .unwrap();
        assert_eq!(gone.url.serialised, "https://example.com/a/page");
    }

    #[test]
    fn a_link_that_does_not_resolve_goes_nowhere_and_says_why() {
        let refused = followed("<a href=page>go</a>", "about:blank").unwrap_err();
        assert!(
            matches!(&refused, NotFollowed::Unresolved { href, .. } if href == "page"),
            "{refused:?}"
        );
        assert!(refused.to_string().contains("\"page\" goes nowhere"));
    }

    #[test]
    fn another_window_is_refused_and_this_tab_by_any_name_is_not() {
        for own in ["", "_self", "_TOP", "_parent"] {
            let markup = format!("<a href=/x target='{own}'>go</a>");
            assert!(followed(&markup, "https://example.com/").is_ok(), "{own:?}");
        }
        for other in ["_blank", "results"] {
            let markup = format!("<a href=/x target={other}>go</a>");
            assert_eq!(
                followed(&markup, "https://example.com/"),
                Err(NotFollowed::AnotherWindow {
                    href: "/x".to_owned(),
                    target: other.to_owned()
                }),
            );
        }
        assert!(
            matches!(
                followed(
                    "<base target=_blank><a href=/x>go</a>",
                    "https://example.com/"
                ),
                Err(NotFollowed::AnotherWindow { .. })
            ),
            "a base target was ignored"
        );
        assert!(
            followed(
                "<base target=_blank><a href=/x target=_self>go</a>",
                "https://example.com/"
            )
            .is_ok(),
            "the link's own target lost to the base's"
        );
    }

    #[test]
    fn a_download_is_not_a_navigation() {
        assert_eq!(
            followed("<a href=/f.txt download>save</a>", "https://example.com/"),
            Err(NotFollowed::Download {
                href: "/f.txt".to_owned()
            }),
        );
    }

    #[test]
    fn the_links_referrer_claims_are_carried() {
        let gone = followed(
            "<a href=/x rel='nofollow NoReferrer' referrerpolicy=origin>go</a>",
            "https://example.com/",
        )
        .unwrap();
        assert!(gone.no_referrer);
        assert_eq!(gone.referrer_policy.as_deref(), Some("origin"));
    }

    #[test]
    fn the_last_navigation_is_kept_and_those_it_replaced_are_counted() {
        let mut ongoing = Ongoing::default();
        assert!(ongoing.is_empty());
        let first = followed("<a href=/one>1</a>", "https://example.com/");
        let second = followed("<a href=/two>2</a>", "https://example.com/");
        assert!(ongoing.start(first));
        assert!(!ongoing.start(Err(NotFollowed::NotALink)));
        assert!(ongoing.start(second));
        assert_eq!(ongoing.replaced(), 1);
        assert_eq!(
            ongoing.last().map(|gone| gone.url.serialised.as_str()),
            Some("https://example.com/two"),
        );
        assert_eq!(ongoing.not_followed(), &[NotFollowed::NotALink]);
    }

    #[test]
    fn what_was_not_followed_is_bounded_and_the_rest_counted() {
        let mut ongoing = Ongoing::default();
        for _ in 0..MOST_NOT_FOLLOWED + 5 {
            ongoing.start(Err(NotFollowed::NotALink));
        }
        assert_eq!(ongoing.not_followed().len(), MOST_NOT_FOLLOWED);
        assert_eq!(ongoing.more_not_followed(), 5);
        assert!(ongoing.last().is_none());
    }
}
