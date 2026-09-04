/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is alive, what is not, and what a page can tell the difference by.
//!
//! ADR 0014 § 10. A collector is the one component whose defects are invisible
//! in a test suite that does not go looking for them: **a heap that never
//! collects passes everything**, and a heap that collects one thing too many
//! passes everything until the day a page reads it back. So the evidence is
//! built rather than hoped for, and two rules shape every case here.
//!
//! **Collection is explicit**, never triggered by pressure, so a case asserts
//! *what was reclaimed* rather than *whether a collection happened*. That is
//! ADR 0013 § 5's property — testable with nothing moving — applied to the part
//! of the engine that is otherwise the most timing-dependent thing in it.
//!
//! **A cycle is counted rather than watched.** Every case here asserts
//! [`Heap::live`], which is a number the heap knows. A test that watched the
//! process's memory would be measuring the allocator.
//!
//! # The one that is standing in for something
//!
//! ADR 0014 § 6 puts the DOM in this graph, and `a_cycle_through_an_embedder_
//! object_is_reclaimed` is that clause: a node, a listener on it, and a closure
//! that mentions the node — the cycle every browser leaked for a decade. The
//! node here is a **test cell implementing [`Trace`]** rather than a real
//! `alo-dom` node, because the crate that joins the two is the bindings crate
//! (queue item 80) and neither crate may depend on the other before it exists
//! (ADR 0013 § 6). What is being asserted is the engine's half, which is all of
//! it that this queue item owns: one graph, one collector, reachability
//! deciding.

use std::rc::Rc;

use alo_js::heap::{Field, Full, Heap, Ref, Survivors, Trace, Tracer, Weak};

/// A native resource behind an object, released by [`Drop`] at the sweep.
///
/// ADR 0014 § 7: *nothing the engine owns is released by a finaliser.* This is
/// what safe Rust and a non-moving heap give that a C++ engine pays for with a
/// finaliser queue — and it is why a page cannot hold an operating-system
/// resource open by never running a callback.
struct Resource(Rc<std::cell::Cell<usize>>);

impl Drop for Resource {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_add(1));
    }
}

/// The kinds of thing this heap holds, which is as much of an object model as
/// a test of reachability needs. The real one is queue item 206.
enum Thing {
    /// An ordinary object: strong fields, and a declared weight so that the
    /// heap's byte count has something to count.
    Object { fields: Vec<Field>, weight: usize },
    /// A `WeakMap`: entries whose value is alive only while the key is.
    WeakMap(Vec<(Ref, Ref)>),
    /// A `WeakRef`.
    WeakRef(Weak),
    /// Something an embedder put here — a node, and what is listening on it.
    Node { listeners: Vec<Field> },
    /// An object with a native resource behind it.
    Native(Resource),
}

impl Thing {
    /// How many times the native resource behind this object has been
    /// released, or [`None`] if there is none behind it.
    fn releases(&self) -> Option<usize> {
        match self {
            Thing::Native(resource) => Some(resource.0.get()),
            _ => None,
        }
    }
}

impl Trace for Thing {
    fn trace(&self, tracer: &mut Tracer) {
        match self {
            Thing::Object { fields, .. } | Thing::Node { listeners: fields } => {
                for field in fields {
                    field.trace(tracer);
                }
            }
            Thing::WeakMap(entries) => {
                for (key, value) in entries {
                    tracer.ephemeron(*key, *value);
                }
            }
            Thing::WeakRef(_) | Thing::Native(_) => {}
        }
    }

    fn footprint(&self) -> usize {
        match self {
            Thing::Object { weight, .. } => *weight,
            _ => 0,
        }
    }

    fn clear_weak(&mut self, survivors: &Survivors) {
        match self {
            Thing::WeakMap(entries) => entries.retain(|(key, _)| survivors.alive(*key)),
            Thing::WeakRef(weak) => {
                weak.clear(survivors);
            }
            _ => {}
        }
    }
}

