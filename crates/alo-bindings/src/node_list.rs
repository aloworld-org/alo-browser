/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A **static** `NodeList`, as `querySelectorAll` answers it (queue item
//! 329).
//!
//! An embedder cell holding the wrappers of the nodes it was made with, in
//! order, and an ordinary object's part. Static: the list is what matched
//! when it was made and never changes after, whatever the document does —
//! the live collections are stage 3's (ADR 0017 § 8), and nothing here
//! recomputes anything.
//!
//! # It answers its indices itself
//!
//! Web IDL makes a `NodeList` a **legacy platform object** with an indexed
//! getter, and this cell is that object's own-property internal methods:
//!
//! - `[[GetOwnProperty]]` of an index below the length is the node there, a
//!   data property that is enumerable and configurable and **not
//!   writable**; any other key is the ordinary part's.
//! - `[[DefineOwnProperty]]` of any array index is refused, since a
//!   `NodeList` has no indexed setter — so `list[0] = x` changes nothing
//!   and `list[9] = x` stores nothing, both a `TypeError` in strict code.
//! - `[[Delete]]` of an index below the length is refused; of one past it,
//!   there was nothing to delete.
//! - `[[OwnPropertyKeys]]` is the indices first, then the ordinary part's.
//! - `[[PreventExtensions]]` is refused.
//!
//! Each node's property is made once, when the list is, so a read hands
//! back a reference to it as every other object's does.
//!
//! The list holds each wrapper **strongly**: a page holding the list keeps
//! the nodes in it, and through them their trees (ADR 0017 § 3).

use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::Interface;

/// A static `NodeList`.
#[derive(Debug)]
pub struct NodeList {
    own: Ordinary,
    /// The node at each index, as the data property that index answers.
    items: Vec<Property>,
}

impl NodeList {
    /// How many nodes it holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The wrapper at `index`, if there is one.
    pub fn get(&self, index: usize) -> Option<Ref> {
        match self.items.get(index)?.value()? {
            Value::Object(wrapper) => Some(wrapper),
            _ => None,
        }
    }

    /// Whether `key` is one of its supported property indices.
    fn supports(&self, key: Key) -> bool {
        key.as_index()
            .and_then(|at| usize::try_from(at).ok())
            .is_some_and(|at| at < self.items.len())
    }
}

impl Internal for NodeList {
    fn own_property(&self, key: Key) -> Option<&Property> {
        match key.as_index() {
            Some(at) => match usize::try_from(at).ok().and_then(|at| self.items.get(at)) {
                Some(item) => Some(item),
                None => self.own.own_property(key),
            },
            None => self.own.own_property(key),
        }
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        if key.as_index().is_some() {
            return false;
        }
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        if self.supports(key) {
            return false;
        }
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        let indices = (0..self.items.len())
            .filter_map(|at| u32::try_from(at).ok())
            .filter_map(Key::index);
        indices.chain(self.own.own_keys()).collect()
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
        false
    }
}

impl Trace for NodeList {
    fn trace(&self, tracer: &mut Tracer) {
        for item in &self.items {
            item.trace(tracer);
        }
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        let items = self
            .items
            .capacity()
            .saturating_mul(core::mem::size_of::<Property>());
        self.own.footprint().saturating_add(items)
    }
}

impl Exotic for NodeList {
    fn describe(&self) -> &'static str {
        "a NodeList"
    }
}

/// A static `NodeList` of `wrappers`, in order, inheriting from
/// `NodeList.prototype` in the document `owner` holds, for a member at
/// source position `at`.
///
/// **A safepoint.** Every wrapper must be held by the caller until this
/// answers, and the list it answers held by the caller after.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling; a fault when `owner` is not a
/// document cell or the interfaces were never made.
pub(crate) fn made(
    objects: &mut Objects,
    owner: Ref,
    wrappers: &[Ref],
    at: usize,
) -> Result<Ref, Escape> {
    let prototype = objects
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .interfaces()
        .prototype(Interface::NodeList)
        .ok_or(Escape::fault(Fault::Gone))?;
    let items = wrappers
        .iter()
        .map(|wrapper| Property::data(Value::Object(*wrapper), false, true, true))
        .collect();
    let list = NodeList {
        own: Ordinary::with_prototype(Some(prototype)),
        items,
    };
    objects
        .foreign(Box::new(list))
        .map_err(|why| Escape::refused(why, at))
}
