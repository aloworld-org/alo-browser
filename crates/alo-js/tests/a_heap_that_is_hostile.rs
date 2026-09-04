/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A graph a stranger's script chose the shape of, and the collector does not
//! stop.
//!
//! `LOOP.md`'s stage 2 clause 2 and ADR 0013 § 4: **a refusal is a result, a
//! crash is a denial of service.** The parser has this file's counterpart, and
//! the graph is the worse of the two problems — item 204's bound of 256 refused
//! nothing because the process aborted before the counter got there, and an
//! object graph has none of a parser's protections. A script builds one with a
//! loop.
//!
//! Every case here ends in one of two ways, which is the whole of what is being
//! asserted: **a collection, or a refusal.** Never an abort, never a panic, and
//! never a wrong answer given quickly.
//!
//! Four shapes, each aimed at a different thing that could be bounded wrongly:
//!
//! - **deep**, at a marker that might call itself (ADR 0014 § 8),
//! - **many**, at an arena that might grow instead of reusing what it swept,
//! - **wide**, at [`bounds::MARKING_WORKLIST`] — a fan larger than the buffer,
//!   which must cost a rescan rather than an object,
//! - **weak**, at [`bounds::MARKING_EPHEMERONS`] — more live weak entries than
//!   the pair buffer holds, which is the bound where being wrong means dropping
//!   an entry a page can still read.
//!
//! And one that is about the ceiling rather than about a shape: a heap that
//! fills up says so.

use alo_js::bounds;
use alo_js::heap::{Field, Heap, Ref, Survivors, Trace, Tracer};

/// The shapes a hostile graph is made of.
enum Knot {
    /// One link of a chain whose length a script chose.
    Link(Field),
    /// One object with a great many children.
    Fan(Vec<Field>),
    /// A weak map with more entries than the marker can remember at once.
    Entries(Vec<(Ref, Ref)>),
    /// A cell that *claims* a great deal without owning any of it.
    ///
    /// [`Trace::footprint`] is how a cell tells the heap what hangs off its
    /// slot, and this one lies on purpose. Reaching a real gibibyte to test the
    /// ceiling would be a test that measures the machine it runs on; declaring
    /// one exercises the same arithmetic and the same refusal.
    Heavy(usize),
}

impl Trace for Knot {
    fn trace(&self, tracer: &mut Tracer) {
        match self {
            Knot::Link(field) => field.trace(tracer),
            Knot::Fan(fields) => {
                for field in fields {
                    field.trace(tracer);
                }
            }
            Knot::Entries(entries) => {
                for (key, value) in entries {
                    tracer.ephemeron(*key, *value);
                }
            }
            Knot::Heavy(_) => {}
        }
    }

    fn footprint(&self) -> usize {
        match self {
            Knot::Heavy(claimed) => *claimed,
            _ => 0,
        }
    }

    fn clear_weak(&mut self, survivors: &Survivors) {
        if let Knot::Entries(entries) = self {
            entries.retain(|(key, _)| survivors.alive(*key));
        }
    }
}

/// See `what_the_collector_reclaims.rs` for why this is a macro.
macro_rules! allocate {
    ($heap:expr, $thing:expr) => {
        match $heap.allocate($thing) {
            Ok(held) => held,
            Err(full) => panic!("this heap holds far less than its ceiling: {full}"),
        }
    };
}

/// A chain a script would write in one line.
const DEEP: usize = 1_000_000;

/// More objects than a page has any reason to make, all of them rubbish.
const MANY: usize = 1_000_000;

#[test]
fn a_graph_a_million_deep_is_marked_without_recursing() {
    // ADR 0014 § 8. A marker written as a recursive walk aborts here, and an
    // abort is not a refusal. The sweep is the same problem from the other end:
    // a million cells are dropped one at a time rather than in a cascade.
    let mut heap: Heap<Knot> = Heap::new();
    let mut top = None;
    let mut previous = None;

    for _ in 0..DEEP {
        let link = allocate!(
            heap,
            Knot::Link(previous.map_or_else(Field::empty, Field::holding))
        );
        // The head of the chain is the one root, and it is moved forward before
        // the old one is let go — the whole chain hangs from it, so a moment
        // with neither rooted would be a moment a collection emptied the heap.
        let next = heap.root(link);
        if let Some(root) = top.replace(next) {
            heap.release(root);
        }
        previous = Some(link);
    }

    heap.collect();
    assert_eq!(heap.live(), DEEP, "every link is reachable from the head");
    assert_eq!(heap.check(), Ok(()));

    if let Some(root) = top.take() {
        heap.release(root);
    }
    heap.collect();
    assert_eq!(
        heap.live(),
        0,
        "and letting go of the head lets go of all of it"
    );
    assert_eq!(heap.check(), Ok(()));
}

#[test]
fn a_million_objects_of_nothing_are_reclaimed_into_the_slots_they_came_from() {
    // The other half of ADR 0014 § 3: a slot is filled again rather than the
    // arena growing. Nothing roots any of these, so what bounds the arena is
    // the trigger — a collection every `COLLECT_AFTER` bytes of allocation —
    // and the number below is that bound rather than a number that felt small
    // enough. A page that makes rubbish in a loop costs the trigger's worth of
    // slots and never more, whether the loop runs a million times or a
    // thousand million.
    let mut heap: Heap<Knot> = Heap::new();
    for _ in 0..MANY {
        allocate!(heap, Knot::Link(Field::empty()));
    }

    heap.collect();
    assert_eq!(heap.live(), 0);
    let most = bounds::COLLECT_AFTER / size_of::<Knot>() + 1;
    assert!(
        heap.slots() <= most,
        "a million objects nobody kept made {} slots, and the trigger allows {most}",
        heap.slots()
    );
    assert!(
        heap.slots() < MANY / 2,
        "which is well under the million that were made"
    );
    assert!(
        heap.collections() > 1,
        "the trigger fired on its own rather than needing to be asked"
    );
    assert_eq!(heap.check(), Ok(()));
}