/// Allocate, reporting a refusal rather than swallowing it.
///
/// A macro rather than a function, and the reason is worth a sentence because
/// it will come up in every test file this engine grows: a `panic!` in a plain
/// helper is production code as far as the lints are concerned, and the lints
/// are right — the gate forbids the panic family everywhere except a test, and
/// a helper is not one. Expanded at the call site, the failure is reported by
/// the test that caused it.
macro_rules! allocate {
    ($heap:expr, $thing:expr) => {
        match $heap.allocate($thing) {
            Ok(held) => held,
            Err(full) => panic!("a heap holding almost nothing refused an allocation: {full}"),
        }
    };
}

/// An object holding strong references to the given slice.
macro_rules! object {
    ($heap:expr, $to:expr) => {{
        let fields: Vec<Field> = $to.iter().copied().map(Field::holding).collect();
        allocate!($heap, Thing::Object { fields, weight: 0 })
    }};
}

/// An object with room for a number of references and none in it yet.
macro_rules! room_for {
    ($heap:expr, $fields:expr) => {{
        let fields: Vec<Field> = (0..$fields).map(|_| Field::empty()).collect();
        allocate!($heap, Thing::Object { fields, weight: 0 })
    }};
}

/// Point `from`'s first field at `to`, through the barrier.
fn point(heap: &mut Heap<Thing>, from: Ref, to: Option<Ref>) {
    let stored = heap.write(from, |thing, barrier| match thing {
        Thing::Object { fields, .. } | Thing::Node { listeners: fields } => {
            match fields.first_mut() {
                Some(field) => {
                    field.set(barrier, to);
                    true
                }
                None => false,
            }
        }
        _ => false,
    });
    assert_eq!(
        stored,
        Some(true),
        "the cell was there and it had a field to write"
    );
}

fn settled(heap: &mut Heap<Thing>) {
    heap.collect();
    assert_eq!(
        heap.check(),
        Ok(()),
        "the invariants of ADR 0014 § 10 hold after a collection"
    );
}

#[test]
fn an_object_nothing_reaches_is_reclaimed() {
    let mut heap = Heap::new();
    object!(heap, &[]);
    object!(heap, &[]);
    assert_eq!(heap.live(), 2);

    settled(&mut heap);
    assert_eq!(heap.live(), 0);
}

#[test]
fn a_root_keeps_everything_it_reaches() {
    let mut heap = Heap::new();
    let last = object!(heap, &[]);
    let middle = object!(heap, &[last]);
    let first = object!(heap, &[middle]);
    let document = heap.root(first);

    settled(&mut heap);
    assert_eq!(
        heap.live(),
        3,
        "reachability decides, and nothing else does"
    );
    assert!(heap.get(last).is_some());

    heap.release(document);
    settled(&mut heap);
    assert_eq!(heap.live(), 0);
}

#[test]
fn a_cycle_is_reclaimed_and_counted() {
    // ADR 0014 § 1, and the reason reference counting is refused: this is not a
    // corner, it is the first line of most pages.
    let mut heap = Heap::new();
    let button = room_for!(heap, 1);
    let listener = object!(heap, &[button]);
    point(&mut heap, button, Some(listener));
    let page = heap.root(button);

    settled(&mut heap);
    assert_eq!(heap.live(), 2, "a cycle a root reaches is alive");

    heap.release(page);
    settled(&mut heap);
    assert_eq!(
        heap.live(),
        0,
        "and a cycle nothing reaches is gone, which a count never sees"
    );
}

#[test]
fn a_cycle_through_an_embedder_object_is_reclaimed() {
    // ADR 0014 § 6. The node holds a listener, the listener closes over the
    // node. Two mechanisms would each see a reason to keep it; one sees none.
    let mut heap = Heap::new();
    let node = allocate!(
        heap,
        Thing::Node {
            listeners: vec![Field::empty()]
        }
    );
    let closure = object!(heap, &[node]);
    point(&mut heap, node, Some(closure));

    let document = heap.root(node);
    settled(&mut heap);
    assert_eq!(heap.live(), 2);

    heap.release(document);
    settled(&mut heap);
    assert_eq!(
        heap.live(),
        0,
        "the document let go of the node, so the cycle went with it"
    );
}

