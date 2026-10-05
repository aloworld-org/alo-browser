/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Changing a tree's shape, under the DOM standard's names and rules.
//!
//! These are the operations a script's `appendChild` and an agent's change
//! both come down to (ADR 0017 § 5). Each asks [`crate::validity`] first and
//! changes nothing when it is refused; each that succeeds advances
//! [`Document::change_count`] by one, however many nodes it moved.
//!
//! A node made here takes its id from the same counter the parser's nodes
//! did (ADR 0003), so on a page of forty parsed nodes the first element a
//! script makes is `#40`, and an agent can name it like any other.
//!
//! The parser does not come through here: the HTML parsing algorithm builds
//! trees the standard's operations would refuse, repairs instead of
//! refusing, and keeps its own crate-private operations in
//! [`crate::document`].

use crate::document::Document;
use crate::name::QualifiedName;
use crate::node::{Element, NodeId, NodeKind};
use crate::validity::{self, Refusal};

impl Document {
    /// Make an HTML element named `local`, in no tree — `createElement`.
    ///
    /// The name is ASCII-lowercased, as the standard does for an HTML
    /// document, and refused with `InvalidCharacterError` when it is not a
    /// valid element local name. A `<template>` is made with its contents
    /// fragment, numbered after it.
    ///
    /// # Errors
    ///
    /// [`Refusal::InvalidCharacter`] for a name no element can have; nothing
    /// is made.
    pub fn create_element(&mut self, local: &str) -> Result<NodeId, Refusal> {
        if !validity::is_valid_element_local_name(local) {
            return Err(Refusal::InvalidCharacter(
                "an element's name must be a valid element local name",
            ));
        }
        let local = local.to_ascii_lowercase();
        let is_template = local == "template";
        let element = self.create(NodeKind::Element(Element {
            name: QualifiedName::html(&local),
            attrs: Vec::new(),
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
            had_duplicate_attributes: false,
        }));
        if is_template {
            let contents = self.create(NodeKind::Fragment);
            self.set_host(contents, element);
            self.edit_element(element, |made| made.template_contents = Some(contents));
        }
        Ok(element)
    }

    /// Make a text node holding `data`, in no tree — `createTextNode`.
    pub fn create_text_node(&mut self, data: &str) -> NodeId {
        self.create(NodeKind::Text(data.to_owned()))
    }

    /// Put `node` into `parent` before `child`, or at the end when `child` is
    /// [`None`] — the standard's *pre-insert*, behind `insertBefore`.
    ///
    /// The node is taken from wherever it was first. A fragment is not
    /// inserted itself: its children are, in order, and it is left empty.
    /// Answers the node inserted.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] the standard's pre-insertion validity names; the tree
    /// is left exactly as it was.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        node: NodeId,
        child: Option<NodeId>,
    ) -> Result<NodeId, Refusal> {
        validity::ensure_pre_insertion(self, node, parent, child)?;
        let reference = if child == Some(node) {
            self.next_sibling(node)
        } else {
            child
        };
        self.insert(node, parent, reference);
        self.note_change();
        Ok(node)
    }

    /// Put `node` at the end of `parent` — `appendChild`.
    ///
    /// # Errors
    ///
    /// As [`Document::insert_before`].
    pub fn append_child(&mut self, parent: NodeId, node: NodeId) -> Result<NodeId, Refusal> {
        self.insert_before(parent, node, None)
    }

    /// Put `node` where `child` is in `parent`, and take `child` out —
    /// `replaceChild`. Answers the node replaced.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] the standard's *replace a child* names; the tree is
    /// left exactly as it was.
    pub fn replace_child(
        &mut self,
        parent: NodeId,
        node: NodeId,
        child: NodeId,
    ) -> Result<NodeId, Refusal> {
        validity::ensure_replacement(self, node, child, parent)?;
        let mut reference = self.next_sibling(child);
        if reference == Some(node) {
            reference = self.next_sibling(node);
        }
        self.detach(child);
        self.insert(node, parent, reference);
        self.note_change();
        Ok(child)
    }

    /// Take `child` out of `parent` — `removeChild`, the standard's
    /// *pre-remove*. Refused with `NotFoundError` unless `child` is a child
    /// of `parent`. Answers the node removed, which keeps its id and its own
    /// children.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFound`] when `child` is not a child of `parent`.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<NodeId, Refusal> {
        if self.get(parent).is_none() || self.parent(child) != Some(parent) {
            return Err(Refusal::NotFound(
                "the node to remove is not a child of this parent",
            ));
        }
        self.detach(child);
        self.note_change();
        Ok(child)
    }

    /// Take `node` out of whatever holds it — `ChildNode.remove()`.
    ///
    /// A node nothing holds is left alone, as the standard says; that is not
    /// a refusal, and it is not a change. Answers whether anything moved.
    pub fn remove(&mut self, node: NodeId) -> bool {
        if self.parent(node).is_none() {
            return false;
        }
        self.detach(node);
        self.note_change();
        true
    }

    /// The standard's *insert*, once validity is settled: `node`, or a
    /// fragment's children, into `parent` before `reference`.
    fn insert(&mut self, node: NodeId, parent: NodeId, reference: Option<NodeId>) {
        let nodes: Vec<NodeId> = if matches!(self.kind(node), Some(NodeKind::Fragment)) {
            self.children(node).collect()
        } else {
            vec![node]
        };
        for moving in nodes {
            self.detach(moving);
            // Validity has already refused every cycle, which is all the
            // arena's own `attach` refuses, so neither answers "no" here.
            match reference {
                Some(reference) => self.attach_before(reference, moving),
                None => self.attach_last(parent, moving),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{NodeId, parse_document};

    #[test]
    fn a_made_element_is_numbered_after_every_parsed_node() {
        let mut document = parse_document(&format!(
            "<!DOCTYPE html><html><head></head><body>{}",
            "<i></i>".repeat(35)
        ));
        assert_eq!(document.node_count(), 40, "the page is forty nodes");
        let made = document.create_element("DIV").unwrap();
        assert_eq!(made.to_string(), "#40");
        let element = document.element(made).unwrap();
        assert!(element.name.is_html("div"), "the name is lowercased");
        assert_eq!(document.create_text_node("x"), NodeId(41));
        assert_eq!(document.change_count(), 0, "making a node changes no tree");
    }

    #[test]
    fn a_made_template_has_contents_that_belong_to_it() {
        let mut document = parse_document("");
        let template = document.create_element("template").unwrap();
        let contents = document
            .element(template)
            .and_then(|e| e.template_contents)
            .unwrap();
        assert_eq!(contents.as_usize(), template.as_usize() + 1);
        assert_eq!(document.host(contents), Some(template));
        assert_eq!(document.host(template), None);
    }
}
