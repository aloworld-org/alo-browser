/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 219's closing conditions, in the table ADR 0013 § 9 asks for.
//!
//! *`f.call(o, 1)` runs `f` with `o` as its `this`; a builtin that asked for a
//! call and was re-entered gets the answer in the place it left; and a builtin
//! that calls itself for ever is the `RangeError` a runaway function is rather
//! than a process that stops.*
//!
//! # The runaway is the test that says the design is right
//!
//! [`a_builtin_that_calls_itself_for_ever_is_a_range_error`] would not fail if
//! the mechanism were wrong — it would **abort the test process**. A builtin
//! that ran the next builtin's body inside its own Rust frame overflows this
//! process's stack somewhere around here, and there is no catching that. So it
//! is not a bound being checked so much as the whole shape being checked: a
//! builtin asks, and the interpreter's loop does the running.
//!
//! # Every program runs twice, and here that is not a formality
//!
//! Once ordinarily and once with [`Heap::stress`] on — collecting at every
//! allocation. A builtin that suspends is the one place in this engine where a
//! value has to survive *between* two runs of the same Rust function, and the
//! argument that it does is that the interpreter writes it into a stack slot
//! rather than keeping it. That argument is checked here rather than believed.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

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

/// One run.
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
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
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

#[test]
fn call_runs_a_function_with_the_this_it_was_given() {
    table(&[
        (
            "let o = { n: 3 }; function f(a) { return this.n + a; } f.call(o, 1)",
            "4",
        ),
        // Every argument after the first goes to the callee, in order.
        (
            "function f(a, b, c) { return a + b + c; } f.call(null, 1, 2, 4)",
            "7",
        ),
        // And a callee that wanted more than were passed gets `undefined`,
        // exactly as it would from an ordinary call.
        (
            "function f(a, b) { return b; } typeof f.call(null, 1)",
            "\"undefined\"",
        ),
        // The receiver is the callee's to interpret: sloppy code turns
        // `undefined` into the global object and strict code does not, and
        // `call` does not decide either one.
        (
            "function f() { return this === undefined; } f.call(undefined)",
            "false",
        ),
        (
            "function f() { 'use strict'; return this === undefined; } f.call(undefined)",
            "true",
        ),
        // An arrow has no `this` of its own, so `call` cannot give it one.
        (
            "let o = { n: 1 }; let a = () => this; a.call(o) === o",
            "false",
        ),
        // The answer is the callee's, and a callee that returns nothing
        // answers `undefined` rather than the receiver.
        ("function f() {} typeof f.call({}, 1, 2)", "\"undefined\""),
    ]);
}

#[test]
fn a_builtin_reached_through_call_is_a_builtin_calling_a_builtin() {
    table(&[
        // `Object.prototype.toString` is the one method whose answer depends
        // only on its `this`, so it is the sharpest thing to hand a receiver to.
        ("({}).toString.call(null)", "\"[object Null]\""),
        ("({}).toString.call(undefined)", "\"[object Undefined]\""),
        (
            "let f = function () {}; ({}).toString.call(f)",
            "\"[object Function]\"",
        ),
        ("let o = { n: 1 }; ({}).hasOwnProperty.call(o, 'n')", "true"),
        (
            "let o = { n: 1 }; ({}).hasOwnProperty.call(o, 'm')",
            "false",
        ),
        // `call` calling `call` calling a function: two builtins suspended at
        // once, which is the case a single-slot design would have got wrong.
        (
            "function f(a) { return this.n + a; } let o = { n: 5 }; f.call.call(f, o, 2)",
            "7",
        ),
    ]);
}

#[test]
fn a_builtin_that_asked_for_a_conversion_carries_on_where_it_left_off() {
    table(&[
        // `ToPropertyKey({})` is `"[object Object]"`, and the answer of that
        // conversion has to arrive back inside `hasOwnProperty` for the lookup
        // that follows it to be the right lookup.
        ("({}).hasOwnProperty({})", "false"),
        (
            "let o = {}; o['[object Object]'] = 1; o.hasOwnProperty({})",
            "true",
        ),
        // The script's own `valueOf` is what runs, and `Hint::String` is why
        // `toString` is asked for first.
        (
            "let k = { toString: function () { return 'n'; } }; ({ n: 1 }).hasOwnProperty(k)",
            "true",
        ),
        (
            "let k = { valueOf: function () { return 'n'; } }; ({ n: 1 }).hasOwnProperty(k)",
            "false",
        ),
        (
            "let k = { toString: function () { return 'n'; } }; ({ n: 1 }).propertyIsEnumerable(k)",
            "true",
        ),
        // A key that converts to a number reaches the same property an index
        // does, because `ToPropertyKey` is one function however it was spelled.
        (
            "let o = {}; o[7] = 'x'; o.hasOwnProperty({ toString: function () { return 7; } })",
            "true",
        ),
        // A conversion that throws throws out of the builtin, rather than the
        // builtin carrying on with something it never got.
        (
            "({}).hasOwnProperty({ toString: 1, valueOf: 1 })",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 0)",
        ),
    ]);
}

