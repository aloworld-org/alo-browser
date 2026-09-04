/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The half of queue item 72 that is `LOOP.md`'s stage 2 clause 2: **a script
//! is a stranger's bytes, and we then execute them**.
//!
//! ADR 0013 § 4 in one sentence — *it never panics, not on any source text, not
//! on any program* — and every case here ends in a value, a refusal or an
//! interruption. The parser's own hostile file
//! (`a_program_that_is_hostile.rs`) covers what a *tree* can do to us; this one
//! covers what a **run** can.

use std::fmt::Write;
use std::thread;
use std::time::Duration;

use alo_js::abrupt::Escape;
use alo_js::compile;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::script;

/// Run a program in a fresh engine, saying in words what went wrong before it
/// started.
fn run(source: &str) -> Result<Value, String> {
    let program = script(source).map_err(|why| format!("did not parse: {why}"))?;
    let mut engine = Engine::new().map_err(|why| format!("no engine: {why}"))?;
    engine
        .evaluate(&program)
        .map_err(|why: Trouble| why.to_string())
}

#[test]
fn a_tree_as_deep_as_the_parser_will_build_compiles_and_runs() {
    // The compiler recurses once per level of tree, and this test thread has
    // two mebibytes. Without `bounds::STACK_FOR_A_COMPILE` this is the case
    // that ends the process rather than answering — the same defect queue item
    // 208 found in the parser, one stage further along.
    let deep = format!("1{}", "+1".repeat(4000));
    match run(&deep) {
        Ok(Value::Number(number)) => assert!((number - 4001.0).abs() < f64::EPSILON),
        other => panic!("a long chain of additions is a number: {other:?}"),
    }

    // And the interpreter does not recurse at all, which is the property a
    // bytecode loop has and a tree walker does not: this runs on whatever stack
    // it was called with.
    let nested = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    assert!(matches!(run(&nested), Ok(Value::Number(_))));
}

#[test]
fn a_tree_deeper_than_that_is_refused_rather_than_compiled() {
    // The parser refuses it, which is queue item 208's bound, and the point
    // here is that the refusal reaches the caller as a refusal.
    let source = format!("1{}", "+1".repeat(100_000));
    assert!(
        script(&source).is_err(),
        "a chain past the tree bound is a syntax error rather than a compile"
    );
}

#[test]
fn a_program_that_is_wide_rather_than_deep_still_runs() {
    // The other direction, which is the one a bundler actually produces: many
    // statements, many bindings, many properties, none of them nested.
    let mut source = String::from("let total = 0;");
    for which in 0..5_000 {
        let _ = write!(source, "total += {which};");
    }
    source.push_str("total");
    match run(&source) {
        Ok(Value::Number(number)) => {
            assert!((number - 12_497_500.0).abs() < f64::EPSILON, "{number}");
        }
        other => panic!("five thousand statements is a number: {other:?}"),
    }

    let mut object = String::from("let held = { ");
    for which in 0..5_000 {
        let _ = write!(object, "a{which}: {which}, ");
    }
    object.push_str("}; held.a4999");
    match run(&object) {
        Ok(Value::Number(number)) => assert!((number - 4999.0).abs() < f64::EPSILON),
        other => panic!("an object of five thousand properties answers: {other:?}"),
    }
}

#[test]
fn a_deep_graph_of_objects_is_built_and_let_go_of_without_recursing() {
    // Every reader of a heap this shape is a place a script could choose the
    // depth of. Building it, walking it, collecting it and dropping it all
    // happen here.
    let mut source = String::from("let held = { down: null }; let at = held;");
    source.push_str("let n = 0; while (n < 2000) { at.down = { down: null }; at = at.down; n++; }");
    source.push('n');
    match run(&source) {
        Ok(Value::Number(number)) => assert!((number - 2000.0).abs() < f64::EPSILON),
        other => panic!("a chain of two thousand objects is built: {other:?}"),
    }
}

#[test]
fn a_recursion_that_will_not_end_is_a_range_error_rather_than_a_stack_overflow() {
    // Queue item 209's third closing condition, and the one clause of it that
    // is about this process rather than about the language: an interpreter that
    // recursed would end the renderer here, which ADR 0013 § 4 forbids in one
    // sentence — *it never panics, not on any source text, not on any program*.
    //
    // The assertion is the error the language specifies, so a page that
    // recurses on purpose can say what to do about it (queue item 210) rather
    // than losing its tab.
    for source in [
        "function f() { return f(); } f()",
        // Not a tail call, so nothing could quietly turn it into a loop.
        "function f(n) { return 1 + f(n + 1); } f(0)",
        // Two functions calling each other, which a bound counting one name
        // would miss.
        "function a() { return b(); } function b() { return a(); } a()",
        // An arrow, which takes a different path to its `this`.
        "var f = () => f(); f()",
        // And one that makes an object per frame, so the heap is under
        // pressure while the frames pile up.
        "function f(n) { var held = { n: n }; return f(held.n + 1); } f(0)",
    ] {
        match run(source) {
            Err(why) => assert!(
                why.contains("RangeError"),
                "{source} should be a RangeError: {why}"
            ),
            Ok(value) => panic!("{source} should not answer: {value:?}"),
        }
    }
}

