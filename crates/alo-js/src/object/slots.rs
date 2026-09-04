/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A list of values that lives **in the heap**, because the collector has to be
//! able to walk it.
//!
//! ADR 0014 § 2 names the closed list of places a live reference may be, and two
//! of the entries on it were owed rather than built: *the interpreter's frames
//! and its value stack, which for that reason live in structures the collector
//! walks rather than in Rust locals.* This is that structure, and queue item 72
//! is where the debt is paid.
//!
//! # Why a cell rather than a `Vec<Value>` in the interpreter
//!
//! A `Vec<Value>` beside the heap would work in every ordinary run and be wrong
//! under [`Heap::stress`](crate::heap::Heap::stress): the collector cannot see
//! it, so an object whose only reference is half way down the operand stack is
//! swept while the interpreter is holding it. Making the stack a cell means the
//! marker reaches it from the roots like anything else, and it means every push
//! and pop goes through [`Heap::write`](crate::heap::Heap::write) and its
//! [`Barrier`] — which is ADR 0014 § 5's *no second way to write a reference*
//! applied to the busiest writer in the engine.
//!
//! # Two uses, one structure
//!
//! The interpreter's stack is one of these: the frame's locals at the bottom and
//! its operand stack above them. A realm's `let` and `const` bindings are
//! another (see [`realm`](crate::realm)), because they outlive the script that
//! declared them and a second script must find the same bindings.
//!
//! # A slot that holds nothing is not a slot holding `undefined`
//!
//! [`Held::Uninitialized`] is the temporal dead zone — `a` before `let a` has
//! run — and the language can tell it from `undefined`: reading one is a
//! `ReferenceError` and reading the other is a value. So it is a state of the
//! slot rather than a value in it, which is also why there is no `Value` variant
//! for it: a sentinel inside [`Value`] would be a thing every `match` in the
//! engine had to remember was not a value.

use crate::heap::{Barrier, Survivors, Trace, Tracer};

use super::value::{Stored, Value};

/// What one slot holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Held {
    /// The slot exists and has never been given a value.
    Uninitialized,
    /// A value.
    Value(Value),
}

/// One slot: a value, and whether it has been given one.
#[derive(Debug, Default)]
struct Slot {
    held: Stored,
    live: bool,
}

/// A list of values the collector can walk.
#[derive(Debug, Default)]
pub struct Slots {
    slots: Vec<Slot>,
}

impl Slots {
    /// An empty list.
    pub const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// How many slots there are, filled or not.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Add a value on the end.
    ///
    /// No barrier: a slot that did not exist a moment ago held no reference, so
    /// there is no store to record — which is [`Field::holding`]'s argument in
    /// the one other place it applies.
    ///
    /// [`Field::holding`]: crate::heap::Field
    pub fn push(&mut self, value: Value) {
        self.slots.push(Slot {
            held: Stored::holding(value),
            live: true,
        });
    }

    /// Add a slot that has not been given a value.
    pub fn push_uninitialized(&mut self) {
        self.slots.push(Slot::default());
    }

    /// Take the last value off, or [`None`] if there is nothing there.
    ///
    /// An uninitialized slot pops as [`Held::Uninitialized`], which the
    /// interpreter never asks for — the operand stack is only ever pushed with
    /// values — and which is answered honestly rather than as `undefined` so
    /// that a compiler bug shows up as itself.
    pub fn pop(&mut self) -> Option<Held> {
        self.slots.pop().map(|slot| slot.read())
    }

    /// What the slot at `at` holds.
    pub fn get(&self, at: usize) -> Option<Held> {
        self.slots.get(at).map(Slot::read)
    }

    /// Put a value in the slot at `at`, answering whether there was one.
    pub fn set(&mut self, barrier: &mut Barrier, at: usize, value: Value) -> bool {
        let Some(slot) = self.slots.get_mut(at) else {
            return false;
        };
        slot.held.set(barrier, value);
        slot.live = true;
        true
    }

    /// Empty the slot at `at` back to [`Held::Uninitialized`], answering whether
    /// there was one.
    ///
    /// This is what makes a `let` in a loop body a fresh dead zone on every
    /// pass: the block is entered again, and the binding it declares has not
    /// been reached yet.
    pub fn uninitialize(&mut self, barrier: &mut Barrier, at: usize) -> bool {
        let Some(slot) = self.slots.get_mut(at) else {
            return false;
        };
        slot.held.set(barrier, Value::Undefined);
        slot.live = false;
        true
    }

    /// Grow to `len` slots, the new ones uninitialized.
    ///
    /// Shrinking is [`Slots::truncate`]; this only ever adds, so a caller that
    /// asks for fewer than there are gets what is already there.
    pub fn grow_to(&mut self, len: usize) {
        while self.slots.len() < len {
            self.push_uninitialized();
        }
    }

    /// Drop everything above `len` slots.
    pub fn truncate(&mut self, len: usize) {
        self.slots.truncate(len);
    }
}

impl Slot {
    /// What this slot holds, as the two states the language can tell apart.
    fn read(&self) -> Held {
        if self.live {
            Held::Value(self.held.get())
        } else {
            Held::Uninitialized
        }
    }
}

impl Trace for Slots {
    fn trace(&self, tracer: &mut Tracer) {
        for slot in &self.slots {
            // An empty slot is traced too, and it costs nothing: it holds
            // `undefined`, which is not an edge. Asking `live` first would be a
            // branch that could only ever agree.
            slot.held.trace(tracer);
        }
    }

    fn footprint(&self) -> usize {
        self.slots.capacity().saturating_mul(size_of::<Slot>())
    }

    fn clear_weak(&mut self, _survivors: &Survivors) {}
}

#[cfg(test)]
mod tests {
    use super::{Held, Slots};
    use crate::object::value::Value;

    #[test]
    fn a_slot_that_was_never_given_a_value_is_not_undefined() {
        let mut slots = Slots::new();
        slots.push_uninitialized();
        assert_eq!(slots.get(0), Some(Held::Uninitialized));
        slots.push(Value::Undefined);
        assert_eq!(slots.get(1), Some(Held::Value(Value::Undefined)));
    }

    #[test]
    fn growing_adds_dead_slots_and_truncating_takes_them_away() {
        let mut slots = Slots::new();
        slots.grow_to(3);
        assert_eq!(slots.len(), 3);
        slots.grow_to(2);
        assert_eq!(slots.len(), 3, "growing never shrinks");
        slots.truncate(1);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots.get(1), None);
    }

    #[test]
    fn popping_an_empty_list_is_nothing_rather_than_a_panic() {
        let mut slots = Slots::new();
        assert_eq!(slots.pop(), None);
    }
}
