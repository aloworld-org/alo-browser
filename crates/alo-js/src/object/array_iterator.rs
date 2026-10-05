/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An array iterator: what `[].values()` answers, and what `for…of` over an
//! array reads from (queue item 230).
//!
//! The specification writes one as a generator over a closure, and the state
//! that closure keeps is three things: the object being iterated, the next
//! index, and whether it hands out keys, values or both. This cell is those
//! three beside an [`Ordinary`] object, which is everything else about it — a
//! page may hang properties off an iterator like off anything.
//!
//! # A finished iterator lets go of what it iterated
//!
//! Once `next` has answered `done`, the generator is *completed* and every
//! later `next` answers `done` without looking at the object again — even if
//! the array has grown since. That is a [`Field`] cleared rather than a flag
//! set, so a finished iterator also stops keeping a large array alive.
//!
//! # What it does not have: a generator's other two states
//!
//! A generator can also be *executing* — `next` called again from inside a
//! getter that `next` itself is running — and an abrupt completion completes
//! it. Neither can happen here yet, because this iterator never calls a
//! script: an element or a `length` behind a call is refused by name
//! ([`Missing::AnIteratedValueBehindACall`](crate::abrupt::Missing)), which is
//! queue item 231's.

use crate::heap::{Barrier, Field, Ref, Tracer};

use super::ordinary::Ordinary;

/// What an array iterator hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `keys()`: the indices.
    Keys,
    /// `values()` and `[Symbol.iterator]()`: the elements.
    Values,
    /// `entries()`: `[index, element]` pairs.
    Entries,
}

/// An array iterator.
#[derive(Debug)]
pub struct ArrayIterator {
    ordinary: Ordinary,
    /// What is being iterated, until the iterator finishes.
    iterated: Field,
    /// The index `next` reads next. A number rather than a `u32` because an
    /// array-like's length is up to 2⁵³−1 rather than an array's 2³²−1, and
    /// every whole number to there is exact in a double — which is also what
    /// `keys()` hands out.
    next: f64,
    kind: Kind,
}

impl ArrayIterator {
    /// An iterator over `iterated`, starting at index zero:
    /// `CreateArrayIterator`.
    pub fn new(prototype: Option<Ref>, iterated: Ref, kind: Kind) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            iterated: Field::holding(iterated),
            next: 0.0,
            kind,
        }
    }

    /// What it iterates, or [`None`] once it has finished.
    pub const fn iterated(&self) -> Option<Ref> {
        self.iterated.get()
    }

    /// The index the next `next` reads.
    pub const fn next_index(&self) -> f64 {
        self.next
    }

    /// What it hands out.
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Move on by one, having handed out the index it was at. It never passes
    /// the length it was compared with, so it never leaves the exact range.
    pub fn advance(&mut self) {
        self.next += 1.0;
    }

    /// Finish: every `next` from now on answers `done`.
    pub fn finish(&mut self, barrier: &mut Barrier) {
        self.iterated.set(barrier, None);
    }

    /// The ordinary object it also is.
    pub const fn ordinary(&self) -> &Ordinary {
        &self.ordinary
    }

    /// The same, to be written through.
    pub const fn ordinary_mut(&mut self) -> &mut Ordinary {
        &mut self.ordinary
    }

    /// Report every edge: the object's, and what is being iterated.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        self.iterated.trace(tracer);
    }

    /// What it owns beyond its slot, which is its table.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }
}

#[cfg(test)]
mod tests {
    use super::{ArrayIterator, Kind};
    use crate::object::Objects;

    #[test]
    fn it_starts_at_zero_counts_up_and_lets_go_when_it_finishes() {
        let mut objects = Objects::new();
        let Ok(array) = objects.array(None, 2) else {
            panic!("an empty heap holds an array");
        };
        let Ok(held) = objects.array_iterator(None, array, Kind::Entries) else {
            panic!("and an iterator over it");
        };
        let Some(iterator) = objects.as_array_iterator(held) else {
            panic!("it is an array iterator");
        };
        assert_eq!(iterator.iterated(), Some(array));
        assert_eq!(iterator.next_index().to_bits(), 0.0_f64.to_bits());
        assert_eq!(iterator.kind(), Kind::Entries);

        let moved = objects.with_array_iterator(held, |iterator, _| iterator.advance());
        assert_eq!(moved, Some(()));
        assert_eq!(
            objects
                .as_array_iterator(held)
                .map(|iterator| iterator.next_index().to_bits()),
            Some(1.0_f64.to_bits())
        );
        let finished = objects.with_array_iterator(held, ArrayIterator::finish);
        assert_eq!(finished, Some(()));
        assert_eq!(
            objects
                .as_array_iterator(held)
                .and_then(ArrayIterator::iterated),
            None,
            "a finished iterator holds nothing"
        );
        assert!(
            objects.as_array_iterator(array).is_none(),
            "and an array is not an iterator"
        );
    }
}
