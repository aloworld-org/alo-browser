/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where everything a script makes lives, and the one thing that tidies it
//! away.
//!
//! ADR 0014, and it is the half of queue item 71 that had to be right before
//! anything is built on it: item 72's interpreter, every builtin in item 73,
//! the promises in 75, the mutation in 80 and the storage in 90 are all things
//! that allocate, and four of this decision's clauses cannot be changed
//! afterwards without touching every one of them.
//!
//! # The shape in five sentences
//!
//! The heap is an **arena of slots in safe Rust** and a reference into it is an
//! **index carrying a generation** ([`Ref`], ADR 0014 § 3), so a mistake here is
//! something the engine can see rather than a use-after-free. One **precise,
//! non-moving, stop-the-world mark-and-sweep** collector owns it (§§ 2 and 4).
//! The **DOM is in the same graph**, traced rather than counted, through the one
//! demand this engine makes of anything an embedder puts here — [`Trace`], § 6.
//! Three things that cannot be added afterwards are in from the first line: a
//! [`Barrier`] every store passes through (§ 5), an **ephemeron fixpoint** in
//! the mark phase (§ 7), and a **marker that never recurses** (§ 8). And the
//! ceilings are ours, in [`bounds`](crate::bounds) with their reasons beside
//! them (§ 9).
//!
//! # What this crate holds and what it does not
//!
//! [`Heap<T>`] is generic in what a cell is, and that is not indirection for
//! its own sake: the collector's business is *reachability*, and what an object
//! **is** — its prototype, its properties, their observable order — is the
//! object model's, which is [`crate::object`] (queue item 206). That has since
//! landed, [`Cell`](crate::object::Cell) is the enumeration this holds, an
//! embedder's object is one of its variants, and **nothing in this file changed
//! when it arrived** — which was the argument for building the two in this
//! order rather than the other.
//!
//! # The discipline, which is the price of precision
//!
//! Every point at which the engine may allocate is a point at which a
//! collection may happen, and a reference held anywhere but [`Scope`], [`Root`]
//! or the keep-alive set is gone (see [`root`]). A builtin that keeps one in a
//! Rust local across an allocation is **correct in every ordinary run** and
//! wrong only under [`Heap::stress`] — which is exactly why that mode exists and
//! why ADR 0014 § 10 calls it not optional.
//!
//! One thing the discipline does *not* have to cover: the cell being built when
//! the allocation that triggered a collection was made. It is traced as a root
//! for that collection, because it is about to be live and it is right here.
//!
//! # A stranger's script cannot stop this
//!
//! `LOOP.md`'s stage 2 clause 2 and ADR 0013 § 4. A graph a million deep is
//! marked without a single recursive call; a heap that is full is a
//! [`Full`] the embedder is told about, never an abort and never a panic; and a
//! marking buffer that fills up costs a rescan rather than an answer. The
//! hostile half is `tests/a_heap_that_is_hostile.rs`, and every case in it ends
//! in a refusal or a collection.

pub mod check;
mod collect;
pub mod reference;
pub mod root;
pub mod trace;

use std::fmt;
use std::mem;

use crate::bounds;

pub use check::Broken;
pub use reference::{Barrier, Field, Ref, Weak};
pub use root::{Root, Scope};
pub use trace::{Survivors, Trace, Tracer};

use collect::Swept;
use root::Roots;
use trace::Buffer;

/// The heap was at its ceiling and a collection did not bring it under.
///
/// ADR 0014 § 9. Where the language specifies an error — a string or an array
/// that cannot be made — the engine produces that error, because a script's own
/// `catch` is the page's way of surviving us. Where it does not, this goes to
/// the **embedder**, which stops the tab and says so: a person gets a page that
/// refused, with a reason. Never an abort, never a panic, and never the browser
/// process (ADR 0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full {
    /// The bytes the allocation needed.
    pub asked: usize,
    /// The bytes the live heap already holds.
    pub held: usize,
    /// The ceiling, which is [`bounds::HEAP_CEILING`].
    pub ceiling: usize,
}

impl fmt::Display for Full {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "the heap is full: {} bytes are held of a ceiling of {}, and {} more were asked for",
            self.held, self.ceiling, self.asked
        )
    }
}

/// An arena of slots, and the collector that owns them.
#[derive(Debug)]
pub struct Heap<T> {
    /// The slots. [`None`] is a slot that is free or retired, and which of the
    /// two it is, is whether it is on `free`.
    cells: Vec<Option<T>>,
    /// Which life each slot is in. Beside the cells rather than inside them
    /// because a sweep needs the generations while it holds the cells by
    /// `&mut`, and because a [`Tracer`] needs them without needing to know what
    /// a cell is.
    generations: Vec<u32>,
    /// The mark bits, for the same reason.
    marks: Vec<bool>,
    /// Slots waiting to be filled again.
    free: Vec<usize>,
    roots: Roots,
    work: Buffer<Ref>,
    pairs: Buffer<(Ref, Ref)>,
    live: usize,
    held: usize,
    since: usize,
    stress: bool,
    collections: u64,
    stores: usize,
    stale: usize,
    retired: usize,
    rescans: usize,
}

