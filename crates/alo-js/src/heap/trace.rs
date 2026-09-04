/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one thing the engine asks of anything in the heap, and the tool it
//! hands over to be told.
//!
//! ADR 0014 § 6: *the engine demands one thing of an embedder object and it is
//! `trace`. Not a free, not a count, not a finaliser: tell the collector which
//! heap references you hold, and it will decide the rest.* [`Trace`] is that
//! demand, and it is why a document's nodes and a script's objects are one
//! graph walked by one collector rather than two mechanisms with two ideas of
//! what is alive.
//!
//! The trait lives here, in `alo-js`, and that is structural rather than
//! tidy — ADR 0013 § 6 gives this crate no dependency on `alo-dom` and gives
//! `alo-dom` none on this one, so stage 1's renderer keeps working with no
//! engine in the process. The crate that implements this for a node is the
//! bindings crate (queue item 80), and it is the only thing that depends on
//! both.
//!
//! # Three kinds of edge, and the collector treats each differently
//!
//! - [`Tracer::edge`] is a **strong** edge: the target is alive because this
//!   cell is.
//! - [`Tracer::ephemeron`] is a **conditional** edge: the value is alive only
//!   while the key is (ADR 0014 § 7). If the key is already marked the value is
//!   marked on the spot; if it is not, the pair is remembered and looked at
//!   again. The mark phase iterates these to a fixpoint, because marking values
//!   always leaks and clearing them in one pass loses entries a chain of maps
//!   keeps live.
//! - A [`Weak`](super::Weak) is **no edge at all** and is simply not reported.
//!   It is cleared at the sweep, by [`Trace::clear_weak`].
//!
//! # And the buffers, which are the reason a collection allocates nothing
//!
//! ADR 0014 § 8: *a collection allocates nothing it has not already got*, since
//! the moment we most need to collect is the moment there is nothing to spare.
//! So the marker's worklist and its list of ephemeron pairs are [`Buffer`]s
//! made once, when the heap is, with a capacity from [`bounds`](crate::bounds)
//! that they never exceed. **Overflowing one is not a failure**: the mark bits
//! are the truth and a buffer is only a list of what to look at next, so an
//! overflow costs a rescan and never correctness.

use crate::heap::reference::{Ref, Weak};

/// What everything in the heap tells the collector.
pub trait Trace {
    /// Report every strong edge and every ephemeron pair this cell holds.
    ///
    /// A [`Weak`](super::Weak) is deliberately not reported: it is not an edge.
    fn trace(&self, tracer: &mut Tracer);

    /// The bytes this cell owns beyond the slot it sits in.
    ///
    /// The slot itself is counted by the heap, which knows how big one is. This
    /// is for what hangs off it — a string's characters, a property table's
    /// entries — and it is what makes the heap's ceiling (ADR 0014 § 9) a bound
    /// on memory rather than on how many objects there happen to be. A cell
    /// that owns nothing beyond its slot leaves it at zero.
    fn footprint(&self) -> usize {
        0
    }

    /// Let go of what did not survive the mark phase.
    ///
    /// Called on every cell that *did* survive, before anything is freed, so
    /// that [`Weak::clear`](super::Weak::clear) and a weak map's own entries
    /// see the marks rather than an emptied slot. A cell holding no weakness at
    /// all leaves this alone.
    fn clear_weak(&mut self, survivors: &Survivors) {
        let _ = survivors;
    }
}

/// Which references came through a collection alive.
///
/// Handed to [`Trace::clear_weak`] during the sweep. It answers about the
/// collection that is happening now: a reference is alive if its slot was
/// marked **and** the slot is still in the life the reference names.
#[derive(Debug, Clone, Copy)]
pub struct Survivors<'a> {
    generations: &'a [u32],
    marks: &'a [bool],
}

impl<'a> Survivors<'a> {
    /// The survivors of a collection, as the sweep sees them.
    pub(in crate::heap) const fn new(generations: &'a [u32], marks: &'a [bool]) -> Self {
        Self { generations, marks }
    }

    /// Whether `held` survived.
    pub fn alive(&self, held: Ref) -> bool {
        let at = held.index();
        self.generations.get(at).copied() == Some(held.generation())
            && self.marks.get(at).copied() == Some(true)
    }

    /// Whether a weak reference's target survived, treating an empty one as
    /// having nothing to lose.
    pub fn kept(&self, weak: &Weak) -> bool {
        weak.get().is_some_and(|held| self.alive(held))
    }
}

/// A list that refuses to grow, and says that it refused.
///
/// The marker's two working sets are these. A [`Vec`] that grew would be an
/// allocation in the middle of a collection, which ADR 0014 § 8 forbids, and
/// its size would be chosen by whoever wrote the script rather than by us —
/// which is `alo-net`'s sentence in a different crate: *a limit somebody else
/// chooses is not a limit*.
#[derive(Debug)]
pub(in crate::heap) struct Buffer<T> {
    items: Vec<T>,
    limit: usize,
    overflowed: bool,
}

impl<T> Buffer<T> {
    /// A buffer that will hold `limit` items and refuse the rest.
    ///
    /// The room is taken once, here, so that no push ever allocates.
    pub(in crate::heap) fn bounded(limit: usize) -> Self {
        Self {
            items: Vec::with_capacity(limit),
            limit,
            overflowed: false,
        }
    }

    /// A buffer with no bound, for the invariant check.
    ///
    /// [`Heap::check`](super::Heap::check) runs under test rather than in a
    /// collection, so it is the one caller allowed to allocate while it walks —
    /// and it must not have a bound of its own, or a heap larger than the
    /// marker's buffer would make it report a fault it invented.
    pub(in crate::heap) fn unbounded() -> Self {
        Self {
            items: Vec::new(),
            limit: usize::MAX,
            overflowed: false,
        }
    }

