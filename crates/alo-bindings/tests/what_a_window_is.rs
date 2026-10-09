/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 362, as ADR 0037 designs it: **the global object is a
//! `Window`**, and it is an event target last on every path.
//!
//! - `window`, `self`, `globalThis` and the top-level `this` are one object;
//!   `window` is unforgeable, `self` replaceable, and `location` the
//!   window's own unforgeable accessor (§ 4).
//! - `Window.prototype` inherits from `EventTarget.prototype`, holds nothing
//!   of its own, and there is no named access (§ 4, law 1).
//! - A listener added to the window is kept through collections and called
//!   by a dispatch at it, and by a dispatch at a node of the document — first
//!   when capturing, last when bubbling — except for `load`, which stops at
//!   the document (§ 3).
//! - `EventTarget`'s brand check accepts a node or a window and nothing else,
//!   and the window's default `passive` is the standard's (§ 2).
//! - `install` refuses an engine whose global object is not a `Window`.
//! - alo Sites' analytics script, which opened the item, runs to its end,
//!   and its `pagehide` listener on the window is reached by a dispatch at
//!   it — and stops at what is not built next.
//!
//! Every script runs twice — once with the collector at every allocation —
//! and the two must agree.

use alo_bindings::{Window, adopt, install};
use alo_dom::parse_document;
use alo_js::abrupt::Thrown;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{Fixed, numeric, script};

/// alo Sites' call-to-action page, frozen: its one inline script listens on
/// the window for `pagehide`.
const CTA: &str = include_str!("../../alo-corpus/cases/alo-sites-cta/page.html");

/// An engine with a page's document installed, and the root on its cell.
struct Page {
    engine: Engine,
    _root: Root,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document(PAGE)).map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
        })
    }

    /// What `source` evaluates to, then every throw a dispatch reported.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        let outcome = self.engine.evaluate(&program);
        let mut said = Vec::new();
        self.engine.hand_over_reported(&mut |_, thrown, _| {
            said.push(match thrown {
                Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
                Thrown::Value { .. } => "threw something".to_owned(),
            });
        });
        let answer = match outcome {
            Ok(value) => self.show(value),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        };
        if said.is_empty() {
            answer
        } else {
            format!("{answer} | {}", said.join(" | "))
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
}

const PAGE: &str = "<!DOCTYPE html><html><head></head><body><div id=d><p id=p>x</p></div>\
                    </body></html>";

/// The walk every script starts from.
const NAMES: &str = "var html = document.documentElement; var body = html.lastChild; \
                     var div = body.firstChild; var p = div.firstChild; \
                     var out = ''; function say(what) { out += (out === '' ? '' : ',') + what; }";

/// Run `source` after [`NAMES`], both ways, and answer what it answered.
fn run(source: &str) -> String {
    let script = format!("{NAMES}\n{source}");
    let answers: Vec<String> = [false, true]
        .into_iter()
        .map(|stress| match Page::new(stress) {
            Ok(mut page) => page.run(&script),
            Err(why) => why,
        })
        .collect();
    assert_eq!(
        answers.first(),
        answers.get(1),
        "{source} answered differently when the collector ran at every allocation"
    );
    answers.first().cloned().unwrap_or_default()
}

/// Check a table of scripts against what each answered.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        assert_eq!(run(source), *expected, "{source}");
    }
}

#[test]
fn window_self_global_this_and_the_top_level_this_are_one_object() {
    table(&[
        // The closing condition, in the item's words.
        ("window === self", "true"),
        ("window === globalThis && self === globalThis", "true"),
        ("this === window", "true"),
        (
            "window.window === window && window.self.self === window",
            "true",
        ),
        ("typeof window", "object"),
        // A `var` is a property of the window, and the window's members are
        // names.
        ("var a = 1; window.a", "1"),
        ("window.b = 2; b", "2"),
        ("function f() { return this; } f() === window", "true"),
        // The language is still there.
        (
            "typeof Date + typeof Promise + typeof undefined",
            "functionfunctionundefined",
        ),
        ("window.document === document", "true"),
    ]);
}

