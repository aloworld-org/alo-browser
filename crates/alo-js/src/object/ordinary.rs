/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The ordinary object: a prototype, a property table and one flag.
//!
//! ADR 0014 § 11's first clause, and there is nothing else in it. Every other
//! object in the language — an array, a function, a proxy, an element — is this
//! plus internal methods of its own, which is why [`Internal`] is a trait and
//! why this is its first implementation rather than its only shape.
//!
//! # Three fields, and why the third is not a property
//!
//! `extensible` is a flag rather than a property because a page must not be
//! able to name it: `Object.preventExtensions` sets it and `Object.isExtensible`
//! reads it, and there is no key that reaches it. A property called
//! `"[[Extensible]]"` would be one an author could collide with.
//!
//! The prototype is a [`Field`], so it is stored through the barrier like every
//! other reference in a cell (ADR 0014 § 5) — `Object.setPrototypeOf` in a loop
//! is a mutation the collector will need to hear about the day marking becomes
//! incremental.
//!
//! # And what is not here
//!
//! Whether the prototype **may** be set — the cycle, the extensibility — is
//! [`access`](super::access)'s, because answering it means walking the heap and
//! a cell cannot see past itself. This file is the storage; that file is the
//! rules.

use crate::heap::{Barrier, Field, Ref, Tracer};

use super::internal::Internal;
use super::key::Key;
use super::property::Property;
use super::table::Properties;

/// An ordinary object.
#[derive(Debug)]
pub struct Ordinary {
    prototype: Field,
    properties: Properties,
    extensible: bool,
}

impl Ordinary {
    /// An object with the given prototype and no properties of its own.
    ///
    /// [`None`] is a null prototype: `Object.create(null)`, and also
    /// `Object.prototype` itself, which is the end of every chain.
    pub fn with_prototype(prototype: Option<Ref>) -> Self {
        Self {
            prototype: match prototype {
                Some(held) => Field::holding(held),
                None => Field::empty(),
            },
            properties: Properties::new(),
            extensible: true,
        }
    }

    /// How many own properties it has.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Report every edge: the prototype, and everything the table holds.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.prototype.trace(tracer);
        self.properties.trace(tracer);
    }

    /// What it owns beyond its slot, which is its table.
    pub fn footprint(&self) -> usize {
        self.properties.footprint()
    }
}

impl Internal for Ordinary {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.properties.get(key)
    }

    fn own_property_mut(&mut self, key: Key) -> Option<&mut Property> {
        self.properties.get_mut(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        // A key that is a reference is a reference this object now holds, so it
        // is a store like any other and the barrier hears about it. The
        // property's own value went through [`Stored`](super::Stored) when it
        // was built, which is the case ADR 0014 § 5 calls a hole in the wording
        // rather than in the barrier: it was not in the heap yet.
        barrier.stored(None, key.reference());
        self.properties.put(key, property);
        true
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.properties.remove(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.properties.keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.prototype.get()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.prototype.set(barrier, to);
        true
    }

    fn is_extensible(&self) -> bool {
        self.extensible
    }

    fn prevent_extensions(&mut self) -> bool {
        self.extensible = false;
        true
    }
}