impl<T> Default for Heap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Heap<T> {
    /// An empty heap, with the marker's buffers taken once and never again.
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            generations: Vec::new(),
            marks: Vec::new(),
            free: Vec::new(),
            roots: Roots::default(),
            work: Buffer::bounded(bounds::MARKING_WORKLIST),
            pairs: Buffer::bounded(bounds::MARKING_EPHEMERONS),
            live: 0,
            held: 0,
            since: 0,
            stress: false,
            collections: 0,
            stores: 0,
            stale: 0,
            retired: 0,
            rescans: 0,
        }
    }

    /// Collect at every safepoint, which today means at every allocation.
    ///
    /// ADR 0014 § 10: *this is how a rooting bug is found, and § 2's discipline
    /// is not credible without it.* An engine's test suite runs its programs
    /// twice, once each way, and the second run is the one that means anything.
    ///
    /// It is not a debug build's behaviour or a feature flag, because a mode
    /// that is only reachable in some builds is a mode nobody runs.
    pub fn stress(&mut self, on: bool) {
        self.stress = on;
    }

    /// How many cells are alive, which is what a test about reclamation asserts
    /// on.
    ///
    /// ADR 0014 § 10: *counted rather than watched* — a test that watched
    /// process memory would be measuring the allocator.
    pub const fn live(&self) -> usize {
        self.live
    }

    /// What the live cells hold, in bytes, counted against
    /// [`bounds::HEAP_CEILING`].
    pub const fn held(&self) -> usize {
        self.held
    }

    /// How many collections have happened.
    pub const fn collections(&self) -> u64 {
        self.collections
    }

    /// How many times a collection has had to look at every marked cell again.
    ///
    /// A marking buffer refused something, so the mark phase went and found it.
    /// It costs work and never an answer (ADR 0014 § 8) — and it is here
    /// because a bound nothing ever reaches is a bound nobody has checked is
    /// reachable, so the tests that make a graph too wide to buffer assert this
    /// rather than assuming the path was taken.
    pub const fn rescans(&self) -> usize {
        self.rescans
    }

    /// How many slots the arena has, filled or not.
    ///
    /// The evidence that a swept slot is filled again rather than added to: a
    /// program that makes a million objects and keeps none of them is a heap of
    /// a handful of slots, and this is the number that says so.
    pub fn slots(&self) -> usize {
        self.cells.len()
    }

    /// How many references have been stored through a [`Barrier`].
    ///
    /// The barrier does nothing today (ADR 0014 § 5), so this is the only
    /// evidence that every mutation went through it — and evidence is the
    /// point: the barrier exists so that incremental marking is a change rather
    /// than an audit of every mutation in the engine.
    pub const fn stores(&self) -> usize {
        self.stores
    }

    /// How many strong edges have named nothing.
    ///
    /// ADR 0014 § 3: it is **our** bug rather than a page's, and it means
    /// something was collected while a live cell still pointed at it.
    /// [`Heap::check`] refuses a heap where this has grown.
    pub const fn stale_edges(&self) -> usize {
        self.stale
    }

    /// How many slots will never be filled again (ADR 0014 § 3).
    pub const fn retired(&self) -> usize {
        self.retired
    }

    /// How many references the open scopes are holding.
    pub fn scoped(&self) -> usize {
        self.roots.scoped()
    }

    /// How many references the job's keep-alive set is holding.
    pub fn kept(&self) -> usize {
        self.roots.kept()
    }

    /// Open a scope for what native code must keep across an allocation.
    pub fn open(&mut self) -> Scope {
        self.roots.open()
    }

    /// Keep `held` alive until the innermost open scope closes.
    pub fn hold(&mut self, held: Ref) {
        self.roots.hold(held);
    }

    /// Close a scope, letting go of everything held in it.
    pub fn close(&mut self, scope: Scope) {
        self.roots.close(scope.spend());
    }

    /// Keep `held` alive until the root is released — a realm's globals, and
    /// the embedder's own (ADR 0014 § 6).
    pub fn root(&mut self, held: Ref) -> Root {
        self.roots.root(held)
    }

    /// Let go of a root.
    pub fn release(&mut self, root: Root) {
        self.roots.release(root.spend());
    }

    /// What a root is holding.
    pub fn holding(&self, root: &Root) -> Option<Ref> {
        self.roots.holding(root)
    }

    /// Keep `held` alive for the rest of the job.
    ///
    /// ADR 0014 § 7's last rule: a `WeakRef` that has been dereferenced keeps
    /// its target alive for the rest of the job, so a script cannot see the same
    /// reference answer twice differently.
    pub fn keep_alive(&mut self, held: Ref) {
        self.roots.keep_alive(held);
    }

    /// The job ended; what it was keeping alive is let go.
    pub fn end_job(&mut self) {
        self.roots.end_job();
    }

    /// The cell a reference names, or [`None`] if it names nothing.
    ///
    /// [`None`] means the slot has been emptied, retired, or filled again since
    /// the reference was made. For a reference the engine believed was strong
    /// that is the internal error of ADR 0014 § 3 — ours rather than a page's —
    /// and the thing that turns it into an error a script sees is the
    /// interpreter, because only it has a script to end. Under
    /// test it is [`Broken::StaleEdges`], loudly.
    pub fn get(&self, held: Ref) -> Option<&T> {
        if self.generations.get(held.index()).copied() != Some(held.generation()) {
            return None;
        }
        self.cells.get(held.index())?.as_ref()
    }

    /// Whether a reference still names the cell it was made for.
    pub fn live_at(&self, held: Ref) -> bool {
        self.get(held).is_some()
    }
}

