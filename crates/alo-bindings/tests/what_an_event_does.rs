/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 254: events from script (ADR 0018 §§ 1–3 and 8).
//!
//! *A page's script adds capture and bubble listeners on a nested tree,
//! dispatches a `CustomEvent`, and the order, `eventPhase`, `currentTarget`,
//! `once`, `passive`, both stops and a throwing listener — reported, the
//! dispatch carrying on — are asserted.*
//!
//! Each script runs against an adopted, installed document, and answers a
//! string it built as it went; a listener's throw is set aside by the engine
//! and handed over after the run, as the renderer will be handed it, and is
//! appended to the answer. Every script runs twice — once with the collector
//! running at every allocation — and the two must agree: a dispatch keeps its
//! path, its current listener and its event across everything the listeners
//! it calls allocate.

use alo_bindings::{Released, adopt, install};
use alo_dom::parse_document;
use alo_js::abrupt::Thrown;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// An engine with a page's document installed, and the root on its cell.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(markup: &str, stress: bool) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell =
            adopt(engine.objects(), parse_document(markup)).map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
            cell,
        })
    }

    /// What `source` evaluates to, then every throw a dispatch reported,
    /// then the run's own escape.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        let outcome = self.engine.evaluate(&program);
        let mut said = Vec::new();
        let more = self.engine.hand_over_reported(&mut |objects, thrown, _| {
            said.push(match thrown {
                Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
                Thrown::Value { value, .. } => match value {
                    Value::Text(held) => format!(
                        "threw {}",
                        objects
                            .units(*held)
                            .map_or_else(|| "?".to_owned(), String::from_utf16_lossy)
                    ),
                    _ => "threw something".to_owned(),
                },
            });
        });
        if more > 0 {
            said.push(format!("and {more} more"));
        }
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

    fn released(&self) -> Released {
        let (_, objects) = self.engine.intrinsics();
        objects
            .embedded::<alo_bindings::DocumentCell>(self.cell)
            .map(alo_bindings::DocumentCell::released)
            .unwrap_or_default()
    }
}

const PAGE: &str = "<!DOCTYPE html><html><head></head><body><div id=d><p id=p><span id=s>x</span>\
                    </p></div></body></html>";

/// The walk every script starts from.
const NAMES: &str = "var html = document.documentElement; var body = html.lastChild; \
                     var div = body.firstChild; var p = div.firstChild; var span = p.firstChild; \
                     var out = ''; function say(what) { out += (out === '' ? '' : ',') + what; }";

/// Run `source` after [`NAMES`], both ways, and answer what it answered.
fn run(source: &str) -> String {
    let script = format!("{NAMES}\n{source}");
    let ordinary = match Page::new(PAGE, false) {
        Ok(mut page) => page.run(&script),
        Err(why) => return why,
    };
    let stressed = match Page::new(PAGE, true) {
        Ok(mut page) => page.run(&script),
        Err(why) => return why,
    };
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// Check a table of scripts against what each answered.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        assert_eq!(&run(source), expected, "{source}");
    }
}

#[test]
fn capture_runs_root_to_target_and_bubble_target_to_root() {
    table(&[
        // Every node on the path has a capture and a bubble listener, each
        // writing its name, the phase and whether `currentTarget` is it.
        (
            "\
             var named = [[document, 'doc'], [html, 'html'], [body, 'body'], [div, 'div'], \
                          [p, 'p'], [span, 'span']]; \
             for (let i = 0; i < named.length; i++) { \
               let node = named[i][0], name = named[i][1]; \
               node.addEventListener('go', e => { \
                 say(name + ':b' + e.eventPhase + (e.currentTarget === node ? '' : '!')); \
               }); \
               node.addEventListener('go', e => { \
                 say(name + ':c' + e.eventPhase + (e.target === span ? '' : '?')); \
               }, true); \
             } \
             var e = new CustomEvent('go', { bubbles: true, detail: 7 }); \
             var answered = span.dispatchEvent(e); \
             out + ' / ' + answered + ' ' + e.eventPhase + ' ' + e.currentTarget + \
               ' ' + (e.target === span) + ' ' + e.detail;",
            "doc:c1,html:c1,body:c1,div:c1,p:c1,span:c2,span:b2,p:b3,div:b3,body:b3,html:b3,\
             doc:b3 / true 0 null true 7",
        ),
        // An event that does not bubble is captured all the way down and
        // stops at its target.
        (
            "\
             div.addEventListener('quiet', () => say('div capture'), true); \
             div.addEventListener('quiet', () => say('div bubble')); \
             span.addEventListener('quiet', () => say('span')); \
             span.dispatchEvent(new Event('quiet')); out;",
            "div capture,span",
        ),
        // Listeners on one target run in the order they were added, and only
        // those for the event's type.
        (
            "var out = ''; \
             span.addEventListener('a', () => out += 1); \
             span.addEventListener('b', () => out += 'no'); \
             span.addEventListener('a', () => out += 2); \
             span.dispatchEvent(new Event('a')); out;",
            "12",
        ),
        // A detached tree's path ends at its own root.
        (
            "var root = document.createElement('section'); \
             var leaf = document.createElement('em'); root.appendChild(leaf); \
             root.addEventListener('x', () => say('root')); \
             document.addEventListener('x', () => say('document')); \
             leaf.dispatchEvent(new Event('x', { bubbles: true })); out;",
            "root",
        ),
    ]);
}

