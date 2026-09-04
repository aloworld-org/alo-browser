/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The invariants, because a collector's defects hide from ordinary tests.
//!
//! ADR 0014 § 10. A collector is the one component whose bugs are invisible in
//! a test suite that does not go looking: **a heap that never collects passes
//! everything**, and a heap that collects something it should not passes
//! everything until the day a page reads it back. So the evidence is built for
//! it rather than hoped for.
//!
//! [`Heap::check`](super::Heap::check) is meant to be called after every
//! collection under test, and it is the one thing in this module allowed to
//! allocate while it walks — it is not a collection, and the rule about a
//! collection allocating nothing exists because the moment we most need to
//! collect is the moment there is nothing spare. There is no such moment here.
//!
//! It walks with the collector's own marker rather than with a second one of
//! its own. ADR 0014 § 1 refuses two mechanisms with two ideas of what is
//! alive; a check that had its own idea would be that mistake made in the place
//! it is hardest to see.

use crate::heap::collect;
use crate::heap::root::Roots;
use crate::heap::trace::{Buffer, Trace, Tracer};

/// A heap that does not hold together.
///
/// Each is a sentence about the heap rather than about the collection that
/// produced it, because the collection is over by the time anybody asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broken {
    /// A strong edge named nothing: a live cell refers to a slot that has been
    /// emptied, retired, or filled again since.
    ///
    /// This is the one in ADR 0014 § 3 — our rooting bug rather than a page's.
    /// Something was collected while a live cell still pointed at it, which
    /// means a root was missed.
    StaleEdges {
        /// How many edges named nothing.
        count: usize,
    },
    /// A slot that is reachable is on the free list.
    ReachableIsFree {
        /// Which slot.
        slot: usize,
    },
    /// A slot on the free list still holds a cell.
    FreeSlotIsFilled {
        /// Which slot.
        slot: usize,
    },
    /// A slot is on the free list twice, so two allocations would be handed the
    /// same one.
    FreeSlotTwice {
        /// Which slot.
        slot: usize,
    },
    /// A mark bit survived the collection that set it.
    ///
    /// Marking starts from all-false, so a bit left behind would make the next
    /// collection keep something it should not — and that is a leak nobody can
    /// attribute, which is the failure this whole design is arranged against.
    MarkSurvived {
        /// Which slot.
        slot: usize,
    },
    /// The bytes the heap believes it holds are not the bytes its live cells
    /// hold.
    ///
    /// The count is what the ceiling in ADR 0014 § 9 is enforced against, so a
    /// count that has drifted is a ceiling that is not being enforced.
    Miscounted {
        /// What the heap believed.
        believed: usize,
        /// What its live cells actually hold.
        actual: usize,
    },
}

/// Every invariant ADR 0014 § 10 names, in the order that a failure of one
/// explains a failure of the next.
pub(in crate::heap) fn run<T: Trace>(
    cells: &[Option<T>],
    generations: &[u32],
    marks: &[bool],
    free: &[usize],
    roots: &Roots,
    believed: usize,
) -> Result<(), Broken> {
    if let Some(slot) = marks.iter().position(|marked| *marked) {
        return Err(Broken::MarkSurvived { slot });
    }

    let mut seen = vec![false; cells.len()];
    for slot in free.iter().copied() {
        match seen.get_mut(slot) {
            Some(already) if *already => return Err(Broken::FreeSlotTwice { slot }),
            Some(already) => *already = true,
            None => return Err(Broken::FreeSlotTwice { slot }),
        }
        if cells.get(slot).is_some_and(Option::is_some) {
            return Err(Broken::FreeSlotIsFilled { slot });
        }
    }

    let mut reachable = vec![false; cells.len()];
    let mut work = Buffer::unbounded();
    let mut pairs = Buffer::unbounded();
    let mut tracer = Tracer::new(generations, &mut reachable, &mut work, &mut pairs);
    let _ = collect::mark(cells, roots, None, &mut tracer);
    let stale = tracer.stale();
    if stale > 0 {
        return Err(Broken::StaleEdges { count: stale });
    }

    for (slot, marked) in reachable.iter().enumerate() {
        if *marked && seen.get(slot).copied() == Some(true) {
            return Err(Broken::ReachableIsFree { slot });
        }
    }

    let actual = cells.iter().flatten().fold(0_usize, |sum, cell| {
        sum.saturating_add(size_of::<T>())
            .saturating_add(cell.footprint())
    });
    if actual != believed {
        return Err(Broken::Miscounted { believed, actual });
    }

    Ok(())
}
