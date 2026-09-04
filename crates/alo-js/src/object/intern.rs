/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One string cell per distinct property name, and a table that lets go.
//!
//! ADR 0014 § 11: *property keys are interned, and the intern table is weak.
//! Interning makes a key comparison an integer comparison; a strong intern
//! table makes a leak that a stranger's script controls, since a page can mint
//! unbounded distinct keys. So the table is swept with everything else.*
//!
//! # Weak here means "holds no edge and owns no text"
//!
//! The table holds [`Ref`]s and **never reports them to the collector**, so
//! interning a name keeps nothing alive: what keeps a key's string alive is the
//! object whose property it is, which reports the key as an edge
//! ([`Properties::trace`](super::table::Properties::trace)). A key nobody uses
//! is collected at the next collection, and [`Interner::prune`] drops the
//! entry.
//!
//! It also holds **no copy of the text**. A `HashMap<Box<[u16]>, Ref>` would be
//! the obvious table and would put a second copy of every property name outside
//! the heap's ceiling — which is the same leak in a different place, since the
//! ceiling is the only thing bounding what a page may allocate. So the table is
//! a hash of the units to the cells that hashed to it, and a lookup compares
//! the text by reading the string cell it already has.
//!
//! # The hash is seeded, and that is a security property rather than a detail
//!
//! A page chooses every property name it writes. With a fixed hash function it
//! could choose a thousand names that collide, turning every lookup into a walk
//! of a thousand comparisons — the hash-flooding attack, which is a denial of
//! service written in a `for` loop. [`RandomState`] is the seeding
//! [`HashMap`](std::collections::HashMap) already gives every other table in
//! this crate, and it reaches no I/O crate: ADR 0013 § 5's rule is about what
//! this crate depends on, and the standard library's own hasher is not a
//! dependency.
//!
//! # When it is pruned
//!
//! After a collection, and the *next* interning is when it notices — the heap
//! collects inside an allocation and knows nothing about this table, so the
//! join is [`Heap::collections`](crate::heap::Heap::collections) being a number
//! this remembers. That bounds what a dead entry costs: a page minting a
//! million names holds at most one collection's worth of dead ones, which
//! `an_object_model_that_is_hostile.rs` asserts with a number rather than
//! assuming.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use crate::heap::{Heap, Ref};

use super::cell::Cell;

/// The interned property names.
#[derive(Debug, Default)]
pub struct Interner {
    /// The hash of a text, to the string cells whose text hashes to it.
    ///
    /// A [`Vec`] rather than one [`Ref`] because a hash collision is a fact of
    /// life rather than an error: two distinct names may land in one bucket,
    /// and the lookup tells them apart by comparing the code units.
    buckets: HashMap<u64, Vec<Ref>>,
    /// The seed, taken once so that the same text hashes the same way for the
    /// life of this engine — and differently in the next process.
    state: RandomState,
    /// How many collections had happened when this was last pruned.
    pruned_at: u64,
    /// How many names are in it, which is what a test about the bound asserts.
    count: usize,
}

impl Interner {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many names it holds, including any whose cell has died since the
    /// last prune.
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether it holds none.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The cell already holding this text, if there is one.
    ///
    /// It reads the heap to compare, which is what makes the table hold no copy
    /// of the text. A reference whose cell has gone — collected since, and not
    /// yet pruned — answers [`None`] and is left for the prune rather than
    /// being removed here, because a lookup that mutated would need the table
    /// by `&mut` at every property read.
    pub fn find(&self, heap: &Heap<Cell>, units: &[u16]) -> Option<Ref> {
        let bucket = self.buckets.get(&self.state.hash_one(units))?;
        bucket
            .iter()
            .copied()
            .find(|held| Self::spells(heap, *held, units))
    }

    /// Remember that `held` spells this text.
    ///
    /// The caller has just allocated it and has already asked [`Interner::find`]
    /// — this does not check again, because the check is a comparison of every
    /// name in the bucket and doing it twice per new name is the sort of waste
    /// that is invisible until a page has ten thousand of them.
    pub fn remember(&mut self, units: &[u16], held: Ref) {
        self.buckets
            .entry(self.state.hash_one(units))
            .or_default()
            .push(held);
        self.count = self.count.saturating_add(1);
    }

    /// Drop the entries whose string has been collected, if there has been a
    /// collection since the last time.
    ///
    /// It answers whether it did anything, which is how a test can tell a table
    /// that is small because it was pruned from one that is small because
    /// nothing was interned.
    pub fn prune(&mut self, heap: &Heap<Cell>) -> bool {
        if heap.collections() == self.pruned_at {
            return false;
        }
        self.pruned_at = heap.collections();

        let mut kept = 0_usize;
        self.buckets.retain(|_, bucket| {
            bucket.retain(|held| heap.live_at(*held));
            kept = kept.saturating_add(bucket.len());
            !bucket.is_empty()
        });
        self.count = kept;
        true
    }

    /// Whether the cell `held` names is a string spelling exactly `units`.
    fn spells(heap: &Heap<Cell>, held: Ref, units: &[u16]) -> bool {
        heap.get(held)
            .and_then(Cell::text)
            .is_some_and(|text| text.units() == units)
    }
}
