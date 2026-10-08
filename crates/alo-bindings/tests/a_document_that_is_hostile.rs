/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 248's hostile half (`LOOP.md`, stage 2 § 2).
//!
//! The shapes a stranger's script will build once it can: a detached tree
//! with more wrappers than the marker holds ephemeron pairs, chains deep
//! enough that a recursive walk would end the process, and a long run of
//! operations in an order nobody chose. Each ends in a collection that keeps
//! exactly what is held and releases the rest, with the heap's invariants
//! and the cell's own checked after every one, and never a panic.

use alo_bindings::{DocumentCell, adopt, change_document, document, wrap};
use alo_dom::{Document, NodeId, parse_document};
use alo_js::bounds;
use alo_js::heap::{Ref, Root};
use alo_js::object::Objects;

macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused}", stringify!($call)),
        }
    };
}

macro_rules! collect {
    ($objects:expr) => {{
        $objects.heap_mut().collect();
        if let Err(broken) = $objects.heap().check() {
            panic!("the heap is broken after a collection: {broken:?}");
        }
    }};
}

macro_rules! change {
    ($objects:expr, $cell:expr, |$document:ident| $body:expr) => {
        change_document(&mut $objects, $cell, |$document| $body).expect("a document cell")
    };
}

fn alive(objects: &Objects, cell: Ref, node: NodeId) -> bool {
    document(objects, cell).is_some_and(|d| d.get(node).is_some())
}

/// Every live node, by walking every tree, template contents included.
fn every_live(document: &Document) -> Vec<NodeId> {
    let mut after = None;
    let mut pending = vec![document.root()];
    while let Some(top) = document.next_detached_root(after) {
        after = Some(top);
        pending.push(top);
    }
    let mut live = Vec::new();
    while let Some(top) = pending.pop() {
        for id in std::iter::once(top).chain(document.descendants(top)) {
            live.push(id);
            if let Some(contents) = document.element(id).and_then(|e| e.template_contents) {
                pending.push(contents);
            }
        }
    }
    live
}

/// What is wrong with what a collection kept, or [`None`]: a held node gone,
/// a wrapper for a node that is not live, or a detached tree no wrapper is in.
fn kept_wrongly(objects: &Objects, cell: Ref, held: &[NodeId]) -> Option<String> {
    let d = document(objects, cell)?;
    let table = objects.embedded::<DocumentCell>(cell)?;
    if let Some(gone) = held.iter().find(|node| d.get(**node).is_none()) {
        return Some(format!("{gone} is held and was released"));
    }
    let live = every_live(d);
    let wrapped: Vec<NodeId> = live
        .iter()
        .copied()
        .filter(|node| table.wrapper(*node).is_some())
        .collect();
    if wrapped.len() != table.wrapped() {
        return Some(format!(
            "{} wrappers, {} of them for live nodes",
            table.wrapped(),
            wrapped.len()
        ));
    }
    let roots_held: Vec<NodeId> = wrapped.iter().map(|node| tree_root(d, *node)).collect();
    let mut after = None;
    while let Some(top) = d.next_detached_root(after) {
        after = Some(top);
        if !roots_held.contains(&top) {
            return Some(format!("{top}'s tree is held by nobody and was kept"));
        }
    }
    None
}

/// The root of `node`'s tree, host-including, as a test computes it.
fn tree_root(document: &Document, node: NodeId) -> NodeId {
    let mut at = node;
    while let Some(up) = document.parent(at).or_else(|| document.host(at)) {
        at = up;
    }
    at
}

#[test]
fn a_ring_wider_than_the_markers_room_is_kept_whole() {
    let mut objects = Objects::new();
    let cell = ok!(adopt(&mut objects, Document::new()));
    let root: Root = objects.heap_mut().root(cell);
    let width = bounds::MARKING_EPHEMERONS + 3000;
    let (list, items) = change!(objects, cell, |d| {
        let list = d.create_element("ul").unwrap();
        let items: Vec<NodeId> = (0..width)
            .map(|_| {
                let item = d.create_element("li").unwrap();
                d.append_child(list, item).unwrap();
                item
            })
            .collect();
        (list, items)
    });
    // Twenty thousand elements are within reach of `COLLECT_AFTER` on their
    // own, and a collection that ran part way through making the ring would
    // be the one that rescanned. So one runs now, with the list held so that
    // it survives, and making the ring is all the next collection is owed
    // for. The list is let go before the collection this test is about.
    let list_wrapper = ok!(wrap(&mut objects, cell, list, None));
    let holding_list = objects.heap_mut().root(list_wrapper);
    collect!(objects);
    let wrappers: Vec<Ref> = items
        .iter()
        .map(|item| ok!(wrap(&mut objects, cell, *item, None)))
        .collect();
    // The last of the ring held: every pair before it is undecided when the
    // cell reports it, so the marker's room overflows, and only a rescan
    // reaches the first wrapper from the last.
    let held = objects.heap_mut().root(wrappers[width - 1]);
    objects.heap_mut().release(holding_list);

    let rescans = objects.heap().rescans();
    collect!(objects);
    assert!(
        objects.heap().rescans() > rescans,
        "the ring overflowed the marker's room and cost a rescan"
    );
    for (item, wrapper) in items.iter().zip(&wrappers) {
        assert!(objects.heap().live_at(*wrapper), "{item}'s wrapper");
    }
    assert!(alive(&objects, cell, list));

    objects.heap_mut().release(held);
    collect!(objects);
    assert!(!alive(&objects, cell, list));
    assert!(wrappers.iter().all(|w| !objects.heap().live_at(*w)));
    objects.heap_mut().release(root);
}

