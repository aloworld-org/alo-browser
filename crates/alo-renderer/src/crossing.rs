/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which nodes a person's pointer leaves and enters as it goes from one node
//! to another (queue item 542), as Chrome 154 was measured to say it.
//!
//! The browser keeps **the node last under the pointer** — none before
//! anything was said of the pointer, and none once it has gone outside the
//! viewport — and, when the node under it changes, fires the boundary
//! events between the two. This file says which: it decides nothing about
//! when, and fires nothing (`event_loop/crossed.rs` does).
//!
//! # Measured
//!
//! In headless Chrome 154 over CDP (`Input.dispatchMouseEvent`), listeners
//! on the window, the document and every element logging each event, the
//! source not read:
//!
//! - **Out, then leave.** `pointerout` at the node left, then a
//!   `pointerleave` at it and at each of its ancestors that does not hold
//!   the node entered, innermost first.
//! - **Over, then enter.** `pointerover` at the node entered, then a
//!   `pointerenter` at each of its ancestors that did not hold the node
//!   left, outermost first, ending at the node itself.
//! - **The document is an ancestor.** A pointer arriving from nowhere
//!   enters the document first and then the root element, and one leaving
//!   the viewport leaves the root element and then the document. Between
//!   two elements the root element holds both, so the document never
//!   hears of it. The window is never entered or left.
//! - **`relatedTarget`** is the other node: the one entered, at an out and
//!   each leave; the one left, at an over and each enter; `null` for
//!   nowhere.
//!
//! # A node that leaves its document under the pointer
//!
//! Chrome does not tell a node that is no longer in its document that the
//! pointer left it. It takes **the nearest of its ancestors still in the
//! document** as the node last under the pointer — its ancestors as they
//! were when it was under the pointer, since a tree taken out of a document
//! no longer knows where it was — and at its next look —
//! measured as a move after a removal, and as a timer when there is no move
//! — fires no `pointerout`, the leaves from that ancestor, and an over at
//! whatever is under the pointer then, with that ancestor as its
//! `relatedTarget` — **even when it is that ancestor itself**: with `b`
//! under the pointer removed from `a`, Chrome fired `pointerover` at `a`
//! with `a` as its `relatedTarget`, and no enter.

use alo_dom::{Document, NodeId};

/// The boundary events between the node last under a person's pointer and
/// the node under it now: where each is fired, in Chrome's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crossing {
    /// The node left, if it hears a `pointerout`: [`None`] when the pointer
    /// came from nowhere, or from a node that has left its document since.
    pub out: Option<NodeId>,
    /// Each node a `pointerleave` is fired at, innermost first.
    pub leave: Vec<NodeId>,
    /// The node entered, if any — [`None`] when the pointer has gone
    /// nowhere — which hears a `pointerover`.
    pub over: Option<NodeId>,
    /// Each node a `pointerenter` is fired at, outermost first.
    pub enter: Vec<NodeId>,
    /// The `relatedTarget` of the over and each enter: the node left.
    pub from: Option<NodeId>,
    /// The `relatedTarget` of the out and each leave: the node entered.
    pub to: Option<NodeId>,
}

impl Crossing {
    /// The crossing from `from`, the node last under the pointer, to `to`,
    /// the node under it now — [`None`] when they are the same, which
    /// crosses nothing.
    pub fn between(document: &Document, from: Option<NodeId>, to: Option<NodeId>) -> Option<Self> {
        if from == to {
            return None;
        }
        let (leave, enter) = lists(document, from, to);
        Some(Self {
            out: from,
            leave,
            over: to,
            enter,
            from,
            to,
        })
    }

    /// The crossing at Chrome's first look after the node last under the
    /// pointer left its document, `was` being it and its ancestors as they
    /// were then, innermost first: from the nearest of them still in the
    /// document, with no out, to `to` — an over even when `to` is that
    /// ancestor.
    pub fn after_removal(document: &Document, was: &[NodeId], to: Option<NodeId>) -> Self {
        let from = was
            .iter()
            .copied()
            .find(|node| is_connected(document, *node));
        let (leave, enter) = lists(document, from, to);
        Self {
            out: None,
            leave,
            over: to,
            enter,
            from,
            to,
        }
    }

    /// Whether it fires nothing at all.
    pub fn is_empty(&self) -> bool {
        self.out.is_none() && self.leave.is_empty() && self.over.is_none() && self.enter.is_empty()
    }
}

/// The leaves from `from` and the enters to `to`: the inclusive ancestors
/// of each, the document among them, that are not the other's — the
/// leaves innermost first, the enters outermost first.
fn lists(
    document: &Document,
    from: Option<NodeId>,
    to: Option<NodeId>,
) -> (Vec<NodeId>, Vec<NodeId>) {
    let above_from = from.map_or_else(Vec::new, |node| inclusive_ancestors(document, node));
    let above_to = to.map_or_else(Vec::new, |node| inclusive_ancestors(document, node));
    let leave = above_from
        .iter()
        .copied()
        .filter(|node| !above_to.contains(node))
        .collect();
    let enter = above_to
        .iter()
        .rev()
        .copied()
        .filter(|node| !above_from.contains(node))
        .collect();
    (leave, enter)
}

