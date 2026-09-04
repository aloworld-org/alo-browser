/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a function's bindings live, and why they outlive the call that made
//! them.
//!
//! A frame slot dies when its call returns, which is correct for everything a
//! script could see **before there were closures**. A closure is exactly the
//! thing that can see one afterwards, so item 209 needed somewhere else to put
//! a function's parameters and its body-level `var`, `let` and `const`: a cell
//! in the heap, kept alive by the function that closed over it and reclaimed
//! when that function is.
//!
//! That is the whole of this file. It is a [`Slots`] with a parent, and both
//! halves are load-bearing:
//!
//! - the **slots** are values the collector walks, for
//!   [`slots`](super::slots)'s reason unchanged;
//! - the **parent** is what makes a chain. A function written inside another
//!   one closes over the environment that was running when it was made, so
//!   `hops` in [`Op::LoadBinding`](crate::code::Op) is a walk up this link and
//!   nothing else.
//!
//! # The parent is written once and never again
//!
//! It is decided when the environment is made — by
//! [`Op::Closure`](crate::code::Op), from the environment the call was given —
//! and nothing can change it afterwards. So it needs no
//! [`Barrier`](crate::heap::Barrier): ADR 0014 § 5 asks a barrier to hear about
//! a store *into a cell the marker has already seen*, and a field set on the way
//! to [`Heap::allocate`](crate::heap::Heap::allocate) has no such cell yet. It
//! is [`Field::holding`](crate::heap::Field), which is the same hole in the
//! wording that an ordinary object's prototype goes through.
//!
//! # A copy is a sibling rather than a child
//!
//! [`Environment::copied`] makes one with the **same parent** and the same
//! values, which is `CreatePerIterationEnvironment` (queue item 216): each pass
//! of a `for (let …)` runs in a copy, so a closure made in one pass keeps values
//! the next pass cannot write to. Making it a child instead would leave every
//! pass able to see the one before it through one more hop, and the compiler
//! counts hops.
//!
//! # A script has no environment
//!
//! A script's own `let` and `const` are the realm's — a second `<script>` sees
//! them — so the script's chunk has no bindings, and a function declared at a
//! script's top level closes over nothing. That is what
//! [`Environment::under`] taking [`None`] means, and it is why the chain ends.

use crate::heap::{Barrier, Field, Ref, Survivors, Trace, Tracer};

use super::slots::{Held, Slots};
use super::value::Value;

/// A function's bindings, and the environment it was made inside.
#[derive(Debug)]
pub struct Environment {
    parent: Field,
    bindings: Slots,
}

impl Environment {
    /// An environment of `bindings` places, none of them given a value yet.
    ///
    /// Uninitialized rather than `undefined`, because that is the dead zone a
    /// body-level `let` needs and the two are things the language can tell
    /// apart. A parameter and a `var` are given their value by the call before
    /// anything runs, which is what makes them readable and a `let` not.
    pub fn under(parent: Option<Ref>, bindings: usize) -> Self {
        let mut slots = Slots::new();
        slots.grow_to(bindings);
        Self {
            parent: match parent {
                Some(held) => Field::holding(held),
                None => Field::empty(),
            },
            bindings: slots,
        }
    }

    /// Another environment under the same parent, holding what this one holds.
    ///
    /// A binding still in its dead zone stays in it, which is what makes the
    /// copy indistinguishable from the original to everything except a closure
    /// that kept the original. No [`Barrier`] for the same reason
    /// [`Environment::under`] needs none: nothing here is in the heap yet, so
    /// there is no cell the marker could already have seen.
    #[must_use]
    pub fn copied(&self) -> Self {
        let mut bindings = Slots::new();
        for at in 0..self.bindings.len() {
            match self.bindings.get(at) {
                Some(Held::Value(value)) => bindings.push(value),
                // A slot that is not there cannot happen — the loop is over the
                // length — and answering it as a dead zone is the reading that
                // cannot invent a value.
                Some(Held::Uninitialized) | None => bindings.push_uninitialized(),
            }
        }
        Self {
            parent: match self.parent.get() {
                Some(held) => Field::holding(held),
                None => Field::empty(),
            },
            bindings,
        }
    }

