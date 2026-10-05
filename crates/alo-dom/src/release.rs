/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Letting go of a detached tree nothing can reach any more.
//!
//! With the parser as the only builder a detached node cost at most what the
//! markup did, so ADR 0003 kept every one. A script can make nodes without
//! end, so ADR 0017 § 3 frees the trees it drops: the bindings decide, at a
//! collection, that no script holds any node of a detached tree, and release
//! it here. Its nodes' contents are dropped and their slots become
//! tombstones, so **their ids answer nothing and are never given out again**
//! — ADR 0003's promise, kept at the level it was made.

use crate::document::Document;
use crate::node::NodeId;
use core::iter;

impl Document {
    /// Release the detached tree whose root is `root`, and say how many nodes
    /// went: the root, everything beneath it, and the contents of every
    /// `<template>` in it, which are part of its tree.
    ///
    /// [`None`], releasing nothing, unless `root` is the root of a detached
    /// tree: not the document, not a node with a parent, not a template's
    /// contents (they go with their template), and not an id that already
    /// answers nothing. Releasing changes no tree anything can see, so it
    /// does not advance [`Document::change_count`].
    pub fn release(&mut self, root: NodeId) -> Option<usize> {
        if root == self.root()
            || self.get(root).is_none()
            || self.parent(root).is_some()
            || self.host(root).is_some()
        {
            return None;
        }
        let mut released = 0_usize;
        let mut pending = vec![root];
        while let Some(top) = pending.pop() {
            // A node already released has no descendants to find, so a
            // template that somehow reached its own tree twice ends here
            // rather than going round.
            let tree: Vec<NodeId> = iter::once(top)
                .filter(|id| self.get(*id).is_some())
                .chain(self.descendants(top))
                .collect();
            for id in &tree {
                if let Some(contents) = self.element(*id).and_then(|e| e.template_contents) {
                    pending.push(contents);
                }
            }
            for id in tree {
                self.tombstone(id);
                released = released.saturating_add(1);
            }
        }
        Some(released)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Document, NodeKind};

    #[test]
    fn a_released_tree_answers_nothing_and_its_ids_stay_spent() {
        let mut document = Document::new();
        let outer = document.create_element("div").unwrap();
        let inner = document.create_element("p").unwrap();
        let text = document.create_text_node("gone");
        document.append_child(outer, inner).unwrap();
        document.append_child(inner, text).unwrap();
        let highest = document.node_count();
        let changes = document.change_count();

        assert_eq!(document.release(outer), Some(3));
        for id in [outer, inner, text] {
            assert!(document.get(id).is_none(), "{id} answers nothing");
            assert_eq!(document.parent(id), None);
            assert!(document.children(id).next().is_none());
        }
        assert_eq!(document.release(outer), None, "it is gone already");
        assert_eq!(document.change_count(), changes, "not a change");
        assert_eq!(document.node_count(), highest, "the slots are kept");
        assert_eq!(document.create_text_node("next").as_usize(), highest);
    }

    #[test]
    fn a_template_takes_its_contents_with_it() {
        let mut document = Document::new();
        let template = document.create_element("template").unwrap();
        let contents = document
            .element(template)
            .and_then(|e| e.template_contents)
            .unwrap();
        let inside = document.create_element("b").unwrap();
        document.append_child(contents, inside).unwrap();

        assert_eq!(document.release(contents), None, "it is its template's");
        assert_eq!(document.release(template), Some(3));
        assert!(document.get(contents).is_none());
        assert!(document.get(inside).is_none());
    }

    #[test]
    fn nothing_attached_is_released() {
        let mut document = Document::new();
        let root = document.root();
        let child = document.create_element("html").unwrap();
        document.append_child(root, child).unwrap();

        assert_eq!(document.release(root), None, "the document");
        assert_eq!(document.release(child), None, "a node with a parent");
        assert_eq!(document.kind(root), Some(&NodeKind::Document));
        assert!(document.is_attached(child));
    }
}