impl<T: Trace> Heap<T> {
    /// Put a cell in the heap.
    ///
    /// **This is a safepoint.** It is where the collector runs, so every
    /// reference the caller means to keep must already be in a [`Scope`], a
    /// [`Root`] or the keep-alive set — except the ones `cell` itself holds,
    /// which are traced for exactly this collection.
    ///
    /// It refuses rather than aborting when the heap is full, which is the
    /// whole of ADR 0014 § 9: a collection first, an error second, and the error
    /// goes to whoever can do something about it.
    ///
    /// # Errors
    ///
    /// [`Full`] when the live heap is at [`bounds::HEAP_CEILING`] and a
    /// collection did not bring it under.
    pub fn allocate(&mut self, cell: T) -> Result<Ref, Full> {
        let cost = size_of::<T>().saturating_add(cell.footprint());
        let over = self.held.saturating_add(cost) > bounds::HEAP_CEILING;
        if self.stress || over || self.since >= bounds::COLLECT_AFTER {
            self.collect_with(Some(&cell));
        }
        if self.held.saturating_add(cost) > bounds::HEAP_CEILING {
            return Err(Full {
                asked: cost,
                held: self.held,
                ceiling: bounds::HEAP_CEILING,
            });
        }

        let reused = self
            .free
            .pop()
            .filter(|at| self.cells.get(*at).is_some_and(Option::is_none));
        let at = if let Some(at) = reused {
            if let Some(slot) = self.cells.get_mut(at) {
                *slot = Some(cell);
            }
            at
        } else {
            self.cells.push(Some(cell));
            self.generations.push(0);
            self.marks.push(false);
            self.cells.len().saturating_sub(1)
        };

        let Ok(slot) = u32::try_from(at) else {
            // More slots than a reference can name. The byte ceiling stops a
            // heap long before this, and answering it with a refusal rather
            // than with a truncated index is the difference between a page that
            // stopped and a reference that names the wrong object.
            if let Some(cell) = self.cells.get_mut(at) {
                *cell = None;
            }
            return Err(Full {
                asked: cost,
                held: self.held,
                ceiling: bounds::HEAP_CEILING,
            });
        };

        self.held = self.held.saturating_add(cost);
        self.since = self.since.saturating_add(cost);
        self.live = self.live.saturating_add(1);
        Ok(Ref::new(
            slot,
            self.generations.get(at).copied().unwrap_or_default(),
        ))
    }

    /// Change a cell, with the barrier every store of a reference goes through.
    ///
    /// [`None`] if the reference names nothing, for [`Heap::get`]'s reasons.
    /// This is the only thing in the crate that hands out `&mut` to a cell, and
    /// with it the only [`Barrier`] there is (ADR 0014 § 5).
    ///
    /// It is **not** a safepoint: nothing here allocates, so a reference in a
    /// Rust local is safe across it.
    pub fn write<R>(
        &mut self,
        held: Ref,
        with: impl FnOnce(&mut T, &mut Barrier) -> R,
    ) -> Option<R> {
        if self.generations.get(held.index()).copied() != Some(held.generation()) {
            return None;
        }
        let cell = self.cells.get_mut(held.index())?.as_mut()?;
        let before = cell.footprint();
        let mut barrier = Barrier::new();
        let outcome = with(cell, &mut barrier);
        let after = cell.footprint();

        self.stores = self.stores.saturating_add(barrier.stores());
        let grew = after.saturating_sub(before);
        self.held = self
            .held
            .saturating_add(grew)
            .saturating_sub(before.saturating_sub(after));
        self.since = self.since.saturating_add(grew);
        Some(outcome)
    }

