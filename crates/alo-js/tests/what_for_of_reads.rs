/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 230: *`for…of` over an array, and the iteration protocol it
//! reads through*. Cut from item 211 (`for…of`), item 75 (iterators) and item
//! 73 (`Symbol.iterator`, the array iterator and `Symbol.toStringTag`).
//!
//! # What opened it
//!
//! A real page's own script. The frozen service worker
//! (`crates/alo-corpus/scripts/alo-service-worker/script.js`, alo's own
//! `sw.js`) was refused at byte 2922 — `for (const account of …)` inside its
//! push handler — once item 210 had compiled the `try` around it.
//!
//! # What closes it
//!
//! `for…of` over an array produces what the specification says in a table of
//! programs; a hole is `undefined`; `let` and `const` are a binding per pass
//! and the head has a dead zone; leaving early by `break`, `return` or a
//! `continue` of an outer loop closes the iterator and a throw from the body
//! closes it ignoring what the closing does, while finishing normally and a
//! throw from the iterator itself close nothing; the protocol is the one a
//! page can intercept, a `next` and a `done` of its own included; and what is
//! not built — a pattern head, an element behind a getter — is refused by name.
//!
//! The iterator's prototype is reached the way a page reaches it,
//! `[].values().__proto__`, since no page can spell `Symbol.iterator` until the
//! `Symbol` function exists (item 73). A `return` placed there is what every
//! closing test counts.
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with the collector firing at every allocation, and
//! the two must agree: an iterator, its `next` and every result object it
//! makes are held only by frame slots and the stack between passes.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alo_js::abrupt::Escape;
use alo_js::compile::{Refusal, What};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Property, Value};
use alo_js::{numeric, script};

