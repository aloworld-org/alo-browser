/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a `<details>`' children are slotted into its boxes (ADR 0048 §§ 1–2).
//!
//! HTML's rendering section gives a `details` two slots: its **summary** —
//! the first `summary` child, laid out first wherever it is written — and
//! a **content** box holding every other child in document order, a second
//! `summary` among them as an ordinary element. While the `details` has no
//! `open` attribute the content box is still there, so a flex or grid
//! `details` keeps the gap before it as Chrome 154 was measured to, but it
//! holds nothing: its children make no boxes, so nothing in them is laid
//! out, drawn or read.
//!
//! This says which child goes where; [`crate::tree`] builds the boxes. Which
//! `summary` is the summary is `alo-dom`'s answer ([`alo_dom::details`]),
//! the same one the agent's role and state and a click's activation ask.

use alo_dom::{Document, NodeId};

/// A `details`' children, slotted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Slots {
    /// Its summary, or [`None`] for a `details` with no `summary` child —
    /// whose summary a browser makes and this engine does not yet (queue
    /// item 433), so its slot is empty and recorded.
    pub(crate) summary: Option<NodeId>,
    /// Every other child, in document order: what the content box holds.
    pub(crate) content: Vec<NodeId>,
    /// Whether it is open, so that the content box holds them at all.
    pub(crate) open: bool,
}

/// The slots of `id`'s children, or [`None`] when `id` is not a `details`.
pub(crate) fn slots(document: &Document, id: NodeId) -> Option<Slots> {
    let element = document.element(id)?;
    if !element.name.is_html("details") {
        return None;
    }
    let summary = alo_dom::details::summary_of(document, id);
    Some(Slots {
        summary,
        content: document
            .children(id)
            .filter(|child| Some(*child) != summary)
            .collect(),
        open: alo_dom::details::is_open(document, id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// The `details` of a document holding one.
    fn details(document: &Document) -> Option<NodeId> {
        document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("details"))
        })
    }

    /// The local names of elements, and `#text` for anything else.
    fn names(document: &Document, ids: &[NodeId]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                document
                    .element(*id)
                    .map_or_else(|| "#text".to_owned(), |e| e.name.local.to_string())
            })
            .collect()
    }

    #[test]
    fn the_first_summary_is_the_summary_and_the_rest_is_content_in_order() {
        let document = parse_document(
            "<details open><p>a</p><summary id=one>s</summary><summary>t</summary></details>",
        );
        let Some(slots) = details(&document).and_then(|id| slots(&document, id)) else {
            panic!("a details is slotted");
        };
        assert_eq!(
            names(&document, &slots.summary.into_iter().collect::<Vec<_>>()),
            ["summary"]
        );
        assert_eq!(names(&document, &slots.content), ["p", "summary"]);
        assert!(slots.open);
    }

    #[test]
    fn a_details_with_no_summary_has_an_empty_summary_slot() {
        let document = parse_document("<details>text<p>a</p></details>");
        let Some(slots) = details(&document).and_then(|id| slots(&document, id)) else {
            panic!("a details is slotted");
        };
        assert_eq!(slots.summary, None);
        assert_eq!(names(&document, &slots.content), ["#text", "p"]);
        assert!(!slots.open);
    }

    #[test]
    fn anything_but_a_details_has_no_slots() {
        let document = parse_document("<div><summary>s</summary></div>");
        assert!(
            document
                .descendants(document.root())
                .all(|id| slots(&document, id).is_none())
        );
    }
}
