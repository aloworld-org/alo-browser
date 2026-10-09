/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 249: the interfaces a script calls (ADR 0017 §§ 1, 4, 5, 8).
//!
//! Each test is a script the engine runs against an adopted, installed
//! document, and an assertion on what it answered **and** on the document it
//! left — the serialisation, the change count and the ids — because a member
//! that answered right and changed the wrong thing is the failure that
//! matters to the agent reading the tree afterwards.

use alo_bindings::{adopt, document, furnish, install, node_of, prototype_of, wrap};
use alo_dom::parse_document;
use alo_js::abrupt::Missing;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Property, Value};
use alo_js::{Escape, numeric, script};

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

/// What `$source` evaluates to, which must parse.
macro_rules! evaluate {
    ($page:expr, $source:expr $(,)?) => {
        match script($source) {
            Ok(program) => $page.engine.evaluate(&program),
            Err(why) => panic!("{} did not parse: {why}", $source),
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
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
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

/// The walk every test starts from.
const NAMES: &str = "var html = document.documentElement; var body = html.lastChild; \
                     var p = body.firstChild;";

#[test]
fn a_script_makes_inserts_moves_replaces_and_removes_and_reads_it_all_back() {
    let mut page = ok!(Page::new(PAGE));
    let parsed = doc!(page).node_count();
    let answer = page.run(&format!(
        "{NAMES}
         var div = document.createElement('DIV');
         var two = document.createTextNode('two');
         var out = '' + div.parentNode + ',' + div.firstChild;
         out += ',' + (div.appendChild(two) === two);
         out += ',' + (body.appendChild(div) === div);
         out += ',' + (body.insertBefore(div, p) === div);
         out += ',' + (body.firstChild === div) + (div.nextSibling === p);
         out += '' + (p.previousSibling === div) + (body.lastChild === p);
         var span = document.createElement('span');
         out += ',' + (body.replaceChild(span, p) === p) + (p.parentNode === null);
         span.setAttribute('Title', 'x');
         out += ',' + span.getAttribute('title') + span.getAttribute('TITLE');
         span.textContent = 'three';
         out += ',' + body.textContent + ',' + span.firstChild.textContent;
         span.firstChild.textContent = 'four';
         out += ',' + (body.removeChild(div) === div) + (div.parentNode === null);
         out += ',' + div.textContent + ',' + p.getAttribute('id');
         p.removeAttribute('ID');
         out += ',' + p.getAttribute('id');
         body.appendChild(p);
         span.remove();
         out += ',' + body.textContent;
         out;"
    ));
    assert_eq!(
        answer,
        "null,null,true,true,true,truetruetruetrue,truetrue,xx,twothree,three,\
         truetrue,two,a,null,one"
    );
    assert_eq!(
        page.serialized(),
        "<!DOCTYPE html><html><head></head><body><p>one</p></body></html>"
    );
    // appendChild ×3, insertBefore, replaceChild, setAttribute, textContent
    // ×2, removeChild, removeAttribute, remove: eleven changes.
    assert_eq!(doc!(page).change_count(), 11);
    // The div, its text, the span and the span's text — setting a text
    // node's `textContent` makes nothing — numbered from the parser's own
    // counter.
    assert_eq!(doc!(page).node_count(), parsed + 4);
}

#[test]
fn a_node_a_script_makes_takes_the_next_id_and_every_parsed_id_survives() {
    let mut page = ok!(Page::new(PAGE));
    let before: Vec<_> = {
        let document = doc!(page);
        document.descendants(document.root()).collect()
    };
    let parsed = doc!(page).node_count();
    let Ok(Value::Object(made)) = evaluate!(
        page,
        &format!(
            "{NAMES} var made = document.createElement('b'); \
         body.insertBefore(made, p); body.insertBefore(p, null); made;"
        )
    ) else {
        panic!("the script answers the element it made");
    };
    let Some((cell, node)) = node_of(page.objects(), made) else {
        panic!("what createElement answers is a node");
    };
    assert_eq!(cell, page.cell);
    assert_eq!(node.as_usize(), parsed, "the next id after the parsed ones");
    let after: Vec<_> = {
        let document = doc!(page);
        document.descendants(document.root()).collect()
    };
    let mut expected = before;
    expected.insert(expected.len().saturating_sub(2), node);
    assert_eq!(after, expected, "every parsed node keeps its id");
}

#[test]
fn each_refusal_is_the_named_dom_exception_a_catch_receives() {
    let mut page = ok!(Page::new(PAGE));
    let before = page.serialized();
    let answer = page.run(&format!(
        "{NAMES}
         function caught(f) {{
           try {{ f(); return 'nothing'; }} catch (e) {{
             return e.name + (Error.prototype.isPrototypeOf(e) ? '' : '!') +
               (e.message !== '' ? '' : '?');
           }}
         }}
         caught(function () {{ body.appendChild(html); }}) + ' ' +
         caught(function () {{ p.appendChild(p); }}) + ' ' +
         caught(function () {{ document.appendChild(document.createElement('i')); }}) + ' ' +
         caught(function () {{ document.appendChild(document.createTextNode('t')); }}) + ' ' +
         caught(function () {{ p.firstChild.appendChild(document.createElement('i')); }}) + ' ' +
         caught(function () {{ body.removeChild(document.createElement('i')); }}) + ' ' +
         caught(function () {{ body.insertBefore(document.createElement('i'), html); }}) + ' ' +
         caught(function () {{ body.replaceChild(document.createElement('i'), html); }}) + ' ' +
         caught(function () {{ document.createElement('a b'); }}) + ' ' +
         caught(function () {{ document.createElement(''); }}) + ' ' +
         caught(function () {{ p.setAttribute('a=b', 'x'); }}) + ' ' +
         caught(function () {{ p.setAttribute('', 'x'); }})"
    ));
    assert_eq!(
        answer,
        "HierarchyRequestError HierarchyRequestError HierarchyRequestError \
         HierarchyRequestError HierarchyRequestError NotFoundError NotFoundError \
         NotFoundError InvalidCharacterError InvalidCharacterError \
         InvalidCharacterError InvalidCharacterError"
    );
    assert_eq!(page.serialized(), before, "no refusal changed the tree");
    assert_eq!(doc!(page).change_count(), 0, "nor counted a change");
    assert_eq!(
        page.run("var e; try { document.createElement('<'); } catch (x) { e = x; } e.message"),
        "an element's name must be a valid element local name"
    );
    // The two attributes are getters on the prototype, as Web IDL has them,
    // and are the exception's own to answer.
    assert_eq!(
        page.run(
            "var g = e.__proto__; var o = {}; o.__proto__ = g; \
             var r; try { o.name; r = 'read'; } catch (x) { r = x.name; } \
             r + ' ' + e.hasOwnProperty('name') + ' ' + g.hasOwnProperty('message') + ' ' + \
             (g.__proto__ === Error.prototype)"
        ),
        "TypeError false true true"
    );
}

#[test]
fn a_member_on_the_wrong_this_or_the_wrong_argument_is_a_type_error() {
    let mut page = ok!(Page::new(PAGE));
    // A second document in the same realm, its own prototypes made, its
    // document node on the global object as `other`.
    let other = ok!(adopt(page.engine.objects(), parse_document("<p>x</p>")));
    let other_root = page.engine.objects().heap_mut().root(other);
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
        "{NAMES}
         function caught(f) {{
           try {{ f(); return 'nothing'; }} catch (e) {{ return e.name; }}
         }}
         var t = document.createTextNode('x');
         var fake = {{}}; fake.__proto__ = p.__proto__;
         var foreign = other.createElement('i');
         caught(function () {{ var o = {{}}; o.f = body.appendChild; o.f(p); }}) + ' ' +
         caught(function () {{ body.appendChild.call({{}}, p); }}) + ' ' +
         caught(function () {{ return fake.firstChild; }}) + ' ' +
         caught(function () {{ fake.textContent = 'x'; }}) + ' ' +
         caught(function () {{ body.getAttribute.call(t, 'id'); }}) + ' ' +
         caught(function () {{ body.remove.call(document); }}) + ' ' +
         caught(function () {{ document.createElement.call(body, 'i'); }}) + ' ' +
         caught(function () {{ body.appendChild({{}}); }}) + ' ' +
         caught(function () {{ body.appendChild(); }}) + ' ' +
         caught(function () {{ body.insertBefore(t); }}) + ' ' +
         caught(function () {{ body.appendChild(foreign); }}) + ' ' +
         caught(function () {{ body.insertBefore(t, foreign); }}) + ' ' +
         caught(function () {{ body.removeChild(other.documentElement); }}) + ' ' +
         caught(function () {{ foreign.appendChild(t); }}) + ' ' +
         caught(function () {{ p.setAttribute('a'); }}) + ' ' +
         caught(function () {{ document.createElement(); }}) + ' ' +
         typeof t.getAttribute + ' ' + typeof document.remove + ' ' + typeof t.remove"
    ));
    assert_eq!(
        answer,
        "TypeError TypeError TypeError TypeError TypeError TypeError TypeError \
         TypeError TypeError TypeError TypeError TypeError TypeError TypeError \
         TypeError TypeError undefined undefined function"
    );
    assert_eq!(page.serialized(), before);
    assert_eq!(doc!(page).change_count(), 0);
    let Some(other) = page.objects().heap().holding(&other_root) else {
        panic!("the other document is rooted");
    };
    let Some(other) = document(page.objects(), other) else {
        panic!("and is a document");
    };
    assert_eq!(
        other.change_count(),
        0,
        "nor was the other document changed"
    );
}

#[test]
fn one_node_read_through_any_member_is_one_object_and_keeps_what_a_page_hung_on_it() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(&format!(
            "{NAMES} p.mark = 7; body.mark = 8;
             (document.firstChild.nextSibling === html) + ' ' +
             (html.parentNode === document) + ' ' + (body.parentNode === html) + ' ' +
             (p.parentNode.firstChild === p) + ' ' + (html.firstChild.nextSibling === body) + ' ' +
             (document.documentElement === html) + ' ' + (p.firstChild.parentNode === p)"
        )),
        "true true true true true true true"
    );
    for _ in 0..3 {
        ok!(page.collect());
    }
    assert_eq!(
        page.run(
            "document.documentElement.lastChild.firstChild.mark + ' ' + \
             document.documentElement.lastChild.mark + ' ' + document.firstChild.mark"
        ),
        "7 8 undefined"
    );
}