/// Run a program in a fresh engine, both ways, and answer what it produced.
fn value(source: &str) -> String {
    let ordinary = fresh(source, false);
    let stressed = fresh(source, true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// One run, in an engine of its own.
fn fresh(source: &str, stress: bool) -> String {
    let Ok(mut engine) = Engine::new() else {
        return "no engine".to_owned();
    };
    engine.objects().heap_mut().stress(stress);
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
        Ok(value) => show(&mut engine, value),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// A value, written out so that a failing test says what it got.
fn show(engine: &mut Engine, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(is) => is.to_string(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match engine.objects().units(held) {
            Some(units) => format!("\"{}\"", String::from_utf16_lossy(units)),
            None => "\"?\"".to_owned(),
        },
        Value::Symbol(_) => "a symbol".to_owned(),
        Value::Object(_) => "an object".to_owned(),
    }
}

/// Check a table of programs against what each evaluates to.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        assert_eq!(&value(source), expected, "{source}");
    }
}

/// The lines every closing test begins with: a `return` on the array
/// iterator's prototype that counts its calls and answers an object.
const COUNTED: &str = "let p = [].values().__proto__; let closed = 0; \
                       p.return = function () { closed++; return {}; }; ";

// --- What it produces --------------------------------------------------------

#[test]
fn an_array_is_iterated_in_order_and_a_hole_is_undefined() {
    table(&[
        ("let s = 0; for (const x of [1, 2, 3]) s += x; s", "6"),
        (
            "let s = ''; for (const x of ['a', 'b']) s += x; s",
            "\"ab\"",
        ),
        ("let n = 0; for (const x of []) n++; n", "0"),
        // A hole is read through, and reads as `undefined` — the element is
        // `Get`, not `HasProperty`.
        (
            "let s = ''; for (const x of [1, , 3]) s += x + ','; s",
            "\"1,undefined,3,\"",
        ),
        // A hole that a prototype fills is read from the prototype.
        (
            "[].__proto__[1] = 'p'; let s = ''; for (const x of [1, , 3]) s += x; s",
            "\"1p3\"",
        ),
    ]);
}

#[test]
fn keys_values_and_entries_each_hand_out_what_they_say() {
    table(&[
        (
            "let s = ''; for (const k of ['a', 'b'].keys()) s += k; s",
            "\"01\"",
        ),
        (
            "let s = ''; for (const v of ['a', 'b'].values()) s += v; s",
            "\"ab\"",
        ),
        (
            "let s = ''; for (const e of ['a', 'b'].entries()) s += e[0] + e[1] + e.length; s",
            "\"0a21b2\"",
        ),
        // An entry is a fresh array each time, from `Array.prototype`.
        (
            "let a = []; for (const e of [1, 2].entries()) a[a.length] = e; \
             a[0] !== a[1] && a[0].__proto__ === [].__proto__",
            "true",
        ),
        // `keys()` of a holey array still counts the holes.
        (
            "let s = ''; for (const k of [, , 'c'].keys()) s += k; s",
            "\"012\"",
        ),
    ]);
}

#[test]
fn an_array_that_grows_while_it_is_iterated_is_iterated_to_its_new_end() {
    table(&[
        (
            "let a = [1]; let n = 0; for (const x of a) { n++; if (a.length < 4) a[a.length] = x + 1; } n",
            "4",
        ),
        // And one that shrinks stops early.
        (
            "let a = [1, 2, 3, 4]; let s = ''; for (const x of a) { s += x; a.length = 2; } s",
            "\"12\"",
        ),
    ]);
}

#[test]
fn a_finished_iterator_stays_finished() {
    table(&[
        (
            "let a = [1]; let it = a.values(); it.next(); let last = it.next(); \
             a[1] = 2; let after = it.next(); \
             last.done + ',' + last.value + ',' + after.done + ',' + after.value",
            "\"true,undefined,true,undefined\"",
        ),
        // A result is `{ value, done }`, from `Object.prototype`.
        (
            "let r = [5].values().next(); r.value + ',' + r.done + ',' + (r.__proto__ === ({}).__proto__)",
            "\"5,false,true\"",
        ),
        (
            "let r = [5].values().next(); let s = ''; s += r.hasOwnProperty('value'); \
             s += r.hasOwnProperty('done'); s += r.propertyIsEnumerable('done'); s",
            "\"truetruetrue\"",
        ),
    ]);
}

#[test]
fn an_iterator_is_itself_iterable_and_carries_on_where_it_was() {
    table(&[
        ("let s = 0; for (const x of [1, 2].values()) s += x; s", "3"),
        (
            "let it = [1, 2, 3].values(); it.next(); let s = 0; for (const x of it) s += x; s",
            "5",
        ),
    ]);
}

#[test]
fn an_array_like_is_iterated_through_the_generic_methods() {
    table(&[
        (
            "let s = ''; for (const x of [].values.call({ length: 2, 0: 'a', 1: 'b', 2: 'c' })) s += x; s",
            "\"ab\"",
        ),
        // `ToLength`: a string converts, a fraction truncates, and a negative
        // or missing length is nothing.
        (
            "let s = ''; for (const x of [].values.call({ length: '2.9', 0: 'a', 1: 'b' })) s += x; s",
            "\"ab\"",
        ),
        (
            "let n = 0; for (const x of [].values.call({ length: -5, 0: 'a' })) n++; \
             for (const x of [].values.call({ 0: 'a' })) n++; n",
            "0",
        ),
        (
            "let s = ''; for (const k of [].keys.call({ length: 3 })) s += k; s",
            "\"012\"",
        ),
    ]);
}

#[test]
fn the_array_iterator_says_what_it_is() {
    table(&[
        ("'' + [].values()", "\"[object Array Iterator]\""),
        (
            "({}).toString.call([].entries())",
            "\"[object Array Iterator]\"",
        ),
        // The tag is a property of the prototype, so an object that inherits
        // from it says the same.
        (
            "'' + { __proto__: [].values().__proto__ }",
            "\"[object Array Iterator]\"",
        ),
        // Every other object is what it was.
        ("'' + {}", "\"[object Object]\""),
        ("({}).toString.call([])", "\"[object Array]\""),
        (
            "({}).toString.call(function () {})",
            "\"[object Function]\"",
        ),
        ("({}).toString.call(null)", "\"[object Null]\""),
        // And the iterator's prototype carries nothing the protocol does not:
        // the helpers are item 73's.
        ("typeof [].values().map", "\"undefined\""),
        ("typeof [].push", "\"undefined\""),
    ]);
}

// --- Bindings ----------------------------------------------------------------

#[test]
fn let_and_const_are_a_binding_per_pass() {
    table(&[
        (
            "let fs = []; for (let x of [1, 2, 3]) fs[fs.length] = () => x; \
             fs[0]() + fs[1]() * 10 + fs[2]() * 100",
            "321",
        ),
        (
            "let fs = []; for (const x of [1, 2, 3]) fs[fs.length] = () => x; \
             fs[0]() + fs[1]() * 10 + fs[2]() * 100",
            "321",
        ),
        // A `let` may be changed inside its own pass and nowhere else.
        (
            "let s = ''; for (let x of [1, 2]) { x = x * 10; s += x; } s",
            "\"1020\"",
        ),
        // The head's name is the loop's own: the outer one is untouched.
        ("let x = 'outer'; for (let x of [1]) {} x", "\"outer\""),
    ]);
}

#[test]
fn a_const_head_cannot_be_assigned_to() {
    table(&[(
        "try { for (const x of [1]) { x = 2; } 'no' } catch (e) { e.constructor === TypeError }",
        "true",
    )]);
}

#[test]
fn the_head_has_a_dead_zone_while_the_iterable_is_evaluated() {
    table(&[
        (
            "let x = [1]; try { for (const x of x) {} 'no' } catch (e) { e.constructor === ReferenceError }",
            "true",
        ),
        // A closure made in the iterable sees that dead zone for ever, not the
        // pass's binding.
        (
            "let f; let n = 0; for (let x of (f = () => x, [1, 2])) n++; \
             try { f(); 'no' } catch (e) { n + ':' + (e.constructor === ReferenceError) }",
            "\"2:true\"",
        ),
    ]);
}

#[test]
fn var_a_name_and_a_property_are_assigned_each_pass() {
    table(&[
        ("var s = 0; for (var x of [1, 2]) s += x; s + x", "5"),
        ("let x; for (x of [7, 8]); x", "8"),
        ("let o = {}; for (o.p of [1, 2]); o.p", "2"),
        ("let o = {}; let k = 'q'; for (o[k] of [3]); o.q", "3"),
        // The target is evaluated each pass, after the value is read.
        (
            "let n = 0; let o = {}; function t() { n++; return o; } for (t().p of [1, 2, 3]); n + ',' + o.p",
            "\"3,3\"",
        ),
        // A setter on the target runs once per value.
        (
            "let seen = ''; let o = { set p(v) { seen += v; } }; for (o.p of ['a', 'b']); seen",
            "\"ab\"",
        ),
    ]);
}

// --- Completion values -------------------------------------------------------

#[test]
fn a_for_of_completes_as_the_specification_says() {
    table(&[
        ("for (const x of [1, 2]) x;", "2"),
        ("1; for (const x of []) x;", "undefined"),
        ("1; for (const x of [1]) { 2; break; }", "2"),
        (
            "let n = 0; out: for (const x of [1, 2]) { n += x; continue out; } n",
            "3",
        ),
    ]);
}

// --- Leaving early closes the iterator --------------------------------------

#[test]
fn finishing_normally_closes_nothing() {
    table(&[
        (
            &format!("{COUNTED} for (const x of [1, 2, 3]) {{}} closed"),
            "0",
        ),
        // `continue` is the next pass, not a way out.
        (
            &format!("{COUNTED} for (const x of [1, 2]) {{ continue; }} closed"),
            "0",
        ),
    ]);
}

#[test]
fn a_break_closes_the_iterator_once() {
    table(&[
        (
            &format!(
                "{COUNTED} let s = ''; for (const x of [1, 2, 3]) {{ s += x; if (x == 2) break; }} s + closed"
            ),
            "\"121\"",
        ),
        // A labelled break of the loop itself.
        (
            &format!("{COUNTED} out: for (const x of [1, 2]) {{ break out; }} closed"),
            "1",
        ),
        // `return` is called with the iterator as its `this`.
        (
            "let p = [].values().__proto__; let it = null; \
             p.return = function () { it = this; return {}; }; \
             for (const x of [1]) break; it.__proto__ === p",
            "true",
        ),
    ]);
}

#[test]
fn a_return_closes_the_iterator_after_its_value_is_made() {
    table(&[(
        &format!(
            "{COUNTED} function f() {{ for (const x of [1, 2]) return x * 10 + closed; }} \
             f() + ',' + closed"
        ),
        "\"10,1\"",
    )]);
}

#[test]
fn a_continue_of_an_outer_loop_closes_the_inner_iterator_only() {
    table(&[
        (
            &format!(
                "{COUNTED} let n = 0; out: for (const a of [1, 2]) {{ for (const b of [1, 2]) {{ n++; continue out; }} }} \
                 n + ',' + closed"
            ),
            "\"2,2\"",
        ),
        // A `for (;;)` outside: the `continue` leaves the `for…of` and closes it.
        (
            &format!(
                "{COUNTED} let n = 0; out: for (let i = 0; i < 3; i++) {{ for (const b of [1, 2]) continue out; }} closed"
            ),
            "3",
        ),
    ]);
}

#[test]
fn a_break_of_an_outer_loop_closes_every_iterator_innermost_first() {
    table(&[(
        "let p = [].values().__proto__; let old = p.next; let ids = 0; let order = ''; \
         p.next = function () { if (this.id === undefined) this.id = ++ids; return old.call(this); }; \
         p.return = function () { order += this.id; return {}; }; \
         out: for (const a of [1]) { for (const b of [1]) { for (const c of [1]) break out; } } order",
        "\"321\"",
    )]);
}

#[test]
fn a_finally_inside_the_body_runs_before_the_iterator_is_closed() {
    table(&[
        (
            "let log = ''; let p = [].values().__proto__; \
             p.return = function () { log += 'c'; return {}; }; \
             for (const x of [1]) { try { break; } finally { log += 'f'; } } log",
            "\"fc\"",
        ),
        // And a `finally` around the loop runs after it.
        (
            "let log = ''; let p = [].values().__proto__; \
             p.return = function () { log += 'c'; return {}; }; \
             function f() { try { for (const x of [1]) return 'r'; } finally { log += 'f'; } } \
             f() + log",
            "\"rcf\"",
        ),
    ]);
}

#[test]
fn a_throw_from_the_body_closes_the_iterator_and_throws_on() {
    table(&[
        (
            &format!(
                "{COUNTED} try {{ for (const x of [1, 2]) throw 'body'; }} catch (e) {{ e + closed }}"
            ),
            "\"body1\"",
        ),
        // What the closing throws is ignored: the body's throw wins.
        (
            "let p = [].values().__proto__; let closed = 0; \
             p.return = function () { closed++; throw 'from return'; }; \
             try { for (const x of [1]) throw 'body'; } catch (e) { e + closed }",
            "\"body1\"",
        ),
        // So is an answer that is not an object.
        (
            "let p = [].values().__proto__; p.return = function () { return 1; }; \
             try { for (const x of [1]) throw 'body'; } catch (e) { e }",
            "\"body\"",
        ),
        // So is a `return` that is not a function, or that is a getter that
        // throws.
        (
            "let p = [].values().__proto__; p.return = 1; \
             try { for (const x of [1]) throw 'body'; } catch (e) { e }",
            "\"body\"",
        ),
        (
            "let p = [].values().__proto__; p.__proto__ = { get return() { throw 'getter'; } }; \
             try { for (const x of [1]) throw 'body'; } catch (e) { e }",
            "\"body\"",
        ),
        // An error this engine throws in the body is closed over too.
        (
            &format!(
                "{COUNTED} try {{ for (const x of [1]) null.a; }} catch (e) {{ (e.constructor === TypeError) + ':' + closed }}"
            ),
            "\"true:1\"",
        ),
        // The blocks the throw was in are left: a closure made afterwards sees
        // the outer binding.
        (
            "let x = 'outer'; try { for (let x of [1]) { let y = 2; throw 0; } } catch { } x",
            "\"outer\"",
        ),
    ]);
}

#[test]
fn a_closing_that_fails_after_a_break_is_not_ignored() {
    table(&[
        (
            "let p = [].values().__proto__; p.return = function () { throw 'from return'; }; \
             try { for (const x of [1]) break; 'no' } catch (e) { e }",
            "\"from return\"",
        ),
        (
            "let p = [].values().__proto__; p.return = function () { return 1; }; \
             try { for (const x of [1]) break; 'no' } catch (e) { e.constructor === TypeError }",
            "true",
        ),
        (
            "let p = [].values().__proto__; p.return = 1; \
             try { for (const x of [1]) break; 'no' } catch (e) { e.constructor === TypeError }",
            "true",
        ),
        // A `return` of `null` is no method at all, which is not a failure.
        (
            "let p = [].values().__proto__; p.return = null; for (const x of [1]) break; 'yes'",
            "\"yes\"",
        ),
    ]);
}

#[test]
fn a_throw_from_the_iterator_itself_closes_nothing() {
    table(&[
        (
            &format!(
                "{COUNTED} p.next = function () {{ throw 'next'; }}; \
                 try {{ for (const x of [1]) {{}} }} catch (e) {{ e + closed }}"
            ),
            "\"next0\"",
        ),
        (
            &format!(
                "{COUNTED} p.next = function () {{ return {{ get done() {{ throw 'done'; }} }}; }}; \
                 try {{ for (const x of [1]) {{}} }} catch (e) {{ e + closed }}"
            ),
            "\"done0\"",
        ),
        (
            &format!(
                "{COUNTED} p.next = function () {{ return {{ done: false, get value() {{ throw 'value'; }} }}; }}; \
                 try {{ for (const x of [1]) {{}} }} catch (e) {{ e + closed }}"
            ),
            "\"value0\"",
        ),
    ]);
}

// --- The protocol is the one a page can intercept ---------------------------

#[test]
fn next_is_read_once_and_called_every_pass_with_the_iterator_as_this() {
    table(&[
        (
            "let p = [].values().__proto__; let old = p.next; let calls = 0; \
             p.next = function () { calls++; return old.call(this); }; \
             for (const x of [1, 2]) { p.next = null; } calls",
            "3",
        ),
        // `done` is read every pass, and `value` only when not done.
        (
            "let p = [].values().__proto__; let old = p.next; let reads = ''; \
             p.next = function () { const r = old.call(this); \
               return { get done() { reads += 'd'; return r.done; }, \
                        get value() { reads += 'v'; return r.value; } }; }; \
             for (const x of [1, 2]) {} reads",
            "\"dvdvd\"",
        ),
        // `done` is converted to a boolean.
        (
            "let p = [].values().__proto__; let n = 0; \
             p.next = function () { n++; return { done: n > 2 ? 'yes' : 0, value: n }; }; \
             let s = 0; for (const x of [9]) s += x; s",
            "3",
        ),
    ]);
}

#[test]
fn what_the_protocol_answers_wrongly_is_the_type_error_the_language_gives() {
    for source in [
        // Not iterable at all.
        "for (const x of {}) {}",
        "for (const x of undefined) {}",
        "for (const x of null) {}",
        // `next` answers something that is not an object.
        "let p = [].values().__proto__; p.next = function () { return 1; }; for (const x of [1]) {}",
        // `next` is not a function.
        "let p = [].values().__proto__; p.next = 1; for (const x of [1]) {}",
        // `next` is called on something that is not an array iterator.
        "[].values().next.call({})",
        // The generic methods on nothing.
        "[].values.call(null)",
    ] {
        let wrapped =
            format!("try {{ {source}; 'no' }} catch (e) {{ e.constructor === TypeError }}");
        assert_eq!(value(&wrapped), "true", "{source}");
    }
}

// --- What is not built is refused by name -----------------------------------

#[test]
fn what_is_not_built_says_which_item_builds_it() {
    for (source, item) in [
        // An element behind a getter would make the generator's states
        // observable, which this iterator does not keep.
        (
            "let o = { length: 1, get 0() { return 1; } }; for (const x of [].values.call(o)) {}",
            "queue item 231",
        ),
        (
            "[].__proto__.__proto__ = { get 0() { return 9; } }; for (const x of [,]) {}",
            "queue item 231",
        ),
        (
            "for (const x of [].values.call({ get length() { return 1; } })) {}",
            "queue item 231",
        ),
        (
            "for (const x of [].values.call({ length: { valueOf() { return 1; } } })) {}",
            "queue item 231",
        ),
        // A string is iterable through a wrapper this engine has not built.
        ("for (const c of 'ab') {}", "queue item 73"),
        ("[].values.call('ab')", "queue item 73"),
    ] {
        let answer = value(source);
        assert!(answer.starts_with('!'), "{source}: {answer}");
        assert!(answer.contains(item), "{source}: {answer}");
    }
    // A getter with nothing to call is no call: it reads as `undefined`.
    assert_eq!(
        value("let s = ''; for (const x of [].values.call({ length: 1, set 0(v) {} })) s += x; s"),
        "\"undefined\""
    );

    for (source, what) in [
        ("for (const [a, b] of c) {}", What::TakingAValueApart),
        ("for (const { a } of c) {}", What::TakingAValueApart),
        ("for ([a] of c) {}", What::TakingAValueApart),
        ("for (a in b) {}", What::TakingAValueApart),
        (
            "async function f() { for await (const a of b) {} }",
            What::ASuspension,
        ),
    ] {
        let Ok(program) = script(source) else {
            panic!("{source} parses");
        };
        match alo_js::compile(&program) {
            Err(Refusal::NotBuiltYet { what: named, .. }) => {
                assert_eq!(named, what, "{source}");
            }
            other => panic!("{source} should name an item: {other:?}"),
        }
    }
}

// --- What an embedder can make iterable, and tag --------------------------

/// Evaluate a program in an engine that already exists, answering its value
/// or why there was none.
fn evaluated(engine: &mut Engine, source: &str) -> Result<Value, String> {
    let program = script(source).map_err(|why| format!("{source} did not parse: {why}"))?;
    engine
        .evaluate(&program)
        .map_err(|trouble| format!("{source}: {trouble}"))
}

/// Put an object called `o` on the global object, with `property` under a
/// well-known symbol — what an embedder's own object would carry, and what no
/// script can write until the `Symbol` function exists.
///
/// The property's value is whatever the last run answered, which the engine
/// holds until the next run; by then the object owns it.
fn an_object_carrying(
    engine: &mut Engine,
    which: WellKnown,
    property: Property,
) -> Result<(), String> {
    let symbol = engine.well_known(which).map_err(|why| why.to_string())?;
    let key = engine
        .objects()
        .symbol_key(symbol)
        .map_err(|why| why.to_string())?;
    let global = engine.global().map_err(|why| why.to_string())?;
    let units: Vec<u16> = "o".encode_utf16().collect();
    let name = engine
        .objects()
        .key(&units)
        .map_err(|why| why.to_string())?;
    let held = engine
        .objects()
        .object(None)
        .map_err(|why| why.to_string())?;
    let named = engine
        .objects()
        .define(global, name, Property::plain(Value::Object(held)));
    let carried = engine.objects().define(held, key, property);
    match (named, carried) {
        (Ok(true), Ok(true)) => Ok(()),
        other => Err(format!("the definitions were refused: {other:?}")),
    }
}

#[test]
fn an_embedders_object_is_iterable_by_the_same_method_an_arrays_is() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let values = evaluated(&mut engine, "[].values").unwrap_or_else(|why| panic!("{why}"));
    an_object_carrying(
        &mut engine,
        WellKnown::Iterator,
        Property::data(values, true, false, true),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let answer = evaluated(
        &mut engine,
        "o.length = 2; o[0] = 'a'; o[1] = 'b'; let s = ''; for (const x of o) s += x; s",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(show(&mut engine, answer), "\"ab\"");
}

#[test]
fn a_symbol_iterator_that_is_not_a_function_or_answers_no_iterator_is_a_type_error() {
    // Not callable: `GetMethod`'s own `TypeError`, before any call.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    an_object_carrying(
        &mut engine,
        WellKnown::Iterator,
        Property::plain(Value::Number(1.0)),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let answer = evaluated(
        &mut engine,
        "try { for (const x of o) {} 'no' } catch (e) { (e.constructor === TypeError) + ': ' + e.message }",
    )
        .unwrap_or_else(|why| panic!("{why}"));
    let answer = show(&mut engine, answer);
    assert!(
        answer.starts_with("\"true: ") && answer.contains("not a function"),
        "{answer}"
    );

    // Callable, and answering `undefined`: there is no iterator to read.
    // `Function.prototype` is a function that answers `undefined`.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let nothing =
        evaluated(&mut engine, "(function () {}).__proto__").unwrap_or_else(|why| panic!("{why}"));
    an_object_carrying(&mut engine, WellKnown::Iterator, Property::plain(nothing))
        .unwrap_or_else(|why| panic!("{why}"));
    let answer = evaluated(
        &mut engine,
        "try { for (const x of o) {} 'no' } catch (e) { (e.constructor === TypeError) + ': ' + e.message }",
    )
        .unwrap_or_else(|why| panic!("{why}"));
    let answer = show(&mut engine, answer);
    assert!(
        answer.starts_with("\"true: ") && answer.contains("not an object"),
        "{answer}"
    );
}

#[test]
fn a_string_tag_names_an_object_and_anything_else_is_ignored() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let tag = evaluated(&mut engine, "'Tagged'").unwrap_or_else(|why| panic!("{why}"));
    an_object_carrying(&mut engine, WellKnown::ToStringTag, Property::plain(tag))
        .unwrap_or_else(|why| panic!("{why}"));
    let answer =
        evaluated(&mut engine, "({}).toString.call(o)").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(show(&mut engine, answer), "\"[object Tagged]\"");

    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    an_object_carrying(
        &mut engine,
        WellKnown::ToStringTag,
        Property::plain(Value::Number(1.0)),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let answer =
        evaluated(&mut engine, "({}).toString.call(o)").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(show(&mut engine, answer), "\"[object Object]\"");
}

#[test]
fn a_string_tag_behind_a_getter_is_read_by_calling_it_once() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    evaluated(&mut engine, "var n = 0;").unwrap_or_else(|why| panic!("{why}"));
    let getter = evaluated(&mut engine, "(function () { n++; return 'Got'; })")
        .unwrap_or_else(|why| panic!("{why}"));
    an_object_carrying(
        &mut engine,
        WellKnown::ToStringTag,
        Property::accessor(getter, Value::Undefined, false, true),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let answer =
        evaluated(&mut engine, "({}).toString.call(o) + n").unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(show(&mut engine, answer), "\"[object Got]1\"");

    // And a getter that throws throws out of `toString`.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let getter = evaluated(&mut engine, "(function () { throw 'boom'; })")
        .unwrap_or_else(|why| panic!("{why}"));
    an_object_carrying(
        &mut engine,
        WellKnown::ToStringTag,
        Property::accessor(getter, Value::Undefined, false, true),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let answer = evaluated(&mut engine, "try { ({}).toString.call(o) } catch (e) { e }")
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(show(&mut engine, answer), "\"boom\"");
}

// --- Bounds ------------------------------------------------------------------

#[test]
fn a_for_of_that_never_finishes_can_be_stopped() {
    let Ok(program) = script(
        "let p = [].values().__proto__; p.next = function () { return { done: false, value: 1 }; }; \
         for (const x of [1]) {}",
    ) else {
        panic!("that parses");
    };
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let stop = engine.stop();
    let asking = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        stop.ask();
    });
    match engine.evaluate(&program) {
        Err(Trouble::Escaped(Escape::Interrupted)) => {}
        other => panic!("an endless iterator is stopped: {other:?}"),
    }
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };
}

