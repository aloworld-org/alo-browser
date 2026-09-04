/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Mark from the roots, sweep what is unmarked, move nothing.
//!
//! ADR 0014 § 4. Correct before fast (law 3), so the first collector is the one
//! whose invariants fit on a page. Non-moving costs fragmentation and locality
//! and both are permanent until somebody has numbers; what it buys is that a
//! slot's contents are an ordinary Rust value with an ordinary [`Drop`], which
//! is why nothing in this engine is released by a finaliser (§ 7) and why a
//! decoded image behind an object is freed at the sweep rather than whenever a
//! page gets round to it.
//!
//! # The marker does not call itself
//!
//! ADR 0014 § 8, and item 204's finding restated for a graph that is worse: a
//! script can build a list a million objects deep in one line, and a marker
//! written as a recursive walk aborts the process. An abort is not a refusal.
//!
//! So marking is an explicit worklist, and the worklist is
//! [bounded](crate::bounds::MARKING_WORKLIST) because it is itself an
//! allocation whose size a stranger's script would otherwise choose. Overflowing
//! it is **not a failure**: the mark bits are the truth and the worklist is only
//! a list of what to look at next, so an overflow costs a [`rescan`] and never
//! correctness.
//!
//! # And it runs to completion
//!
//! Once begun a collection finishes. A half-marked heap is not a state anything
//! can resume from, and the work is bounded rather than open because the live
//! heap is bounded by the ceiling in [`bounds`](crate::bounds).

use crate::heap::reference::{RETIRED, next_life};
use crate::heap::root::Roots;
use crate::heap::trace::{Survivors, Trace, Tracer};

/// Mark everything reachable, and nothing else.
///
/// `arriving` is the cell on its way into the heap when an allocation was the
/// thing that triggered this collection. It is traced as a root: it is about to
/// be live, it is right here, and the alternative is that the object a caller is
/// halfway through building is collected out from under it — a discipline that
/// no amount of care makes reasonable and that ADR 0014 § 2 never asked for.
///
/// # The one loop, and why it ends where it does
///
/// A round follows every strong edge, then looks at every ephemeron pair whose
/// key has since been marked, then — **only if a buffer has refused
/// something** — looks at every marked cell again. It returns when a whole
/// round marks nothing new.
///
/// That condition is what makes a bounded pair buffer safe. A pair the buffer
/// could not hold is not lost, it is *forgotten*, and the rescan derives it
/// again from the cell that holds it — by which time its key may be marked, in
/// which case [`Tracer::ephemeron`] settles it on the spot rather than needing
/// anywhere to put it. So the last thing a collection with an overflow in it
/// does is a full pass that found nothing, and there is no pair it can have
/// failed to consider.
///
/// It terminates because marking never unmarks and the heap is finite:
/// [`Tracer::marked_count`] rises or the loop is over.
///
/// It answers with how many rescans it needed, which is nothing to the
/// collector and is the only way a test can tell that a buffer really did
/// refuse something — a bound that is never reached is a bound nobody has
/// checked is reachable, which is what item 204's `DEEPEST_NESTING` had to
/// learn the hard way.
pub(in crate::heap) fn mark<T: Trace>(
    cells: &[Option<T>],
    roots: &Roots,
    arriving: Option<&T>,
    tracer: &mut Tracer,
) -> usize {
    tracer.reset();
    for root in roots.each() {
        tracer.edge(root);
    }
    if let Some(cell) = arriving {
        cell.trace(tracer);
    }

    let mut rescans = 0_usize;
    loop {
        drain(cells, tracer);
        let before = tracer.marked_count();

        discharge(tracer);

        if tracer.refused() {
            tracer.forget_pairs();
            rescan(cells, tracer);
            rescans = rescans.saturating_add(1);
        }

        if tracer.marked_count() == before {
            return rescans;
        }
    }
}

/// Follow edges until there is nothing left to look at.
fn drain<T: Trace>(cells: &[Option<T>], tracer: &mut Tracer) {
    while let Some(held) = tracer.pop() {
        if let Some(Some(cell)) = cells.get(held.index()) {
            cell.trace(tracer);
        }
    }
}

/// Mark the value of every remembered pair whose key has since been marked.
///
/// This is the fixpoint of ADR 0014 § 7 in its cheap form: the pairs that were
/// undecided when they were reported, asked again. Marking a value can mark a
/// key of another pair, which is why the round repeats rather than passing once.
fn discharge(tracer: &mut Tracer) {
    let mut nth = 0;
    while let Some((key, value)) = tracer.pair(nth) {
        nth = nth.saturating_add(1);
        if tracer.marked(key) {
            tracer.edge(value);
        }
    }
}

/// Look at every marked cell again.
///
/// The answer to a buffer that refused something. It re-reports every edge and
/// every pair the marked cells hold: an edge to something already marked costs
/// a comparison, and a pair whose key is marked is settled without being
/// remembered at all.
fn rescan<T: Trace>(cells: &[Option<T>], tracer: &mut Tracer) {
    for (at, cell) in cells.iter().enumerate() {
        if let Some(cell) = cell {
            if tracer.marked_slot(at) {
                cell.trace(tracer);
            }
        }
    }
}

/// What a sweep found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::heap) struct Swept {
    /// How many cells are still alive.
    pub(in crate::heap) live: usize,
    /// What those cells hold, in bytes.
    pub(in crate::heap) bytes: usize,
    /// How many slots were freed.
    pub(in crate::heap) freed: usize,
    /// How many slots will never be filled again.
    pub(in crate::heap) retired: usize,
}

/// Clear what the survivors held weakly, then free what did not survive.
///
/// In that order, and the order is the whole of it: a weak reference is cleared
/// while the marks still say what died, and a cell is dropped only after
/// everything that might have wanted to notice its death has been asked.
pub(in crate::heap) fn sweep<T: Trace>(
    cells: &mut [Option<T>],
    generations: &mut [u32],
    marks: &[bool],
    free: &mut Vec<usize>,
) -> Swept {
    let mut swept = Swept {
        live: 0,
        bytes: 0,
        freed: 0,
        retired: 0,
    };

    {
        let survivors = Survivors::new(generations, marks);
        for (at, slot) in cells.iter_mut().enumerate() {
            if marks.get(at).copied() != Some(true) {
                continue;
            }
            if let Some(cell) = slot {
                cell.clear_weak(&survivors);
                swept.live = swept.live.saturating_add(1);
                swept.bytes = swept
                    .bytes
                    .saturating_add(size_of::<T>())
                    .saturating_add(cell.footprint());
            }
        }
    }

    for (at, slot) in cells.iter_mut().enumerate() {
        if marks.get(at).copied() == Some(true) || slot.is_none() {
            continue;
        }
        // `Drop` runs here, which is where a native resource behind an object
        // is released — deterministically, by Rust, rather than by a finaliser
        // a page can decline to run (ADR 0014 § 7).
        *slot = None;
        let Some(generation) = generations.get_mut(at) else {
            continue;
        };
        if let Some(next) = next_life(*generation) {
            *generation = next;
            free.push(at);
            swept.freed = swept.freed.saturating_add(1);
        } else {
            *generation = RETIRED;
            swept.retired = swept.retired.saturating_add(1);
        }
    }

    swept
}