#[test]
fn a_scope_is_what_native_code_holds_across_an_allocation() {
    // ADR 0014 § 2's closed list, and the half of it a builtin uses.
    let mut heap = Heap::new();
    heap.stress(true);

    let scope = heap.open();
    let kept = object!(heap, &[]);
    heap.hold(kept);
    let dropped = object!(heap, &[]);

    object!(heap, &[]);
    assert!(
        heap.get(kept).is_some(),
        "what a scope holds survives a collection"
    );
    assert!(heap.get(dropped).is_none(), "what nothing holds does not");

    heap.close(scope);
    assert_eq!(heap.scoped(), 0);
    settled(&mut heap);
    assert_eq!(heap.live(), 0);
}

#[test]
fn stress_mode_is_what_finds_a_reference_nobody_rooted() {
    // The bug § 10 says is otherwise invisible: correct in every ordinary run,
    // wrong only here. Both halves are asserted, because a mode that reclaimed
    // everything would pass the first half by doing nothing useful.
    let mut heap = Heap::new();
    let unrooted = object!(heap, &[]);
    let rooted = object!(heap, &[]);
    let held = heap.root(rooted);

    heap.stress(true);
    object!(heap, &[]);

    assert!(
        heap.get(unrooted).is_none(),
        "a reference in a Rust local did not survive"
    );
    assert!(heap.get(rooted).is_some(), "one in a root did");
    assert_eq!(heap.holding(&held), Some(rooted));
}

#[test]
fn the_cell_being_allocated_is_not_collected_while_it_is_being_allocated() {
    // The one thing the discipline does not have to cover: the references a
    // cell carries in are traced for the collection its own allocation caused.
    let mut heap = Heap::new();
    heap.stress(true);

    let target = object!(heap, &[]);
    // Nothing roots `target`. The allocation below collects before it places,
    // and the only thing reaching `target` at that moment is the cell arriving.
    let holder = object!(heap, &[target]);
    assert!(
        heap.get(target).is_some(),
        "the cell that arrived holding it was traced"
    );

    let holder = heap.root(holder);
    heap.collect();
    assert_eq!(heap.live(), 2);
    assert_eq!(heap.check(), Ok(()));
    heap.release(holder);
}

#[test]
fn a_weak_map_entry_lives_exactly_as_long_as_its_key() {
    let mut heap = Heap::new();
    let key = object!(heap, &[]);
    let value = object!(heap, &[]);
    let map = allocate!(heap, Thing::WeakMap(vec![(key, value)]));

    let held_map = heap.root(map);
    let held_key = heap.root(key);
    settled(&mut heap);
    assert_eq!(heap.live(), 3, "the value is alive because the key is");
    assert!(heap.get(value).is_some());

    heap.release(held_key);
    settled(&mut heap);
    assert_eq!(heap.live(), 1, "the key went and took the value with it");
    assert!(heap.get(value).is_none());

    let entries = heap.get(map).map(|thing| match thing {
        Thing::WeakMap(entries) => entries.len(),
        _ => usize::MAX,
    });
    assert_eq!(
        entries,
        Some(0),
        "and the entry was cleared rather than left naming nothing"
    );
    heap.release(held_map);
}

#[test]
fn a_chain_of_weak_maps_keeps_what_a_single_pass_would_lose() {
    // ADR 0014 § 7: *mark values always and it leaks, clear them in one pass and
    // a chain of maps loses entries that are live.* The entries are written in
    // the order that breaks a single pass — the one whose key is not yet known
    // to be alive comes first — so this fails against a marker without a
    // fixpoint and passes against one with it.
    let mut heap = Heap::new();
    let first = object!(heap, &[]);
    let second = object!(heap, &[]);
    let third = object!(heap, &[]);
    let map = allocate!(heap, Thing::WeakMap(vec![(second, third), (first, second)]));

    let held_map = heap.root(map);
    let held_first = heap.root(first);
    settled(&mut heap);
    assert_eq!(heap.live(), 4);
    assert!(
        heap.get(third).is_some(),
        "the second entry's key was reached by the first entry"
    );

    heap.release(held_first);
    settled(&mut heap);
    assert_eq!(
        heap.live(),
        1,
        "and letting go of the head of the chain drops all of it"
    );
    heap.release(held_map);
}

