/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A node, as a script holds it.
//!
//! ADR 0017 § 3: a wrapper is an embedder cell holding **the node's id, a
//! reference to the document cell, and an ordinary object's part** — its
//! prototype and its own properties, so a page may hang expandos off a node
//! as every page does. Everything it is as an object comes from that part;
//! everything it is as a node comes from asking the document cell about its
//! id.
//!
//! It holds its document **strongly**: a node a script holds keeps the page's
//! document, which is what makes `node.ownerDocument` an answer rather than a
//! hope. What keeps the wrapper is not here but in the document cell, which
//! traces it by the rule of its tree ([`crate::liveness`]).

use alo_dom::NodeId;
use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

/// A node's one object (ADR 0014 § 6: one per node for as long as it lives).
#[derive(Debug)]
pub struct Wrapper {
    node: NodeId,
    document: Field,
    own: Ordinary,
}

impl Wrapper {
    /// The wrapper of `node`, in the document `document` holds, inheriting
    /// from `prototype`.
    pub fn new(node: NodeId, document: Ref, prototype: Option<Ref>) -> Self {
        Self {
            node,
            document: Field::holding(document),
            own: Ordinary::with_prototype(prototype),
        }
    }

    /// The node this is the wrapper of.
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// The document cell that holds the node.
    pub const fn document(&self) -> Option<Ref> {
        self.document.get()
    }
}

impl Internal for Wrapper {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Wrapper {
    fn trace(&self, tracer: &mut Tracer) {
        self.document.trace(tracer);
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for Wrapper {
    fn describe(&self) -> &'static str {
        "a node"
    }
}
