/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What names a heap object, and the two kinds of field that may hold one.
//!
//! ADR 0014 § 3. A reference is an **index carrying a generation**, which is
//! ADR 0004's move for `taffy`'s handle made again for the same reason: an
//! index is safe code where a pointer is `unsafe`, and law 4 forbids the
//! second one anywhere near a stranger's script.
//!
//! # Why a generation, and what ADR 0003 has to do with it
//!
//! Sweeping returns a slot to the free list and a later object takes it. That
//! collides with ADR 0003, which says an identity is allocated once and never
//! reused *because a reused number makes two different things look like one*.
//!
//! So what is never reused is the **pair**. A slot carries a generation that
//! increases every time the slot is emptied, a [`Ref`] carries the generation
//! it was made with, and a reference whose generation no longer matches names
//! **nothing** — never whatever took the slot. ADR 0003's promise is kept at
//! the level it was made, and the arithmetic that keeps it is a comparison.
//!
//! When a generation would wrap the slot is [retired](next_life) rather than
//! wrapped: one slot spent, and the one hole in that argument closed rather
//! than described.
//!
//! # Two kinds of field, and only one of them passes the barrier
//!
//! A [`Field`] is a strong reference held by a heap object, and the only way
//! to write one is through a [`Barrier`] (ADR 0014 § 5). A [`Weak`] is a
//! reference that does not keep its target alive, and it needs no barrier —
//! a store that creates no strong edge is a store an incremental marker has
//! nothing to do about. That is the whole difference, and it is why they are
//! two types rather than one with a flag.

use super::trace::{Survivors, Tracer};

/// A reference into the heap: which slot, and which of that slot's lives.
///
/// Eight bytes and [`Copy`], because everything in an engine holds one. It is
/// made by [`Heap::allocate`](super::Heap::allocate) and by nothing else — a
/// reference nobody allocated is a reference to whatever happens to be there,
/// which is the bug class this whole design exists to make unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref {
    slot: u32,
    generation: u32,
}

impl Ref {
    /// The reference to a slot in the life it is in now.
    pub(in crate::heap) const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Which slot, as something a [`slice::get`](slice) will take.
    ///
    /// Saturating rather than casting, because `u32 as usize` is a truncation
    /// on a machine narrower than this one and a truncated index names the
    /// wrong slot rather than none. [`usize::MAX`] names no slot on any
    /// machine, so the saturation answers *nothing* the way § 3 asks.
    pub fn index(self) -> usize {
        usize::try_from(self.slot).unwrap_or(usize::MAX)
    }

    /// Which of the slot's lives this reference was made in.
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// A reference naming a slot nothing allocated, for a unit test about
    /// something that is not the heap.
    ///
    /// [`Heap::allocate`](super::Heap::allocate) is the only thing that makes a
    /// reference, and that is the invariant the whole design rests on — so this
    /// is `#[cfg(test)]`, reaches no further than this crate's own unit tests,
    /// and exists for one reason: the property table's business is the **order**
    /// of keys, and making a real heap to get two distinct names for it would
    /// test the heap in the file that tests the order.
    ///
    /// Everything that is about a real graph — every integration test, and
    /// every test that collects — allocates properly.
    #[cfg(test)]
    pub(crate) const fn for_a_test(slot: u32) -> Self {
        Self {
            slot,
            generation: 0,
        }
    }
}

/// The generation of a slot that will never be filled again.
///
/// Reserved rather than reached: [`next_life`] stops one below it, so no
/// [`Ref`] is ever made carrying it. That is what makes a retired slot
/// unreachable *through the generation itself* — otherwise the last reference
/// handed out before retirement would go on matching for ever, and retiring a
/// slot would keep alive the one thing it was retired to let go of.
pub(in crate::heap) const RETIRED: u32 = u32::MAX;

/// The generation a slot takes when it is emptied, or [`None`] if it must be
/// retired instead.
///
/// ADR 0014 § 3's last paragraph, as a function rather than as a sentence in
/// the sweep. Four billion fills of one slot is not a thing a test can run —
/// at a fill a nanosecond it is most of an hour of doing nothing else — so the
/// deciding is a function that is asserted at its boundary and the use of it is
/// one line. That is the shape items 55, 154 and 188 already use in `alo-net`,
/// for the same reason: a rule about what may be reused is asserted honestly
/// only when nothing else is moving.
pub(in crate::heap) const fn next_life(generation: u32) -> Option<u32> {
    let next = generation.saturating_add(1);
    if next == RETIRED { None } else { Some(next) }
}

/// A strong reference held by a heap object.
///
/// Not [`Clone`], on purpose. Copying a field into another cell is a *store*,
/// and a store goes through [`Field::set`] and its [`Barrier`] — a derive here
/// would be a second way to write one, which is exactly what ADR 0014 § 5 says
/// there must not be.
#[derive(Debug, Default)]
pub struct Field(Option<Ref>);

impl Field {
    /// A field holding nothing.
    pub const fn empty() -> Self {
        Self(None)
    }

