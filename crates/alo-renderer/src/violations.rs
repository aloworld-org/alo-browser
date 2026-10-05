/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page's own policy objected to as it loaded, said by the renderer and
//! reported by the browser process (queue item 237, cut from 236).
//!
//! # Why it is split across the boundary
//!
//! Only the renderer sees a page's inline script, so only the renderer knows
//! that a policy objected to one. Only the browser process may post anything
//! (ADR 0005), so only it can tell the policy's author. A renderer cannot
//! simply hand over a finished report, because **a renderer is the process
//! that parsed a hostile page**, and everything it says may be the page
//! talking: a report it wrote could name any collector, carry any body, and
//! be posted with the browser's own network stack.
//!
//! So what crosses is an [`Objection`] — *the policy at this place objected to
//! a script written into the page* — and nothing else. The place is a place in
//! [`Page::stated`], the list both processes make from the same headers, and
//! the browser process writes the report from **its own** copy of that policy
//! with [`alo_net::Policies::inline_violation_of`]: where it goes, the policy's
//! text and the directive are all things the browser process read, never
//! things it was told. What a lying renderer can still do is claim an
//! objection that did not happen, and a page's own script will be able to
//! cause real ones on purpose once script can reach the document, so that is
//! not a power worth more than a bound — [`MOST_OBJECTIONS`]. A claim naming a
//! policy that does not exist, or one that lets every inline script in and so
//! could not have objected, is not believed and is said rather than posted.
//!
//! # Both dispositions
//!
//! An enforced policy that refused a script and a report-only policy that
//! would have are reported the same way, which is the whole of what makes a
//! report-only header useful: a site watches a policy for a week, reads what
//! it would have refused, and only then enforces it.
//!
//! # What is not reported here
//!
//! A `<meta>` policy's objections. CSP drops `report-uri` from a policy
//! delivered in markup, and the browser process has never seen that markup —
//! so it is not in [`Page::stated`] and cannot be named. A `<meta>` policy
//! carrying `report-to` is queue item 240. The renderer still *obeys* one and
//! says what it refused in [`crate::FromRenderer::Loaded`]'s issues.

use alo_net::csp::{Inline, Placement};
use alo_net::csp_report;

use crate::page::Page;

/// The most objections one load may carry across the boundary.
///
/// Every objection becomes at least one post from the browser process, and
/// both how many objections there are and how many endpoints each policy
/// names are chosen by the page — so without a ceiling a page with ten
/// thousand inline scripts, or a renderer that says it had, is ten thousand
/// requests somebody else chose to make. A renderer sends no more than this
/// and says how many it left out; the wire refuses a load claiming more.
///
/// The number is ours rather than any other browser's. It is generous for a
/// page whose author is reading their reports and small against the cost of a
/// request.
pub const MOST_OBJECTIONS: usize = 64;

/// One policy objecting to one piece of inline content, as a renderer says it.
///
/// Inline content in an element of its own — a `<script>` with its code in
/// the page — which is the only inline content a renderer runs today. An event
/// handler is queue item 81's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Objection {
    /// The objecting policy's place in [`Page::stated`].
    pub policy: usize,
    /// What it objected to: script or style.
    pub kind: Inline,
}

/// What the browser process makes of a load's objections.
#[derive(Debug, Clone, Default)]
pub struct Reports {
    /// The reports to post, with [`alo_net::Pool::report`].
    pub posts: Vec<alo_net::Request>,
    /// Endpoints a policy named that could not be used, each said in words.
    pub unusable: Vec<String>,
    /// Objections the renderer claimed and the browser process does not
    /// believe, each said in words. Nothing is posted for one.
    pub disbelieved: Vec<String>,
}

/// The reports a load's objections ask for.
///
/// `page` is the page the browser process sent, whose headers are the only
/// policies a report is written from; `about` is what the browser process
/// knows about the document — its URL, its status, its reporting endpoints and
/// what caused it to load (ADR 0012 § 2: a report is attributed to the load it
/// is about).
pub fn reports(page: &Page, about: &csp_report::Page, objections: &[Objection]) -> Reports {
    let stated = page.stated();
    let mut reports = Reports::default();
    if objections.len() > MOST_OBJECTIONS {
        reports.disbelieved.push(format!(
            "a renderer said {} policy objections for one load, more than the {MOST_OBJECTIONS} \
             one may carry, and only the first {MOST_OBJECTIONS} were reported",
            objections.len()
        ));
    }
    for objection in objections.iter().take(MOST_OBJECTIONS) {
        let Some(violation) =
            stated.inline_violation_of(objection.policy, objection.kind, Placement::Element)
        else {
            reports.disbelieved.push(format!(
                "a renderer said the policy at place {} objected to inline {}, and this page \
                 has no such policy that could have, so nothing was reported",
                objection.policy,
                match objection.kind {
                    Inline::Script => "script",
                    Inline::Style => "style",
                }
            ));
            continue;
        };
        let posting = violation.posts(about);
        reports.posts.extend(posting.posts);
        reports.unusable.extend(posting.unusable);
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::Size;
    use alo_net::cause::{Cause, Identities};

    fn about() -> csp_report::Page {
        let url = alo_url::parse("https://shop.example/checkout").expect("a URL");
        csp_report::Page::at(
            url,
            Cause::Document {
                document: Identities::default().a_document(),
            },
        )
    }

    fn page() -> Page {
        Page::new("<p>hi</p>", Size::new(1.0, 1.0))
            .with_policy("script-src 'unsafe-inline'; report-uri /enforced")
            .watched_by("script-src 'none'; report-uri /watched")
    }

    fn script(policy: usize) -> Objection {
        Objection {
            policy,
            kind: Inline::Script,
        }
    }

    #[test]
    fn an_objection_is_reported_from_the_browser_processs_own_copy_of_the_policy() {
        let reports = reports(&page(), &about(), &[script(1)]);
        assert!(reports.disbelieved.is_empty(), "{:?}", reports.disbelieved);
        assert_eq!(reports.posts.len(), 1);
        let post = reports.posts.first().expect("a post");
        assert_eq!(post.url.to_string(), "https://shop.example/watched");
        let body = String::from_utf8_lossy(&post.body);
        assert!(body.contains("\"blocked-uri\":\"inline\""), "{body}");
        assert!(body.contains("\"disposition\":\"report\""), "{body}");
        assert!(body.contains("\"original-policy\":\"script-src 'none'; report-uri /watched\""));
    }

    #[test]
    fn a_policy_that_could_not_have_objected_is_not_believed() {
        let reports = reports(
            &page(),
            &about(),
            &[script(0), script(2), script(usize::MAX)],
        );
        assert!(
            reports.posts.is_empty(),
            "a claim nobody could have made was posted"
        );
        assert_eq!(reports.disbelieved.len(), 3, "{:?}", reports.disbelieved);
    }

    #[test]
    fn no_more_objections_are_reported_than_a_load_may_carry() {
        let flood = vec![script(1); MOST_OBJECTIONS * 3];
        let reports = reports(&page(), &about(), &flood);
        assert_eq!(reports.posts.len(), MOST_OBJECTIONS);
        assert_eq!(reports.disbelieved.len(), 1, "{:?}", reports.disbelieved);
    }

    #[test]
    fn a_page_with_no_policy_reports_nothing_whatever_it_is_told() {
        let bare = Page::new("", Size::new(1.0, 1.0));
        let reports = reports(&bare, &about(), &[script(0)]);
        assert!(reports.posts.is_empty());
        assert_eq!(reports.disbelieved.len(), 1);
    }
}
