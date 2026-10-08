/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 331's closing conditions for `Array.prototype.forEach`, the
//! first builtin that keeps state in slots (ADR 0031).
//!
//! *It answers a table of arrays and array-likes as the specification does —
//! holes skipped, `thisArg` passed, the length read once, a throwing
//! callback ending it — with the collector at every allocation; and a
//! `{ length: 2 ** 53 - 1 }` with no elements is ended by the embedder's
//! stop rather than run to its end.*
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with [`Heap::stress`] on. `forEach` keeps `len`
//! and `k` in slots across every call it asks for, and the elements it
//! hands the callback are objects the callback allocates around; a slot or
//! an argument the collector could not see would answer differently under
//! stress, or fail [`Heap::check`] afterwards.
//!
//! [`Heap::stress`]: alo_js::Heap::stress
//! [`Heap::check`]: alo_js::Heap::check

use std::thread;
use std::time::Duration;

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{Escape, numeric, script};

/// Run a program in a fresh engine, both ways, and answer what it produced.
fn value(source: &str) -> String {
    let ordinary = run(source, false);
    let stressed = run(source, true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// One run, with the heap checked after it.
fn run(source: &str, stress: bool) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    let mut engine = match Engine::new() {
        Ok(engine) => engine,
        Err(why) => return format!("no engine: {why}"),
    };
    engine.objects().heap_mut().stress(stress);
    let answer = match engine.evaluate(&program) {
        Ok(value) => show(&mut engine, value),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    };
    engine.objects().heap_mut().stress(false);
    assert_eq!(
        engine.objects().heap().check(),
        Ok(()),
        "{source}: the heap is sound afterwards"
    );
    answer
}

/// A value, written out so that a failing test says what it got.
fn show(engine: &mut Engine, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match engine.objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "?".to_owned(),
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

#[test]
fn it_visits_each_element_with_its_index_and_the_object() {
    table(&[
        (
            "var s = ''; var a = [1, 2, 3]; \
             a.forEach(function (x, i, o) { s += x + ':' + i + (o === a) + ' '; }); s",
            "1:0true 2:1true 3:2true ",
        ),
        // It answers `undefined`, whatever the callback returned.
        ("typeof [1].forEach(function () { return 4; })", "undefined"),
        // An empty array calls nothing, and asks for no callback first.
        ("var n = 0; [].forEach(function () { n++; }); n", "0"),
        // Each element is a value the callback can keep and allocate around.
        (
            "var kept = []; [{ n: 1 }, { n: 2 }, { n: 3 }].forEach(function (o, i) { \
               kept[i] = { o: o, pad: [i, i, i] }; }); \
             kept[0].o.n + kept[1].o.n + kept[2].o.n",
            "6",
        ),
        // It is one function, generic, on every array.
        (
            "[].forEach === [1].forEach && [].forEach === [].__proto__.forEach",
            "true",
        ),
        ("typeof [].forEach", "function"),
    ]);
}

#[test]
fn holes_are_skipped_and_an_inherited_index_is_not() {
    table(&[
        (
            "var s = ''; [1, , 3].forEach(function (x) { s += x; }); s",
            "13",
        ),
        (
            "var a = []; a[3] = 'd'; var s = ''; \
             a.forEach(function (x, i) { s += i + x; }); s",
            "3d",
        ),
        // `HasProperty` follows the chain: an index the prototype has is
        // visited.
        (
            "var o = { length: 3, 0: 'a' }; o.__proto__ = { 1: 'p' }; var s = ''; \
             [].forEach.call(o, function (x, i) { s += i + x; }); s",
            "0a1p",
        ),
        // An element deleted before it is reached is a hole by then.
        (
            "var a = [1, 2, 3]; var s = ''; \
             a.forEach(function (x, i) { if (i === 0) { delete a[1]; } s += x; }); s",
            "13",
        ),
    ]);
}

#[test]
fn this_arg_is_the_callbacks_this() {
    table(&[
        (
            "var o = { n: 5 }; var r = 0; \
             [1, 2].forEach(function (x) { r += this.n * x; }, o); r",
            "15",
        ),
        // Without one, a strict callback's `this` is `undefined`.
        (
            "var r; [1].forEach(function () { 'use strict'; r = this; }); r === undefined",
            "true",
        ),
    ]);
}

#[test]
fn the_length_is_read_once() {
    table(&[
        // What the callback appends is not visited.
        (
            "var a = [1, 2]; var n = 0; \
             a.forEach(function (x) { n++; a[a.length] = x; }); n + ' ' + a.length",
            "2 4",
        ),
        // A `length` getter runs once, with the object as its `this`.
        (
            "var count = 0; var o = { get length() { count++; return this.n; }, n: 2, 0: 'a', 1: 'b' }; \
             var s = ''; [].forEach.call(o, function (x) { s += x; }); s + count",
            "ab1",
        ),
        // An object as the length is converted, its `valueOf` run.
        (
            "var o = { length: { valueOf: function () { return 2; } }, 0: 'a', 1: 'b', 2: 'c' }; \
             var s = ''; [].forEach.call(o, function (x) { s += x; }); s",
            "ab",
        ),
        // `ToLength`: a string is a number, and NaN, negatives and nothing are
        // zero.
        (
            "var s = ''; var f = function (x) { s += x; }; \
             [].forEach.call({ length: '2', 0: 'a', 1: 'b' }, f); \
             [].forEach.call({ length: -1, 0: 'x' }, f); \
             [].forEach.call({ length: 'many', 0: 'x' }, f); \
             [].forEach.call({ 0: 'x' }, f); \
             [].forEach.call({ length: 1.9, 0: 'c' }, f); s",
            "abc",
        ),
        // A function is an object with no `length` of its own to read yet
        // (item 220), so it is walked as empty.
        (
            "var n = 0; [].forEach.call(function () {}, function () { n++; }); n",
            "0",
        ),
    ]);
}

#[test]
fn an_element_behind_a_getter_is_read_by_calling_it() {
    table(&[
        (
            "var o = { length: 2, get 0() { return this.x + 1; }, x: 4, 1: 'b' }; \
             var s = ''; [].forEach.call(o, function (v) { s += v; }); s",
            "5b",
        ),
        // A getter that allocates hands over what it made, under stress too.
        (
            "var o = { length: 1, get 0() { return { made: [1, 2, 3] }; } }; var r; \
             [].forEach.call(o, function (v) { var pad = { a: {} }; r = v.made[2]; }); r",
            "3",
        ),
    ]);
}

#[test]
fn a_throw_ends_the_walk_where_it_was() {
    table(&[
        (
            "var n = 0; var s; \
             try { [1, 2, 3].forEach(function (x) { n++; if (x === 2) { throw 'out'; } }); } \
             catch (e) { s = e; } n + s",
            "2out",
        ),
        // From a `length` getter, before any callback.
        (
            "var n = 0; var o = { get length() { throw 'len'; } }; var s; \
             try { [].forEach.call(o, function () { n++; }); } catch (e) { s = e; } \
             s + n",
            "len0",
        ),
        // From an element's getter, after the elements before it.
        (
            "var s = ''; var o = { length: 3, 0: 'a', get 1() { throw '!'; }, 2: 'c' }; \
             try { [].forEach.call(o, function (x) { s += x; }); } catch (e) { s += e; } s",
            "a!",
        ),
        // And the stack is as it was: the next statement runs as usual.
        (
            "try { [1].forEach(function () { throw 1; }); } catch (e) {} \
             var t = 0; [1, 2].forEach(function (x) { t += x; }); t",
            "3",
        ),
    ]);
}

#[test]
fn what_it_refuses_it_refuses_as_the_specification_says() {
    table(&[
        ("try { [1].forEach(3); } catch (e) { e.name }", "TypeError"),
        ("try { [1].forEach(); } catch (e) { e.name }", "TypeError"),
        // The length is read before the callback is checked.
        (
            "var read = 0; var o = { get length() { read++; return 1; } }; \
             try { [].forEach.call(o, {}); } catch (e) { read + e.name }",
            "1TypeError",
        ),
        (
            "try { [].forEach.call(null, function () {}); } \
             catch (e) { e.name }",
            "TypeError",
        ),
        (
            "try { [].forEach.call(undefined, function () {}); } \
             catch (e) { e.name }",
            "TypeError",
        ),
    ]);
    // A string `this` needs its wrapper object, which is item 73's.
    let refused = value("[].forEach.call('ab', function () {})");
    assert!(
        refused.contains("queue item 73"),
        "a primitive `this` is refused by name: {refused}"
    );
}

#[test]
fn a_callback_that_calls_for_each_again_nests() {
    table(&[(
        "var s = ''; [1, 2].forEach(function (x) { \
           [10, 20].forEach(function (y) { s += (x * y) + ' '; }); }); s",
        "10 20 20 40 ",
    )]);
}

#[test]
fn a_walk_over_nothing_but_holes_is_stopped_by_the_embedder() {
    // 2⁵³ − 1 holes: no call and no backward jump the interpreter sees, so
    // only `forEach`'s own question ends it (ADR 0031 § 7). Without it this
    // test would not finish.
    let Ok(program) =
        script("var n = 0; [].forEach.call({ length: 2 ** 53 - 1 }, function () { n++; }); n")
    else {
        panic!("it parses");
    };
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let stop = engine.stop();
    let asking = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        stop.ask();
    });
    let outcome = engine.evaluate(&program);
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };
    assert!(
        matches!(outcome, Err(Trouble::Escaped(Escape::Interrupted))),
        "{outcome:?}"
    );
    assert_eq!(engine.objects().heap().check(), Ok(()));
}