#[test]
fn an_accessor_that_will_not_end_is_a_range_error_too() {
    // Queue item 214's fourth closing condition. Every one of these is a call
    // *inside* an instruction, so a frame is pushed while an operand stack is
    // half way through an expression — which is exactly the shape an engine
    // that re-entered itself in Rust would end the process on.
    for source in [
        // The item's own words: a getter that calls something that reads the
        // same property.
        "const o = { get a() { return f(); } }; function f() { return o.a; } o.a",
        // The same without a function in between.
        "const o = { get a() { return o.a; } }; o.a",
        // A setter, which is the other half and takes a different path out.
        "const o = { set a(v) { o.a = v; } }; o.a = 1;",
        // Reading through a prototype, where the receiver is the child every
        // time and the property is found one step up.
        "const p = { get a() { return this.a; } }; const o = { __proto__: p }; o.a",
        // A conversion, which is the deepest of them: an instruction half way
        // through an addition, calling a method, which starts the same addition
        // again.
        "const o = { valueOf() { return o + 1; } }; o + 1",
        // And one where each frame allocates, so the heap is under pressure
        // while the frames pile up.
        "const o = { get a() { return { held: o.a }; } }; o.a",
    ] {
        match run(source) {
            Err(why) => assert!(
                why.contains("RangeError"),
                "{source} should be a RangeError: {why}"
            ),
            Ok(value) => panic!("{source} should not answer: {value:?}"),
        }
    }
}

#[test]
fn an_engine_that_refused_a_recursion_still_works_afterwards() {
    // The frames are given back however the run ended, so an engine that has
    // just refused a runaway recursion is an engine, not a leak — the same
    // property as being stopped by the embedder, for the same reason.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let Ok(runaway) = script("function f() { return f(); } f()") else {
        panic!("that parses");
    };
    assert!(engine.evaluate(&runaway).is_err());
    engine.objects().heap_mut().collect();

    let Ok(again) = script("function g(n) { return n * 2; } g(21)") else {
        panic!("that parses");
    };
    match engine.evaluate(&again) {
        Ok(Value::Number(number)) => assert!((number - 42.0).abs() < f64::EPSILON),
        other => panic!("the engine still runs a program: {other:?}"),
    }
}

#[test]
fn a_script_that_will_not_finish_is_stopped_by_the_embedder() {
    // ADR 0013 § 4: *the interpreter is interruptible. A script that will not
    // finish is stopped by the embedder* — and there is no clock in `alo-js`,
    // so the asking happens on another thread, which is where a browser
    // process's judgement about a tab would come from.
    let Ok(program) = script("let n = 0; while (true) { n = n + 1; }") else {
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
        other => panic!("a loop that never ends is stopped: {other:?}"),
    }
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };

    // And the engine is still usable afterwards, which is what makes stopping a
    // tab different from losing one.
    engine.stop().clear();
    let Ok(again) = script("1 + 1") else {
        panic!("that parses");
    };
    assert!(matches!(engine.evaluate(&again), Ok(Value::Number(_))));
}

#[test]
fn every_shape_that_ends_a_script_early_is_a_refusal_rather_than_a_crash() {
    // One case per way out of `Escape`, so that a new way of ending a script
    // has to be added here to be complete.
    let cases = [
        // The page's own, which its `catch` will survive (queue item 210).
        "nobody",
        "null.a",
        "const a = 1; a = 2;",
        "throw 1;",
        "({}) + ''",
        "({})[{}]",
        "f()",
        "(1)()",
        "'use strict'; function f() { return this.a; } f()",
        // Not built yet, each naming its queue item.
        "'a'.length",
        "function f() { return this.a; } f.call",
        "new f()",
        // Nonsense a program can still be made of.
        "let a = {}; a[a] = a; typeof a",
        "let a = ''; a += a; a += a; a.length",
        // A call whose arguments are the awkward part rather than the callee.
        "function f(a) { return a; } f(f(f(f(1))))",
        "function f() { return f; } f()()()()",
    ];
    for source in cases {
        let Ok(program) = script(source) else {
            panic!("{source} parses");
        };
        let Ok(mut engine) = Engine::new() else {
            panic!("an empty heap holds an engine");
        };
        // The assertion is that this line returns at all: an escape is a
        // result, and the process is still here to read it.
        let _ = engine.evaluate(&program);
    }
}

#[test]
fn a_program_the_compiler_refuses_never_reaches_the_interpreter() {
    let Ok(program) = script("try { 1; } catch {}") else {
        panic!("that parses");
    };
    match compile::compile(&program) {
        Err(compile::Refusal::NotBuiltYet { what, .. }) => {
            assert_eq!(what.item(), 210, "`try` is queue item 210's");
        }
        other => panic!("a `try` is refused by name: {other:?}"),
    }
}
