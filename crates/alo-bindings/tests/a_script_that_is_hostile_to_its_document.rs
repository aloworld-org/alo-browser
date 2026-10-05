/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 249's hostile half (`LOOP.md`, stage 2 § 2).
//!
//! What a stranger's script does once it can reach the document: puts a node
//! inside its own child, appends to the page until the heap is full, makes a
//! million nodes and drops every one, and runs every member with the
//! collector firing at each allocation. Each refuses, collects or stops with
//! a reason — and never panics, which in a renderer is a denial of service.

use alo_bindings::{DocumentCell, adopt, document, install};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{Escape, script};

macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused:?}", stringify!($call)),
        }
    };
}

const PAGE: &str = "<!DOCTYPE html><html><head></head><body><p>one</p></body></html>";

/// What `$source` evaluates to in `$engine`, which must parse.
macro_rules! run {
    ($engine:expr, $source:expr $(,)?) => {
        match script($source) {
            Ok(program) => $engine.evaluate(&program),
            Err(why) => panic!("{} did not parse: {why}", $source),
        }
    };
}

/// The string `$source` evaluates to.
macro_rules! text {
    ($engine:expr, $source:expr $(,)?) => {
        match run!($engine, $source) {
            Ok(Value::Text(held)) => $engine
                .objects()
                .units(held)
                .map(String::from_utf16_lossy)
                .unwrap_or_default(),
            other => panic!("{} answered {other:?}", $source),
        }
    };
}

/// The document `$cell` holds, borrowed.
macro_rules! doc {
    ($engine:expr, $cell:expr) => {
        match document($engine.objects(), $cell) {
            Some(held) => held,
            None => panic!("the cell holds a document"),
        }
    };
}

/// An engine with the page installed, collecting at every allocation when
/// `stress` says so; the root that keeps its document cell, and the cell.
fn page(stress: bool) -> Result<(Engine, Root, Ref), String> {
    let mut engine = Engine::new().map_err(|why| why.to_string())?;
    engine.objects().heap_mut().stress(stress);
    let cell = adopt(engine.objects(), parse_document(PAGE)).map_err(|why| why.to_string())?;
    let root = engine.objects().heap_mut().root(cell);
    install(&mut engine, cell).map_err(|why| why.to_string())?;
    Ok((engine, root, cell))
}

fn check(engine: &mut Engine) -> Result<(), String> {
    engine
        .objects()
        .heap()
        .check()
        .map_err(|broken| format!("the heap is broken: {broken:?}"))
}

#[test]
fn a_node_put_inside_its_own_child_is_refused_and_the_tree_is_as_it_was() {
    let (mut engine, _root, cell) = ok!(page(false));
    let before = {
        let parsed = doc!(engine, cell);
        parsed.serialize_node(parsed.root())
    };
    let answer = text!(
        engine,
        "var html = document.documentElement; var body = html.lastChild;
         var a = document.createElement('a'); var b = document.createElement('b');
         var c = document.createElement('c'); a.appendChild(b); b.appendChild(c);
         function caught(f) { try { f(); return 'nothing'; } catch (e) { return e.name; } }
         caught(function () { c.appendChild(a); }) + ' ' +
         caught(function () { b.insertBefore(a, c); }) + ' ' +
         caught(function () { c.replaceChild(a, c); }) + ' ' +
         caught(function () { body.appendChild(html); }) + ' ' +
         caught(function () { body.firstChild.appendChild(document); }) + ' ' +
         caught(function () { a.appendChild(a); }) + ' ' +
         (c.parentNode === b) + (b.parentNode === a) + (a.parentNode === null)",
    );
    assert_eq!(
        answer,
        "HierarchyRequestError HierarchyRequestError HierarchyRequestError \
         HierarchyRequestError HierarchyRequestError HierarchyRequestError truetruetrue"
    );
    let after = doc!(engine, cell);
    assert_eq!(after.serialize_node(after.root()), before);
    assert_eq!(
        after.change_count(),
        2,
        "only the two appends a script made"
    );
}