    /// Collect now.
    ///
    /// The embedder may ask for one, and ADR 0014 § 9 says why that is a
    /// judgement rather than a number: the browser process is the thing that
    /// knows a tab is in the background or that a person has stopped typing.
    pub fn collect(&mut self) {
        self.collect_with(None);
    }

    /// Every invariant of ADR 0014 § 10, checked.
    ///
    /// Meant to be run after every collection under test. It allocates while it
    /// walks, which a collection may not — it is not one, and there is no
    /// moment here where there is nothing to spare.
    ///
    /// # Errors
    ///
    /// [`Broken`], naming the invariant that does not hold and where.
    pub fn check(&self) -> Result<(), Broken> {
        if self.stale > 0 {
            return Err(Broken::StaleEdges { count: self.stale });
        }
        check::run(
            &self.cells,
            &self.generations,
            &self.marks,
            &self.free,
            &self.roots,
            self.held,
        )
    }

    fn collect_with(&mut self, arriving: Option<&T>) {
        // The buffers come out of the heap rather than being made here: a
        // collection allocates nothing it has not already got (ADR 0014 § 8),
        // and what is left in their place holds nothing and is put back before
        // this returns.
        let mut work = mem::replace(&mut self.work, Buffer::spent());
        let mut pairs = mem::replace(&mut self.pairs, Buffer::spent());

        let mut tracer = Tracer::new(&self.generations, &mut self.marks, &mut work, &mut pairs);
        let rescans = collect::mark(&self.cells, &self.roots, arriving, &mut tracer);
        let stale = tracer.stale();

        self.work = work;
        self.pairs = pairs;
        self.stale = self.stale.saturating_add(stale);
        self.rescans = self.rescans.saturating_add(rescans);

        let swept: Swept = collect::sweep(
            &mut self.cells,
            &mut self.generations,
            &self.marks,
            &mut self.free,
        );
        // No mark bit survives a cycle (ADR 0014 § 10): marking starts from
        // all-false, so a bit left behind would keep something alive next time
        // for no reason anybody could find.
        self.marks.fill(false);

        self.live = swept.live;
        self.held = swept.bytes;
        self.retired = self.retired.saturating_add(swept.retired);
        self.since = 0;
        self.collections = self.collections.saturating_add(1);
    }

    /// Wear a slot out, so that the next sweep retires it.
    ///
    /// A test seam, and the narrowest one that closes ADR 0014 § 3's last
    /// paragraph: four billion fills of one slot is not a thing a test can run,
    /// so this sets the number the production path reaches on its own and the
    /// retirement that follows is the ordinary one in [`collect::sweep`]. It
    /// changes no behaviour — there is no branch anywhere that asks whether a
    /// generation was set here or reached.
    #[cfg(test)]
    fn wear_out(&mut self, held: Ref) {
        if let Some(generation) = self.generations.get_mut(held.index()) {
            *generation = reference::RETIRED.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Heap, Ref, Trace, Tracer};

    /// A cell that holds nothing, for the tests that are about slots rather
    /// than about graphs.
    struct Nothing;

    impl Trace for Nothing {
        fn trace(&self, _tracer: &mut Tracer) {}
    }

    fn one(heap: &mut Heap<Nothing>) -> Ref {
        match heap.allocate(Nothing) {
            Ok(held) => held,
            Err(full) => panic!("a heap of one cell is not full: {full}"),
        }
    }

    #[test]
    fn a_slot_filled_again_is_a_different_reference() {
        let mut heap = Heap::new();
        let first = one(&mut heap);
        heap.collect();
        assert_eq!(heap.live(), 0);

        let second = one(&mut heap);
        assert_eq!(second.index(), first.index(), "the slot is reused");
        assert_ne!(second, first, "and the reference to it is not");
        assert!(heap.get(first).is_none(), "the old reference names nothing");
        assert!(heap.get(second).is_some());
    }

    #[test]
    fn a_slot_on_its_last_life_is_retired_rather_than_wrapped() {
        let mut heap = Heap::new();
        let first = one(&mut heap);
        heap.wear_out(first);
        heap.collect();

        assert_eq!(heap.retired(), 1);
        let second = one(&mut heap);
        assert_ne!(
            second.index(),
            first.index(),
            "the retired slot is not handed out again"
        );
        assert_eq!(heap.check(), Ok(()));
    }
}
