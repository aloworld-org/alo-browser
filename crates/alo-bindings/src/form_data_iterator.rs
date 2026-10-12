/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `FormData`'s iterator (queue item 472): Web IDL's *default iterator
//! object* for an interface iterable over pairs.
//!
//! It holds its **target**, the `FormData` it walks, and its **index**, the
//! next entry to hand out. Its kind is always *key+value*: `entries()` and
//! `[Symbol.iterator]` are the only ways to make one here, because `keys()`
//! and `values()` are absent until a page needs them. Its `next` is
//! [`crate::interface::form_data_iterator`]'s, and reads the target's list
//! **again at every step**, as Web IDL's does, so a list that changed while
//! it was walked is walked to its new end.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};

/// A `FormData`'s iterator.
#[derive(Debug)]
pub struct FormDataIterator {
    own: Ordinary,
    target: Field,
    index: usize,
}

impl FormDataIterator {
    /// One walking `target` from its first entry, inheriting from
    /// `prototype`.
    pub fn new(prototype: Ref, target: Ref) -> Self {
        Self {
            own: Ordinary::with_prototype(Some(prototype)),
            target: Field::holding(target),
            index: 0,
        }
    }

    /// The `FormData` it walks.
    pub fn target(&self) -> Option<Ref> {
        self.target.get()
    }

    /// The index of the next entry it hands out.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Move on past the entry it just handed out.
    pub(crate) fn advance(&mut self) {
        self.index = self.index.saturating_add(1);
    }
}

impl Internal for FormDataIterator {
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

impl Trace for FormDataIterator {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.target.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for FormDataIterator {
    fn describe(&self) -> &'static str {
        "a FormData Iterator"
    }
}
