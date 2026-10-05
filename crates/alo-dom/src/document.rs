/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The document: the arena that owns every node, and the only thing that may
//! change the links between them.
//!
//! Nodes live in one vector and refer to each other by [`NodeId`]. An id is
//! never reused (ADR 0003), so a detached node keeps its identity and a stale
//! id resolves to a node that is out of the tree rather than to some later
//! node that is not the one that was meant.
//!
//! # What may change, and who decides
//!
//! **Everything, through the DOM standard's own operations.** Attributes came
//! first, because an agent that can only describe what it would do is not an
//! agent (queue item 42). The shape of the tree followed with ADR 0017: insert,
//! append, replace and remove are public in [`crate::mutation`], under the
//! standard's validity rules in [`crate::validity`], because a script and an
//! agent change one document and two copies of *what is a valid insertion* is
//! how they come to disagree. The parser keeps the crate-private operations
//! here, which the HTML parsing algorithm specifies differently and which
//! never refuse.
//!
//! **Every change advances [`Document::change_count`]**, which is how a
//! renderer that kept what it rendered knows it is stale (ADR 0017 § 6).
//!
//! # A slot is kept after its node is gone
//!
//! A detached tree nothing can reach any more is released
//! ([`crate::release`]): its nodes' contents are dropped and their slots are
//! left as **tombstones**, one pointer wide, so their ids answer nothing from
//! then on and the next node is still numbered one past the highest ever
//! made. That keeps ADR 0003's promise — no id is ever reused — once a script
//! can make nodes without end, which the parser never could.

use crate::node::{Element, Node, NodeId, NodeKind};

/// What the parser thought of the document's doctype.
///
/// **Recorded, never honoured.** Law 1 of `CLAUDE.md` refuses quirks mode
/// outright, so layout always behaves as though this were
/// [`QuirksSignal::NoQuirks`]. It is kept because a diagnostic that can say
/// "other engines would render this in quirks mode" is worth having, and
/// because throwing away what the parser observed is how an engine ends up
/// guessing later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuirksSignal {
    /// Standards mode. What every document we render is treated as.
    #[default]
    NoQuirks,
    /// Almost-standards mode, as the parser saw it.
    LimitedQuirks,
    /// Full quirks mode, as the parser saw it. Not implemented, by decision.
    Quirks,
}

/// A tree of nodes, and everything the parser observed while building it.
#[derive(Debug, Clone)]
pub struct Document {
    /// Every node ever made, at its id. [`None`] is a tombstone: a node that
    /// was released, whose id is never given out again.
    nodes: Vec<Option<Box<Node>>>,
    root: NodeId,
    changes: u64,
    /// What every live node owns, kept as the nodes change so that
    /// [`Document::footprint`] is a sum rather than a walk.
    bytes: usize,
    quirks_signal: QuirksSignal,
    issues: Vec<crate::parse::ParseIssue>,
}

impl Document {
    /// An empty document: one [`NodeKind::Document`] node and nothing else.
    pub fn new() -> Self {
        Self {
            nodes: vec![Some(Box::new(Node::new(NodeKind::Document)))],
            root: NodeId(0),
            changes: 0,
            bytes: Node::new(NodeKind::Document).footprint(),
            quirks_signal: QuirksSignal::NoQuirks,
            issues: Vec::new(),
        }
    }

    /// The document node. Every attached node is a descendant of this one.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// What the parser thought of the doctype. See [`QuirksSignal`]: this is
    /// recorded and never acted on.
    pub fn quirks_signal(&self) -> QuirksSignal {
        self.quirks_signal
    }

    /// Everything the parser objected to, in the order it objected.
    ///
    /// A document with issues is still a document — malformed input produces a
    /// usable tree, and the issues say what had to be repaired to get one.
    pub fn issues(&self) -> &[crate::parse::ParseIssue] {
        &self.issues
    }

    /// How many nodes this document has ever created, attached, detached or
    /// released.
    ///
    /// This is also the id the next node would be given, which is what makes it
    /// worth asserting on in a test about identity.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// How many times the document has been changed since it was made.
    ///
    /// Every successful insertion, removal, replacement and attribute change
    /// advances it by one; a refusal, making a node and releasing an
    /// unreachable tree do not, since none of them changes a tree anything can
    /// see. The parser's building does not count either: a document starts at
    /// zero whatever its markup was. A renderer compares this number to the one
    /// it rendered at to know whether what it holds is stale (ADR 0017 § 6).
    pub fn change_count(&self) -> u64 {
        self.changes
    }

