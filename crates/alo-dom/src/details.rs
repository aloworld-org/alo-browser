/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<details>` and the one `<summary>` that is its summary (ADR 0048).
//!
//! HTML's *summary for its parent details* is a `summary` element whose
//! parent is a `details` and which is that `details`' **first `summary`
//! child**. Only that one is shown first, carries the `expanded` state and
//! opens its `details` when it is activated; any other `summary` is an
//! ordinary element. Three readers ask the question — the box tree, which
//! lays the summary out first; the agent tree's role and state; and
//! [`crate::activation`], which toggles `open` — so it is answered once,
//! here, rather than three times in ways that could come to disagree.
//!
//! Whether a `details` is open is its `open` attribute, as HTML says: there
//! is no state apart from it.

use crate::document::Document;
use crate::node::NodeId;

/// Whether `node` is a `<details>` element.
fn is_details(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| element.name.is_html("details"))
}

/// The summary of the `details` element `details`: its first `summary`
/// child, wherever it is written among its children. [`None`] for a
/// `details` that has none, and for anything that is not a `details`.
pub fn summary_of(document: &Document, details: NodeId) -> Option<NodeId> {
    if !is_details(document, details) {
        return None;
    }
    document.children(details).find(|child| {
        document
            .element(*child)
            .is_some_and(|element| element.name.is_html("summary"))
    })
}

/// The `details` that `summary` is the summary of, if it is the summary for
/// its parent `details` — and [`None`] for a `summary` anywhere else, a
/// second one included.
pub fn details_of(document: &Document, summary: NodeId) -> Option<NodeId> {
    let parent = document.parent(summary)?;
    (summary_of(document, parent) == Some(summary)).then_some(parent)
}

/// Whether the `details` element `details` is open: whether it has an
/// `open` attribute.
pub fn is_open(document: &Document, details: NodeId) -> bool {
    document
        .element(details)
        .is_some_and(|element| element.attr("open").is_some())
}

/// Open a closed `details` or close an open one, answering whether it is
/// open now — HTML's activation behaviour of its summary, which removes the
/// `open` attribute if it is there and sets it to the empty string if not.
/// [`None`], changing nothing, for anything that is not a `details`.
pub fn toggle(document: &mut Document, details: NodeId) -> Option<bool> {
    if !is_details(document, details) {
        return None;
    }
    if is_open(document, details) {
        document.remove_attribute(details, "open")?;
        Some(false)
    } else {
        document.set_attribute(details, "open", "")?;
        Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    /// Every element named `local`, in tree order.
    fn all(document: &Document, local: &str) -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.name.is_html(local))
            })
            .collect()
    }

    #[test]
    fn a_details_summary_is_its_first_summary_child_wherever_it_is_written() {
        let document = parse_document(
            "<details><p>a</p><summary>one</summary><summary>two</summary></details>",
        );
        let details = all(&document, "details");
        let summaries = all(&document, "summary");
        assert_eq!(summary_of(&document, details[0]), Some(summaries[0]));
        assert_eq!(details_of(&document, summaries[0]), Some(details[0]));
        assert_eq!(
            details_of(&document, summaries[1]),
            None,
            "a second summary is an ordinary element"
        );
    }

    #[test]
    fn a_summary_that_is_not_a_details_child_is_nobodys_summary() {
        let document = parse_document(
            "<details><div><summary>deep</summary></div></details><summary>x</summary>",
        );
        for summary in all(&document, "summary") {
            assert_eq!(details_of(&document, summary), None);
        }
        let details = all(&document, "details");
        assert_eq!(summary_of(&document, details[0]), None);
        let div = all(&document, "div");
        assert_eq!(summary_of(&document, div[0]), None, "not a details");
    }

    #[test]
    fn toggling_opens_a_closed_details_and_closes_an_open_one() {
        let mut document = parse_document("<details><summary>s</summary></details><p>p</p>");
        let details = all(&document, "details")[0];
        assert!(!is_open(&document, details));
        assert_eq!(toggle(&mut document, details), Some(true));
        assert_eq!(
            document.element(details).and_then(|e| e.attr("open")),
            Some(""),
            "set to the empty string"
        );
        assert_eq!(toggle(&mut document, details), Some(false));
        assert!(!is_open(&document, details));
        let paragraph = all(&document, "p")[0];
        assert_eq!(toggle(&mut document, paragraph), None);
        assert_eq!(
            document.element(paragraph).and_then(|e| e.attr("open")),
            None
        );
    }
}
