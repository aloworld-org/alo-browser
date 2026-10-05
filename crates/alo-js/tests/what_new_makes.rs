/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 212, cut to `[[Construct]]`: *`new f()` makes an object whose
//! prototype is `f.prototype`* — and everything that sentence leans on. A
//! function written with `function` is given a `prototype` when it is made, with
//! a `constructor` pointing back; `new` makes an instance from it, runs the body
//! with the instance as `this`, and answers with the instance unless the body
//! returned an object of its own. Anything else in front of `new` is a
//! `TypeError`, checked only after the arguments have been evaluated.
//!
//! Classes, `super`, `new.target` and private names are queue item 223, and
//! `instanceof` — which reads what this writes — is item 224.
//!
//! # What opened it
//!
//! A real page's own script. `crates/alo-corpus/scripts/alo-service-worker/` is
//! alo's offline-shell service worker, frozen with its provenance beside it,
//! and it did not compile: the first thing it could not get past was
//! `new Request(OFFLINE_URL, …)`. The last test in this file is that script
//! compiling past that `new`, to the next thing this engine has not built.
//! The script's other `new`, `new Response(…)`, is beyond that next thing, so
//! it is the table rather than the script that shows one compiling.
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with the collector firing at every allocation, and
//! the two must agree. `[[Construct]]` allocates between reading the
//! constructor's `prototype` and entering the body, and `MakeConstructor`
//! allocates three times while a function exists only on the stack — exactly
//! the windows in which a reference held only by Rust would be collected.

use std::path::PathBuf;

use alo_js::compile::{Refusal, What};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
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
    run(&mut engine, source)
}

/// One run, in an engine that may already have run something.
fn run(engine: &mut Engine, source: &str) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
        Ok(value) => show(engine, value),
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

#[test]
fn new_makes_an_object_whose_prototype_is_the_constructors_prototype() {
    table(&[
        // The closing condition, in the item's own words.
        (
            "function F() {} var o = new F(); o.__proto__ === F.prototype",
            "true",
        ),
        // With no argument list at all, which is the same construction.
        (
            "function F() {} var o = new F; o.__proto__ === F.prototype",
            "true",
        ),
        ("function F() {} F.prototype.isPrototypeOf(new F())", "true"),
        // Two constructions are two objects sharing one prototype.
        ("function F() {} new F() !== new F()", "true"),
        (
            "function F() {} new F().__proto__ === new F().__proto__",
            "true",
        ),
        // A method put on the prototype is found through the instance.
        (
            "function P(x, y) { this.x = x; this.y = y; } \
             P.prototype.sum = function () { return this.x + this.y; }; \
             new P(3, 4).sum()",
            "7",
        ),
        // A function expression, named or not, constructs as well.
        (
            "var F = function G() { this.g = G; }; new F().g === F",
            "true",
        ),
        ("var F = function () { this.a = 1; }; new F().a", "1"),
    ]);
}

#[test]
fn the_body_runs_with_the_instance_as_its_this_and_gets_its_arguments() {
    table(&[
        (
            "function P(x, y) { this.x = x; this.y = y; } var p = new P(1, 2); p.x * 10 + p.y",
            "12",
        ),
        // A parameter nobody passed is `undefined`, as for a call.
        (
            "function P(x, y) { this.y = y; } typeof new P(1).y",
            "\"undefined\"",
        ),
        // In **sloppy** code too, `this` is the instance and not the global
        // object: the sloppy replacement applies to `undefined` and `null`, and
        // a construction never passes either.
        (
            "var seen; function F() { seen = this; } var o = new F(); \
             seen === o && seen !== globalThis",
            "true",
        ),
        (
            "'use strict'; var seen; function F() { seen = this; } var o = new F(); seen === o",
            "true",
        ),
        // An arrow made inside the body keeps the instance as its `this`.
        (
            "function Counter() { this.n = 0; this.inc = () => { this.n = this.n + 1; return this.n; }; } \
             var c = new Counter(); c.inc(); c.inc()",
            "2",
        ),
        // A constructor may construct, and the two `this`es stay apart.
        (
            "function A() { this.b = new B(); this.v = 1; } function B() { this.v = 7; } \
             var a = new A(); a.v * 10 + a.b.v",
            "17",
        ),
    ]);
}