#[test]
fn to_locale_string_invokes_the_to_string_the_object_actually_has() {
    table(&[
        ("({}).toLocaleString()", "\"[object Object]\""),
        (
            "let o = { toString: function () { return 'x'; } }; o.toLocaleString()",
            "\"x\"",
        ),
        // Finding the method is itself a call when it is behind an accessor,
        // which is why this builtin has three steps rather than two.
        (
            "let o = { get toString() { return function () { return 'g'; }; } }; o.toLocaleString()",
            "\"g\"",
        ),
        // Inherited, because `Invoke` is an ordinary property read.
        (
            "let a = { toString: function () { return 'up'; } }; let b = {}; b.__proto__ = a; b.toLocaleString()",
            "\"up\"",
        ),
        // Not a function is the `TypeError` `Invoke` gives, and a `toString`
        // that is missing altogether is the same answer for the same reason.
        (
            "let o = { toString: 1 }; o.toLocaleString()",
            "! TypeError: toLocaleString needs a toString to call, and this object's is not a function (at byte 25)",
        ),
        (
            "let o = {}; o.__proto__ = null; o.toLocaleString",
            "undefined",
        ),
        // It is written on the value rather than on an object made from it, so
        // `undefined` and `null` are the `TypeError` `ToObject` gives.
        (
            "({}).toLocaleString.call(null)",
            "! TypeError: Object.prototype.toLocaleString was called on undefined or null (at byte 0)",
        ),
    ]);
}

#[test]
fn a_builtin_that_calls_itself_for_ever_is_a_range_error() {
    // A `toLocaleString` whose object's `toString` **is** `toLocaleString`
    // invokes itself with the same receiver for ever, and there is not one
    // frame anywhere in it: every call is a builtin waiting on a builtin. So
    // this is the case that says a waiting builtin is counted as a call on the
    // stack. Counting only frames would leave it to run until the *value* bound
    // noticed, and running the bodies inside one another rather than in the
    // loop would end this test process rather than this script.
    assert_eq!(
        value("let o = {}; o.toString = ({}).toLocaleString; o.toLocaleString()"),
        "! RangeError: this script calls more deeply than this engine will go (at byte 46)"
    );
    // The same runaway with the two kinds of call interleaved, so that neither
    // bound is reached by one kind alone.
    assert_eq!(
        value("function f() { return f.call(this); } f()"),
        "! RangeError: this script calls more deeply than this engine will go (at byte 22)"
    );
}

#[test]
fn a_hostile_program_that_leans_on_a_builtin_refuses_rather_than_breaking() {
    table(&[
        // A `toString` that calls the thing that is calling it: each turn is a
        // frame, so the bound is reached and reported rather than the process
        // running out of stack.
        (
            "let o = {}; o.toString = function () { return o.toLocaleString(); }; o.toLocaleString()",
            "! RangeError: this script calls more deeply than this engine will go (at byte 46)",
        ),
        // `c.call(c)` does **not** run away, and it is worth writing down why:
        // each hop takes the receiver from the arguments, so the argument list
        // shortens by one every time and the third hop is `call` with no `this`
        // to call. A runaway builtin has to hold its own state still, which is
        // what the test above does and this cannot.
        (
            "let c = ({}).toString.call; c.call(c)",
            "! TypeError: undefined is not a function (at byte 28)",
        ),
        // A `toString` that hands back an object is not a conversion, so the
        // search moves to `valueOf` and then gives up — it does not ask the
        // same name again.
        (
            "let k = { toString: function () { return {}; }, valueOf: function () { return {}; } }; ({}).hasOwnProperty(k)",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 87)",
        ),
        // Calling `call` on something that is not a function is the `TypeError`
        // every other call on a non-function is, said in the same words.
        (
            "let c = ({}).toString.call; c.call(undefined)",
            "! TypeError: undefined is not a function (at byte 28)",
        ),
        (
            "let c = ({}).toString.call; c.call(4)",
            "! TypeError: 4 is not a function (at byte 28)",
        ),
    ]);
}

#[test]
fn a_throw_through_suspended_builtins_leaves_the_engine_reusable() {
    for stress in [false, true] {
        let mut engine = Engine::new().expect("a fresh engine");
        engine.objects().heap_mut().stress(stress);
        let throwing = script(
            "({ toString: function () { throw 7; } }).toLocaleString.call({ toString: function () { throw 7; } })",
        )
        .expect("the throwing script parses");
        assert!(matches!(
            engine.evaluate(&throwing),
            Err(Trouble::Escaped(_))
        ));
        engine.objects().heap_mut().collect();
        assert_eq!(engine.objects().heap_mut().check(), Ok(()));
        let next = script("(function (n) { return n + 1; }).call(null, 41)")
            .expect("the next script parses");
        assert!(matches!(engine.evaluate(&next), Ok(Value::Number(42.0))));
    }
}
