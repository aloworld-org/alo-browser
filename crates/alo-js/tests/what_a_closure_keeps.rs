/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 209's second closing condition: *a closure keeps its scope alive
//! after the frame it was made in has gone — **counted** rather than watched,
//! which is item 71's rule.*
//!
//! Two halves, and a test of only the first would pass on an engine that leaks
//! every environment it ever made.
//!
//! - **It survives.** The call that made the closure has returned, the
//!   collector has run, and the closure still reads what it closed over. A
//!   frame's slots would be gone by now, which is exactly why a function's
//!   bindings are a cell in the heap and not slots.
//! - **It is let go of.** Once nothing can reach the closure, the environment
//!   goes with it, and [`Heap::live`] is the number that says so. Asserting the
//!   process's memory would be measuring the allocator; asserting a count is
//!   asserting the collector.
//!
//! [`Heap::live`]: alo_js::Heap::live
//!
//! Everything here runs **two scripts in one engine**, which is not decoration:
//! a function made by the first and called by the second is the case where the
//! callee's code, its strings and its keys belong to a program the running one
//! has never seen, and it is the only way to be sure they are the callee's
//! rather than the caller's.

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// Run one script in an engine that may already have run others, and say what
/// it evaluated to.
fn run(engine: &mut Engine, source: &str) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(Value::Undefined) => "undefined".to_owned(),
        Ok(Value::Text(held)) => match engine.objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "?".to_owned(),
        },
        Ok(_) => "a value".to_owned(),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// Collect, check the heap's invariants, and answer how many cells are alive.
fn settled(engine: &mut Engine) -> usize {
    engine.objects().heap_mut().collect();
    assert!(
        engine.objects().heap().check().is_ok(),
        "the heap is not well formed after a collection"
    );
    engine.objects().heap().live()
}

#[test]
fn a_closure_reads_what_it_closed_over_after_the_call_has_returned() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "var keep = null; \
             function make(start) { var n = start; return function () { n = n + 1; return n; }; } \
             keep = make(41); undefined",
        ),
        "undefined"
    );
    // The frame `make` ran in is gone and everything in the heap has been
    // walked. A frame slot would have been reused by now; a binding is a cell.
    let _ = settled(&mut engine);

    // A second script, so the call reaches code the running program never
    // compiled.
    assert_eq!(run(&mut engine, "keep()"), "42");
    let _ = settled(&mut engine);
    assert_eq!(
        run(&mut engine, "keep()"),
        "43",
        "and it is the same binding"
    );
}

#[test]
fn two_calls_are_two_environments_and_neither_can_see_the_other() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "function make() { var n = 0; return function () { n = n + 1; return n; }; } \
             var a = make(); var b = make(); undefined",
        ),
        "undefined"
    );
    assert_eq!(run(&mut engine, "a(); a(); a()"), "3");
    assert_eq!(run(&mut engine, "b()"), "1", "b's own binding, untouched");
}

#[test]
fn an_environment_nothing_can_reach_is_reclaimed_and_counted() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    // The shape is fixed first — the global properties, the strings that name
    // them, the function `make` itself — so that what the counts differ by is
    // the closure and its environment and nothing else.
    assert_eq!(
        run(
            &mut engine,
            "var keep = null; \
             function make() { var n = 0; return function () { return n; }; } undefined",
        ),
        "undefined"
    );
    let empty = settled(&mut engine);

    assert_eq!(run(&mut engine, "keep = make(); undefined"), "undefined");
    let holding = settled(&mut engine);
    assert!(
        holding > empty,
        "a closure and the environment it keeps are cells: {empty} became {holding}"
    );

    assert_eq!(run(&mut engine, "keep = null; undefined"), "undefined");
    assert_eq!(
        settled(&mut engine),
        empty,
        "and letting go of the closure lets go of the environment with it"
    );
}

#[test]
fn a_call_that_keeps_nothing_leaves_nothing_behind() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(
        run(
            &mut engine,
            "var i = 0; var total = 0; \
             function work(n) { var held = { deep: { deeper: n } }; return held.deep.deeper; } \
             undefined",
        ),
        "undefined"
    );
    let empty = settled(&mut engine);

    // A thousand calls, each of which makes two objects and an environment and
    // keeps none of them. A heap that held on to a returned call's environment
    // would grow by three thousand cells here.
    assert_eq!(
        run(
            &mut engine,
            "i = 0; total = 0; while (i < 1000) { total = total + work(i); i = i + 1; } total",
        ),
        "499500"
    );
    assert_eq!(
        settled(&mut engine),
        empty,
        "a call that returns keeps nothing"
    );
}

#[test]
fn the_environments_a_closure_reaches_through_are_kept_too() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    // `c` reads `x`, which is two environments out — so keeping `c` has to keep
    // `b`'s environment *and* `a`'s, which is the chain rather than one link.
    assert_eq!(
        run(
            &mut engine,
            "function a(x) { return function b() { return function c() { return x; }; }; } \
             var held = a(9)(); undefined",
        ),
        "undefined"
    );
    let _ = settled(&mut engine);
    assert_eq!(run(&mut engine, "held()"), "9");
}