#[test]
fn window_is_unforgeable_and_self_is_replaceable() {
    table(&[
        ("window.hasOwnProperty('window')", "true"),
        ("delete window", "false"),
        ("window = 1; window === globalThis", "true"),
        // Strict, in a function of its own: the walk every script starts
        // from is in front of the directive at the top level.
        (
            "(function () { 'use strict'; try { window = 1; return 'assigned'; } \
               catch (e) { return e.name; } })()",
            "TypeError",
        ),
        ("window.hasOwnProperty('self')", "true"),
        // `[Replaceable]`: assigning defines an own data property in its place.
        ("self = 1; self", "1"),
        (
            "(function () { 'use strict'; self = 'mine'; })(); self + (window === globalThis)",
            "minetrue",
        ),
        ("self = 1; delete self; typeof self", "undefined"),
        ("delete self; typeof self", "undefined"),
    ]);
}

#[test]
fn location_is_the_windows_own_and_answers_the_documents() {
    table(&[
        ("window.location === document.location", "true"),
        ("location === window.location", "true"),
        ("window.hasOwnProperty('location')", "true"),
        ("delete window.location", "false"),
        ("window.__proto__.hasOwnProperty('location')", "false"),
        ("location.href", "about:blank"),
        (
            "try { window.location = 'https://elsewhere.example/'; 'went' } catch (e) { 'refused' }",
            "! a script navigating by 'location' is queue item 85, through the ask of ADR 0020",
        ),
    ]);
}

#[test]
fn window_prototype_is_an_event_target_and_holds_nothing_of_its_own() {
    // `EventTarget.prototype`, reached from the document: `Document`,
    // `Node`, then `EventTarget`.
    let target = "var et = document.__proto__.__proto__.__proto__;";
    table(&[
        (
            &format!("{target} window.__proto__.__proto__ === et"),
            "true",
        ),
        (
            &format!("{target} window.__proto__ !== et && window.__proto__ !== ({{}}).__proto__"),
            "true",
        ),
        (
            "window.addEventListener === document.addEventListener",
            "true",
        ),
        ("window.hasOwnProperty('addEventListener')", "false"),
        (
            "window.__proto__.hasOwnProperty('addEventListener')",
            "false",
        ),
        ("window.__proto__.hasOwnProperty('self')", "false"),
        (
            "typeof addEventListener + typeof dispatchEvent",
            "functionfunction",
        ),
        // No named access (law 1): an element's id is not a name.
        ("typeof d + typeof window.d", "undefinedundefined"),
        // No interface object, as no interface a page cannot construct has.
        ("typeof Window", "undefined"),
    ]);
}

#[test]
fn a_listener_on_the_window_is_called_by_a_dispatch_at_it() {
    table(&[
        (
            "window.addEventListener('x', function (e) { \
               say(e.eventPhase); say(e.currentTarget === window); say(e.target === window); \
               say(this === window); say(e.composedPath().length); \
               say(e.composedPath()[0] === window); }); \
             say(window.dispatchEvent(new Event('x'))); out",
            "2,true,true,true,1,true,true",
        ),
        // Through the global's own name, and a bare call.
        (
            "addEventListener('x', function () { say('heard'); }); \
             dispatchEvent(new Event('x')); out",
            "heard",
        ),
        // `once`, removal, and a callback object's `handleEvent`.
        (
            "var n = 0; window.addEventListener('x', function () { n = n + 1; }, { once: true }); \
             window.dispatchEvent(new Event('x')); window.dispatchEvent(new Event('x')); n",
            "1",
        ),
        (
            "var n = 0; function f() { n = n + 1; } window.addEventListener('x', f); \
             window.removeEventListener('x', f); window.dispatchEvent(new Event('x')); n",
            "0",
        ),
        (
            "window.addEventListener('x', { handleEvent: function (e) { say(this !== window); } }); \
             window.dispatchEvent(new Event('x')); out",
            "true",
        ),
        // A cancelled event answers `false`.
        (
            "window.addEventListener('x', function (e) { e.preventDefault(); }); \
             window.dispatchEvent(new Event('x', { cancelable: true }))",
            "false",
        ),
        // A listener that throws is reported, and the next one runs.
        (
            "window.addEventListener('x', function () { throw new TypeError('no'); }); \
             window.addEventListener('x', function () { say('next'); }); \
             window.dispatchEvent(new Event('x')); out",
            "next | threw something",
        ),
        // Dispatching an event already being dispatched is refused.
        (
            "var e = new Event('x'); var caught; \
             window.addEventListener('x', function () { \
               try { window.dispatchEvent(e); } catch (x) { caught = x.name; } }); \
             window.dispatchEvent(e); caught",
            "InvalidStateError",
        ),
    ]);
}

