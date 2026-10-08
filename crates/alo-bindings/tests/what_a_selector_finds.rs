/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 329: `querySelectorAll`, and the static `NodeList` it
//! answers — what `alo-downloads`' script finds its buttons with.
//!
//! Each row is a script and what the DOM standard says it answers. A query's
//! answer is written as the `id`s of what it found, in the list's order,
//! which must be tree order. Every script runs twice, the second time with
//! the collector running at every allocation, and the two must agree.

use alo_bindings::{adopt, install};
use alo_dom::parse_document;
use alo_js::heap::Root;
use alo_js::interpret::{Engine, Trouble};
use alo_js::numeric;
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Found, Value};
use alo_js::{Escape, script};

const PAGE: &str = "<!DOCTYPE html><html><head></head><body>\
                    <div id=a class=card>\
                    <a id=a1 class=btn href=/x>x</a><a id=a2 class=btn>y</a></div>\
                    <div id=b class='card rec'>\
                    <p id=b1><a id=b2 class=btn href=/y>z</a></p></div>\
                    <template><a id=t class=btn href=/t></a></template>\
                    <svg><g id=g class=btn></g></svg>\
                    </body></html>";

/// `alo-downloads`, the frozen page that opened this item.
const DOWNLOADS: &str = include_str!("../../alo-corpus/cases/alo-downloads/page.html");

/// An engine with `html` installed.
struct Page {
    engine: Engine,
    _root: Root,
}

impl Page {
    fn new(html: &str, stress: bool) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document(html)).map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
        })
    }

    /// What `source` evaluates to, as text.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        let answer = self.engine.evaluate(&program);
        self.text(answer)
    }

    fn text(&mut self, answer: Result<Value, Trouble>) -> String {
        match answer {
            Ok(Value::Text(held)) => self
                .engine
                .objects()
                .units(held)
                .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
            Ok(Value::Bool(answer)) => answer.to_string(),
            Ok(Value::Number(number)) => numeric::text_of(number),
            Ok(Value::Null) => "null".to_owned(),
            Ok(Value::Undefined) => "undefined".to_owned(),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }
}

/// What `source` answers on `html`, run both ways.
fn answer_on(html: &str, source: &str) -> String {
    let mut answers = [false, true].map(|stress| match Page::new(html, stress) {
        Ok(mut page) => page.run(source),
        Err(why) => format!("! no page: {why}"),
    });
    let [plain, stressed] = &mut answers;
    assert_eq!(plain, stressed, "{source}");
    core::mem::take(plain)
}

/// What `source` answers on [`PAGE`].
fn answer(source: &str) -> String {
    answer_on(PAGE, source)
}

/// The `id`s of what `on.querySelectorAll(selectors)` found, in its order,
/// joined by commas; `on` is a script expression.
fn found(on: &str, selectors: &str) -> String {
    answer(&format!(
        "(function () {{ var list = {on}.querySelectorAll({selectors:?}), out = ''; \
         for (var i = 0; i < list.length; i++) {{ \
         out += (i ? ',' : '') + (list[i].getAttribute('id') || \
         (list[i] === document.documentElement ? 'HTML' : '?')); }} \
         return out; }})()"
    ))
}

/// A script that returns what `body` threw, by name, or `nothing`.
fn thrown(body: &str) -> String {
    answer(&format!(
        "(function () {{ try {{ {body}; return 'nothing'; }} catch (e) {{ \
         return e.name + (Error.prototype.isPrototypeOf(e) ? '' : ' (not an Error)'); }} }})()"
    ))
}

#[test]
fn a_document_finds_its_descendants_in_tree_order() {
    for (selectors, expected) in [
        // The page's own query: a template's contents are not descendants,
        // and the drawing's `g` has no `href`.
        (".btn[href]", "a1,b2"),
        (".btn", "a1,a2,b2,g"),
        ("a", "a1,a2,b2"),
        ("div a", "a1,a2,b2"),
        ("div > a", "a1,a2"),
        (".card.rec a", "b2"),
        ("[href^='/']", "a1,b2"),
        ("#a2 ~ *", ""),
        ("#a + div", "b"),
        ("p:first-child > a:only-child", "b2"),
        (".card:not(.rec) > :nth-child(2)", "a2"),
        (":is(#b1, #a) .btn", "a1,a2,b2"),
        // A list answers in tree order, each element once, whatever order
        // its selectors were written in.
        ("#b2, #a1, a", "a1,a2,b2"),
        // Matching nothing is an empty list, not `null`.
        ("table", ""),
        // The interaction states and a pseudo-element parse and match
        // nothing, as in style.
        ("a:hover", ""),
        ("a::before", ""),
        // Asked of a document, `:scope` is `:root`.
        (":scope", "HTML"),
        (":scope > body > div", "a,b"),
        // Names are matched as HTML says: case-insensitively for an HTML
        // element, but a class exactly.
        ("DIV.card > A", "a1,a2"),
        (".Card", ""),
    ] {
        assert_eq!(found("document", selectors), expected, "{selectors}");
    }
}

