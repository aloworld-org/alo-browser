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
//!
//! # Nothing goes while the parser is still building
//!
//! A script runs at its own end tag, with the rest of the markup still to
//! read (ADR 0017 § 7), and the parser holds nodes of its own: the elements
//! it has open, which it goes on inserting into whether or not a script has
//! since taken them out of the document — the HTML standard's parser does
//! exactly that. No wrapper marks them, so a collection would see a detached
//! `<body>` nobody holds and release it, and the parser would then be asking
//! a tombstone its name. Which nodes the parser holds is html5ever's to know
//! and not to tell, so **while a document is being parsed, no tree is
//! released at all**: what a script drops during a page's load is let go at
//! the first collection after it, and is counted against the heap's ceiling
//! until then, like everything else.

use crate::document::Document;
use crate::node::NodeId;

impl Document {
    /// Release the detached tree whose root is `root`, and say how many nodes
    /// went: the root, everything beneath it, and the contents of every
    /// `<template>` in it, which are part of its tree.
    ///
    /// [`None`], releasing nothing, while the document is being parsed (see
    /// the module), and unless `root` is the root of a detached tree: not the
    /// document, not a node with a parent, not a template's contents (they go
    /// with their template), and not an id that already answers nothing.
    /// Releasing changes no tree anything can see, so it
    /// does not advance [`Document::change_count`].
    ///
    /// **It allocates nothing**, because the bindings call it from inside a
    /// collection's sweep, and ADR 0014 § 8 says a collection allocates
    /// nothing it has not already got. So it keeps no list of what is left to
    /// visit: it walks down unlinking each child as it takes it, releases a
    /// node once it has no children left, and climbs back by the parent
    /// link, which costs two steps per node whatever the tree's shape.
    pub fn release(&mut self, root: NodeId) -> Option<usize> {
        if self.is_being_parsed()
            || root == self.root()
            || self.get(root).is_none()
            || self.parent(root).is_some()
            || self.host(root).is_some()
        {
            return None;
        }
        let mut released = 0_usize;
        let mut current = root;
        loop {
            // Down first: a template's contents, then its children, each
            // unlinked as it is taken so that nothing is visited twice and
            // nothing needs remembering on the way back up.
            if let Some(contents) = self
                .edit_element(current, |element| element.template_contents.take())
                .flatten()
                && self.get(contents).is_some()
            {
                current = contents;
                continue;
            }
            if let Some(child) = self.take_first_child(current) {
                if self.get(child).is_some() {
                    current = child;
                }
                continue;
            }
            // A leaf now: it goes, and the walk climbs to what held it.
            let up = if current == root {
                None
            } else {
                self.parent(current).or_else(|| self.host(current))
            };
            self.tombstone(current);
            released = released.saturating_add(1);
            match up {
                Some(up) => current = up,
                None => break,
            }
        }
        Some(released)
    }

    /// The first root of a detached tree whose id comes after `after` — or
    /// from the start, for [`None`] — in the order the nodes were made.
    ///
    /// A cursor rather than an iterator, so that a caller can release each
    /// tree as it finds it: the bindings walk every detached tree at a
    /// collection's sweep, where nothing may be allocated to remember a list
    /// (ADR 0014 § 8). A root is what [`Document::release`] accepts: live,
    /// not the document, with no parent and not a template's contents.
    pub fn next_detached_root(&self, after: Option<NodeId>) -> Option<NodeId> {
        let from = after.map_or(0, |id| id.0.saturating_add(1));
        (from..self.node_count()).map(NodeId).find(|id| {
            *id != self.root()
                && self.get(*id).is_some()
                && self.parent(*id).is_none()
                && self.host(*id).is_none()
        })
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