#[test]
fn once_passive_and_the_two_stops() {
    table(&[
        // `once` runs once.
        (
            "var out = 0; span.addEventListener('x', () => out++, { once: true }); \
             span.dispatchEvent(new Event('x')); span.dispatchEvent(new Event('x')); out;",
            "1",
        ),
        // A passive listener cannot cancel; a later ordinary one can, and
        // `dispatchEvent` answers that it was.
        (
            "var out = ''; \
             span.addEventListener('x', e => { e.preventDefault(); out += e.defaultPrevented; }, \
                                   { passive: true }); \
             span.addEventListener('x', e => { e.preventDefault(); out += e.defaultPrevented; }); \
             var e = new Event('x', { cancelable: true }); var answered = span.dispatchEvent(e); \
             out + ' ' + answered + ' ' + e.defaultPrevented;",
            "falsetrue false true",
        ),
        // An event that is not cancelable is not cancelled.
        (
            "span.addEventListener('x', e => e.preventDefault()); \
             var e = new Event('x'); span.dispatchEvent(e) + ' ' + e.defaultPrevented;",
            "true false",
        ),
        // `stopPropagation` finishes the current target's listeners and
        // goes no further.
        (
            "\
             div.addEventListener('x', e => { say('div1'); e.stopPropagation(); }, true); \
             div.addEventListener('x', () => say('div2'), true); \
             p.addEventListener('x', () => say('p'), true); \
             span.addEventListener('x', () => say('span')); \
             span.dispatchEvent(new Event('x', { bubbles: true })); out;",
            "div1,div2",
        ),
        // `stopImmediatePropagation` stops after the current listener.
        (
            "\
             span.addEventListener('x', e => { say(1); e.stopImmediatePropagation(); }); \
             span.addEventListener('x', () => say(2)); \
             p.addEventListener('x', () => say(3)); \
             span.dispatchEvent(new Event('x', { bubbles: true })); out;",
            "1",
        ),
        // A stop at the target in the capture pass ends the bubble pass at
        // the target too; and a new dispatch starts unstopped.
        (
            "var e = new Event('x', { bubbles: true }); \
             span.addEventListener('x', ev => { say('c'); ev.stopPropagation(); }, true); \
             span.addEventListener('x', () => say('b')); \
             span.dispatchEvent(e); span.dispatchEvent(e); out;",
            "c,c",
        ),
        // `passive` defaults to true for a wheel listener on the body, and
        // to false on anything else.
        (
            "var out = ''; \
             body.addEventListener('wheel', e => { e.preventDefault(); out += e.defaultPrevented; }); \
             div.addEventListener('wheel', e => { e.preventDefault(); out += e.defaultPrevented; }); \
             div.dispatchEvent(new Event('wheel', { bubbles: true, cancelable: true })); out;",
            "truetrue",
        ),
        (
            "var out = ''; \
             body.addEventListener('wheel', e => { e.preventDefault(); out += e.defaultPrevented; }); \
             body.dispatchEvent(new Event('wheel', { cancelable: true })); out;",
            "false",
        ),
    ]);
}