#[test]
fn text_content_reads_and_writes_as_the_standard_says_for_each_kind_of_node() {
    let mut page = ok!(Page::new(
        "<!DOCTYPE html><html><head></head><body><p>a<b>b</b><!--c-->d</p></body></html>",
    ));
    assert_eq!(
        page.run(&format!(
            "{NAMES}
             var out = document.textContent + ' ' + document.firstChild.textContent + ' ';
             out += p.textContent + ' ' + p.lastChild.previousSibling.textContent + ' ';
             p.lastChild.previousSibling.textContent = 'C';
             p.firstChild.textContent = {{ toString: function () {{ return 'A'; }} }};
             out += p.textContent + ' ';
             document.textContent = 'ignored';
             document.firstChild.textContent = 'ignored';
             p.textContent = null;
             out += (p.firstChild === null) + ' ';
             p.textContent = 12;
             out += p.firstChild.textContent + ' ';
             p.textContent = '';
             out + (p.firstChild === null);"
        )),
        "null null abd c Abd true 12 true"
    );
    assert_eq!(
        page.serialized(),
        "<!DOCTYPE html><html><head></head><body><p></p></body></html>"
    );
}

#[test]
fn an_argument_that_is_an_object_runs_the_pages_to_string_once() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(&format!(
            "{NAMES}
             var calls = 0;
             function named(text) {{ return {{ toString: function () {{ calls++; return text; }} }}; }}
             var made = document.createElement(named('EM'));
             made.setAttribute(named('one'), '1');
             made.setAttribute('two', named('2'));
             var out = made.getAttribute(named('one')) + made.getAttribute('two');
             made.removeAttribute(named('one'));
             body.appendChild(made);
             made.appendChild(document.createTextNode(named('t')));
             out + ' ' + made.getAttribute('one') + ' ' + calls"
        )),
        "12 null 6"
    );
    assert_eq!(
        page.serialized(),
        "<!DOCTYPE html><html><head></head><body><p id=\"a\">one</p>\
         <em two=\"2\">t</em></body></html>"
    );

    // Both objects needs a second conversion's answer kept across the first,
    // which a native cannot yet do: refused by name, never converted twice.
    let refused = evaluate!(
        page,
        "var n = 0; var o = { toString: function () { n++; return 'x'; } }; \
         document.documentElement.setAttribute(o, o);",
    );
    assert_eq!(
        refused,
        Err(Trouble::Escaped(Escape::NotBuiltYet(
            Missing::ASecondArgumentBehindACall
        )))
    );
    assert_eq!(page.run("n"), "1", "the name was converted once");
    assert_eq!(
        page.run("document.documentElement.getAttribute('x')"),
        "null"
    );
}

#[test]
fn every_other_member_is_absent() {
    let mut page = ok!(Page::new(PAGE));
    assert_eq!(
        page.run(&format!(
            "{NAMES}
             typeof document.head + typeof document.createRange + typeof document.write +
             typeof document.querySelector + typeof p.childNodes + typeof p.children +
             typeof p.innerHTML + typeof p.id + typeof p.nodeType + typeof p.firstChild.data +
             typeof Node + typeof Element + typeof DOMException + typeof html.before"
        )),
        "undefined".repeat(14)
    );
    assert_eq!(
        page.run("document = 1; typeof document"),
        "object",
        "a sloppy assignment to `document` does nothing"
    );
    assert_eq!(page.run("delete document"), "false");
}