#[test]
fn the_window_is_first_when_capturing_and_last_when_bubbling() {
    let listen = "function on(target, name) { \
                    target.addEventListener('x', function (e) { say(name + 'c' + e.eventPhase); }, true); \
                    target.addEventListener('x', function (e) { say(name + 'b' + e.eventPhase); }); } \
                  on(window, 'w'); on(document, 'd'); on(body, 'y'); on(p, 'p');";
    table(&[
        // The closing condition: after the document when bubbling, before it
        // when capturing.
        (
            &format!("{listen} p.dispatchEvent(new Event('x', {{ bubbles: true }})); out"),
            "wc1,dc1,yc1,pc2,pb2,yb3,db3,wb3",
        ),
        // An event that does not bubble still reaches the window's capturing
        // listener, and not its bubbling one.
        (
            &format!("{listen} p.dispatchEvent(new Event('x')); out"),
            "wc1,dc1,yc1,pc2,pb2",
        ),
        // At the document, the window is still beyond it.
        (
            &format!("{listen} document.dispatchEvent(new Event('x', {{ bubbles: true }})); out"),
            "wc1,dc2,db2,wb3",
        ),
        // Stopped at the document, the window does not hear the bubble.
        (
            &format!(
                "{listen} document.addEventListener('x', function (e) {{ e.stopPropagation(); }}); \
                 p.dispatchEvent(new Event('x', {{ bubbles: true }})); out"
            ),
            "wc1,dc1,yc1,pc2,pb2,yb3,db3",
        ),
        // Stopped at the window while capturing, nothing else hears it.
        (
            &format!(
                "window.addEventListener('x', function (e) {{ e.stopPropagation(); }}, true); \
                 {listen} p.dispatchEvent(new Event('x', {{ bubbles: true }})); out"
            ),
            "wc1",
        ),
        // `currentTarget` is the window on its turn, and `composedPath()`
        // ends with it.
        (
            "var seen = []; window.addEventListener('x', function (e) { \
               seen = e.composedPath(); say(e.currentTarget === window); say(e.target === p); }); \
             p.dispatchEvent(new Event('x', { bubbles: true })); \
             say(seen.length); say(seen[seen.length - 1] === window); out",
            "true,true,6,true",
        ),
    ]);
}

#[test]
fn a_load_event_stops_at_the_document() {
    table(&[
        (
            "window.addEventListener('load', function () { say('w'); }, true); \
             window.addEventListener('load', function () { say('w'); }); \
             document.addEventListener('load', function () { say('d'); }); \
             p.dispatchEvent(new Event('load', { bubbles: true })); out",
            "d",
        ),
        (
            "var seen; p.addEventListener('load', function (e) { seen = e.composedPath(); }); \
             p.dispatchEvent(new Event('load')); '' + seen.length + (seen[seen.length - 1] === document)",
            "5true",
        ),
        // At the window itself, a `load` is heard there.
        (
            "window.addEventListener('load', function () { say('w'); }); \
             window.dispatchEvent(new Event('load')); out",
            "w",
        ),
    ]);
}

#[test]
fn a_node_in_no_document_does_not_reach_the_window() {
    table(&[(
        "window.addEventListener('x', function () { say('w'); }, true); \
         var a = document.createElement('a'); var b = document.createElement('b'); a.appendChild(b); \
         a.addEventListener('x', function () { say('a'); }); \
         b.dispatchEvent(new Event('x', { bubbles: true })); out",
        "a",
    )]);
}

#[test]
fn the_brand_check_takes_a_node_or_a_window_and_nothing_else() {
    table(&[
        (
            "try { window.addEventListener.call({}, 'x', function () {}); 'added' } \
             catch (e) { e.name + ': ' + e.message }",
            "TypeError: 'addEventListener' was called on something that is not an EventTarget",
        ),
        (
            "try { document.dispatchEvent.call(location, new Event('x')); 'sent' } \
             catch (e) { e.name }",
            "TypeError",
        ),
        (
            "var f = function () { say('called'); }; \
             document.addEventListener.call(window, 'x', f); window.dispatchEvent(new Event('x')); \
             document.removeEventListener.call(window, 'x', f); window.dispatchEvent(new Event('x')); \
             out",
            "called",
        ),
    ]);
}

