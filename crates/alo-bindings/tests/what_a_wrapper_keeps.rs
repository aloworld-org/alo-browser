/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 248: a page's document in its heap, and a wrapper that lives
//! as long as its tree (ADR 0017 §§ 2–4).
//!
//! No script runs here — no member exists yet for one to call (item 249).
//! What is asserted is the clause a script will observe: one node is one
//! object, its expandos survive any number of collections while its tree is
//! reachable, a detached tree nothing holds is freed and its ids answer
//! nothing, and a detached tree something holds is not — counted, after
//! forced collections, with the heap's own invariants checked after each.

use alo_bindings::{
    DocumentCell, Released, Wrapping, adopt, change_document, document, node_of, wrap,
};
use alo_dom::{Document, NodeId, parse_document};
use alo_js::heap::{Ref, Root};
use alo_js::object::{Found, Key, Objects, Property, Value};

/// Take what a call answered, reporting a refusal as the failure it is. A
/// macro because the panic family is denied outside a `#[test]`.
macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused}", stringify!($call)),
        }
    };
}

/// Collect, and check every invariant ADR 0014 § 10 names.
macro_rules! collect {
    ($objects:expr) => {{
        $objects.heap_mut().collect();
        if let Err(broken) = $objects.heap().check() {
            panic!("the heap is broken after a collection: {broken:?}");
        }
    }};
}

/// Change the document in the cell, answering what the change answered.
macro_rules! change {
    ($objects:expr, $cell:expr, |$document:ident| $body:expr) => {
        change_document(&mut $objects, $cell, |$document| $body).expect("a document cell")
    };
}

fn units(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}

/// A page's document, adopted and rooted, as the renderer will hold it.
macro_rules! adopted {
    ($objects:expr, $markup:expr) => {{
        let cell = ok!(adopt(&mut $objects, parse_document($markup)));
        let root: Root = $objects.heap_mut().root(cell);
        (cell, root)
    }};
}

fn first(document: &Document, local: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|id| document.get(*id).is_some_and(|n| n.is_html_element(local)))
}

fn released(objects: &Objects, cell: Ref) -> Released {
    objects
        .embedded::<DocumentCell>(cell)
        .map(DocumentCell::released)
        .unwrap_or_default()
}

fn wrapped(objects: &Objects, cell: Ref) -> usize {
    objects
        .embedded::<DocumentCell>(cell)
        .map_or(0, DocumentCell::wrapped)
}

fn alive(objects: &Objects, cell: Ref, node: NodeId) -> bool {
    document(objects, cell).is_some_and(|d| d.get(node).is_some())
}

#[test]
fn one_node_asked_for_twice_is_one_object_and_its_expando_survives() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<p>one</p><p>two</p>");
    let paragraph = first(document(&objects, cell).unwrap(), "p").unwrap();
    let ids_before: Vec<NodeId> = {
        let d = document(&objects, cell).unwrap();
        d.descendants(d.root()).collect()
    };

    let wrapper = ok!(wrap(&mut objects, cell, paragraph, None));
    assert_eq!(ok!(wrap(&mut objects, cell, paragraph, None)), wrapper);
    let key: Key = ok!(objects.key(&units("seen")));
    assert!(ok!(objects.define(
        wrapper,
        key,
        Property::plain(Value::Number(7.0))
    )));

    // Nothing roots the wrapper: only the document does, and the node is in
    // the document's tree, so the wrapper is a strong edge of the cell.
    for _ in 0..3 {
        collect!(objects);
    }
    let again = ok!(wrap(&mut objects, cell, paragraph, None));
    assert_eq!(again, wrapper, "the same object, not a new one");
    let key = ok!(objects.key(&units("seen")));
    assert_eq!(
        ok!(objects.get(again, key)),
        Found::Value(Value::Number(7.0))
    );
    assert_eq!(node_of(&objects, again), Some((cell, paragraph)));

    let d = document(&objects, cell).unwrap();
    let ids_after: Vec<NodeId> = d.descendants(d.root()).collect();
    assert_eq!(ids_after, ids_before, "every parsed node keeps its id");
    assert_eq!(released(&objects, cell), Released::default());
    objects.heap_mut().release(root);
}