    /// A field holding `to`, at the moment the object holding it is built.
    ///
    /// No barrier, because there is no object yet: a barrier records a store
    /// *into the heap*, and this value is on its way to
    /// [`Heap::allocate`](super::Heap::allocate). An object the marker has
    /// never seen has no edge to tell it about, which is why this is a hole in
    /// the wording rather than in the barrier.
    ///
    /// What the shape does *not* stop is somebody writing a whole field over —
    /// `*field = Field::holding(r)` — inside a
    /// [`Heap::write`](super::Heap::write). Rust has no way to forbid it, so
    /// the second half of ADR 0014 § 5 is the one that closes it: a cell's
    /// reference-bearing fields are **private to the object model**, reached
    /// through methods that take a [`Barrier`]. Nothing in this crate assigns
    /// a field, and a cell that exposes one publicly has given itself a second
    /// way to be written.
    pub const fn holding(to: Ref) -> Self {
        Self(Some(to))
    }

    /// What it holds.
    pub const fn get(&self) -> Option<Ref> {
        self.0
    }

    /// Store a reference, telling the collector.
    pub fn set(&mut self, barrier: &mut Barrier, to: Option<Ref>) {
        barrier.stored(self.0, to);
        self.0 = to;
    }

    /// Report the edge to the collector.
    pub fn trace(&self, tracer: &mut Tracer) {
        if let Some(to) = self.0 {
            tracer.edge(to);
        }
    }
}

/// A reference that does not keep its target alive.
///
/// ADR 0014 § 7. It is never reported to the marker, and it is cleared by
/// [`Weak::clear`] during the sweep of the collection that decided its target
/// was unreachable — so a cell holding one sees [`None`] rather than a
/// reference that names nothing, and the difference is what lets a
/// `FinalizationRegistry` notice the loss it has to report.
#[derive(Debug, Default)]
pub struct Weak(Option<Ref>);

impl Weak {
    /// A weak reference to `to`.
    pub const fn to(to: Ref) -> Self {
        Self(Some(to))
    }

    /// A weak reference holding nothing.
    pub const fn empty() -> Self {
        Self(None)
    }

    /// What it holds, which is [`None`] once its target has gone.
    pub const fn get(&self) -> Option<Ref> {
        self.0
    }

    /// Forget the target if it did not survive, and say what was lost.
    ///
    /// The returned reference is the one thing a registry needs and the one
    /// thing it must not be handed as an object: the specification gives a
    /// finalisation callback a **held value rather than the target**, which is
    /// what makes resurrection impossible here (ADR 0014 § 7). This says *that*
    /// it died, in a life that is already over.
    pub fn clear(&mut self, survivors: &Survivors) -> Option<Ref> {
        let held = self.0?;
        if survivors.alive(held) {
            return None;
        }
        self.0 = None;
        Some(held)
    }
}

/// The hook every store of a heap reference into a heap object passes through.
///
/// ADR 0014 § 5, and today it counts. It exists now because both answers to a
/// pause somebody can see need it — incremental marking needs the tri-colour
/// invariant maintained on every store, and a generational nursery needs a
/// remembered set of old-to-new references — and because installing it later
/// means auditing every mutation in an engine that by then has builtins, a DOM
/// binding and a compiler emitting stores.
///
/// It is reached only from [`Heap::write`](super::Heap::write), which is the
/// only thing in this crate that hands out `&mut` to a cell. That, plus
/// [`Field`] having no other mutator, is what makes "there is no second way to
/// write one" a property of the shape rather than of somebody's memory.
#[derive(Debug)]
pub struct Barrier {
    stores: usize,
}

impl Barrier {
    /// A barrier for one visit to one cell.
    pub(in crate::heap) const fn new() -> Self {
        Self { stores: 0 }
    }

    /// How many references were stored through it.
    pub const fn stores(&self) -> usize {
        self.stores
    }

    /// A reference was overwritten.
    ///
    /// `was` and `now` are both taken because the two future readers want
    /// different ones: an incremental marker shades the reference being lost,
    /// a nursery remembers the reference being gained. Taking one of them today
    /// would be choosing between them today.
    ///
    /// It is public because a [`Field`] is not the only shape a stored
    /// reference has: a value is one *sometimes*
    /// ([`Stored`](crate::object::Stored)), and a type that holds a reference
    /// in a cell records through this. That is not a second way to write a
    /// field — the thing ADR 0014 § 5 forbids is a **mutator that skips the
    /// barrier**, and reaching the barrier is the opposite of skipping it. What
    /// still cannot be reached from outside is a [`Barrier`] itself: the only
    /// one there is comes from
    /// [`Heap::write`](super::Heap::write).
    pub fn stored(&mut self, was: Option<Ref>, now: Option<Ref>) {
        let _ = (was, now);
        self.stores = self.stores.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{RETIRED, Ref, next_life};

    #[test]
    fn a_slot_that_has_lives_left_gets_the_next_one() {
        assert_eq!(next_life(0), Some(1));
        assert_eq!(next_life(41), Some(42));
        assert_eq!(next_life(RETIRED - 2), Some(RETIRED - 1));
    }

    #[test]
    fn a_slot_whose_next_life_would_be_the_retired_one_is_retired_instead() {
        // One below the reserved generation is the last life a slot has: the
        // reference handed out in it must never go on matching afterwards.
        assert_eq!(next_life(RETIRED - 1), None);
        assert_eq!(next_life(RETIRED), None);
    }

    #[test]
    fn two_lives_of_one_slot_are_two_different_references() {
        assert_ne!(Ref::new(7, 0), Ref::new(7, 1));
        assert_eq!(Ref::new(7, 1), Ref::new(7, 1));
    }
}