#[test]
fn the_frozen_pages_query_finds_its_two_buttons_and_walks_them() {
    // Line 17 of its script with the body of its callback's first line: the
    // two download buttons, in tree order, each handed to `forEach`'s
    // callback with its index (queue items 329 and 331). Its next line
    // calls `fetch`, which the renderer offers a page (item 335) and this
    // test's realm is not given.
    assert_eq!(
        answer_on(
            DOWNLOADS,
            "var out = ''; \
             document.querySelectorAll('.btn[href]').forEach(function (a, i) { \
               var href = a.getAttribute('href'); out += i + href + ' '; }); \
             out + typeof fetch"
        ),
        "0/download/alomails-windows-x64-setup.exe \
         1/download/alomails-mac-universal.dmg undefined"
    );
}

/// What an array iterator says on reaching a `length` behind a getter.
const ITERATOR_REFUSES: &str = "! an array iterator reading an element or a length \
                                through a getter or a conversion is queue item 231";

#[test]
fn a_node_list_is_walked_by_arrays_own_functions() {
    let list = "var list = document.querySelectorAll('.btn');";
    for (body, expected) in [
        // Web IDL makes each the very function `Array.prototype` has.
        (
            "list.forEach === [].forEach && list.keys === [].keys && \
             list.values === [].values && list.entries === [].entries",
            "true",
        ),
        // On the prototype, enumerable as an operation is, and not the list's
        // own.
        ("list.hasOwnProperty('forEach')", "false"),
        (
            "list.__proto__.propertyIsEnumerable('forEach') && \
             list.__proto__.propertyIsEnumerable('values')",
            "true",
        ),
        // `forEach` reads the list's `length` through its getter once, then
        // each index; `thisArg` is passed.
        (
            "var out = ''; list.forEach(function (a, i, l) { \
               out += this.p + i + a.getAttribute('id') + (l === list) + ' '; }, { p: '#' }); out",
            "#0a1true #1a2true #2b2true #3gtrue ",
        ),
        // `for…of` goes through `[Symbol.iterator]`, which is `values`, and
        // its `next` reads the list's `length` — a getter, which an array
        // iterator refuses by name until it keeps the state a call from
        // inside it needs (queue item 231).
        (
            "var out = ''; for (var a of list) { out += a.getAttribute('id'); } out",
            ITERATOR_REFUSES,
        ),
        (
            "var out = ''; for (var k of list.keys()) { out += k; } out",
            ITERATOR_REFUSES,
        ),
        // A throw from the callback ends the walk.
        (
            "var n = 0; try { list.forEach(function () { n++; throw 'x'; }); } catch (e) {} n",
            "1",
        ),
        // An empty list calls nothing.
        (
            "var n = 0; document.querySelectorAll('.none').forEach(function () { n++; }); n",
            "0",
        ),
        // `DOMTokenList` is not iterable yet (queue item 328).
        ("typeof document.body.classList.forEach", "undefined"),
    ] {
        assert_eq!(answer(&format!("{list} {body}")), expected, "{body}");
    }
    assert_eq!(
        thrown(&format!("{list} list.forEach(4)")),
        "TypeError",
        "a callback that is not a function"
    );
}

#[test]
fn a_node_lists_symbol_iterator_is_arrays_values() {
    let Ok(mut page) = Page::new(PAGE, false) else {
        panic!("the page installs");
    };
    let mut prototype_of = |source: &str| {
        let Ok(program) = script(source) else {
            panic!("{source} parses");
        };
        match page.engine.evaluate(&program) {
            Ok(Value::Object(held)) => held,
            other => panic!("{source}: {other:?}"),
        }
    };
    let nodes = prototype_of("document.querySelectorAll('a').__proto__");
    let arrays = prototype_of("[].__proto__");
    let Ok(symbol) = page.engine.well_known(WellKnown::Iterator) else {
        panic!("the realm has Symbol.iterator");
    };
    let objects = page.engine.objects();
    let Ok(key) = objects.symbol_key(symbol) else {
        panic!("a symbol is a key");
    };
    let (Ok(Found::Value(on_nodes)), Ok(Found::Value(on_arrays))) =
        (objects.get(nodes, key), objects.get(arrays, key))
    else {
        panic!("both have [Symbol.iterator] as a data property");
    };
    assert_eq!(on_nodes, on_arrays, "the very function");
    let Ok(Some(property)) = objects.own_property(nodes, key) else {
        panic!("NodeList.prototype has its own");
    };
    assert!(property.is_writable() && !property.is_enumerable() && property.is_configurable());
}

