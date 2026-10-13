/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The element a person's pointer is over: what `:hover` matches (queue item
//! 550, ADR 0024 § 5).
//!
//! **One element, kept by the document.** The renderer finds it by its hit
//! test and says so here; style matches `:hover` on it and on every element
//! above it; a script's `querySelectorAll(':hover')` asks the same document
//! and so gets the same answer. Two places holding it would be two answers
//! the day they disagreed.
//!
//! What follows was measured in headless Chrome 154 over CDP
//! (`Input.dispatchMouseEvent`, listeners reading
//! `querySelectorAll(':hover')`), the source not read:
//!
//! - **The new element is hovered before any event of the crossing fires.**
//!   A `pointerout` listener at the element left already reads the chain of
//!   the element entered.
//! - **The chain is the element and its ancestors, the root element
//!   included**: over the page's background it is `<html>` alone. Outside
//!   the viewport with no button held nothing is hovered; with the button
//!   held, `<html>` is, as the pointer is over it.
//! - **A drag hovers what it is over**, not what was pressed.
//! - **Taking the hovered element out of the document** — or any element
//!   holding it — hovers, at once, the parent it was taken from: a script
//!   that removes it reads that parent's chain on its next line. Putting it
//!   back does not hover it again; only the pointer's next look does.
//! - A box moved out from under the pointer by its style stays hovered until
//!   the pointer moves.
//!
//! A `<label>` hovered also hovers the control it labels, in Chrome; that is
//! queue item 554's, with the rule for which control a label labels.

use crate::document::Document;
use crate::node::NodeId;

impl Document {
    /// The element a person's pointer is over, if any.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Say that a person's pointer is over `element`, or over nothing.
    ///
    /// A change, counted ([`Document::change_count`]), when it is not the
    /// element already hovered, because what is drawn depends on it; the
    /// same element again changes nothing. A node that is not an element in
    /// this document's tree is refused, and changes nothing: [`false`].
    pub fn hover(&mut self, element: Option<NodeId>) -> bool {
        if let Some(node) = element
            && (self.element(node).is_none() || !self.is_attached(node))
        {
            return false;
        }
        if self.hovered != element {
            self.hovered = element;
            self.note_change();
        }
        true
    }

    /// Whether `node` is hovered: the element a person's pointer is over, or
    /// one of its ancestors.
    pub fn is_hovered(&self, node: NodeId) -> bool {
        let mut current = self.hovered;
        while let Some(at) = current {
            if at == node {
                return true;
            }
            current = self.parent(at);
        }
        false
    }

    /// `node` is about to be taken out of its parent: if it holds the
    /// hovered element, that parent is hovered instead, as Chrome's is —
    /// or nothing, when the parent is not an element.
    ///
    /// Not a change of its own: the caller's taking it out is the change.
    pub(crate) fn hover_leaving(&mut self, node: NodeId) {
        if !self.is_hovered(node) {
            return;
        }
        self.hovered = self
            .parent(node)
            .filter(|parent| self.element(*parent).is_some());
    }
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::node::NodeId;

    fn find(document: &Document, wanted: &str) -> NodeId {
        document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))
            .unwrap_or_else(|| panic!("no element {wanted}"))
    }

    const PAGE: &str = "<html id=html><body id=body><div id=a><div id=b><span id=d>x</span>\
         </div><div id=c></div></div></body></html>";

    #[test]
    fn the_hovered_element_and_its_ancestors_are_hovered() {
        let mut document = crate::parse_document(PAGE);
        let [html, body, a, b, c, d] =
            ["html", "body", "a", "b", "c", "d"].map(|id| find(&document, id));
        assert_eq!(document.hovered(), None);
        assert!(!document.is_hovered(html), "nothing before the pointer");

        assert!(document.hover(Some(d)));
        for hovered in [html, body, a, b, d] {
            assert!(document.is_hovered(hovered));
        }
        assert!(!document.is_hovered(c), "a sibling is not");
    }

    #[test]
    fn hovering_another_element_is_a_change_and_the_same_one_is_not() {
        let mut document = crate::parse_document(PAGE);
        let [b, c] = ["b", "c"].map(|id| find(&document, id));
        let before = document.change_count();
        assert!(document.hover(Some(b)));
        assert_eq!(document.change_count(), before + 1);
        assert!(document.hover(Some(b)));
        assert_eq!(document.change_count(), before + 1, "the same element");
        assert!(document.hover(Some(c)));
        assert!(document.hover(None));
        assert_eq!(document.change_count(), before + 3);
        assert!(document.hover(None));
        assert_eq!(document.change_count(), before + 3, "nothing, again");
    }

    #[test]
    fn what_is_not_an_element_in_the_tree_is_refused() {
        let mut document = crate::parse_document(PAGE);
        let [b, d] = ["b", "d"].map(|id| find(&document, id));
        let text = document.first_child(d).expect("the text");
        assert!(document.hover(Some(b)));
        let before = document.change_count();
        assert!(!document.hover(Some(text)), "a text node");
        assert!(!document.hover(Some(document.root())), "the document");
        assert!(!document.hover(Some(NodeId(10_000))), "no node at all");
        assert!(document.remove(d));
        assert!(!document.hover(Some(d)), "out of the tree");
        assert_eq!(document.hovered(), Some(b), "and nothing changed");
        assert_eq!(document.change_count(), before + 1, "but the removal");
    }

    #[test]
    fn taking_the_hovered_element_out_hovers_the_parent_it_left() {
        let mut document = crate::parse_document(PAGE);
        let [a, b, c, d] = ["a", "b", "c", "d"].map(|id| find(&document, id));
        assert!(document.hover(Some(d)));
        assert!(document.remove(d));
        assert_eq!(document.hovered(), Some(b));

        assert!(document.remove(b), "the parent hovered now");
        assert_eq!(document.hovered(), Some(a));

        assert!(document.hover(Some(c)));
        assert!(document.remove(a), "the hovered element's ancestor");
        assert_eq!(
            document
                .hovered()
                .and_then(|n| document.element(n))
                .map(|e| &*e.name.local),
            Some("body")
        );
    }

    #[test]
    fn moving_the_hovered_element_hovers_the_parent_it_left_and_putting_it_back_does_not() {
        let mut document = crate::parse_document(PAGE);
        let [a, b, c, d] = ["a", "b", "c", "d"].map(|id| find(&document, id));
        assert!(document.hover(Some(b)));
        assert!(document.append_child(c, b).is_ok(), "b into c");
        assert_eq!(document.hovered(), Some(a));

        assert!(document.hover(Some(d)));
        assert!(document.remove(d));
        assert!(document.append_child(b, d).is_ok(), "d put back");
        assert_eq!(document.hovered(), Some(b));
    }

    #[test]
    fn taking_out_something_else_leaves_the_hovered_element_alone() {
        let mut document = crate::parse_document(PAGE);
        let [b, c] = ["b", "c"].map(|id| find(&document, id));
        assert!(document.hover(Some(b)));
        assert!(document.remove(c));
        assert_eq!(document.hovered(), Some(b));
    }

    #[test]
    fn taking_out_the_root_element_hovers_nothing() {
        let mut document = crate::parse_document(PAGE);
        let [html, d] = ["html", "d"].map(|id| find(&document, id));
        assert!(document.hover(Some(d)));
        assert!(document.remove(html));
        assert_eq!(document.hovered(), None);
    }
}
