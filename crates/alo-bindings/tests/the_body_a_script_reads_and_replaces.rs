/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 253: `document.body`, read and assigned from script.
//!
//! The getter is HTML's *the body element*, answered as the node's one
//! wrapper; the setter converts its value as Web IDL's `HTMLElement?` and
//! then replaces, appends or refuses as `alo-dom`'s `set_body` says. Each
//! test asserts what the script answered **and** the document it left.

use alo_bindings::{adopt, document, furnish, install, prototype_of, wrap};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Property, Value};
use alo_js::{numeric, script};

macro_rules! ok {
    ($call:expr) => {
        match $call {
            Ok(answer) => answer,
            Err(refused) => panic!("{}: {refused:?}", stringify!($call)),
        }
    };
}

/// The document `$page` holds, borrowed.
macro_rules! doc {
    ($page:expr) => {
        match document($page.objects(), $page.cell) {
            Some(held) => held,
            None => panic!("the cell holds a document"),
        }
    };
}

/// An engine with a page's document installed, and the root on its cell.
struct Page {
    engine: Engine,
    /// Keeps the cell, as the renderer will (ADR 0017 § 2).
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(markup: &str) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell =
            adopt(engine.objects(), parse_document(markup)).map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        Ok(Self {
            engine,
            _root: root,
            cell,
        })
    }

    fn objects(&self) -> &alo_js::Objects {
        let (_, objects) = self.engine.intrinsics();
        objects
    }

    fn serialized(&self) -> String {
        document(self.objects(), self.cell)
            .map(|held| held.serialize_node(held.root()))
            .unwrap_or_default()
    }

    /// What `source` evaluates to, as text a test can compare.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        match self.engine.evaluate(&program) {
            Ok(value) => self.show(value),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }

    fn show(&mut self, value: Value) -> String {
        match value {
            Value::Undefined => "undefined".to_owned(),
            Value::Null => "null".to_owned(),
            Value::Bool(answer) => answer.to_string(),
            Value::Number(number) => numeric::text_of(number),
            Value::Text(held) => match self.engine.objects().units(held) {
                Some(units) => String::from_utf16_lossy(units),
                None => "?".to_owned(),
            },
            Value::Symbol(_) => "a symbol".to_owned(),
            Value::Object(_) => "an object".to_owned(),
        }
    }

    fn collect(&mut self) -> Result<(), String> {
        self.engine.objects().heap_mut().collect();
        self.engine
            .objects()
            .heap()
            .check()
            .map_err(|broken| format!("the heap is broken after a collection: {broken:?}"))
    }
}

const PAGE: &str = "<!DOCTYPE html><html><head></head><body><p id=a>one</p></body></html>";

/// A function the tests' scripts share: the name of what `f` throws.
const CAUGHT: &str = "function caught(f) {
                        try { f(); return 'nothing'; } catch (e) { return e.name; }
                      }";

#[test]
fn the_body_is_the_html_elements_body_child_and_one_wrapper() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(
            "var html = document.documentElement; \
             (document.body === html.lastChild) + ' ' + \
             (document.body === document.body) + ' ' + \
             document.body.firstChild.getAttribute('id')"
        ),
        "true true a"
    );
    // An expando hung on it is there the next time it is read: the same
    // object, not a wrapper remade.
    assert_eq!(page.run("document.body.mark = 7; document.body.mark"), "7");
    assert_eq!(doc!(page).change_count(), 0, "reading changes nothing");
}

#[test]
fn with_no_body_element_the_body_is_null() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(
            "var html = document.documentElement; html.removeChild(document.body); \
             '' + document.body"
        ),
        "null"
    );
    // And with a document element that is not `html`, a body under it is
    // not the body element.
    assert_eq!(
        page.run(
            "var div = document.createElement('div'); \
             div.appendChild(document.createElement('body')); \
             document.replaceChild(div, html); \
             '' + document.body"
        ),
        "null"
    );
}

#[test]
fn assigning_a_body_replaces_the_old_one_which_keeps_what_it_held() {
    let mut page = ok!(Page::new(PAGE));
    let parsed = doc!(page).node_count();
    assert_eq!(
        page.run(
            "var old = document.body; old.mark = 'kept'; \
             var made = document.createElement('body'); \
             made.appendChild(document.createTextNode('new')); \
             var answered = (document.body = made); \
             (answered === made) + ' ' + (document.body === made) + ' ' + \
             (made.parentNode === document.documentElement) + ' ' + \
             old.parentNode + ' ' + old.mark + ' ' + old.firstChild.getAttribute('id')"
        ),
        "true true true null kept a"
    );
    assert_eq!(
        page.serialized(),
        "<!DOCTYPE html><html><head></head><body>new</body></html>"
    );
    assert_eq!(
        doc!(page).change_count(),
        2,
        "the append, and the replacement"
    );
    let body = doc!(page).body();
    assert_eq!(
        body.map(alo_dom::NodeId::as_usize),
        Some(parsed),
        "numbered after every parsed node (ADR 0003)"
    );
}