#[test]
fn an_element_finds_only_below_itself_and_scope_is_it() {
    let b = "document.getElementById('b')";
    let b1 = "document.getElementById('b1')";
    for (on, selectors, expected) in [
        (b, "*", "b1,b2"),
        // Never the element itself.
        (b, ".card", ""),
        (b, ":scope", ""),
        (b, ":scope > p", "b1"),
        (b, ":scope > a", ""),
        // A selector may look above the element: the `div` is its parent.
        (b1, "div a", "b2"),
        (b1, "div > p > a", "b2"),
        ("document.body", ":scope > div", "a,b"),
    ] {
        assert_eq!(found(on, selectors), expected, "{on} {selectors}");
    }
}

#[test]
fn a_string_that_is_not_a_selector_list_is_a_syntax_error() {
    for selectors in [
        "",
        "   ",
        "a,",
        ",a",
        "a b)",
        "a {",
        "#1",
        "!",
        // Selectors this engine does not have.
        ":has(a)",
        ":made-up",
        "::made-up",
        // A namespace prefix nobody declared.
        "svg|g",
    ] {
        assert_eq!(
            thrown(&format!("document.querySelectorAll({selectors:?})")),
            "SyntaxError",
            "{selectors:?}"
        );
    }
    // It is a `DOMException`, with the name and an inherited message.
    assert_eq!(
        answer(
            "(function () { try { document.querySelectorAll('a,'); } catch (e) { \
             return e.name + ' ' + typeof e.message + ' ' + Error.prototype.isPrototypeOf(e); } })()"
        ),
        "SyntaxError string true"
    );
}

#[test]
fn the_argument_is_a_dom_string_and_this_must_be_a_parent() {
    // An object's own `toString` runs.
    assert_eq!(
        answer("document.querySelectorAll({ toString: function () { return '.btn'; } }).length"),
        "4"
    );
    // `null` and `undefined` are the strings they spell.
    assert_eq!(answer("document.querySelectorAll(null).length"), "0");
    assert_eq!(answer("document.querySelectorAll(undefined).length"), "0");
    assert_eq!(thrown("document.querySelectorAll()"), "TypeError");
    assert_eq!(
        thrown(
            "document.querySelectorAll({ toString: function () { throw new RangeError('x'); } })"
        ),
        "RangeError"
    );
    // A text node is not a `ParentNode`; neither is a plain object.
    assert_eq!(
        thrown("document.querySelectorAll.call(document.getElementById('a1').firstChild, 'a')"),
        "TypeError"
    );
    assert_eq!(
        thrown("document.querySelectorAll.call({}, 'a')"),
        "TypeError"
    );
    // The mixin's one function is on each prototype that includes it.
    assert_eq!(answer("typeof document.body.querySelectorAll"), "function");
    assert_eq!(
        answer("typeof document.getElementById('a1').firstChild.querySelectorAll"),
        "undefined"
    );
}