#[test]
fn a_chain_too_deep_to_recurse_is_walked_and_released() {
    let mut objects = Objects::new();
    let cell = ok!(adopt(&mut objects, parse_document("<body></body>")));
    let root: Root = objects.heap_mut().root(cell);
    let depth = 200_000;
    // Built from the bottom up: inserting into a node checks its ancestry,
    // so a chain grown downwards costs its depth per link.
    let (top, bottom) = change!(objects, cell, |d| {
        let bottom = d.create_element("div").unwrap();
        let mut top = bottom;
        for _ in 0..depth {
            let above = d.create_element("div").unwrap();
            d.append_child(above, top).unwrap();
            top = above;
        }
        (top, bottom)
    });

    let deepest = ok!(wrap(&mut objects, cell, bottom, None));
    let held = objects.heap_mut().root(deepest);
    collect!(objects);
    assert!(alive(&objects, cell, top), "held from the bottom");

    objects.heap_mut().release(held);
    collect!(objects);
    assert!(!alive(&objects, cell, top));
    assert!(!alive(&objects, cell, bottom));
    let released = objects
        .embedded::<DocumentCell>(cell)
        .map(DocumentCell::released)
        .unwrap_or_default();
    assert_eq!(released.nodes, depth + 1);
    objects.heap_mut().release(root);
}

#[test]
fn a_chain_in_the_page_with_every_link_held_costs_one_walk() {
    // Every link wrapped and none rooted: each is kept by being in the
    // document's tree. Asking each wrapper for its root would cost the depth
    // per link; the cell walks the tree once.
    let mut objects = Objects::new();
    let cell = ok!(adopt(&mut objects, parse_document("<body></body>")));
    let root: Root = objects.heap_mut().root(cell);
    let depth = 20_000;
    let links = change!(objects, cell, |d| {
        let body = d
            .descendants(d.root())
            .find(|id| d.get(*id).is_some_and(|n| n.is_html_element("body")))
            .unwrap();
        let mut links: Vec<NodeId> = vec![d.create_element("div").unwrap()];
        for _ in 1..depth {
            let above = d.create_element("div").unwrap();
            d.append_child(above, links[links.len() - 1]).unwrap();
            links.push(above);
        }
        d.append_child(body, links[links.len() - 1]).unwrap();
        links
    });
    let wrappers: Vec<Ref> = links
        .iter()
        .map(|link| ok!(wrap(&mut objects, cell, *link, None)))
        .collect();
    for _ in 0..3 {
        collect!(objects);
    }
    for (link, wrapper) in links.iter().zip(&wrappers) {
        assert_eq!(ok!(wrap(&mut objects, cell, *link, None)), *wrapper);
    }
    objects.heap_mut().release(root);
}

#[test]
fn a_long_run_of_operations_keeps_exactly_what_is_held() {
    // A fixed pseudo-random run: make, insert, remove, wrap, hold, let go and
    // collect, in an order nobody chose. After every collection: every held
    // wrapper's node is alive, every table entry names a live node, every
    // detached tree left has a wrapped node in it, and the heap is sound.
    let mut objects = Objects::new();
    let cell = ok!(adopt(
        &mut objects,
        parse_document("<body><p>a</p><template><i>t</i></template></body>")
    ));
    let root: Root = objects.heap_mut().root(cell);
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut roll = |below: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        usize::try_from(seed % u64::try_from(below.max(1)).unwrap()).unwrap()
    };
    let mut held: Vec<(Root, NodeId)> = Vec::new();

    for step in 0..4000 {
        let live = every_live(document(&objects, cell).unwrap());
        let pick = |n: usize| live.get(n % live.len().max(1)).copied();
        match roll(8) {
            0 | 1 => {
                change!(objects, cell, |d| {
                    let made = d.create_element(["div", "template", "b"][roll(3)]).unwrap();
                    if roll(2) == 0
                        && let Some(parent) = pick(roll(1 << 20))
                    {
                        let _ = d.append_child(parent, made);
                    }
                });
            }
            2 => {
                if let (Some(parent), Some(child)) = (pick(roll(1 << 20)), pick(roll(1 << 20))) {
                    change!(objects, cell, |d| {
                        let _ = d.append_child(parent, child);
                    });
                }
            }
            3 => {
                if let Some(node) = pick(roll(1 << 20)) {
                    change!(objects, cell, |d| {
                        let _ = d.remove(node);
                    });
                }
            }
            4 | 5 => {
                if let Some(node) = pick(roll(1 << 20)) {
                    let wrapper = ok!(wrap(&mut objects, cell, node, None));
                    if roll(2) == 0 {
                        held.push((objects.heap_mut().root(wrapper), node));
                    }
                }
            }
            6 => {
                if !held.is_empty() {
                    let (gone, _) = held.swap_remove(roll(held.len()));
                    objects.heap_mut().release(gone);
                }
            }
            _ => {
                collect!(objects);
                let nodes: Vec<NodeId> = held.iter().map(|(_, node)| *node).collect();
                assert_eq!(kept_wrongly(&objects, cell, &nodes), None, "step {step}");
            }
        }
    }
    for (gone, _) in held {
        objects.heap_mut().release(gone);
    }
    collect!(objects);
    let d = document(&objects, cell).unwrap();
    assert_eq!(
        d.next_detached_root(None),
        None,
        "nothing held, nothing left"
    );
    objects.heap_mut().release(root);
}
