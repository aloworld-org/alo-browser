/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A document's footprint, kept as a sum (queue item 248, ADR 0017 § 2).
//!
//! The heap a document moves into measures it before and after every change,
//! so `Document::footprint` must cost nothing to ask — and must still be
//! right. Every test here recounts by hand, node by node, and compares.

use alo_dom::{Document, NodeId, parse_document};

/// The footprint, recounted: every live node's own, and one slot for every
/// node ever made.
fn recount(document: &Document) -> usize {
    let mut live = Vec::new();
    let mut pending = vec![document.root()];
    let mut after = None;
    while let Some(top) = document.next_detached_root(after) {
        after = Some(top);
        pending.push(top);
    }
    while let Some(top) = pending.pop() {
        for id in std::iter::once(top).chain(document.descendants(top)) {
            live.push(id);
            if let Some(contents) = document.element(id).and_then(|e| e.template_contents) {
                pending.push(contents);
            }
        }
    }
    live.iter()
        .filter_map(|id| document.get(*id))
        .map(alo_dom::Node::footprint)
        .sum::<usize>()
        + document.node_count() * size_of::<Option<Box<alo_dom::Node>>>()
}

#[test]
fn a_parsed_page_weighs_what_its_nodes_do() {
    let document = parse_document(
        "<!doctype html><html lang=en><body class=a><p id=x>text <b>bold</b></p>\
         <!-- note --><template><i>in</i></template><p>more</p></body></html>",
    );
    assert_eq!(document.footprint(), recount(&document));
    assert_eq!(document.clone().footprint(), document.footprint());
    assert!(document.footprint() > document.node_count() * size_of::<alo_dom::Node>());
}

#[test]
fn every_change_keeps_the_sum() {
    let mut document = parse_document("<body><p>a</p></body>");
    let body = document
        .descendants(document.root())
        .find(|id| document.get(*id).is_some_and(|n| n.is_html_element("body")))
        .unwrap();

    let made = document.create_element("div").unwrap();
    assert_eq!(document.footprint(), recount(&document), "made");
    let before = document.footprint();
    document
        .set_attribute(made, "data-long", &"x".repeat(4096))
        .unwrap();
    assert_eq!(document.footprint(), recount(&document), "set");
    assert!(document.footprint() >= before + 4096);
    document.set_attribute(made, "data-long", "short").unwrap();
    assert_eq!(
        document.footprint(),
        recount(&document),
        "set again, shorter"
    );
    document.remove_attribute(made, "data-long").unwrap();
    assert_eq!(document.footprint(), recount(&document), "removed");

    let text = document.create_text_node(&"t".repeat(1000));
    document.append_child(made, text).unwrap();
    document.append_child(body, made).unwrap();
    let template = document.create_element("template").unwrap();
    assert_eq!(document.footprint(), recount(&document), "a template");

    let with_tree = document.footprint();
    assert!(document.remove(made));
    assert_eq!(document.footprint(), with_tree, "detached is not gone");
    assert_eq!(document.release(made), Some(2));
    assert_eq!(document.release(template), Some(2));
    assert_eq!(document.footprint(), recount(&document), "released");
    assert!(document.footprint() + 1000 <= with_tree);
}

#[test]
fn a_deep_chain_is_released_without_recursion() {
    // Release walks down and climbs back by the parent link, keeping no list,
    // because the bindings call it from a collection's sweep.
    let mut document = Document::new();
    let bottom = document.create_element("div").unwrap();
    let mut top = bottom;
    for _ in 0..300_000 {
        let above = document.create_element("div").unwrap();
        document.append_child(above, top).unwrap();
        top = above;
    }
    assert_eq!(document.release(top), Some(300_001));
    assert!(document.get(bottom).is_none());
    assert_eq!(document.footprint(), recount(&document));
    assert_eq!(document.next_detached_root(None), None);
}

#[test]
fn the_cursor_finds_every_detached_root_and_nothing_else() {
    let mut document = parse_document("<template><b>x</b></template><p>y</p>");
    let first = document.create_element("div").unwrap();
    let child = document.create_element("span").unwrap();
    document.append_child(first, child).unwrap();
    let second = document.create_text_node("z");
    let third = document.create_element("template").unwrap();

    let mut found: Vec<NodeId> = Vec::new();
    let mut after = None;
    while let Some(top) = document.next_detached_root(after) {
        after = Some(top);
        found.push(top);
    }
    assert_eq!(
        found,
        vec![first, second, third],
        "not a child, not contents"
    );

    document.release(second);
    assert_eq!(document.next_detached_root(Some(first)), Some(third));
}
