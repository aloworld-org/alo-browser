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
//! # A label hovers its control (queue item 554)
//!
//! A `<label>` in the chain also hovers the control it labels
//! ([`Document::labelled_control`]) — that control alone, not its
//! ancestors. Chrome 154, measured the same way:
//!
//! - **The control is decided when the label is hovered**, and kept while
//!   it stays hovered. A `for` changed, an `id` changed or a control put
//!   in front of the old one leaves the old one hovered; a label that
//!   labelled nothing when it was hovered hovers nothing until it is
//!   hovered again.
//! - **The control leaving the chain ends it**, though its label stays: the
//!   pointer moved from the control inside a label to another element in
//!   the same label leaves the control unhovered, until the label is
//!   hovered again.
//! - **The control taken out of the document** is no longer hovered, and
//!   putting it back does not hover it again.
//!
//! Where this engine **does not** follow Chrome, deliberately: Chrome's
//! hover on such a control outlives the label's. After a `for` or `id`
//! change it stays when the pointer leaves the label, and stays for as long
//! as the page is open; after the label is taken out of the document it
//! stays until the pointer crosses the control itself. That is a hover
//! stuck on an element nothing is over, which no page can depend on and a
//! person sees as a broken page, so here the control's hover ends with its
//! label's, whichever way the label stops being hovered.

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
    ///
    /// A `<label>` newly in the chain hovers the control it labels now; one
    /// still in it keeps the control it hovered, unless that control was in
    /// the chain and is no longer; one no longer in it takes its control's
    /// hover away.
    pub fn hover(&mut self, element: Option<NodeId>) -> bool {
        if let Some(node) = element
            && (self.element(node).is_none() || !self.is_attached(node))
        {
            return false;
        }
        if self.hovered == element {
            return true;
        }
        let was = self.hovered;
        self.hovered = element;
        self.keep_labels(|document, label, control| {
            document.holds(element, label) && !document.leaves(was, element, control)
        });
        let entering: Vec<NodeId> = self
            .chain(element)
            .filter(|node| !self.holds(was, *node))
            .collect();
        for label in entering {
            if let Some(control) = self.labelled_control(label) {
                self.hovered_by_label.push((label, control));
            }
        }
        self.note_change();
        true
    }

    /// Whether `node` is hovered: the element a person's pointer is over,
    /// one of its ancestors, or a control a hovered `<label>` labels.
    pub fn is_hovered(&self, node: NodeId) -> bool {
        self.holds(self.hovered, node)
            || self
                .hovered_by_label
                .iter()
                .any(|(_, control)| *control == node)
    }

    /// `node` is about to be taken out of its parent: if it holds the
    /// hovered element, that parent is hovered instead, as Chrome's is —
    /// or nothing, when the parent is not an element. A label taken out
    /// with it takes its control's hover too, and a control taken out is no
    /// longer hovered.
    ///
    /// Not a change of its own: the caller's taking it out is the change.
    pub(crate) fn hover_leaving(&mut self, node: NodeId) {
        if self.holds(self.hovered, node) {
            self.hovered = self
                .parent(node)
                .filter(|parent| self.element(*parent).is_some());
            let hovered = self.hovered;
            self.keep_labels(|document, label, _| document.holds(hovered, label));
        }
        self.keep_labels(|document, _, control| !document.holds(Some(control), node));
    }

    /// Keep the hovered labels, and their controls, that `keep` says to.
    fn keep_labels(&mut self, keep: impl Fn(&Self, NodeId, NodeId) -> bool) {
        let mut labels = core::mem::take(&mut self.hovered_by_label);
        labels.retain(|(label, control)| keep(self, *label, *control));
        self.hovered_by_label = labels;
    }

    /// `at` and its ancestors, nearest first.
    fn chain(&self, at: Option<NodeId>) -> impl Iterator<Item = NodeId> + '_ {
        core::iter::successors(at, |node| self.parent(*node))
    }

    /// Whether `node` is in the chain at `was` and not in the one at `now`.
    fn leaves(&self, was: Option<NodeId>, now: Option<NodeId>, node: NodeId) -> bool {
        self.holds(was, node) && !self.holds(now, node)
    }

    /// Whether `ancestor` is `at` or one of its ancestors.
    fn holds(&self, at: Option<NodeId>, ancestor: NodeId) -> bool {
        self.chain(at).any(|node| node == ancestor)
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

    /// The page of labels Chrome 154 was measured on, as far as these
    /// tests need it.
    const LABELS: &str = "<html id=html><body id=body><label id=l1 for=f></label>\
         <div id=fw><input id=f></div><label id=l2><span id=s2></span><input id=g>\
         <input id=g2></label><label id=l3 for=p></label><div id=p></div></body></html>";

    /// The ids of every hovered element, in tree order, as
    /// `querySelectorAll(':hover')` answers.
    fn hovered_ids(document: &Document) -> Vec<&str> {
        core::iter::once(document.root())
            .chain(document.descendants(document.root()))
            .filter(|node| document.is_hovered(*node))
            .filter_map(|node| document.element(node).and_then(|e| e.attr("id")))
            .collect()
    }

    #[test]
    fn a_hovered_label_hovers_its_control_and_not_the_controls_ancestors() {
        let mut document = crate::parse_document(LABELS);
        let [l1, f, s2, g, l3] = ["l1", "f", "s2", "g", "l3"].map(|id| find(&document, id));
        assert!(document.hover(Some(l1)));
        assert_eq!(hovered_ids(&document), ["html", "body", "l1", "f"]);
        assert!(document.hover(Some(f)), "the control itself");
        assert_eq!(hovered_ids(&document), ["html", "body", "fw", "f"]);
        assert!(document.hover(Some(s2)), "inside a label wrapping two");
        assert_eq!(hovered_ids(&document), ["html", "body", "l2", "s2", "g"]);
        assert!(document.hover(Some(g)));
        assert_eq!(hovered_ids(&document), ["html", "body", "l2", "g"]);
        assert!(document.hover(Some(l3)), "a for naming a div");
        assert_eq!(hovered_ids(&document), ["html", "body", "l3"]);
        assert!(document.hover(None));
        assert_eq!(hovered_ids(&document), Vec::<&str>::new());
    }

    #[test]
    fn the_control_is_the_one_labelled_when_the_label_was_hovered() {
        let mut document = crate::parse_document(LABELS);
        let [l1, l2, s2, g, l3] = ["l1", "l2", "s2", "g", "l3"].map(|id| find(&document, id));
        assert!(document.hover(Some(l1)));
        assert!(document.set_attribute(l1, "for", "g").is_some());
        assert_eq!(hovered_ids(&document), ["html", "body", "l1", "f"]);
        assert!(document.hover(Some(l1)), "the same element again");
        assert_eq!(hovered_ids(&document), ["html", "body", "l1", "f"]);
        assert!(document.hover(None));
        assert_eq!(hovered_ids(&document), Vec::<&str>::new(), "not stuck");
        assert!(document.hover(Some(l1)));
        assert_eq!(hovered_ids(&document), ["html", "body", "l1", "g"]);

        assert!(document.hover(Some(s2)));
        let t = document.create_element("textarea").expect("a textarea");
        assert!(document.insert_before(l2, t, Some(g)).is_ok());
        assert!(document.hover(Some(l2)), "still in the label");
        assert!(document.is_hovered(g) && !document.is_hovered(t));

        assert!(document.hover(Some(l3)));
        assert!(document.set_attribute(l3, "for", "f").is_some());
        assert_eq!(hovered_ids(&document), ["html", "body", "l3"]);
        assert!(document.hover(None) && document.hover(Some(l3)));
        assert_eq!(hovered_ids(&document), ["html", "body", "f", "l3"]);
    }

    #[test]
    fn a_control_leaving_the_chain_is_not_hovered_by_the_label_still_in_it() {
        let mut document = crate::parse_document(LABELS);
        let [s2, g, g2] = ["s2", "g", "g2"].map(|id| find(&document, id));
        assert!(document.hover(Some(s2)));
        assert!(document.hover(Some(g)));
        assert!(document.hover(Some(g2)), "from the control to its sibling");
        assert_eq!(hovered_ids(&document), ["html", "body", "l2", "g2"]);
        assert!(
            document.hover(Some(s2)),
            "and back, the label hovered still"
        );
        assert_eq!(hovered_ids(&document), ["html", "body", "l2", "s2"]);
    }

    #[test]
    fn a_control_taken_out_is_not_hovered_and_is_not_again_when_put_back() {
        let mut document = crate::parse_document(LABELS);
        let [l1, fw, f] = ["l1", "fw", "f"].map(|id| find(&document, id));
        assert!(document.hover(Some(l1)));
        assert!(document.remove(f));
        assert!(!document.is_hovered(f));
        assert!(document.append_child(fw, f).is_ok());
        assert_eq!(hovered_ids(&document), ["html", "body", "l1"]);

        assert!(document.hover(None) && document.hover(Some(l1)));
        assert!(document.remove(fw), "the control's ancestor");
        assert!(!document.is_hovered(f));
    }

    #[test]
    fn a_label_taken_out_takes_its_controls_hover_with_it() {
        let mut document = crate::parse_document(LABELS);
        let [l1, s2, g] = ["l1", "s2", "g"].map(|id| find(&document, id));
        assert!(document.hover(Some(l1)));
        assert!(document.remove(l1));
        assert_eq!(hovered_ids(&document), ["html", "body"]);

        assert!(document.hover(Some(s2)));
        assert!(document.remove(s2), "inside the label: the label stays");
        assert!(document.is_hovered(g));
    }
}
