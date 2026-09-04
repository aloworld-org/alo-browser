/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A symbol, which is a heap object because its identity *is* its value.
//!
//! Two symbols with the same description are different property keys, and the
//! only thing that can carry that distinction is a cell of its own: a slot in
//! the heap, named by a [`Ref`](crate::heap::Ref) that is equal to itself and
//! to nothing else. That is why a symbol is here rather than being a number
//! with a label — and it is the mirror image of a string, which is interned so
//! that the *same text* is the same key.
//!
//! # What a symbol is not, yet
//!
//! The well-known symbols (`Symbol.iterator` and the rest) are queue item 73's,
//! and the cross-realm registry behind `Symbol.for` is too. Both are objects a
//! realm holds rather than a change to what a symbol is, which is why neither
//! blocks this file: a well-known symbol is one of these, made once and rooted
//! by the realm that owns it.

use crate::heap::{Field, Ref, Tracer};

/// A symbol in the heap.
#[derive(Debug)]
pub struct Symbol {
    /// The text a person reads in a stack trace, if it was given one.
    ///
    /// A [`Field`] rather than a [`Text`](super::Text) held inline, because a
    /// description is an ordinary string that the same page may also be holding
    /// — and two copies of it would be two strings a page could not tell apart
    /// but the heap could.
    description: Field,
}

impl Symbol {
    /// A symbol with a description, which is a string cell, or without one.
    pub fn new(description: Option<Ref>) -> Self {
        Self {
            description: match description {
                Some(held) => Field::holding(held),
                None => Field::empty(),
            },
        }
    }

    /// The string cell describing it, if it has one.
    pub const fn description(&self) -> Option<Ref> {
        self.description.get()
    }

    /// Report the edge to the description.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.description.trace(tracer);
    }
}
