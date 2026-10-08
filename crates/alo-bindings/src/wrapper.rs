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
//! And it holds the node's **listener list** (ADR 0018 § 1,
//! [`crate::listeners`]): a listener is held by its target's wrapper, so it
//! lives exactly as long as the wrapper does.
//!
//! And while the node's `click()` runs, its `Clicking`: HTML's click in
//! progress flag, and what that native made and must keep across its
//! listeners (queue item 261).
//!
//! And, once a page has read it, the element's `classList` (queue item 327,
//! [`crate::token_list`]): `[SameObject]`, so made once and kept here — and
//! its `style` (queue item 342, [`crate::style_declaration`]), the same way.
//!
//! It holds its document **strongly**: a node a script holds keeps the page's
//! document, which is what makes `node.ownerDocument` an answer rather than a
//! hope. What keeps the wrapper is not here but in the document cell, which
//! traces it by the rule of its tree ([`crate::liveness`]).

use alo_dom::NodeId;
use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

use crate::clicking::Clicking;
use crate::listeners::Listeners;

/// A node's one object (ADR 0014 § 6: one per node for as long as it lives).
#[derive(Debug)]
pub struct Wrapper {
    node: NodeId,
    document: Field,
    own: Ordinary,
    listeners: Listeners,
    clicking: Option<Clicking>,
    class_list: Field,
    style: Field,
}

impl Wrapper {
    /// The wrapper of `node`, in the document `document` holds, inheriting
    /// from `prototype`.
    pub fn new(node: NodeId, document: Ref, prototype: Option<Ref>) -> Self {
        Self {
            node,
            document: Field::holding(document),
            own: Ordinary::with_prototype(prototype),
            listeners: Listeners::default(),
            clicking: None,
            class_list: Field::default(),
            style: Field::default(),
        }
    }

    /// Its node's listeners.
    pub const fn listeners(&self) -> &Listeners {
        &self.listeners
    }

    /// The same, to change — each change through the barrier its methods
    /// take.
    pub const fn listeners_mut(&mut self) -> &mut Listeners {
        &mut self.listeners
    }

    /// Its node's `click()` in progress, if there is one.
    pub const fn clicking(&self) -> Option<&Clicking> {
        self.clicking.as_ref()
    }

    /// The same, to change.
    pub(crate) const fn clicking_mut(&mut self) -> Option<&mut Clicking> {
        self.clicking.as_mut()
    }

    /// Begin its node's `click()`: the click in progress flag set.
    pub(crate) fn begin_click(&mut self, clicking: Clicking) {
        self.clicking = Some(clicking);
    }

    /// End its node's `click()`: the flag unset and what it held let go.
    pub(crate) fn end_click(&mut self, barrier: &mut Barrier) {
        if let Some(mut clicking) = self.clicking.take() {
            clicking.let_go(barrier);
        }
    }

    /// Its element's `classList`, once one has been made.
    pub const fn class_list(&self) -> Option<Ref> {
        self.class_list.get()
    }

    /// Keep `list` as its element's `classList`, through the barrier.
    pub(crate) fn keep_class_list(&mut self, barrier: &mut Barrier, list: Ref) {
        self.class_list.set(barrier, Some(list));
    }

    /// Its element's `style`, once one has been made.
    pub const fn style(&self) -> Option<Ref> {
        self.style.get()
    }

    /// Keep `declaration` as its element's `style`, through the barrier.
    pub(crate) fn keep_style(&mut self, barrier: &mut Barrier, declaration: Ref) {
        self.style.set(barrier, Some(declaration));
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
        self.listeners.trace(tracer);
        self.class_list.trace(tracer);
        self.style.trace(tracer);
        if let Some(clicking) = &self.clicking {
            clicking.trace(tracer);
        }
    }

    fn footprint(&self) -> usize {
        self.own
            .footprint()
            .saturating_add(self.listeners.footprint())
            .saturating_add(self.clicking.as_ref().map_or(0, Clicking::footprint))
    }
}

impl Exotic for Wrapper {
    fn describe(&self) -> &'static str {
        "a node"
    }
}
