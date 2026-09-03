/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Nodes: what one is, what it holds, and how it is named.
//!
//! A node's links to its neighbours live here alongside its content, but they
//! are private: the tree is edited through [`crate::Document`], which is the
//! only thing that can keep the links consistent.

use crate::name::QualifiedName;
use core::fmt;

/// The identity of a node within one [`crate::Document`].
///
/// Allocated once, in creation order, and **never reused** — see ADR 0003. A
/// node that is detached from the tree keeps its id, and no later node is ever
/// given that id, so a stale id names a node that is gone rather than a
/// different node that is not. ADR 0002's agent tree has to be able to name a
/// node and come back to it, and identity that could be recycled would make
/// "come back to it" quietly wrong.
///
/// An id is meaningful only in the document that minted it. Passing one to a
/// different document returns [`None`] rather than another document's node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    /// The id as a number, for diagnostics and test assertions.
    ///
    /// This is not an index into anything a caller can address; it is the
    /// order in which the node was created.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// An attribute: a qualified name and its value, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// The attribute's name.
    pub name: QualifiedName,
    /// The attribute's value, with character references already resolved.
    pub value: String,
}

impl Attribute {
    /// An attribute with no namespace — how nearly every attribute is written.
    pub fn plain(local: &str, value: &str) -> Self {
        Self {
            name: QualifiedName::new(crate::name::Namespace::None, local),
            value: value.to_owned(),
        }
    }
}

/// An element: its name, its attributes, and the two facts the parser records
/// about it that later stages need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The element's qualified name.
    pub name: QualifiedName,
    /// The element's attributes, in the order they were written. Order is kept
    /// so that serialising a document reproduces it, which is what makes a
    /// round-trip test meaningful.
    pub attrs: Vec<Attribute>,
    /// For a `<template>`, the separate fragment holding its contents. The
    /// contents of a template are not children of the template.
    pub template_contents: Option<NodeId>,
    /// Whether this is a MathML `annotation-xml` element that is an HTML
    /// integration point. The parser asks us this back while parsing, so we
    /// have to remember what it told us.
    pub mathml_annotation_xml_integration_point: bool,
}

impl Element {
    /// The value of the first attribute with this local name and no namespace.
    pub fn attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name.is_plain(local))
            .map(|a| &*a.value)
    }

    /// Set an attribute, adding it if it is not there.
    ///
    /// **Where it already exists it keeps its place**, so that setting a
    /// value does not shuffle the attribute order and change what serialising
    /// the document produces. A round-trip test is only meaningful while that
    /// is true.
    pub fn set_attr(&mut self, local: &str, value: &str) {
        match self.attrs.iter_mut().find(|a| a.name.is_plain(local)) {
            Some(held) => value.clone_into(&mut held.value),
            None => self.attrs.push(Attribute::plain(local, value)),
        }
    }

    /// Take an attribute away, and say whether there was one.
    ///
    /// Every one with the name, not the first: a document can be handed to us
    /// with a repeated attribute, and leaving the second behind would make
    /// removing look as though it had not worked.
    pub fn remove_attr(&mut self, local: &str) -> bool {
        let before = self.attrs.len();
        self.attrs.retain(|a| !a.name.is_plain(local));
        self.attrs.len() != before
    }
}

/// What a node *is*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// The root of a document.
    Document,
    /// A detached fragment root: a `<template>`'s contents.
    Fragment,
    /// A doctype. Its public and system identifiers are kept even though the
    /// HTML serialiser does not write them, because dropping what the author
    /// wrote is how a later stage ends up re-parsing.
    Doctype {
        /// The doctype name, normally `html`.
        name: String,
        /// The public identifier, empty when absent.
        public_id: String,
        /// The system identifier, empty when absent.
        system_id: String,
    },
    /// An element.
    Element(Element),
    /// Character data. Adjacent text is merged into one node by the parser, so
    /// two text nodes are never siblings.
    Text(String),
    /// A comment.
    Comment(String),
    /// A processing instruction.
    ProcessingInstruction {
        /// The target name.
        target: String,
        /// Everything after the target.
        data: String,
    },
}

/// A node, and its place among its neighbours.
#[derive(Debug, Clone)]
pub struct Node {
    /// What this node is.
    pub kind: NodeKind,
    pub(crate) parent: Option<NodeId>,
    pub(crate) first_child: Option<NodeId>,
    pub(crate) last_child: Option<NodeId>,
    pub(crate) previous_sibling: Option<NodeId>,
    pub(crate) next_sibling: Option<NodeId>,
}

impl Node {
    pub(crate) fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            parent: None,
            first_child: None,
            last_child: None,
            previous_sibling: None,
            next_sibling: None,
        }
    }

    /// The element this node is, if it is one.
    pub fn element(&self) -> Option<&Element> {
        match &self.kind {
            NodeKind::Element(element) => Some(element),
            _ => None,
        }
    }

    /// The character data this node holds, if it is a text node.
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Whether this node is an element named `local` in the HTML namespace.
    pub fn is_html_element(&self, local: &str) -> bool {
        self.element().is_some_and(|e| e.name.is_html(local))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_prints_as_itself() {
        assert_eq!(NodeId(7).to_string(), "#7");
        assert_eq!(NodeId(7).as_usize(), 7);
    }

    #[test]
    fn asking_a_text_node_for_an_element_returns_none() {
        let node = Node::new(NodeKind::Text("hello".to_owned()));
        assert_eq!(node.text(), Some("hello"));
        assert!(node.element().is_none());
        assert!(!node.is_html_element("div"));
    }

    #[test]
    fn asking_an_element_for_a_missing_attribute_returns_none() {
        let element = Element {
            name: QualifiedName::html("input"),
            attrs: vec![Attribute::plain("type", "text")],
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
        };
        assert_eq!(element.attr("type"), Some("text"));
        assert_eq!(element.attr("value"), None);

        let node = Node::new(NodeKind::Element(element));
        assert!(node.is_html_element("input"));
        assert!(!node.is_html_element("div"));
        assert!(node.text().is_none());
    }
}