    /// The environment this one was made inside, or [`None`] at the end of the
    /// chain.
    pub const fn parent(&self) -> Option<Ref> {
        self.parent.get()
    }

    /// What the binding at `at` holds.
    pub fn get(&self, at: usize) -> Option<Held> {
        self.bindings.get(at)
    }

    /// Put a value in the binding at `at`, answering whether there was one.
    pub fn set(&mut self, barrier: &mut Barrier, at: usize, value: Value) -> bool {
        self.bindings.set(barrier, at, value)
    }

    /// How many bindings there are.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether there are none, which is an ordinary function of no parameters
    /// that declares nothing.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Trace for Environment {
    fn trace(&self, tracer: &mut Tracer) {
        self.parent.trace(tracer);
        self.bindings.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.bindings.footprint()
    }

    fn clear_weak(&mut self, _survivors: &Survivors) {}
}

#[cfg(test)]
mod tests {
    use super::Environment;
    use crate::heap::Heap;
    use crate::object::slots::Held;
    use crate::object::value::Value;

    #[test]
    fn a_copy_keeps_the_values_the_parent_and_the_dead_zones() {
        let mut heap: Heap<Holder> = Heap::new();
        let Ok(parent) = heap.allocate(Holder(Environment::under(None, 0))) else {
            panic!("an empty heap holds one cell");
        };
        let Ok(held) = heap.allocate(Holder(Environment::under(Some(parent), 2))) else {
            panic!("and it holds a second");
        };
        let wrote = heap
            .write(held, |cell, barrier| {
                cell.0.set(barrier, 0, Value::Number(7.0))
            })
            .unwrap_or_default();
        assert!(wrote);

        let Some(copy) = heap.get(held).map(|cell| cell.0.copied()) else {
            panic!("the environment is where it was put");
        };
        assert_eq!(copy.parent(), Some(parent), "a sibling, not a child");
        assert_eq!(copy.get(0), Some(Held::Value(Value::Number(7.0))));
        assert_eq!(
            copy.get(1),
            Some(Held::Uninitialized),
            "and a dead zone is copied as one rather than as `undefined`"
        );
        assert_eq!(copy.len(), 2);
    }

    #[test]
    fn a_binding_starts_in_its_dead_zone_and_the_chain_ends() {
        let environment = Environment::under(None, 2);
        assert_eq!(environment.parent(), None);
        assert_eq!(environment.get(0), Some(Held::Uninitialized));
        assert_eq!(environment.get(2), None);
        assert_eq!(environment.len(), 2);
    }

    #[test]
    fn a_binding_is_written_through_the_barrier() {
        // The barrier is the heap's, so this needs one — and writing to a cell
        // that is *in* the heap is exactly the case ADR 0014 § 5 is about.
        let mut heap: Heap<Holder> = Heap::new();
        let Ok(held) = heap.allocate(Holder(Environment::under(None, 1))) else {
            panic!("an empty heap holds one cell");
        };
        let wrote = heap
            .write(held, |cell, barrier| {
                cell.0.set(barrier, 0, Value::Number(1.0))
            })
            .unwrap_or_default();
        assert!(wrote);
        assert_eq!(
            heap.get(held).map(|cell| cell.0.get(0)),
            Some(Some(Held::Value(Value::Number(1.0))))
        );
    }

    /// A cell of nothing but an environment, so that this file's test needs no
    /// [`Cell`](crate::object::Cell) and therefore no object model at all.
    #[derive(Debug)]
    struct Holder(Environment);

    impl crate::heap::Trace for Holder {
        fn trace(&self, tracer: &mut crate::heap::Tracer) {
            self.0.trace(tracer);
        }
    }
}