    /// The sum of every live node's [`Node::footprint`], kept as they
    /// change; [`Document::footprint`] adds the arena's slots.
    pub(crate) fn live_bytes(&self) -> usize {
        self.bytes
    }

    /// The node an id names, or [`None`] if the id came from somewhere else or
    /// its node has been released.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0).and_then(Option::as_deref)
    }

    /// What the node an id names *is*.
    pub fn kind(&self, id: NodeId) -> Option<&NodeKind> {
        self.get(id).map(|node| &node.kind)
    }

    /// The element an id names, or [`None`] if it names something else.
    pub fn element(&self, id: NodeId) -> Option<&Element> {
        self.get(id).and_then(Node::element)
    }

    /// The parent of a node, or [`None`] for the root and for a detached node.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.parent)
    }

    /// The first child of a node.
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.first_child)
    }

    /// The last child of a node.
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.last_child)
    }

    /// The sibling before a node.
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.previous_sibling)
    }

    /// The sibling after a node.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.next_sibling)
    }

    /// The template a `<template>`'s contents belong to, for the fragment
    /// that holds them; [`None`] for every other node.
    ///
    /// The contents are not the template's children, but they are part of its
    /// tree: the DOM standard's *host-including* ancestry walks through this
    /// link, so a template cannot be put inside its own contents, and a
    /// released template takes its contents with it.
    pub fn host(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|node| node.host)
    }

    /// Change the element at this id with `change`.
    ///
    /// Crate-private so that nothing outside changes an element without
    /// [`Document::change_count`] hearing of it, and a closure rather than a
    /// borrow so that [`Document::footprint`] hears of it too.
    pub(crate) fn edit_element<R>(
        &mut self,
        id: NodeId,
        change: impl FnOnce(&mut Element) -> R,
    ) -> Option<R> {
        self.edit(id, |node| match &mut node.kind {
            NodeKind::Element(element) => Some(change(element)),
            _ => None,
        })?
    }

    /// Change what the node at this id holds, keeping the footprint's sum.
    ///
    /// Every change to a node's *content* comes through here; a change to its
    /// links alone does not, since a link owns nothing.
    fn edit<R>(&mut self, id: NodeId, change: impl FnOnce(&mut Node) -> R) -> Option<R> {
        let node = self.node_mut(id)?;
        let before = node.footprint();
        let outcome = change(node);
        let after = node.footprint();
        self.bytes = self.bytes.saturating_sub(before).saturating_add(after);
        Some(outcome)
    }

    /// Set an attribute on an element.
    ///
    /// [`None`] for an id that is not an element, which is a real answer
    /// rather than a failure — a caller acting on a tree it read a moment ago
    /// may well name something that is no longer what it was.
    ///
    /// Setting an attribute is a change even when the value is the one it
    /// already had, as it is in the standard.
    pub fn set_attribute(&mut self, id: NodeId, local: &str, value: &str) -> Option<()> {
        self.edit_element(id, |element| element.set_attr(local, value))?;
        self.note_change();
        Some(())
    }

    /// Take an attribute away from an element, and say whether there was one.
    ///
    /// Taking away one that was not there changes nothing, and is not counted.
    pub fn remove_attribute(&mut self, id: NodeId, local: &str) -> Option<bool> {
        let removed = self.edit_element(id, |element| element.remove_attr(local))?;
        if removed {
            self.note_change();
        }
        Some(removed)
    }

    /// Whether a node is reachable from the root.
    ///
    /// A node the parser built and then discarded stays in the arena keeping
    /// its id; this is how to ask whether it is still part of the tree.
    pub fn is_attached(&self, id: NodeId) -> bool {
        let mut current = id;
        loop {
            if current == self.root {
                return true;
            }
            match self.parent(current) {
                Some(parent) => current = parent,
                None => return false,
            }
        }
    }

    /// The children of a node, in order.
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            document: self,
            next: self.first_child(id),
        }
    }

    /// Every node beneath this one, in document order, not including it.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        Descendants {
            document: self,
            start: id,
            next: self.first_child(id),
        }
    }

    /// The text a person would read from this subtree: every descendant text
    /// node, in order, joined.
    ///
    /// Comments and processing instructions contribute nothing, and a
    /// `<template>`'s contents are not part of it — they are not children.
    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        if let Some(text) = self.get(id).and_then(Node::text) {
            out.push_str(text);
        }
        for descendant in self.descendants(id) {
            if let Some(text) = self.get(descendant).and_then(Node::text) {
                out.push_str(text);
            }
        }
        out
    }

    // --- building; `pub(crate)`, the parser's and `mutation`'s ---------------

    pub(crate) fn create(&mut self, kind: NodeKind) -> NodeId {
        let id = NodeId(self.nodes.len());
        let node = Node::new(kind);
        self.bytes = self.bytes.saturating_add(node.footprint());
        self.nodes.push(Some(Box::new(node)));
        id
    }

    /// Record one change. See [`Document::change_count`] for what counts.
    pub(crate) fn note_change(&mut self) {
        self.changes = self.changes.wrapping_add(1);
    }

    /// Say which template a contents fragment belongs to.
    pub(crate) fn set_host(&mut self, fragment: NodeId, template: NodeId) {
        if let Some(node) = self.node_mut(fragment) {
            node.host = Some(template);
        }
    }

    /// Drop a node's contents and leave its slot a tombstone, so that its id
    /// answers nothing from now on and is never given out again. The caller
    /// releases whole trees, so no live node is left linking to this one.
    pub(crate) fn tombstone(&mut self, id: NodeId) {
        if let Some(slot) = self.nodes.get_mut(id.0)
            && let Some(node) = slot.take()
        {
            self.bytes = self.bytes.saturating_sub(node.footprint());
        }
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0).and_then(Option::as_deref_mut)
    }

    pub(crate) fn set_quirks_signal(&mut self, signal: QuirksSignal) {
        self.quirks_signal = signal;
    }

    pub(crate) fn record_issue(&mut self, issue: crate::parse::ParseIssue) {
        self.issues.push(issue);
    }

    fn set_parent(&mut self, id: NodeId, parent: Option<NodeId>) {
        if let Some(node) = self.node_mut(id) {
            node.parent = parent;
        }
    }

    fn set_previous_sibling(&mut self, id: NodeId, sibling: Option<NodeId>) {
        if let Some(node) = self.node_mut(id) {
            node.previous_sibling = sibling;
        }
    }

    fn set_next_sibling(&mut self, id: NodeId, sibling: Option<NodeId>) {
        if let Some(node) = self.node_mut(id) {
            node.next_sibling = sibling;
        }
    }

    fn set_first_child(&mut self, id: NodeId, child: Option<NodeId>) {
        if let Some(node) = self.node_mut(id) {
            node.first_child = child;
        }
    }

    fn set_last_child(&mut self, id: NodeId, child: Option<NodeId>) {
        if let Some(node) = self.node_mut(id) {
            node.last_child = child;
        }
    }

    /// Take a node out of the tree, leaving its neighbours consistent. The node
    /// keeps its id and its own children.
    pub(crate) fn detach(&mut self, id: NodeId) {
        let Some(node) = self.get(id) else {
            return;
        };
        let (parent, previous, next) = (node.parent, node.previous_sibling, node.next_sibling);

        match next {
            Some(next) => self.set_previous_sibling(next, previous),
            None => {
                if let Some(parent) = parent {
                    self.set_last_child(parent, previous);
                }
            }
        }
        match previous {
            Some(previous) => self.set_next_sibling(previous, next),
            None => {
                if let Some(parent) = parent {
                    self.set_first_child(parent, next);
                }
            }
        }

        self.set_parent(id, None);
        self.set_previous_sibling(id, None);
        self.set_next_sibling(id, None);
    }

    /// Unhook the first child of `parent` from it, for a release taking the
    /// tree apart: the child keeps its own parent link, which is the way back
    /// up a walk that remembers nothing else.
    pub(crate) fn take_first_child(&mut self, parent: NodeId) -> Option<NodeId> {
        let child = self.first_child(parent)?;
        let next = self.next_sibling(child);
        self.set_first_child(parent, next);
        if next.is_none() {
            self.set_last_child(parent, None);
        }
        Some(child)
    }

    /// Make `child` the last child of `parent`, detaching it from wherever it
    /// was first.
    ///
    /// A node cannot be appended to itself or to its own descendant; that
    /// request is refused and the tree is left alone, because a cycle in the
    /// tree is a hang in every traversal that follows.
    pub(crate) fn attach_last(&mut self, parent: NodeId, child: NodeId) -> bool {
        if !self.may_hold(parent, child) {
            return false;
        }
        self.detach(child);
        self.set_parent(child, Some(parent));
        match self.last_child(parent) {
            Some(last) => {
                self.set_next_sibling(last, Some(child));
                self.set_previous_sibling(child, Some(last));
            }
            None => self.set_first_child(parent, Some(child)),
        }
        self.set_last_child(parent, Some(child));
        true
    }

    /// Put `new_node` immediately before `sibling`, detaching it from wherever
    /// it was first. Refused, leaving the tree alone, if `sibling` has no
    /// parent or if the move would make a cycle.
    pub(crate) fn attach_before(&mut self, sibling: NodeId, new_node: NodeId) -> bool {
        let Some(parent) = self.parent(sibling) else {
            return false;
        };
        if !self.may_hold(parent, new_node) || sibling == new_node {
            return false;
        }
        self.detach(new_node);
        let previous = self.previous_sibling(sibling);
        self.set_parent(new_node, Some(parent));
        self.set_previous_sibling(new_node, previous);
        self.set_next_sibling(new_node, Some(sibling));
        self.set_previous_sibling(sibling, Some(new_node));
        match previous {
            Some(previous) => self.set_next_sibling(previous, Some(new_node)),
            None => self.set_first_child(parent, Some(new_node)),
        }
        true
    }

    /// Move every child of `from` to the end of `to`, in order.
    pub(crate) fn reparent_children(&mut self, from: NodeId, to: NodeId) {
        while let Some(child) = self.first_child(from) {
            if !self.attach_last(to, child) {
                // Refused only by `may_hold`, which cannot change while this
                // loop runs; detaching stops it from spinning on the same node.
                self.detach(child);
            }
        }
    }

    /// Append text to `parent`, merging into its last child when that child is
    /// already a text node — the parser requires that adjacent text be one
    /// node, and every consumer downstream assumes it.
    pub(crate) fn append_text(&mut self, parent: NodeId, text: &str) {
        if let Some(last) = self.last_child(parent)
            && self.edit(last, |node| match &mut node.kind {
                NodeKind::Text(existing) => {
                    existing.push_str(text);
                    true
                }
                _ => false,
            }) == Some(true)
        {
            return;
        }
        let id = self.create(NodeKind::Text(text.to_owned()));
        self.attach_last(parent, id);
    }

    /// Insert text before `sibling`, merging into the previous sibling when
    /// that is already a text node.
    pub(crate) fn insert_text_before(&mut self, sibling: NodeId, text: &str) {
        if let Some(previous) = self.previous_sibling(sibling)
            && self.edit(previous, |node| match &mut node.kind {
                NodeKind::Text(existing) => {
                    existing.push_str(text);
                    true
                }
                _ => false,
            }) == Some(true)
        {
            return;
        }
        let id = self.create(NodeKind::Text(text.to_owned()));
        self.attach_before(sibling, id);
    }

    /// Add attributes that the element does not already have, keeping the ones
    /// it does. The parser needs this for `<html>` and `<body>`, whose
    /// attributes can arrive on a second start tag.
    pub(crate) fn add_attrs_if_missing(&mut self, id: NodeId, attrs: Vec<crate::node::Attribute>) {
        self.edit_element(id, |element| {
            for attr in attrs {
                if !element.attrs.iter().any(|held| held.name == attr.name) {
                    element.attrs.push(attr);
                }
            }
        });
    }

    /// Whether `parent` may hold `child`: not itself, and not one of its own
    /// ancestors.
    fn may_hold(&self, parent: NodeId, child: NodeId) -> bool {
        if parent == child || self.get(parent).is_none() || self.get(child).is_none() {
            return false;
        }
        let mut current = Some(parent);
        while let Some(id) = current {
            if id == child {
                return false;
            }
            current = self.parent(id);
        }
        true
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// The children of a node, in order. Made by [`Document::children`].
#[derive(Debug, Clone)]
pub struct Children<'a> {
    document: &'a Document,
    next: Option<NodeId>,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let current = self.next?;
        self.next = self.document.next_sibling(current);
        Some(current)
    }
}