#[test]
fn a_node_list_answers_its_indices_and_its_length() {
    let list = "var list = document.querySelectorAll('.btn');";
    for (body, expected) in [
        ("list.length", "4"),
        // The same wrapper as every other way to the element.
        ("list[0] === document.getElementById('a1')", "true"),
        ("list[3] === document.getElementById('g')", "true"),
        ("list[4]", "undefined"),
        ("0 in list", "true"),
        ("3 in list", "true"),
        ("4 in list", "false"),
        ("list.hasOwnProperty(0)", "true"),
        ("list.hasOwnProperty('length')", "false"),
        // `item`: an `unsigned long`, so converted modulo 2³².
        ("list.item(1) === list[1]", "true"),
        ("list.item(4)", "null"),
        ("list.item(-1)", "null"),
        ("list.item(4294967296) === list[0]", "true"),
        ("list.item(4294967297) === list[1]", "true"),
        ("list.item(1.9) === list[1]", "true"),
        ("list.item('2') === list[2]", "true"),
        ("list.item(NaN) === list[0]", "true"),
        ("list.item(null) === list[0]", "true"),
        (
            "list.item({ valueOf: function () { return 3; } }) === list[3]",
            "true",
        ),
        // Each call answers a new list.
        ("list === document.querySelectorAll('.btn')", "false"),
        // An expando stays, as on any object.
        ("list.mine = 7; list.mine", "7"),
        // The indices are the list's own and not writable or deletable.
        (
            "list[0] = 1; list[0] === document.getElementById('a1')",
            "true",
        ),
        ("delete list[0]", "false"),
        ("list[9] = 1; list[9]", "undefined"),
        ("delete list[9]", "true"),
        // `for…in` over the indices is item 211's, which builds `for…in`.
        // `forEach` and the iterators are `Array.prototype`'s (queue item
        // 331), which `a_node_list_is_walked_by_arrays_own_functions` checks.
        ("typeof document.querySelector", "undefined"),
    ] {
        assert_eq!(answer(&format!("{list} {body}")), expected, "{body}");
    }
    for (body, expected) in [
        ("'use strict'; list[0] = 1", "TypeError"),
        ("'use strict'; list[9] = 1", "TypeError"),
        ("'use strict'; delete list[0]", "TypeError"),
        ("list.item()", "TypeError"),
        ("list.item.call({}, 0)", "TypeError"),
        (
            "list.item({ valueOf: function () { throw new RangeError('x'); } })",
            "RangeError",
        ),
    ] {
        let (strict, rest) = body
            .strip_prefix("'use strict'; ")
            .map_or(("", body), |rest| ("'use strict'; ", rest));
        assert_eq!(
            thrown(&format!("(function () {{ {strict}{list} {rest}; }})()")),
            expected,
            "{body}"
        );
    }
}

#[test]
fn a_node_list_is_static() {
    for (body, expected) in [
        // Taken out of the document, an element is still in the list.
        (
            "var list = document.querySelectorAll('.btn'); \
             document.getElementById('a2').remove(); \
             list.length + ' ' + list[1].getAttribute('id') + ' ' + list[1].parentNode",
            "4 a2 null",
        ),
        // A new match is not added, and a changed one not taken away.
        (
            "var list = document.querySelectorAll('.rec'); \
             document.getElementById('a').classList.add('rec'); \
             document.getElementById('b').classList.remove('rec'); \
             list.length + ' ' + list[0].getAttribute('id') + ' ' + \
             document.querySelectorAll('.rec')[0].getAttribute('id')",
            "1 b a",
        ),
    ] {
        assert_eq!(answer(body), expected, "{body}");
    }
}

#[test]
fn a_list_keeps_a_detached_node_through_every_collection() {
    // The removed element is held only by the list; the collector runs at
    // every allocation in the second run, and the node must still answer.
    assert_eq!(
        answer(
            "var list = document.querySelectorAll('#b1'); \
             document.getElementById('b').removeChild(list[0]); \
             for (var i = 0; i < 50; i++) { document.createElement('i'); } \
             list[0].firstChild.getAttribute('href')"
        ),
        "/y"
    );
}

#[test]
fn a_hostile_selector_or_a_large_document_is_answered_rather_than_crashing() {
    let nested = |depth: usize| format!("{}a{}", ":is(".repeat(depth), ")".repeat(depth));
    // Deeper than `alo-css` hands its parser: refused, not a stack overflow.
    assert_eq!(
        thrown(&format!("document.querySelectorAll({:?})", nested(100_000))),
        "SyntaxError"
    );
    assert_eq!(
        thrown(&format!("document.querySelectorAll({:?})", nested(33))),
        "SyntaxError"
    );
    assert_eq!(
        answer(&format!(
            "document.querySelectorAll({:?}).length",
            nested(32)
        )),
        "3"
    );
    // Two thousand elements, every one matched and wrapped.
    let large = format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        "<p class=x><b></b></p>".repeat(1_000)
    );
    assert_eq!(
        answer_on(&large, "document.querySelectorAll('p, b').length"),
        "2000"
    );
    assert_eq!(
        answer_on(
            &large,
            "document.querySelectorAll('p:nth-child(1000) > b').length"
        ),
        "1"
    );
}

#[test]
fn the_embedders_stop_ends_a_query() {
    let Ok(mut page) = Page::new(PAGE, false) else {
        panic!("a page");
    };
    let Ok(program) = script("document.querySelectorAll('a')") else {
        panic!("a program");
    };
    page.engine.stop().ask();
    assert_eq!(
        page.engine.evaluate(&program),
        Err(Trouble::Escaped(Escape::Interrupted))
    );
    page.engine.stop().clear();
    let answered = page.run("document.querySelectorAll('a').length");
    assert_eq!(answered, "3");
}
