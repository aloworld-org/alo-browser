/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `String` object: an ordinary object with the `[[StringData]]` slot
//! beside it (queue item 488).
//!
//! The specification writes one as an exotic object whose index properties
//! and `length` are worked out from its string rather than stored. Every one
//! of them is fixed — not writable and not configurable — so storing them in
//! the ordinary table when the object is made answers every question the
//! same way: a definition that would change one is refused by
//! [`Property::may_replace`](super::Property::may_replace) exactly as
//! `IsCompatiblePropertyDescriptor` refuses it, a delete is refused because
//! none is configurable, and the table's order — indices ascending, then
//! names as they were made — is `[[OwnPropertyKeys]]`'s, `length` being the
//! first name. So this file is the slot and nothing else, as
//! [`Date`](super::Date) is, and `StringCreate` (which can allocate, where a
//! cell may not) is the builtin's.
//!
//! The slot holds a string cell, which is an edge: a `String` object keeps
//! its string alive.

use crate::heap::{Ref, Tracer};

use super::ordinary::Ordinary;

/// A `String` object.
#[derive(Debug)]
pub struct StringObject {
    ordinary: Ordinary,
    /// `[[StringData]]`: a string cell.
    data: Ref,
}

impl StringObject {
    /// A `String` object of `data` inheriting from `prototype`, with no
    /// properties yet: what `StringCreate` makes before it defines them.
    pub fn new(prototype: Option<Ref>, data: Ref) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            data,
        }
    }

    /// `[[StringData]]`.
    pub const fn data(&self) -> Ref {
        self.data
    }

    /// The ordinary object it also is.
    pub const fn ordinary(&self) -> &Ordinary {
        &self.ordinary
    }

    /// The same, to be written through.
    pub const fn ordinary_mut(&mut self) -> &mut Ordinary {
        &mut self.ordinary
    }

    /// Report every edge: the ordinary object's, and the string.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        tracer.edge(self.data);
    }

    /// What it owns beyond its slot, which is its table: the string is a cell
    /// of its own and is counted there.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }
}