/// Every node beneath one node, in document order. Made by
/// [`Document::descendants`].
#[derive(Debug, Clone)]
pub struct Descendants<'a> {
    document: &'a Document,
    start: NodeId,
    next: Option<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let current = self.next?;
        self.next = if let Some(child) = self.document.first_child(current) {
            Some(child)
        } else {
            let mut climbing = current;
            loop {
                if climbing == self.start {
                    break None;
                }
                if let Some(sibling) = self.document.next_sibling(climbing) {
                    break Some(sibling);
                }
                match self.document.parent(climbing) {
                    Some(parent) => climbing = parent,
                    None => break None,
                }
            }
        };
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::QualifiedName;
    use crate::node::Attribute;

    fn element(document: &mut Document, local: &str) -> NodeId {
        document.create(NodeKind::Element(Element {
            name: QualifiedName::html(local),
            attrs: Vec::new(),
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
            had_duplicate_attributes: false,
        }))
    }

    #[test]
    fn a_new_document_is_a_single_root() {
        let document = Document::new();
        assert_eq!(document.node_count(), 1);
        assert_eq!(document.root(), NodeId(0));
        assert_eq!(document.kind(document.root()), Some(&NodeKind::Document));
        assert!(document.children(document.root()).next().is_none());
        assert!(document.is_attached(document.root()));
    }

    #[test]
    fn ids_are_allocated_in_order_and_never_reused() {
        let mut document = Document::new();
        let a = element(&mut document, "a");
        let b = element(&mut document, "b");
        assert_eq!((a, b), (NodeId(1), NodeId(2)));

        document.attach_last(document.root(), a);
        document.attach_last(document.root(), b);
        document.detach(a);

        // The detached node keeps its id, and the next node gets a fresh one.
        let c = element(&mut document, "c");
        assert_eq!(c, NodeId(3));
        assert!(document.get(a).is_some());
        assert!(!document.is_attached(a));
        assert!(document.is_attached(b));
        assert_eq!(document.node_count(), 4);
    }

    #[test]
    fn appending_keeps_both_sibling_links() {
        let mut document = Document::new();
        let root = document.root();
        let (a, b, c) = (
            element(&mut document, "a"),
            element(&mut document, "b"),
            element(&mut document, "c"),
        );
        for id in [a, b, c] {
            assert!(document.attach_last(root, id));
        }

        assert_eq!(document.children(root).collect::<Vec<_>>(), vec![a, b, c]);
        assert_eq!(document.first_child(root), Some(a));
        assert_eq!(document.last_child(root), Some(c));
        assert_eq!(document.next_sibling(a), Some(b));
        assert_eq!(document.previous_sibling(c), Some(b));
        assert_eq!(document.previous_sibling(a), None);
        assert_eq!(document.next_sibling(c), None);
        assert_eq!(document.parent(b), Some(root));
    }

    #[test]
    fn detaching_the_middle_child_joins_its_neighbours() {
        let mut document = Document::new();
        let root = document.root();
        let (a, b, c) = (
            element(&mut document, "a"),
            element(&mut document, "b"),
            element(&mut document, "c"),
        );
        for id in [a, b, c] {
            document.attach_last(root, id);
        }
        document.detach(b);

        assert_eq!(document.children(root).collect::<Vec<_>>(), vec![a, c]);
        assert_eq!(document.next_sibling(a), Some(c));
        assert_eq!(document.previous_sibling(c), Some(a));
        assert_eq!(document.parent(b), None);
    }

    #[test]
    fn detaching_an_only_child_empties_its_parent() {
        let mut document = Document::new();
        let root = document.root();
        let a = element(&mut document, "a");
        document.attach_last(root, a);
        document.detach(a);
        assert_eq!(document.first_child(root), None);
        assert_eq!(document.last_child(root), None);
    }

    #[test]
    fn inserting_before_the_first_child_moves_the_head() {
        let mut document = Document::new();
        let root = document.root();
        let (a, b) = (element(&mut document, "a"), element(&mut document, "b"));
        document.attach_last(root, a);
        assert!(document.attach_before(a, b));
        assert_eq!(document.children(root).collect::<Vec<_>>(), vec![b, a]);
        assert_eq!(document.first_child(root), Some(b));
        assert_eq!(document.last_child(root), Some(a));
    }

    #[test]
    fn a_cycle_is_refused_rather_than_built() {
        let mut document = Document::new();
        let root = document.root();
        let (parent, child) = (element(&mut document, "p"), element(&mut document, "c"));
        document.attach_last(root, parent);
        document.attach_last(parent, child);

        assert!(
            !document.attach_last(child, parent),
            "an ancestor is refused"
        );
        assert!(!document.attach_last(parent, parent), "itself is refused");
        assert!(!document.attach_before(child, parent), "so is inserting it");
        assert_eq!(document.parent(parent), Some(root));
        assert_eq!(document.parent(child), Some(parent));
    }

    #[test]
    fn inserting_before_a_node_with_no_parent_is_refused() {
        let mut document = Document::new();
        let (a, b) = (element(&mut document, "a"), element(&mut document, "b"));
        assert!(!document.attach_before(a, b));
        assert_eq!(document.parent(b), None);
    }

    #[test]
    fn an_id_from_another_document_resolves_to_nothing() {
        let mut other = Document::new();
        let a = element(&mut other, "a");
        let document = Document::new();
        assert!(document.get(a).is_none());
        assert!(document.kind(a).is_none());
        assert!(document.element(a).is_none());
        assert_eq!(document.parent(a), None);
        assert!(!document.is_attached(a));
        assert!(document.children(a).next().is_none());
    }

    #[test]
    fn adjacent_text_becomes_one_node() {
        let mut document = Document::new();
        let root = document.root();
        document.append_text(root, "one ");
        document.append_text(root, "two");
        assert_eq!(document.children(root).count(), 1);
        assert_eq!(document.text_content(root), "one two");
    }

    #[test]
    fn text_inserted_before_a_sibling_merges_backwards() {
        let mut document = Document::new();
        let root = document.root();
        document.append_text(root, "one ");
        let marker = element(&mut document, "b");
        document.attach_last(root, marker);
        document.insert_text_before(marker, "two");
        assert_eq!(document.children(root).count(), 2);
        assert_eq!(document.text_content(root), "one two");
    }

    #[test]
    fn reparenting_moves_every_child_in_order() {
        let mut document = Document::new();
        let root = document.root();
        let (from, to) = (element(&mut document, "from"), element(&mut document, "to"));
        document.attach_last(root, from);
        document.attach_last(root, to);
        let children: Vec<_> = ["a", "b", "c"]
            .into_iter()
            .map(|local| {
                let id = element(&mut document, local);
                document.attach_last(from, id);
                id
            })
            .collect();

        document.reparent_children(from, to);
        assert_eq!(document.children(from).count(), 0);
        assert_eq!(document.children(to).collect::<Vec<_>>(), children);
    }

    #[test]
    fn descendants_are_in_document_order_and_exclude_the_start() {
        let mut document = Document::new();
        let root = document.root();
        let outer = element(&mut document, "outer");
        document.attach_last(root, outer);
        let inner = element(&mut document, "inner");
        document.attach_last(outer, inner);
        document.append_text(inner, "deep");
        let after = element(&mut document, "after");
        document.attach_last(outer, after);

        let names: Vec<String> = document
            .descendants(outer)
            .filter_map(|id| match document.kind(id) {
                Some(NodeKind::Element(e)) => Some(e.name.local.to_string()),
                Some(NodeKind::Text(t)) => Some(format!("{t:?}")),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["inner", "\"deep\"", "after"]);
    }

    #[test]
    fn attributes_already_present_are_kept_not_replaced() {
        let mut document = Document::new();
        let id = element(&mut document, "html");
        document.add_attrs_if_missing(id, vec![Attribute::plain("lang", "en")]);
        document.add_attrs_if_missing(
            id,
            vec![
                Attribute::plain("lang", "fr"),
                Attribute::plain("dir", "ltr"),
            ],
        );
        let element = document.element(id).unwrap();
        assert_eq!(element.attr("lang"), Some("en"));
        assert_eq!(element.attr("dir"), Some("ltr"));
        assert_eq!(element.attrs.len(), 2);
    }

    #[test]
    fn adding_attributes_to_a_text_node_changes_nothing() {
        let mut document = Document::new();
        let id = document.create(NodeKind::Text("x".to_owned()));
        document.add_attrs_if_missing(id, vec![Attribute::plain("lang", "en")]);
        assert_eq!(document.kind(id), Some(&NodeKind::Text("x".to_owned())));
    }

    #[test]
    fn the_quirks_signal_is_recorded_and_defaults_to_standards() {
        let mut document = Document::new();
        assert_eq!(document.quirks_signal(), QuirksSignal::NoQuirks);
        document.set_quirks_signal(QuirksSignal::Quirks);
        assert_eq!(document.quirks_signal(), QuirksSignal::Quirks);
    }
}
