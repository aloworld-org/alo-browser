/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's document, in its heap.
//!
//! ADR 0017 § 2: when a page's first script is about to run, its document
//! moves into the page's heap as **one embedder cell** — this — and has that
//! one owner for the rest of the page's life. Style, layout, paint and the
//! agent borrow it from here for as long as a read lasts, and a change is a
//! write to this cell, so it is an ordinary Rust borrow out of the heap: no
//! second owner, no shared pointer, no runtime flag to fail.
//!
//! The cell also keeps **the table** from node to wrapper, which is what makes
//! asking for a node twice give one object (ADR 0014 § 6), and it is the cell
//! whose tracing keeps each wrapper as long as its tree — that rule is
//! [`crate::liveness`], kept apart because it is the one part of this that a
//! collection runs.
//!
//! Its footprint is the document's size (§ 2): a node lives in `alo-dom`'s
//! arena rather than in a heap slot, and a script that made nodes the heap did
//! not count could fill a renderer below the heap's ceiling.
//!
//! It is an object only because everything in the heap that is not the
//! engine's own is one. No script is ever handed it — the document *node* a
//! page sees is a wrapper like any other — so as an object it is the plainest
//! there is: no prototype, no properties, and it refuses any.

use alo_dom::{Document, NodeId};
use alo_js::heap::{Barrier, Ref};
use alo_js::object::{Exotic, Internal, Key, Property};

/// One slot of the table: the node, and its wrapper.
pub(crate) type Entry = Option<(NodeId, Ref)>;

/// A page's document, as one cell of its heap.
#[derive(Debug)]
pub struct DocumentCell {
    pub(crate) document: Document,
    /// The wrapper of each node that has one, at the node's id.
    pub(crate) table: Vec<Entry>,
    /// A node whose wrapper is being made: its tree is kept by the collection
    /// that making the wrapper may cause, since nothing else holds it yet.
    pub(crate) pending: Option<NodeId>,
    pub(crate) released: Released,
}

/// What collections have let go of, counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Released {
    /// Detached trees released, each because no script held any node in it.
    pub trees: u64,
    /// The nodes those trees held.
    pub nodes: u64,
}

impl DocumentCell {
    /// The cell `document` becomes.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            table: Vec::new(),
            pending: None,
            released: Released::default(),
        }
    }

    /// The document, to read.
    pub const fn document(&self) -> &Document {
        &self.document
    }

    /// The document, to change through `alo-dom`'s own operations (ADR 0017
    /// § 5), whose validity rules hold for every caller alike.
    pub const fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// The wrapper `node` has, if it has one.
    pub fn wrapper(&self, node: NodeId) -> Option<Ref> {
        self.table
            .get(node.as_usize())
            .copied()
            .flatten()
            .map(|(_, wrapper)| wrapper)
    }

    /// How many nodes have a wrapper.
    pub fn wrapped(&self) -> usize {
        self.table.iter().flatten().count()
    }

    /// What collections have let go of, so far.
    pub const fn released(&self) -> Released {
        self.released
    }

    /// Record that `wrapper` is `node`'s, through the barrier every store of
    /// a reference passes (ADR 0014 § 5).
    pub(crate) fn remember(&mut self, barrier: &mut Barrier, node: NodeId, wrapper: Ref) {
        let at = node.as_usize();
        if self.table.len() <= at {
            self.table
                .resize(self.document.node_count().max(at.saturating_add(1)), None);
        }
        if let Some(slot) = self.table.get_mut(at) {
            barrier.stored(slot.map(|(_, was)| was), Some(wrapper));
            *slot = Some((node, wrapper));
        }
    }
}

impl Internal for DocumentCell {
    fn own_property(&self, _key: Key) -> Option<&Property> {
        None
    }

    fn define_own(&mut self, _barrier: &mut Barrier, _key: Key, _property: Property) -> bool {
        false
    }

    fn delete_own(&mut self, _key: Key) -> bool {
        // There is nothing to delete, and deleting what is not there succeeds.
        true
    }

    fn own_keys(&self) -> Vec<Key> {
        Vec::new()
    }

    fn prototype(&self) -> Option<Ref> {
        None
    }

    fn set_prototype(&mut self, _barrier: &mut Barrier, to: Option<Ref>) -> bool {
        to.is_none()
    }

    fn is_extensible(&self) -> bool {
        false
    }

    fn prevent_extensions(&mut self) -> bool {
        true
    }
}

impl Exotic for DocumentCell {
    fn describe(&self) -> &'static str {
        "a page's document"
    }
}
