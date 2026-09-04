/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 216's four closing conditions: *a closure made in one pass of a
//! `for (let …)` and one made in the next answer differently, a `let` in a
//! loop's **body** is a fresh binding each pass in the same way, a `break` out
//! of three nested blocks leaves three environments, and nothing that compiled
//! before is refused after.*
//!
//! The last one is the whole of the rest of this crate's suite and is not here.
//!
//! # Every case needs two closures, and one of them is not enough
//!
//! `for (let i = 0; i < 2; …)` with one closure made at the end answers `1`
//! whether each pass had a binding of its own or all of them shared one — the
//! shape is only ever wrong about a pass that has **finished**. So each of these
//! keeps a function from one pass and a function from a later one and asks both,
//! and the wrong answer is the one where they agree.
//!
//! # And leaving a block is asserted by what a name means afterwards
//!
//! A `break` that failed to leave its environments would not crash: the next
//! name read in the enclosing body would count its hops from the wrong link and
//! find *some* binding, which is the kind of wrong that a test looking for an
//! error would miss. So the assertion is a value read after the jump, which is
//! the binding the compiler counted or is not.

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// Run one script and say what it evaluated to.
fn value(source: &str) -> String {
    let Ok(mut engine) = Engine::new() else {
        return "no engine".to_owned();
    };
    run(&mut engine, source)
}

/// The same, in an engine that may already have run something.
fn run(engine: &mut Engine, source: &str) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(Value::Undefined) => "undefined".to_owned(),
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
fn a_closure_from_one_pass_of_a_let_head_and_one_from_the_next_are_two_bindings() {
    // `first` sees 0 and `second` sees 1. One shared binding would answer 22 —
    // the value the head held when the loop ended — and a copy made in the
    // wrong place would answer 11.
    assert_eq!(
        value(
            "var first = null; var second = null; \
             for (let i = 0; i < 2; i = i + 1) { \
               if (i === 0) { first = function () { return i; }; } \
               else { second = function () { return i; }; } \
             } \
             first() * 10 + second()",
        ),
        "1"
    );
}

#[test]
fn a_let_in_a_loops_body_is_a_fresh_binding_each_pass() {
    // The head here is a `var`, so nothing is copied: what makes each `x` its
    // own is that the block is entered again and makes an environment again.
    assert_eq!(
        value(
            "var first = null; var second = null; var i = 0; \
             while (i < 2) { \
               let x = i; \
               if (i === 0) { first = function () { return x; }; } \
               else { second = function () { return x; }; } \
               i = i + 1; \
             } \
             first() * 10 + second()",
        ),
        "1"
    );
}

#[test]
fn a_break_out_of_three_blocks_leaves_three_environments() {
    // `out` is binding zero of the function's own environment. A jump that left
    // no environments would read binding zero of the innermost block instead,
    // which holds `4` — a wrong answer rather than an error, which is why this
    // asserts a value.
    assert_eq!(
        value(
            "function f() { \
               var out = 1; \
               outer: { let a = 2; { let b = 3; { let c = 4; break outer; } } } \
               return out; \
             } \
             f()",
        ),
        "1"
    );
}

#[test]
fn a_continue_leaves_them_too_and_the_next_pass_still_copies_the_head() {
    // The `continue` is inside a block of its own, so it has an environment to
    // leave — and what the closure keeps is the **head's** binding, so the copy
    // has to have happened as well.
    //
    // `mark` is there to take binding zero of the block. Without it `seen` and
    // `i` are both binding zero of their own environments and hold the same
    // number, so a `continue` that left nothing would read and write the wrong
    // cell and still answer correctly — a coincidence of layout rather than a
    // test.
    assert_eq!(
        value(
            "var kept = null; var total = 0; \
             for (let i = 0; i < 3; i = i + 1) { \
               { let mark = 100; let seen = i + mark - mark; \
                 if (seen === 1) { kept = function () { return i; }; continue; } \
                 total = total + seen; } \
             } \
             kept() * 10 + total",
        ),
        "12",
        "the closure kept the pass it was made in, and the other two passes ran"
    );
}

#[test]
fn a_function_reads_a_name_a_block_declared_outside_it() {
    // The program item 216 existed to refuse. It is ordinary code.
    assert_eq!(
        value("var read = null; { let a = 5; read = function () { return a; }; } read()"),
        "5"
    );
    // And writing through it reaches the same binding rather than a copy.
    assert_eq!(
        value(
            "var read = null; var write = null; \
             { let a = 5; read = function () { return a; }; \
               write = function (n) { a = n; }; } \
             write(9); read()",
        ),
        "9"
    );
}

#[test]
fn a_switch_declares_in_one_environment_however_many_cases_it_has() {
    // A `let` written in a case belongs to the whole `switch`, so the
    // environment is the switch's rather than one per case — and a `break` out
    // of a case leaves it.
    assert_eq!(
        value(
            "function f(n) { \
               var out = 0; \
               switch (n) { case 1: let a = 10; out = a; break; default: out = 99; } \
               return out; \
             } \
             f(1) * 1000 + f(2)",
        ),
        "10099"
    );
}

#[test]
fn a_block_a_thousand_passes_entered_leaves_nothing_behind() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    // The shape is fixed first, so that what the counts differ by is the
    // environments the loop made and nothing else.
    assert_eq!(
        run(&mut engine, "var i = 0; var total = 0; undefined"),
        "undefined"
    );
    let empty = settled(&mut engine);

    // Two environments a pass — the head's copy and the block's — and two
    // thousand of them are garbage the moment the pass they belong to ends. An
    // engine that kept them would be a loop that allocates for ever.
    assert_eq!(
        run(
            &mut engine,
            "total = 0; for (let n = 0; n < 1000; n = n + 1) { let x = n; total = total + x; } \
             total",
        ),
        "499500"
    );
    assert_eq!(
        settled(&mut engine),
        empty,
        "a pass that keeps nothing keeps nothing"
    );
}

#[test]
fn a_closure_kept_from_a_loop_keeps_its_pass_and_nothing_else() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    assert_eq!(run(&mut engine, "var kept = null; undefined"), "undefined");
    let empty = settled(&mut engine);

    assert_eq!(
        run(
            &mut engine,
            "for (let i = 0; i < 100; i = i + 1) { if (i === 7) { kept = function () { \
             return i; }; } } undefined",
        ),
        "undefined"
    );
    let holding = settled(&mut engine);
    assert!(
        holding > empty,
        "the closure and the one pass it kept are cells: {empty} became {holding}"
    );
    assert_eq!(run(&mut engine, "kept()"), "7");

    assert_eq!(run(&mut engine, "kept = null; undefined"), "undefined");
    assert_eq!(
        settled(&mut engine),
        empty,
        "and the other ninety-nine passes were let go of as they ended"
    );
}