#[test]
fn a_detached_tree_no_script_holds_is_freed_and_one_held_is_not() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<p>page</p>");
    let highest = document(&objects, cell).unwrap().node_count();
    let (outer, inner, loose) = change!(objects, cell, |d| {
        let outer = d.create_element("div").unwrap();
        let inner = d.create_element("span").unwrap();
        d.append_child(outer, inner).unwrap();
        (outer, inner, d.create_text_node("nobody"))
    });
    assert_eq!(outer.as_usize(), highest, "numbered after the parse");

    let wrapper = ok!(wrap(&mut objects, cell, inner, None));
    let held = objects.heap_mut().root(wrapper);
    let changes = document(&objects, cell).unwrap().change_count();
    collect!(objects);

    assert!(alive(&objects, cell, outer), "held through its child");
    assert!(alive(&objects, cell, inner));
    assert!(!alive(&objects, cell, loose), "never wrapped: gone");
    assert_eq!(released(&objects, cell), Released { trees: 1, nodes: 1 });

    let live = objects.heap().live();
    objects.heap_mut().release(held);
    collect!(objects);

    assert!(!alive(&objects, cell, outer));
    assert!(!alive(&objects, cell, inner));
    assert_eq!(released(&objects, cell), Released { trees: 2, nodes: 3 });
    assert_eq!(wrapped(&objects, cell), 0, "its table entry went with it");
    assert_eq!(objects.heap().live(), live - 1, "and its wrapper");
    assert!(!objects.heap().live_at(wrapper));
    assert_eq!(
        document(&objects, cell).unwrap().change_count(),
        changes,
        "releasing is not a change anything can see"
    );
    assert_eq!(
        wrap(&mut objects, cell, inner, None),
        Err(Wrapping::NoSuchNode(inner)),
        "a released node's id answers nothing"
    );
    let next = change!(objects, cell, |d| d.create_text_node("next"));
    assert_eq!(next.as_usize(), loose.as_usize() + 1, "no id is reused");
    objects.heap_mut().release(root);
}

#[test]
fn one_held_wrapper_keeps_every_wrapper_of_its_tree() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "");
    let (outer, a, b, c) = change!(objects, cell, |d| {
        let outer = d.create_element("ul").unwrap();
        let [a, b, c] = ["li", "li", "li"].map(|name| d.create_element(name).unwrap());
        for item in [a, b, c] {
            d.append_child(outer, item).unwrap();
        }
        (outer, a, b, c)
    });
    let first_item = ok!(wrap(&mut objects, cell, a, None));
    let held = objects.heap_mut().root(first_item);
    let middle = ok!(wrap(&mut objects, cell, b, None));
    let last = ok!(wrap(&mut objects, cell, c, None));
    let key = ok!(objects.key(&units("expando")));
    assert!(ok!(objects.define(
        last,
        key,
        Property::plain(Value::Bool(true))
    )));

    for _ in 0..3 {
        collect!(objects);
    }
    assert_eq!(ok!(wrap(&mut objects, cell, b, None)), middle);
    let again = ok!(wrap(&mut objects, cell, c, None));
    assert_eq!(again, last, "kept by the ring, not remade");
    let key = ok!(objects.key(&units("expando")));
    assert_eq!(
        ok!(objects.get(again, key)),
        Found::Value(Value::Bool(true))
    );
    assert_eq!(wrapped(&objects, cell), 3);

    objects.heap_mut().release(held);
    collect!(objects);
    for node in [outer, a, b, c] {
        assert!(!alive(&objects, cell, node), "{node} went with its ring");
    }
    assert_eq!(released(&objects, cell), Released { trees: 1, nodes: 4 });
    assert_eq!(wrapped(&objects, cell), 0);
    objects.heap_mut().release(root);
}

#[test]
fn a_templates_contents_are_part_of_its_tree() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<template><b>in</b></template>");
    let attached = {
        let d = document(&objects, cell).unwrap();
        let template = first(d, "template").unwrap();
        let contents = d.element(template).unwrap().template_contents.unwrap();
        d.first_child(contents).unwrap()
    };
    let (template, inside) = change!(objects, cell, |d| {
        let template = d.create_element("template").unwrap();
        let contents = d.element(template).unwrap().template_contents.unwrap();
        let inside = d.create_element("i").unwrap();
        d.append_child(contents, inside).unwrap();
        (template, inside)
    });

    let held_inside = ok!(wrap(&mut objects, cell, inside, None));
    let held = objects.heap_mut().root(held_inside);
    let in_page = ok!(wrap(&mut objects, cell, attached, None));
    collect!(objects);
    assert!(alive(&objects, cell, template), "its contents reach it");
    assert_eq!(ok!(wrap(&mut objects, cell, attached, None)), in_page);

    objects.heap_mut().release(held);
    collect!(objects);
    assert!(!alive(&objects, cell, template));
    assert!(!alive(&objects, cell, inside));
    assert_eq!(released(&objects, cell), Released { trees: 1, nodes: 3 });
    assert_eq!(
        ok!(wrap(&mut objects, cell, attached, None)),
        in_page,
        "the page's own template's contents are the document's tree"
    );
    objects.heap_mut().release(root);
}