    /// The placeholder left behind while the real buffer is borrowed out of the
    /// heap for a collection. It holds nothing and refuses everything, and the
    /// original — capacity and all — is put back before the collection returns.
    pub(in crate::heap) const fn spent() -> Self {
        Self {
            items: Vec::new(),
            limit: 0,
            overflowed: false,
        }
    }

    fn push(&mut self, item: T) {
        if self.items.len() < self.limit {
            self.items.push(item);
        } else {
            self.overflowed = true;
        }
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    fn clear(&mut self) {
        self.items.clear();
    }
}

/// What a cell reports its edges to.
///
/// It is the marker's state while one cell is being looked at: the mark bits,
/// the generations that say which references still name anything, and the two
/// buffers. Nothing about it is a cell's business except [`Tracer::edge`] and
/// [`Tracer::ephemeron`].
#[derive(Debug)]
pub struct Tracer<'a> {
    generations: &'a [u32],
    marks: &'a mut [bool],
    work: &'a mut Buffer<Ref>,
    pairs: &'a mut Buffer<(Ref, Ref)>,
    marked: usize,
    stale: usize,
}

impl<'a> Tracer<'a> {
    /// A tracer over one heap's marks and buffers.
    pub(in crate::heap) fn new(
        generations: &'a [u32],
        marks: &'a mut [bool],
        work: &'a mut Buffer<Ref>,
        pairs: &'a mut Buffer<(Ref, Ref)>,
    ) -> Self {
        Self {
            generations,
            marks,
            work,
            pairs,
            marked: 0,
            stale: 0,
        }
    }

    /// This cell keeps `to` alive.
    ///
    /// An edge whose generation no longer matches is counted rather than
    /// followed: ADR 0014 § 3 says a stale strong reference is **our** bug
    /// rather than a page's — something was collected while a live cell still
    /// pointed at it, which means a root was missed — and § 10 says it must
    /// fail a test loudly. [`Heap::stale_edges`](super::Heap::stale_edges) is
    /// where the count is read, and [`Heap::check`](super::Heap::check)
    /// requires it to be zero.
    pub fn edge(&mut self, to: Ref) {
        let at = to.index();
        if self.generations.get(at).copied() != Some(to.generation()) {
            self.stale = self.stale.saturating_add(1);
            return;
        }
        let Some(mark) = self.marks.get_mut(at) else {
            self.stale = self.stale.saturating_add(1);
            return;
        };
        if *mark {
            return;
        }
        *mark = true;
        self.marked = self.marked.saturating_add(1);
        self.work.push(to);
    }

    /// This cell keeps `value` alive only while `key` is.
    ///
    /// A key that is already marked settles the question here rather than in
    /// the buffer, which is the common case — a `WeakMap` whose keys something
    /// holds strongly — and is what keeps the buffer for the pairs that are
    /// genuinely still undecided.
    ///
    /// A pair whose key names nothing is dropped rather than counted stale: a
    /// weak map holding an entry for a key that has already gone is the state
    /// [`Trace::clear_weak`] exists to tidy, and it is not a rooting bug.
    pub fn ephemeron(&mut self, key: Ref, value: Ref) {
        if !self.names_something(key) || !self.names_something(value) {
            return;
        }
        if self.marked(key) {
            self.edge(value);
        } else {
            self.pairs.push((key, value));
        }
    }

    fn names_something(&self, held: Ref) -> bool {
        self.generations.get(held.index()).copied() == Some(held.generation())
    }

    pub(in crate::heap) fn pop(&mut self) -> Option<Ref> {
        self.work.pop()
    }

    pub(in crate::heap) fn marked(&self, held: Ref) -> bool {
        self.names_something(held) && self.marks.get(held.index()).copied() == Some(true)
    }

    pub(in crate::heap) fn marked_slot(&self, at: usize) -> bool {
        self.marks.get(at).copied() == Some(true)
    }

    pub(in crate::heap) fn pair(&self, nth: usize) -> Option<(Ref, Ref)> {
        self.pairs.items.get(nth).copied()
    }

    /// Whether either buffer has refused anything during this collection.
    ///
    /// Sticky until [`Tracer::reset`], because forgetting a pair is a thing the
    /// mark phase has to make up for by looking at every marked cell again —
    /// and having made up for it once says nothing about the round after.
    pub(in crate::heap) const fn refused(&self) -> bool {
        self.work.overflowed || self.pairs.overflowed
    }

    /// How many cells are marked, which is the mark phase's measure of
    /// progress: marking never unmarks and the heap is finite, so a round that
    /// does not raise this is a round that found nothing left to find.
    pub(in crate::heap) const fn marked_count(&self) -> usize {
        self.marked
    }

    /// Forget the ephemeron pairs, before a rescan finds them all again.
    pub(in crate::heap) fn forget_pairs(&mut self) {
        self.pairs.clear();
    }

    /// Empty both buffers, before a mark phase.
    ///
    /// The worklist is empty already — a mark phase ends by draining it — and
    /// the pairs are not, because the fixpoint stops when a pass marks nothing
    /// rather than when the list is exhausted.
    pub(in crate::heap) fn reset(&mut self) {
        self.work.clear();
        self.work.overflowed = false;
        self.pairs.clear();
        self.pairs.overflowed = false;
        self.marked = 0;
    }

    /// How many strong edges named nothing.
    pub(in crate::heap) const fn stale(&self) -> usize {
        self.stale
    }
}