#[test]
fn a_fan_wider_than_the_worklist_costs_a_rescan_rather_than_an_object() {
    // ADR 0014 § 8: *overflowing it is not a failure — the mark bits are the
    // truth and the worklist is only a list of what to look at next.* The fan is
    // half again as wide as the buffer, and each of its children has a child of
    // its own, so a rescan that only re-looked at the fan would lose the
    // grandchildren.
    let wide = bounds::MARKING_WORKLIST + bounds::MARKING_WORKLIST / 2;
    let mut heap: Heap<Knot> = Heap::new();
    let scope = heap.open();

    let mut children = Vec::with_capacity(wide);
    for _ in 0..wide {
        let grandchild = allocate!(heap, Knot::Link(Field::empty()));
        heap.hold(grandchild);
        let child = allocate!(heap, Knot::Link(Field::holding(grandchild)));
        heap.hold(child);
        children.push(Field::holding(child));
    }
    let fan = allocate!(heap, Knot::Fan(children));
    let root = heap.root(fan);
    heap.close(scope);

    heap.collect();
    assert_eq!(
        heap.live(),
        1 + wide * 2,
        "the fan, its children, and their children"
    );
    assert!(
        heap.rescans() > 0,
        "and the worklist really did refuse something"
    );
    assert_eq!(heap.check(), Ok(()));

    heap.release(root);
    heap.collect();
    assert_eq!(heap.live(), 0);
    assert_eq!(heap.check(), Ok(()));
}

#[test]
fn more_live_weak_entries_than_the_buffer_holds_still_keep_their_values() {
    // The bound where being wrong is worst: a `WeakMap` entry dropped while its
    // key is alive is a value a page can still ask for and will not get. There
    // are half as many entries again as the buffer holds, so the pairs the
    // buffer refuses are the ones this is really about — ADR 0014 § 7's fixpoint
    // has to find them by looking again rather than by remembering them.
    let entries = bounds::MARKING_EPHEMERONS + bounds::MARKING_EPHEMERONS / 2;
    let mut heap: Heap<Knot> = Heap::new();
    let scope = heap.open();

    let mut keys = Vec::with_capacity(entries);
    let mut pairs = Vec::with_capacity(entries);
    for _ in 0..entries {
        let key = allocate!(heap, Knot::Link(Field::empty()));
        heap.hold(key);
        let value = allocate!(heap, Knot::Link(Field::empty()));
        heap.hold(value);
        keys.push(Field::holding(key));
        pairs.push((key, value));
    }
    // The keys are rooted first and the map second, so the map is reached
    // while its keys are still unknown and every pair has to be remembered.
    // That is what fills the buffer: a pair whose key is already marked is
    // settled where it is reported and never stored at all.
    let holder = allocate!(heap, Knot::Fan(keys));
    let held_keys = heap.root(holder);
    let map = allocate!(heap, Knot::Entries(pairs));
    let held_map = heap.root(map);
    heap.close(scope);

    heap.collect();
    assert_eq!(
        heap.live(),
        2 + entries * 2,
        "every value is alive because every key is"
    );
    assert!(
        heap.rescans() > 0,
        "and the pair buffer really did refuse something"
    );
    assert_eq!(heap.check(), Ok(()));

    heap.release(held_keys);
    heap.collect();
    assert_eq!(
        heap.live(),
        1,
        "and every one of them goes when its key does"
    );
    assert_eq!(heap.check(), Ok(()));
    heap.release(held_map);
}

#[test]
fn a_heap_at_its_ceiling_refuses_with_a_reason_rather_than_stopping() {
    // ADR 0014 § 9. A collection first, an error second, and the error goes to
    // whoever can do something about it — never an abort, and never the browser
    // process (ADR 0005).
    let claim = bounds::HEAP_CEILING / 4;
    let mut heap: Heap<Knot> = Heap::new();
    let mut roots = Vec::new();

    let mut refusal = None;
    for _ in 0..8 {
        match heap.allocate(Knot::Heavy(claim)) {
            Ok(held) => roots.push(heap.root(held)),
            Err(full) => {
                refusal = Some(full);
                break;
            }
        }
    }

    let Some(full) = refusal else {
        panic!("eight quarters of the ceiling fitted inside it");
    };
    assert_eq!(full.ceiling, bounds::HEAP_CEILING);
    assert!(full.held <= bounds::HEAP_CEILING);
    assert!(full.to_string().contains("the heap is full"));

    // And the heap is still a heap: letting go of what was held makes room.
    for root in roots.drain(..) {
        heap.release(root);
    }
    heap.collect();
    assert_eq!(heap.live(), 0);
    assert_eq!(heap.check(), Ok(()));
    assert!(
        heap.allocate(Knot::Heavy(claim)).is_ok(),
        "a refusal is not a heap that has stopped"
    );
}