/// Whether `node` is in `document`: whether its last ancestor is the
/// document itself rather than the top of a tree taken out of it.
pub fn is_connected(document: &Document, node: NodeId) -> bool {
    inclusive_ancestors(document, node).last() == Some(&document.root())
}

/// `node`, its parent, and so on up to the node with none.
pub fn inclusive_ancestors(document: &Document, node: NodeId) -> Vec<NodeId> {
    let mut found = vec![node];
    let mut next = document.parent(node);
    while let Some(parent) = next {
        found.push(parent);
        next = document.parent(parent);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page put to Chrome: `a` holding `b`, and `c` holding the span
    /// `d`, and `o` beside `a`.
    const PAGE: &str = "<!doctype html><html id=html><body id=body>\
         <div id=a><div id=b></div><div id=c><span id=d>x</span></div></div>\
         <div id=o></div>";

    /// The page's document, and its nodes by id, the document first.
    fn page() -> (Document, [NodeId; 8]) {
        let document = alo_dom::parse_document(PAGE);
        let by = |id: &str| {
            document
                .descendants(document.root())
                .find(|node| {
                    document
                        .element(*node)
                        .is_some_and(|element| element.attr("id") == Some(id))
                })
                .unwrap_or_else(|| panic!("no element {id}"))
        };
        let nodes = [
            document.root(),
            by("html"),
            by("body"),
            by("a"),
            by("b"),
            by("c"),
            by("d"),
            by("o"),
        ];
        (document, nodes)
    }

    #[test]
    fn arriving_from_nowhere_enters_the_document_first() {
        let (document, [doc, html, body, a, b, ..]) = page();
        let crossed = Crossing::between(&document, None, Some(b));
        assert_eq!(
            crossed,
            Some(Crossing {
                out: None,
                leave: vec![],
                over: Some(b),
                enter: vec![doc, html, body, a, b],
                from: None,
                to: Some(b),
            })
        );
    }

    #[test]
    fn between_siblings_only_they_are_left_and_entered() {
        let (document, [.., b, c, _, _]) = page();
        let crossed = Crossing::between(&document, Some(b), Some(c));
        assert_eq!(
            crossed,
            Some(Crossing {
                out: Some(b),
                leave: vec![b],
                over: Some(c),
                enter: vec![c],
                from: Some(b),
                to: Some(c),
            })
        );
    }

    #[test]
    fn into_a_child_leaves_nothing_and_out_to_an_ancestor_enters_nothing() {
        let (document, [.., a, _, c, d, _]) = page();
        let inward = Crossing::between(&document, Some(c), Some(d));
        assert_eq!(
            inward
                .as_ref()
                .map(|crossed| (&crossed.leave, &crossed.enter)),
            Some((&vec![], &vec![d])),
            "c is still under the pointer, around d"
        );
        assert_eq!(inward.and_then(|crossed| crossed.out), Some(c));
        let outward = Crossing::between(&document, Some(d), Some(a));
        assert_eq!(
            outward.map(|crossed| (crossed.leave, crossed.enter)),
            Some((vec![d, c], vec![])),
            "innermost first"
        );
    }

    #[test]
    fn going_nowhere_leaves_everything_up_to_the_document() {
        let (document, [doc, html, body, a, b, ..]) = page();
        let crossed = Crossing::between(&document, Some(b), None);
        assert_eq!(
            crossed,
            Some(Crossing {
                out: Some(b),
                leave: vec![b, a, body, html, doc],
                over: None,
                enter: vec![],
                from: Some(b),
                to: None,
            })
        );
    }

    #[test]
    fn the_same_node_crosses_nothing() {
        let (document, [.., b, _, _, _]) = page();
        assert_eq!(Crossing::between(&document, Some(b), Some(b)), None);
        assert_eq!(Crossing::between(&document, None, None), None);
    }

    #[test]
    fn after_a_removal_the_nearest_ancestor_in_the_document_is_left_without_an_out() {
        let (mut document, [_, html, body, a, b, c, d, _]) = page();
        let was_b = inclusive_ancestors(&document, b);
        let was_d = inclusive_ancestors(&document, d);
        assert!(document.remove(b));
        assert!(!is_connected(&document, b));
        let crossed = Crossing::after_removal(&document, &was_b, Some(a));
        assert_eq!(
            crossed,
            Crossing {
                out: None,
                leave: vec![],
                over: Some(a),
                enter: vec![],
                from: Some(a),
                to: Some(a),
            },
            "an over at a, from a, as Chrome fired it"
        );
        assert!(document.remove(a));
        let crossed = Crossing::after_removal(&document, &was_d, Some(html));
        assert_eq!(
            crossed.from,
            Some(body),
            "d's parent and grandparent left too, and a no longer knows it was in body"
        );
        assert_eq!(
            (crossed.out, crossed.leave, crossed.over, crossed.from),
            (None, vec![body], Some(html), Some(body))
        );
        assert!(!crossed.enter.contains(&c));
    }
}