#[test]
fn a_body_that_returns_an_object_answers_with_it_and_anything_else_is_ignored() {
    table(&[
        (
            "function F() { this.a = 1; return { b: 2 }; } var o = new F(); \
             o.b === 2 && o.a === undefined",
            "true",
        ),
        // A function is an object, so it is what `new` answers with.
        (
            "function G() {} function F() { return G; } new F() === G",
            "true",
        ),
        // A primitive, `null` and `undefined` are not, and the instance wins.
        ("function F() { this.a = 1; return 5; } new F().a", "1"),
        ("function F() { this.a = 1; return 'x'; } new F().a", "1"),
        ("function F() { this.a = 1; return null; } new F().a", "1"),
        ("function F() { this.a = 1; return; } new F().a", "1"),
        ("function F() { this.a = 1; } new F().a", "1"),
        // What `new` answered with is itself constructed from: `new F()` is G,
        // and `new` in front of that makes a G.
        (
            "function G() { this.z = 9; } function F() { return G; } new (new F())().z",
            "9",
        ),
    ]);
}

#[test]
fn a_constructor_has_a_prototype_with_the_attributes_the_specification_gives() {
    table(&[
        ("function F() {} typeof F.prototype", "\"object\""),
        ("function F() {} F.prototype.constructor === F", "true"),
        // The prototype object is an ordinary object inheriting from
        // `Object.prototype`.
        (
            "function F() {} F.prototype.__proto__ === ({}).__proto__",
            "true",
        ),
        // `prototype`: own, writable, not enumerable, not configurable.
        ("function F() {} F.hasOwnProperty('prototype')", "true"),
        (
            "function F() {} F.propertyIsEnumerable('prototype')",
            "false",
        ),
        ("function F() {} delete F.prototype", "false"),
        (
            "'use strict'; function F() {} delete F.prototype",
            "! TypeError: property 'prototype' cannot be deleted (at byte 30)",
        ),
        ("function F() {} F.prototype = { a: 1 }; new F().a", "1"),
        // `constructor`: own, writable, not enumerable, configurable.
        (
            "function F() {} F.prototype.propertyIsEnumerable('constructor')",
            "false",
        ),
        (
            "function F() {} delete F.prototype.constructor && \
             !F.prototype.hasOwnProperty('constructor')",
            "true",
        ),
        // Each function has a prototype of its own.
        (
            "function F() {} function G() {} F.prototype !== G.prototype",
            "true",
        ),
    ]);
}

#[test]
fn a_prototype_that_is_not_an_object_means_object_prototype() {
    table(&[
        (
            "function F() {} F.prototype = 3; new F().__proto__ === ({}).__proto__",
            "true",
        ),
        (
            "function F() {} F.prototype = null; new F().__proto__ === ({}).__proto__",
            "true",
        ),
    ]);
}

#[test]
fn what_is_not_a_constructor_is_a_type_error() {
    table(&[
        // Nothing callable at all.
        ("new 1", "! TypeError: 1 is not a constructor (at byte 0)"),
        (
            "new undefined",
            "! TypeError: undefined is not a constructor (at byte 0)",
        ),
        (
            "new ({})",
            "! TypeError: that value is not a constructor (at byte 0)",
        ),
        // Callable, and still not a constructor: an arrow, a method, a getter's
        // own function and a builtin.
        (
            "new (() => 1)",
            "! TypeError: that value is not a constructor (at byte 0)",
        ),
        (
            "new ({ m() {} }).m()",
            "! TypeError: that value is not a constructor (at byte 0)",
        ),
        (
            "new ({}).toString()",
            "! TypeError: that value is not a constructor (at byte 0)",
        ),
        (
            "new (function () {}).__proto__()",
            "! TypeError: that value is not a constructor (at byte 0)",
        ),
        // None of them has a `prototype`, which is the other half of the same
        // fact.
        ("typeof (() => 1).prototype", "\"undefined\""),
        ("typeof ({ m() {} }).m.prototype", "\"undefined\""),
        ("typeof ({}).toString.prototype", "\"undefined\""),
        // A method is still callable: refusing `new` is not refusing a call.
        ("({ m() { return 3; } }).m()", "3"),
    ]);
}

#[test]
fn the_constructor_is_checked_after_the_arguments_are_evaluated() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "var log = ''; function f() { log += 'f'; } new (log += 'c', 1)(f())",
        ),
        "! TypeError: 1 is not a constructor (at byte 43)"
    );
    // The callee first, then the arguments, then the refusal.
    assert_eq!(run(&mut engine, "log"), "\"cf\"");
}

#[test]
fn a_constructor_that_throws_throws_and_the_engine_carries_on() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "var made = 0; function F() { made = made + 1; null.a; } new F()"
        ),
        "! TypeError: cannot read property 'a' of null (at byte 46)"
    );
    assert_eq!(
        run(&mut engine, "function G() { this.a = made; } new G().a"),
        "1"
    );
}