#[test]
fn a_listener_that_throws_is_reported_and_the_dispatch_carries_on() {
    table(&[
        (
            "\
             span.addEventListener('x', () => { say(1); throw 'first'; }); \
             span.addEventListener('x', () => { say(2); null.y; }); \
             span.addEventListener('x', () => say(3)); \
             var caught = 'nothing caught'; \
             try { say(span.dispatchEvent(new Event('x'))); } catch (e) { caught = e; } \
             out + ' ' + caught;",
            "1,2,3,true nothing caught | threw first | TypeError: cannot read property 'y' of \
             null",
        ),
        // Dispatching an event that is being dispatched is an
        // `InvalidStateError` thrown into the listener, which may catch it.
        (
            "var out = ''; var e = new Event('x'); \
             span.addEventListener('x', () => { \
               try { span.dispatchEvent(e); } catch (error) { out += error.name; } }); \
             span.dispatchEvent(e); out;",
            "InvalidStateError",
        ),
        // Uncaught, it is reported by the outer dispatch like any throw.
        (
            "var e = new Event('x'); \
             span.addEventListener('x', () => span.dispatchEvent(e)); \
             span.dispatchEvent(e);",
            "true | threw something",
        ),
        // Another event may be dispatched from a listener, and nests.
        (
            "\
             span.addEventListener('outer', () => { say('o1'); \
               p.dispatchEvent(new Event('inner')); say('o2'); }); \
             p.addEventListener('inner', () => say('i')); \
             span.dispatchEvent(new Event('outer')); out;",
            "o1,i,o2",
        ),
    ]);
}

#[test]
fn a_list_changed_during_its_own_dispatch() {
    table(&[
        // One added to the current target during its turn does not run; one
        // removed before its turn does not either.
        (
            "function late() { say('late'); } function gone() { say('gone'); } \
             span.addEventListener('x', () => { say(1); span.addEventListener('x', late); \
               span.removeEventListener('x', gone); }); \
             span.addEventListener('x', gone); \
             span.dispatchEvent(new Event('x')); \
             say('|'); span.dispatchEvent(new Event('x')); out;",
            "1,|,1,late",
        ),
        // One added to an ancestor during the capture pass runs when the
        // bubble pass reaches it.
        (
            "\
             span.addEventListener('x', () => { say('span'); \
               div.addEventListener('x', () => say('div')); }); \
             span.dispatchEvent(new Event('x', { bubbles: true })); out;",
            "span,div",
        ),
        // The same type, callback and capture is added once; a different
        // capture is a different listener, and removal needs the same one.
        (
            "var out = 0; function f() { out++; } \
             span.addEventListener('x', f); span.addEventListener('x', f); \
             span.addEventListener('x', f, true); \
             span.dispatchEvent(new Event('x')); \
             span.removeEventListener('x', f, true); span.dispatchEvent(new Event('x')); \
             span.removeEventListener('x', f, { capture: false }); \
             span.dispatchEvent(new Event('x')); out;",
            "3",
        ),
    ]);
}

#[test]
fn a_callback_object_has_its_handle_event_called() {
    table(&[
        (
            "var out = ''; var o = { handleEvent(e) { out += (this === o) + e.type; } }; \
             span.addEventListener('x', o); span.dispatchEvent(new Event('x')); out;",
            "truex",
        ),
        // Looked up when it is called, so a page may change it.
        (
            "var out = ''; var o = {}; span.addEventListener('x', o); \
             o.handleEvent = () => out += 'late'; span.dispatchEvent(new Event('x')); out;",
            "late",
        ),
        // Read through a getter, whose throw is reported like a listener's.
        (
            "var out = ''; \
             var o = { get handleEvent() { out += 'got;'; return () => out += 'called'; } }; \
             var bad = { get handleEvent() { throw 'no'; } }; \
             span.addEventListener('x', bad); span.addEventListener('x', o); \
             span.dispatchEvent(new Event('x')); out;",
            "got;called | threw no",
        ),
        // Not callable is the `TypeError` the standard reports.
        (
            "var after = 0; span.addEventListener('x', {}); \
             span.addEventListener('x', () => after++); \
             span.dispatchEvent(new Event('x')); after;",
            "1 | TypeError: undefined is not a function",
        ),
    ]);
}