#[test]
fn a_weak_reference_is_cleared_rather_than_left_naming_nothing() {
    let mut heap = Heap::new();
    let target = object!(heap, &[]);
    let watcher = allocate!(heap, Thing::WeakRef(Weak::to(target)));
    let held = heap.root(watcher);

    settled(&mut heap);
    assert_eq!(heap.live(), 1, "a weak reference is not an edge");

    let still_held = heap.get(watcher).and_then(|thing| match thing {
        Thing::WeakRef(weak) => Some(weak.get()),
        _ => None,
    });
    assert_eq!(
        still_held,
        Some(None),
        "it was cleared at the sweep that took its target"
    );
    heap.release(held);
}

#[test]
fn a_weak_reference_that_was_read_survives_the_rest_of_the_job() {
    // ADR 0014 § 7's last rule. Without it a script sees one reference answer
    // twice differently inside a single job, which the language does not permit.
    let mut heap = Heap::new();
    let target = object!(heap, &[]);
    heap.keep_alive(target);

    settled(&mut heap);
    assert_eq!(heap.live(), 1);
    assert_eq!(heap.kept(), 1);

    heap.end_job();
    settled(&mut heap);
    assert_eq!(heap.live(), 0, "and the job ending is what lets go of it");
}

#[test]
fn a_native_resource_is_released_at_the_sweep_rather_than_by_a_callback() {
    let mut heap = Heap::new();
    let released = Rc::new(std::cell::Cell::new(0));
    let native = allocate!(heap, Thing::Native(Resource(Rc::clone(&released))));

    assert_eq!(heap.get(native).and_then(Thing::releases), Some(0));
    settled(&mut heap);
    assert_eq!(
        released.get(),
        1,
        "Drop ran when the slot was swept, deterministically"
    );
}

#[test]
fn every_store_of_a_reference_goes_through_the_barrier() {
    // ADR 0014 § 5. The barrier does nothing today, so the count is the only
    // evidence that incremental marking would be a change rather than an audit.
    let mut heap = Heap::new();
    let holder = room_for!(heap, 1);
    let target = object!(heap, &[]);
    let before = heap.stores();

    point(&mut heap, holder, Some(target));
    assert_eq!(heap.stores(), before + 1);

    let held = heap.root(holder);
    settled(&mut heap);
    assert_eq!(
        heap.live(),
        2,
        "and the reference that was stored is an edge the marker follows"
    );

    point(&mut heap, holder, None);
    assert_eq!(heap.stores(), before + 2);
    settled(&mut heap);
    assert_eq!(heap.live(), 1, "as is the reference that was taken away");
    heap.release(held);
}

#[test]
fn the_heap_counts_what_its_cells_hold() {
    // The count the ceiling in ADR 0014 § 9 is enforced against. It is checked
    // rather than assumed, because a count that has drifted is a ceiling that is
    // not being enforced.
    let mut heap = Heap::new();
    let held = allocate!(
        heap,
        Thing::Object {
            fields: Vec::new(),
            weight: 4096
        }
    );
    let root = heap.root(held);
    assert!(heap.held() >= 4096);

    settled(&mut heap);
    assert!(
        heap.held() >= 4096,
        "a live cell's footprint survives the collection it survives"
    );

    heap.release(root);
    settled(&mut heap);
    assert!(heap.held() < 4096, "and goes with the cell");
}

#[test]
fn a_reference_to_a_slot_that_was_filled_again_names_nothing() {
    // ADR 0014 § 3, from the outside. In an engine with pointers this is a
    // use-after-free; here it is an answer.
    let mut heap = Heap::new();
    let first = object!(heap, &[]);
    settled(&mut heap);

    let second = object!(heap, &[]);
    assert_eq!(second.index(), first.index());
    assert!(heap.get(first).is_none());
    assert!(heap.get(second).is_some());
    assert_eq!(heap.stale_edges(), 0, "and nothing followed the old one");
}

#[test]
fn a_full_heap_is_a_refusal_with_a_reason_in_words() {
    // The message a person eventually sees, asserted here so that a reworded
    // one fails a test rather than nothing.
    let full = Full {
        asked: 8,
        held: 16,
        ceiling: 24,
    };
    assert!(full.to_string().contains("the heap is full"));
    assert!(full.to_string().contains("24"));
}
