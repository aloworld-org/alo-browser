/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How many bytes a document owns, so the heap it moves into can count them.
//!
//! ADR 0017 § 2: once a page runs script its document is one cell in the
//! page's heap, and *the cell's footprint is the document's size*. A node
//! lives in this arena rather than in a heap slot, so a script that made
//! nodes without the heap counting them could fill a renderer below the
//! heap's ceiling. Counted, a page that builds an unbounded document meets
//! the same ceiling an unbounded array does.
//!
//! # A sum kept, never a walk
//!
//! The heap measures a cell before and after every write to it, and every
//! change a script makes is one, so [`Document::footprint`] is asked once per
//! tree operation. A walk would make building a page of *n* nodes cost *n²*;
//! instead the document keeps the sum as nodes are made, changed and released
//! (`document.rs`), and this file says what one node counts for.
//!
//! # What is counted
//!
//! Lengths rather than capacities, so that a document and its clone answer
//! the same and a test can recount by hand: a node's own size and the box it
//! sits in, the text it holds, its element's name and every attribute's name
//! and value — and, for the arena, one slot per node ever made, which is what
//! a tombstone costs (ADR 0017 § 3). What the parser said about the markup is
//! not counted: it is bounded by the markup, and script cannot add to it.

use crate::document::Document;
use crate::name::QualifiedName;
use crate::node::{Attribute, Node, NodeKind};

impl Node {
    /// The bytes this node owns: itself, in its box, and every string it
    /// holds.
    pub fn footprint(&self) -> usize {
        let held = match &self.kind {
            NodeKind::Document | NodeKind::Fragment => 0,
            NodeKind::Doctype {
                name,
                public_id,
                system_id,
            } => name
                .len()
                .saturating_add(public_id.len())
                .saturating_add(system_id.len()),
            NodeKind::Element(element) => element.attrs.iter().fold(
                name_bytes(&element.name)
                    .saturating_add(element.attrs.len().saturating_mul(size_of::<Attribute>())),
                |sum, attribute| {
                    sum.saturating_add(name_bytes(&attribute.name))
                        .saturating_add(attribute.value.len())
                },
            ),
            NodeKind::Text(data) | NodeKind::Comment(data) => data.len(),
            NodeKind::ProcessingInstruction { target, data } => {
                target.len().saturating_add(data.len())
            }
        };
        size_of::<Node>().saturating_add(held)
    }
}

impl Document {
    /// The bytes this document owns: every live node's
    /// [`Node::footprint`], and one slot for every node it ever made.
    ///
    /// Kept as a sum, so asking costs nothing however large the document is.
    pub fn footprint(&self) -> usize {
        self.live_bytes()
            .saturating_add(self.node_count().saturating_mul(SLOT))
    }
}

/// What one slot of the arena costs, live or a tombstone.
const SLOT: usize = size_of::<Option<Box<Node>>>();

/// The bytes a name's strings hold.
fn name_bytes(name: &QualifiedName) -> usize {
    name.local
        .len()
        .saturating_add(name.prefix.as_ref().map_or(0, |prefix| prefix.len()))
}