#[test]
fn the_arguments_are_converted_as_web_idl_says() {
    table(&[
        // A `null` callback adds nothing; anything else that is not an
        // object is a `TypeError`.
        (
            "span.addEventListener('x', null); span.removeEventListener('x', undefined); \
             var out = ''; try { span.addEventListener('x', 3); } catch (e) { out = e.name; } out;",
            "TypeError",
        ),
        // The dictionary's getters run in order, and a boolean is `capture`.
        (
            "var out = ''; \
             var options = { get signal() { out += 's'; }, get passive() { out += 'p'; }, \
                             get once() { out += 'o'; return true; }, \
                             get capture() { out += 'c'; return 1; } }; \
             var n = 0; span.addEventListener('x', () => n++, options); \
             span.dispatchEvent(new Event('x')); span.dispatchEvent(new Event('x')); \
             out + n;",
            "cops1",
        ),
        // `signal` is an `AbortSignal`, which does not exist yet: anything
        // but `undefined` is the conversion's `TypeError`.
        (
            "var out = ''; \
             try { span.addEventListener('x', () => {}, { signal: null }); } \
             catch (e) { out = e.name; } out;",
            "TypeError",
        ),
        // The type is a string, made by an object's `toString`.
        (
            "var out = ''; span.addEventListener({ toString() { return 'made'; } }, \
                                                 () => out += 'ran'); \
             span.dispatchEvent(new Event('made')); out;",
            "ran",
        ),
        // Too few arguments, and a `this` that is not a target.
        (
            "var out = ''; try { span.addEventListener('x'); } catch (e) { out += e.name; } \
             try { span.addEventListener.call({}, 'x', () => {}); } catch (e) { out += e.name; } \
             try { span.dispatchEvent({}); } catch (e) { out += e.name; } out;",
            "TypeErrorTypeErrorTypeError",
        ),
    ]);
}

#[test]
fn an_event_is_made_by_its_constructor() {
    table(&[
        (
            "var e = new Event('look'); \
             say(e.type); say(e.bubbles); say(e.cancelable); say(e.composed); \
             say(e.defaultPrevented); say(e.eventPhase); say(e.target); say(e.currentTarget); out;",
            "look,false,false,false,false,0,null,null",
        ),
        (
            "var e = new Event('x', { bubbles: 1, cancelable: 'yes', composed: 0 }); \
             '' + e.bubbles + e.cancelable + e.composed;",
            "truetruefalse",
        ),
        // The dictionary's getters run in its order, and the type before
        // them, even when it is an object.
        (
            "var out = ''; var e = new CustomEvent({ toString() { out += 't'; return 'ty'; } }, \
               { get detail() { out += 'd'; return 5; }, get composed() { out += 'm'; }, \
                 get bubbles() { out += 'b'; return true; }, get cancelable() { out += 'c'; } }); \
             out + ' ' + e.type + e.bubbles + e.detail;",
            "tbcmd tytrue5",
        ),
        (
            "var out = ''; try { Event('x'); } catch (e) { out += e.name; } \
             try { new Event(); } catch (e) { out += e.name; } \
             try { new Event('x', 4); } catch (e) { out += e.name; } out;",
            "TypeErrorTypeErrorTypeError",
        ),
        // The constants, on the constructor and the prototype.
        (
            "say(Event.NONE); say(Event.CAPTURING_PHASE); say(Event.AT_TARGET); \
             say(Event.BUBBLING_PHASE); say(new Event('x').AT_TARGET); out;",
            "0,1,2,3,2",
        ),
        // The chain, and `constructor` back.
        (
            "var c = new CustomEvent('x'); \
             '' + (c.__proto__ === CustomEvent.prototype) + \
             (CustomEvent.prototype.__proto__ === Event.prototype) + \
             (CustomEvent.__proto__ === Event) + (Event.prototype.constructor === Event) + \
             c.detail + (new CustomEvent('y', { detail: undefined }).detail);",
            "truetruetruetruenullnull",
        ),
        // `detail` is a `CustomEvent`'s alone.
        (
            "var out = ''; var plain = new Event('x'); plain.__proto__ = CustomEvent.prototype; \
             try { plain.detail; } catch (e) { out += e.name; } \
             try { span.dispatchEvent(1); } catch (e) { out += e.name; } out;",
            "TypeErrorTypeError",
        ),
    ]);
}