#[test]
fn passes_leave_nothing_behind_that_a_collection_cannot_take() {
    // Every pass makes a result object, an environment and — for `entries` —
    // an array; a loop of ten thousand passes, closed early or not, must leave
    // the heap as it found it.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let mut live = Vec::new();
    for _ in 0..3 {
        let Ok(program) = script(
            "(function () { let a = []; for (let i = 0; i < 10000; i++) a[i] = i; \
             let n = 0; for (const e of a.entries()) { n += e[1]; } \
             for (let x of a) { if (x == 5000) break; } })();",
        ) else {
            panic!("that parses");
        };
        let outcome = engine.evaluate(&program);
        assert!(outcome.is_ok(), "{outcome:?}");
        engine.objects().heap_mut().collect();
        live.push(engine.objects().heap().live());
    }
    assert_eq!(
        live.first(),
        live.last(),
        "live cells after each of three runs: {live:?}"
    );
}

// --- Hostile input -----------------------------------------------------------

#[test]
fn every_cut_of_a_for_of_program_is_a_result_rather_than_a_crash() {
    let source = "let p = [].values().__proto__; let c = 0; p.return = function () { c++; return {}; }; \
                  let s = ''; function f(a) { for (const x of a) { if (x == 3) return x; s += x; } } \
                  out: for (let x of [[1, 2], [3]].values()) { for (var y of x.entries()) { \
                  s += y[0]; if (y[1] == 2) continue out; } s += f([1, 3]); } \
                  for (o of [1]) try { throw 0; } catch { s += '!'; } s + c";
    // `s += f(…)` reads `s` before `f` writes it, so what `f` appended is
    // lost; it still closed its iterator on the way out with `return`.
    assert_eq!(value(source), "\"0103!2\"");
    for end in 0..=source.len() {
        let Some(cut) = source.get(..end) else {
            continue;
        };
        let _ = value(cut);
    }
}