#[test]
fn the_body_it_already_is_or_a_frameset_with_none_counts_as_the_standard_says() {
    let mut page = ok!(Page::new(PAGE));
    let before = page.serialized();
    assert_eq!(page.run("document.body = document.body; 'ok'"), "ok");
    assert_eq!(page.serialized(), before);
    assert_eq!(
        doc!(page).change_count(),
        0,
        "the same body changes nothing"
    );
    // With the body taken out, a frameset is appended and is the body
    // element.
    assert_eq!(
        page.run(
            "document.documentElement.removeChild(document.body); \
             var f = document.createElement('frameset'); document.body = f; \
             (document.body === f) + ' ' + (document.documentElement.lastChild === f)"
        ),
        "true true"
    );
}

#[test]
fn what_is_not_a_body_is_a_hierarchy_request_error_and_changes_nothing() {
    let mut page = ok!(Page::new(PAGE));
    let before = page.serialized();
    let answer = page.run(&format!(
        "{CAUGHT}
         caught(function () {{ document.body = null; }}) + ' ' +
         caught(function () {{ document.body = undefined; }}) + ' ' +
         caught(function () {{ document.body = document.createElement('div'); }}) + ' ' +
         caught(function () {{ document.body = document.documentElement; }}) + ' ' +
         caught(function () {{ document.body = document.body.firstChild; }})"
    ));
    assert_eq!(
        answer,
        "HierarchyRequestError HierarchyRequestError HierarchyRequestError \
         HierarchyRequestError HierarchyRequestError"
    );
    assert_eq!(
        page.run(
            "var e; try { document.body = null; } catch (x) { e = x; } \
             e.message + ' ' + Error.prototype.isPrototypeOf(e)"
        ),
        "the body must be a body or a frameset element true"
    );
    assert_eq!(page.serialized(), before, "no refusal changed the tree");
    assert_eq!(doc!(page).change_count(), 0, "nor counted a change");
}

#[test]
fn with_no_document_element_a_body_has_nowhere_to_go() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(&format!(
            "{CAUGHT}
             document.removeChild(document.documentElement);
             var e; try {{ document.body = document.createElement('body'); }} \
             catch (x) {{ e = x; }}
             e.name + ': ' + e.message"
        )),
        "HierarchyRequestError: there is no document element to put a body in"
    );
    assert_eq!(page.serialized(), "<!DOCTYPE html>");
}

#[test]
fn what_is_not_an_html_element_or_not_this_documents_is_a_type_error() {
    let mut page = ok!(Page::new(
        "<!DOCTYPE html><html><head></head><body><svg></svg></body></html>"
    ));
    // A second document in the same realm, as `other`.
    let other = ok!(adopt(page.engine.objects(), parse_document("<p>x</p>")));
    let _other_root = page.engine.objects().heap_mut().root(other);
    ok!(furnish(&mut page.engine, other));
    let objects = page.engine.objects();
    let root = ok!(document(objects, other).ok_or("a document")).root();
    let prototype = prototype_of(objects, other, root);
    let wrapper = ok!(wrap(objects, other, root, prototype));
    let global = ok!(page.engine.global());
    let name: Vec<u16> = "other".encode_utf16().collect();
    ok!(page.engine.objects().define_named(
        global,
        &name,
        Property::data(Value::Object(wrapper), true, false, true),
    ));

    let before = page.serialized();
    let answer = page.run(&format!(
        "{CAUGHT}
         var fake = {{}}; fake.__proto__ = document.__proto__;
         var svg = document.body.firstChild;
         caught(function () {{ document.body = svg; }}) + ' ' +
         caught(function () {{ document.body = document.createTextNode('body'); }}) + ' ' +
         caught(function () {{ document.body = document; }}) + ' ' +
         caught(function () {{ document.body = {{}}; }}) + ' ' +
         caught(function () {{ document.body = 'body'; }}) + ' ' +
         caught(function () {{ document.body = other.createElement('body'); }}) + ' ' +
         caught(function () {{ return fake.body; }}) + ' ' +
         caught(function () {{ fake.body = document.createElement('body'); }}) + ' ' +
         caught(function () {{ var o = {{}}; o.__proto__ = document; return o.body; }})"
    ));
    assert_eq!(
        answer,
        "TypeError TypeError TypeError TypeError TypeError TypeError TypeError \
         TypeError TypeError"
    );
    assert_eq!(page.serialized(), before);
    assert_eq!(doc!(page).change_count(), 0);
}

#[test]
fn a_page_that_replaces_its_body_ten_thousand_times_and_collects() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(
            "var last; for (var i = 0; i < 10000; i++) { \
               last = document.createElement('body'); last.mark = i; document.body = last; } \
             (document.body === last) + ' ' + document.body.mark"
        ),
        "true 9999"
    );
    ok!(page.collect());
    assert_eq!(
        page.run("document.body.mark"),
        "9999",
        "kept across a collection"
    );
    assert_eq!(doc!(page).change_count(), 10000);
}