#[test]
fn what_is_read_during_a_dispatch_and_after() {
    table(&[
        // `composedPath()` is the path from the target up while it is
        // dispatched, and empty after.
        (
            "var seen; span.addEventListener('x', e => { seen = e.composedPath(); }); \
             var e = new Event('x'); span.dispatchEvent(e); \
             '' + seen.length + (seen[0] === span) + (seen[1] === p) + (seen[5] === document) + \
             ' ' + e.composedPath().length;",
            "6truetruetrue 0",
        ),
        // A page may hang its own properties off an event.
        (
            "var e = new Event('x'); e.note = 'kept'; \
             span.addEventListener('x', ev => { ev.note += '!'; }); span.dispatchEvent(e); e.note;",
            "kept!",
        ),
    ]);
}

#[test]
fn the_legacy_members_and_what_waits_for_its_item_are_absent() {
    table(&[(
        "var e = new Event('x'); \
         say(typeof e.returnValue); say(typeof e.cancelBubble); say(typeof e.srcElement); \
         say(typeof e.initEvent); say(typeof document.createEvent); say(typeof e.timeStamp); \
         say(typeof addEventListener); say(typeof EventTarget); \
         say(typeof new CustomEvent('x').initCustomEvent); out;",
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined,\
         undefined",
    )]);
}

#[test]
fn a_node_on_the_path_is_kept_until_the_dispatch_ends() {
    // `a` is reachable from script only through its own listener's closure
    // and the dispatch: the target's listener detaches the target from it
    // and drops every reference, and a collection runs at every allocation.
    // The standard's path keeps `a`, so its bubble listener still runs.
    table(&[(
        "\
         function onA() { say('a'); } function onRoot() { say('root'); } \
         function onB(e) { say('b'); e.target.remove(); \
           var junk = []; for (let i = 0; i < 50; i++) junk[i] = [i]; } \
         function build() { \
           var root = document.createElement('section'); \
           var a = document.createElement('article'); root.appendChild(a); \
           var b = document.createElement('b'); a.appendChild(b); \
           a.addEventListener('x', onA); root.addEventListener('x', onRoot); \
           b.addEventListener('x', onB); return b; \
         } \
         var target = build(); \
         target.dispatchEvent(new Event('x', { bubbles: true })); out;",
        "b,a,root",
    )]);
}

#[test]
fn a_listener_goes_with_its_detached_tree() {
    let Ok(mut page) = Page::new(PAGE, false) else {
        panic!("a page");
    };
    let before = page.released();
    let answer = page.run(
        "(function () { var lost = document.createElement('div'); \
           var big = []; for (let i = 0; i < 100; i++) big[i] = [i]; \
           lost.addEventListener('x', () => big.length); })(); 'made';",
    );
    assert_eq!(answer, "made");
    page.engine.objects().heap_mut().collect();
    let after = page.released();
    assert_eq!(
        after.trees,
        before.trees + 1,
        "the detached tree, listener, closure and all, was released"
    );
    assert!(page.engine.objects().heap().check().is_ok());
}

#[test]
fn a_hostile_script_meets_a_bound_rather_than_a_crash() {
    table(&[
        // A path five thousand deep is walked, both ways, and nothing on it
        // costs this process's stack.
        (
            "var top = document.createElement('div'); var at = top; \
             for (let i = 0; i < 5000; i++) { var next = document.createElement('i'); \
               at.appendChild(next); at = next; } \
             var n = 0; top.addEventListener('x', () => n++, true); \
             top.addEventListener('x', () => n++); \
             at.dispatchEvent(new Event('x', { bubbles: true })); n;",
            "2",
        ),
        // A listener that dispatches again for ever reaches the engine's call
        // depth: the innermost call is a `RangeError`, reported by the
        // dispatch it was in, and every dispatch above it finishes.
        (
            "var depth = 0; span.addEventListener('x', () => { depth++; \
               span.dispatchEvent(new Event('x')); }); \
             span.dispatchEvent(new Event('x')); depth > 1000;",
            "true | RangeError: this script calls more deeply than this engine will go",
        ),
        // A thousand listeners that throw: the first 256 are kept, the rest
        // counted, and every one of them ran.
        (
            "var n = 0; for (let i = 0; i < 1000; i++) \
               span.addEventListener('x', () => { n++; throw 'e'; }); \
             span.dispatchEvent(new Event('x')); n;",
            &format!("1000 | {} | and 744 more", vec!["threw e"; 256].join(" | ")),
        ),
        // A getter in the dictionary that throws is the constructor's throw,
        // not a report.
        (
            "var out = ''; try { new Event('x', { get bubbles() { throw 'g'; } }); } \
             catch (e) { out = e; } out;",
            "g",
        ),
    ]);
}