#[test]
fn a_node_moves_between_the_rules_as_it_moves_between_trees() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<body></body>");
    let body = first(document(&objects, cell).unwrap(), "body").unwrap();
    let made = change!(objects, cell, |d| d.create_element("section").unwrap());
    let wrapper = ok!(wrap(&mut objects, cell, made, None));
    let key = ok!(objects.key(&units("mark")));
    assert!(ok!(objects.define(
        wrapper,
        key,
        Property::plain(Value::Number(1.0))
    )));

    change!(objects, cell, |d| d.append_child(body, made).unwrap());
    collect!(objects);
    assert_eq!(
        ok!(wrap(&mut objects, cell, made, None)),
        wrapper,
        "in the document now, so kept by it"
    );

    change!(objects, cell, |d| assert!(d.remove(made)));
    collect!(objects);
    assert!(!alive(&objects, cell, made), "detached and held by nobody");
    assert!(!objects.heap().live_at(wrapper));
    assert!(alive(&objects, cell, body));
    objects.heap_mut().release(root);
}

#[test]
fn a_wrapper_made_while_every_allocation_collects_keeps_its_node() {
    // `document.createElement`: a node in a tree of its own with no wrapper,
    // whose wrapper is about to be made — and the allocation that makes it
    // collects, under stress, every time.
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<p>x</p>");
    objects.heap_mut().stress(true);
    for round in 0..20 {
        let node = change!(objects, cell, |d| d.create_element("div").unwrap());
        let wrapper = ok!(wrap(&mut objects, cell, node, None));
        assert!(alive(&objects, cell, node), "round {round}");
        assert_eq!(node_of(&objects, wrapper), Some((cell, node)));
        if let Err(broken) = objects.heap().check() {
            panic!("round {round}: {broken:?}");
        }
    }
    objects.heap_mut().stress(false);
    collect!(objects);
    assert_eq!(
        released(&objects, cell),
        Released {
            trees: 20,
            nodes: 20
        }
    );
    objects.heap_mut().release(root);
}

#[test]
fn nothing_but_a_wrapper_answers_as_a_node() {
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<p>x</p>");
    let paragraph = first(document(&objects, cell).unwrap(), "p").unwrap();
    let plain = ok!(objects.object(None));
    let text = ok!(objects.text(units("p")));
    let held = [plain, text].map(|r| objects.heap_mut().root(r));

    assert_eq!(node_of(&objects, plain), None);
    assert_eq!(node_of(&objects, text), None);
    assert_eq!(node_of(&objects, cell), None, "the cell is not a node");
    assert!(objects.embedded::<DocumentCell>(plain).is_none());
    assert_eq!(
        wrap(&mut objects, plain, paragraph, None),
        Err(Wrapping::NotADocument)
    );

    let other = parse_document("<p>a</p><p>b</p><p>c</p><p>d</p>");
    let foreign_id = other.node_count() - 1;
    let beyond = other
        .descendants(other.root())
        .find(|id| id.as_usize() == foreign_id)
        .unwrap();
    assert!(document(&objects, cell).unwrap().node_count() <= foreign_id);
    assert_eq!(
        wrap(&mut objects, cell, beyond, None),
        Err(Wrapping::NoSuchNode(beyond)),
        "an id this document never made"
    );
    for root in held {
        objects.heap_mut().release(root);
    }
    objects.heap_mut().release(root);
}

#[test]
fn the_heap_counts_the_document() {
    // ADR 0017 § 2: the cell's footprint is the document's size, so a page
    // that builds an unbounded document meets the heap's ceiling.
    let mut objects = Objects::new();
    let (cell, root) = adopted!(objects, "<body></body>");
    let body = first(document(&objects, cell).unwrap(), "body").unwrap();
    let before = objects.heap().held();
    let weighed = document(&objects, cell).unwrap().footprint();
    let value = "v".repeat(1024);

    let holder = change!(objects, cell, |d| {
        let holder = d.create_element("div").unwrap();
        for _ in 0..1000 {
            let item = d.create_element("p").unwrap();
            d.set_attribute(item, "data-x", &value).unwrap();
            d.append_child(holder, item).unwrap();
        }
        d.append_child(body, holder).unwrap();
        holder
    });
    let grown = objects.heap().held();
    assert!(
        grown >= before + 1000 * 1024,
        "{grown} held after a megabyte of attributes, from {before}"
    );
    assert_eq!(
        grown - before,
        document(&objects, cell).unwrap().footprint() - weighed,
        "what the heap holds grew by exactly what the document did"
    );

    change!(objects, cell, |d| assert!(d.remove(holder)));
    collect!(objects);
    let after = objects.heap().held();
    assert!(
        after < before + 1000 * size_of::<usize>() * 8,
        "released: {after} held, from {before} before the megabyte"
    );
    objects.heap_mut().release(root);
}
