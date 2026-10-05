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
//! # The ones the language names, and the ones it does not yet
//!
//! A well-known symbol is one of these, made once and rooted by the realm that
//! owns it — not a change to what a symbol is. [`WellKnown`] names the two
//! this engine makes (queue item 230): `Symbol.iterator`, which `for…of`
//! calls, and `Symbol.toStringTag`, which `Object.prototype.toString` reads.
//! The other eleven, the `Symbol` function that would let a page name any of
//! them, and the cross-realm registry behind `Symbol.for` are queue item 73's.

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

/// A symbol the language itself names, made once per realm (queue item 230).
///
/// Only the two something here reads. Each is a key on an intrinsic, and the
/// realm's [`Intrinsics`](crate::builtin::Intrinsics) hold one of each,
/// rooted, for as long as the realm lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellKnown {
    /// `Symbol.iterator`: the method `GetIterator` calls to begin iterating.
    Iterator,
    /// `Symbol.toStringTag`: the name `Object.prototype.toString` puts in its
    /// answer, in place of the one it would work out for itself.
    ToStringTag,
}

impl WellKnown {
    /// Every one, in the order a realm makes them.
    pub const ALL: [WellKnown; 2] = [WellKnown::Iterator, WellKnown::ToStringTag];

    /// Where it is in [`WellKnown::ALL`].
    pub const fn index(self) -> usize {
        match self {
            WellKnown::Iterator => 0,
            WellKnown::ToStringTag => 1,
        }
    }

    /// Its description, which is the specification's spelling of its name.
    pub const fn description(self) -> &'static str {
        match self {
            WellKnown::Iterator => "Symbol.iterator",
            WellKnown::ToStringTag => "Symbol.toStringTag",
        }
    }
}