#[test]
fn the_windows_scrolling_listeners_are_passive_unless_told_otherwise() {
    table(&[
        (
            "window.addEventListener('wheel', function (e) { e.preventDefault(); }); \
             window.dispatchEvent(new Event('wheel', { cancelable: true }))",
            "true",
        ),
        (
            "window.addEventListener('touchmove', function (e) { e.preventDefault(); }, \
               { passive: false }); \
             window.dispatchEvent(new Event('touchmove', { cancelable: true }))",
            "false",
        ),
        (
            "window.addEventListener('x', function (e) { e.preventDefault(); }); \
             window.dispatchEvent(new Event('x', { cancelable: true }))",
            "false",
        ),
    ]);
}

#[test]
fn a_listener_on_the_window_is_kept_through_collections() {
    let Ok(mut page) = Page::new(false) else {
        panic!("a page installs");
    };
    assert_eq!(
        page.run(
            "var heard = 0; \
             window.addEventListener('x', function () { heard = heard + 1; }); \
             document.documentElement.addEventListener('x', function () {});"
        ),
        "undefined"
    );
    for _ in 0..4 {
        page.engine.objects().heap_mut().collect();
    }
    let Ok(global) = page.engine.global() else {
        panic!("the engine has a global object");
    };
    assert_eq!(
        page.engine
            .objects()
            .embedded::<Window>(global)
            .map(|window| window.listeners().len()),
        Some(1),
        "the window holds its one listener"
    );
    assert_eq!(
        page.run(
            "window.dispatchEvent(new Event('x')); \
             document.body.dispatchEvent(new Event('x', { bubbles: true })); heard"
        ),
        "2"
    );
    assert!(
        page.engine.objects().heap().check().is_ok(),
        "the heap is whole"
    );
}

#[test]
fn install_refuses_an_engine_whose_global_object_is_not_a_window() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let Ok(cell) = adopt(engine.objects(), parse_document(PAGE)) else {
        panic!("an empty heap holds a document");
    };
    let _root = engine.objects().heap_mut().root(cell);
    let refused = install(&mut engine, cell).map(|_: Ref| ());
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.to_string().contains("not a Window")),
        "{refused:?}"
    );
}

#[test]
fn alo_sites_analytics_script_runs_to_its_end_and_its_pagehide_listener_is_reached() {
    let (Some(start), Some(end)) = (CTA.find("<script>"), CTA.find("</script>")) else {
        panic!("the frozen page has its inline script");
    };
    let source = CTA.get(start + "<script>".len()..end).unwrap_or_default();
    for stress in [false, true] {
        let Ok(mut engine) = alo_bindings::engine(Some(std::rc::Rc::new(Fixed::at(1.0e12)))) else {
            panic!("an empty heap holds an engine");
        };
        let Ok(cell) = adopt(engine.objects(), parse_document(CTA)) else {
            panic!("an empty heap holds the page");
        };
        let root = engine.objects().heap_mut().root(cell);
        let Ok(_) = install(&mut engine, cell) else {
            panic!("the page installs");
        };
        engine.objects().heap_mut().stress(stress);
        let mut page = Page {
            engine,
            _root: root,
        };
        // Past line 32, `window.addEventListener("pagehide", record)`, to the
        // end: nothing is thrown.
        assert_eq!(page.run(source), "undefined", "the script runs to its end");
        // A click is heard by its capturing listener on the document, which
        // finds nothing it sends.
        assert_eq!(
            page.run("document.body.dispatchEvent(new Event('click', { bubbles: true }))"),
            "true"
        );
        // `visibilitychange` at the document: `visibilityState` is item 364's,
        // so the listener takes its other branch and reads the clock.
        assert_eq!(
            page.run("document.dispatchEvent(new Event('visibilitychange'))"),
            "true"
        );
        // `pagehide` at the window reaches `record`, which reads the window's
        // `scrollY` and `innerHeight` — absent, so `undefined` — and stops at
        // `Math.max` in `height()`, the script's seventeenth line: what is
        // next, queue item 365.
        assert_eq!(
            page.run("window.dispatchEvent(new Event('pagehide'))"),
            "true | ReferenceError: 'Math' is not defined"
        );
    }
}