#[test]
fn a_tower_of_for_ofs_is_refused_by_the_parser_or_run_and_never_more() {
    let deep = format!("{}1{}", "for (const x of [1]) ".repeat(20_000), "");
    assert!(
        value(&deep).starts_with("did not parse"),
        "twenty thousand nested loops are refused by the parser's bound"
    );
    // Within it, a break of the outermost closes every one.
    let nested = |depth: usize| {
        let mut source = format!("{COUNTED} out: ");
        source.push_str(&"for (const x of [1, 2]) ".repeat(depth));
        source.push_str("break out; closed");
        source
    };
    assert_eq!(value(&nested(200)), "200");
    // And many in a row, each taking slots of its own.
    let many = format!(
        "let n = 0; {} n",
        "for (const x of [1, 2]) n += x; ".repeat(2_000)
    );
    assert_eq!(value(&many), "6000");
}

// --- The page that opened it ------------------------------------------------

/// The frozen service worker, or nothing if it is not where it should be.
fn service_worker() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/scripts/alo-service-worker/script.js");
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_frozen_service_worker_compiles_past_the_for_of_that_stopped_it() {
    let source = service_worker();
    assert!(!source.is_empty(), "the frozen script is in the corpus");
    assert_eq!(
        source.get(2922..2925),
        Some("for"),
        "the frozen bytes are the ones the item was opened against"
    );
    let Ok(program) = script(&source) else {
        panic!("the frozen script parses");
    };
    // It was the last thing the script was refused at: all of it compiles.
    // Running it is another matter — it needs `self`, which is an embedder's
    // (a worker's global, item 91), and `Object.values`, `concat` and
    // promises (items 73 and 75).
    if let Err(refusal) = alo_js::compile(&program) {
        panic!("the whole frozen service worker compiles: {refusal}");
    }
}