#[test]
fn a_constructor_that_constructs_itself_for_ever_is_a_range_error() {
    // Run ordinarily only, as `an_engine_that_is_hostile.rs` runs its runaway
    // recursions: ten thousand frames each collecting the whole heap at every
    // allocation is quadratic, and the rooting it would exercise is exercised
    // under stress by the bounded nesting below.
    for (source, expected) in [
        (
            "function F() { return new F(); } new F()",
            "! RangeError: this script calls more deeply than this engine will go (at byte 22)",
        ),
        // Through a call in between, so the bound counts both kinds: it is
        // reached at the call `f()`, which is the frame that would have been
        // one too many.
        (
            "function f() { return new F(); } function F() { return f(); } new F()",
            "! RangeError: this script calls more deeply than this engine will go (at byte 55)",
        ),
    ] {
        assert_eq!(fresh(source, false), expected, "{source}");
    }
}

#[test]
fn constructions_nested_fifty_deep_each_keep_their_own_instance() {
    // Fifty `[[Construct]]` frames on the stack at once, each with an instance
    // in its `this` slot that only the stack holds — run both ways, so a slot
    // the collector did not walk would answer differently under stress.
    table(&[(
        "function F(n) { this.n = n; this.c = n > 0 ? new F(n - 1) : null; }          var d = 0; var sum = 0; var p = new F(50);          while (p) { d = d + 1; sum = sum + p.n; p = p.c; } d * 10000 + sum",
        "511275",
    )]);
}

#[test]
fn many_constructions_leave_nothing_behind_that_nobody_holds() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "function P(x) { this.x = x; } var kept = null; \
             for (let i = 0; i < 2000; i = i + 1) { kept = new P(i); } kept.x",
        ),
        "1999"
    );
    engine.objects().heap_mut().collect();
    assert!(
        engine.objects().heap().check().is_ok(),
        "the heap is well formed after a collection"
    );
    let after_one = engine.objects().heap().live();
    assert_eq!(
        run(
            &mut engine,
            "for (let i = 0; i < 2000; i = i + 1) { kept = new P(i); } kept.x"
        ),
        "1999"
    );
    engine.objects().heap_mut().collect();
    let after_two = engine.objects().heap().live();
    // The second loop made two thousand instances and kept one, in place of the
    // one the first loop kept — so the heap is the size it was, give or take
    // the second program's own constants.
    assert!(
        after_two <= after_one.saturating_add(16),
        "{after_one} cells after one loop and {after_two} after two"
    );
}

#[test]
fn every_cut_of_a_construction_is_a_result_rather_than_a_crash() {
    let source = "function P(x, y) { this.x = x; return y; } \
                  var o = new P(1, { a: 2 }); new (new P(3, P))(4).x + o.a";
    assert_eq!(value(source), "6");
    for end in 0..=source.len() {
        let Some(cut) = source.get(..end) else {
            continue;
        };
        // The assertion is that this returns at all, both ways.
        let _ = value(cut);
    }
    // And a tower of `new`s, which the parser bounds rather than recursing
    // until the process stops.
    let tower = format!("{}f", "new ".repeat(20_000));
    assert!(
        value(&tower).starts_with("did not parse"),
        "a tower of `new` is refused by the parser's bound"
    );
}

/// The frozen service worker, or nothing if it is not where it should be.
fn service_worker() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/scripts/alo-service-worker/script.js");
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_frozen_service_worker_compiles_past_the_new_that_stopped_it() {
    let source = service_worker();
    assert!(!source.is_empty(), "the frozen script is in the corpus");
    // The `new` that refused it before this item, at the byte the refusal named.
    assert_eq!(
        source.get(1438..1450),
        Some("new Request("),
        "the frozen bytes are the ones the item was opened against"
    );
    let Ok(program) = script(&source) else {
        panic!("the frozen script parses");
    };
    match alo_js::compile(&program) {
        Err(Refusal::NotBuiltYet { what, at }) => {
            assert_ne!(what, What::AClass, "no construction stops it now");
            assert!(at > 1438, "it gets past the `new` at 1438, to {at}");
            // What stopped it next was the array literal at 2847, which item
            // 225 built; `what_an_array_is.rs` says where it stops now.
            assert!(at > 2847, "and past the array literal at 2847, to {at}");
        }
        // Queue item 230 built the last thing it was refused at.
        Ok(_) => {}
        other => panic!("expected the next unbuilt item, got {other:?}"),
    }
}