#[test]
fn a_page_that_appends_to_itself_until_the_heap_is_full_is_stopped_with_a_reason() {
    let (mut engine, _root, cell) = ok!(page(false));
    // A string of a mebibyte of code units, put in a fresh text node on
    // every turn: each one a mebibyte the document holds and the heap counts.
    let stopped = run!(
        engine,
        "var s = 'x'; for (var i = 0; i < 20; i++) s = s + s;
         var html = document.documentElement; var turns = 0;
         for (;;) { html.appendChild(document.createTextNode(s)); turns++; }",
    );
    let Err(Trouble::Escaped(Escape::Full(full))) = stopped else {
        panic!("the loop ends at the heap's ceiling, and said {stopped:?}");
    };
    // The ceiling is enforced where the heap allocates, and a change to the
    // document is a write rather than an allocation: the write that added
    // the last node may take the heap past the ceiling by that one node,
    // and the next allocation is refused. Never by more than one change.
    assert!(
        full.held.saturating_sub(full.ceiling) <= (1 << 20) + 4096,
        "{full}"
    );
    ok!(check(&mut engine));
    let filled = doc!(engine, cell);
    let Some(html) = filled
        .children(filled.root())
        .find(|node| filled.element(*node).is_some())
    else {
        panic!("the page has its element");
    };
    let appended = filled.children(html).count();
    // The page's own head and body, and several hundred mebibytes of text.
    assert!(appended > 500, "{appended} children");
}

#[test]
fn a_million_nodes_made_and_dropped_are_released_and_their_ids_never_reused() {
    let (mut engine, _root, cell) = ok!(page(false));
    let parsed = doc!(engine, cell).node_count();
    let answer = run!(
        engine,
        "for (var i = 0; i < 1000000; i++) document.createElement('p'); i",
    );
    assert_eq!(answer, Ok(Value::Number(1_000_000.0)));
    engine.objects().heap_mut().collect();
    ok!(check(&mut engine));
    let Some(kept) = engine.objects().embedded::<DocumentCell>(cell) else {
        panic!("the cell is a document cell");
    };
    assert_eq!(kept.released().nodes, 1_000_000, "every one was released");
    assert_eq!(kept.released().trees, 1_000_000);
    assert_eq!(kept.document().node_count(), parsed + 1_000_000);
    // The page's own wrapper, the document's, is all that is left.
    assert_eq!(kept.wrapped(), 1);
    // And the next node is numbered past every one of them.
    assert_eq!(
        text!(
            engine,
            "var made = document.createElement('i'); document.documentElement.appendChild(made); \
             made.parentNode === document.documentElement ? 'kept' : 'lost'"
        ),
        "kept"
    );
    let document = doc!(engine, cell);
    assert_eq!(document.node_count(), parsed + 1_000_001);
}

#[test]
fn every_member_holds_what_it_makes_while_every_allocation_collects() {
    let (mut engine, _root, cell) = ok!(page(true));
    let answer = text!(
        engine,
        "var html = document.documentElement; var body = html.lastChild;
         var p = body.firstChild; p.mark = 'm';
         var made = document.createElement('div');
         made.appendChild(document.createTextNode('two'));
         made.setAttribute('a', { toString: function () { return 'b'; } });
         body.insertBefore(made, p);
         var e; try { made.appendChild(made); } catch (x) { e = x; }
         var lost = document.createElement('lost'); lost = null;
         body.replaceChild(document.createElement('span'), p);
         p.textContent = 'gone';
         body.firstChild.getAttribute('a') + body.textContent + e.name + p.mark +
         (p.parentNode === null) + body.lastChild.textContent",
    );
    assert_eq!(answer, "btwoHierarchyRequestErrormtrue");
    engine.objects().heap_mut().stress(false);
    ok!(check(&mut engine));
    let document = doc!(engine, cell);
    assert_eq!(
        document.serialize_node(document.root()),
        "<!DOCTYPE html><html><head></head><body><div a=\"b\">two</div><span></span>\
         </body></html>"
    );
}
