/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 241, cut from item 78: the engine's half of placing a throw
//! nothing caught.
//!
//! *A throw that escapes a run leaves, innermost first, the program and byte
//! offset of every call it was inside — the throw itself in the innermost, the
//! waiting call in each outside it — at most
//! [`PLACES_IN_A_TRACE`](alo_js::bounds::PLACES_IN_A_TRACE) of them with the
//! rest counted; a caught throw, and the next run, leave nothing.*
//!
//! Offsets are counted by hand from the source in each test, because the
//! thing being tested is that they are the offsets a person would point at.

use std::rc::Rc;

use alo_js::abrupt::{Escape, Kind, Thrown};
use alo_js::bounds::{CALLS_ON_THE_STACK, PLACES_IN_A_TRACE};
use alo_js::interpret::Engine;
use alo_js::object::Value;
use alo_js::{Unit, compile, script};

/// A program compiled from `source`, or why it could not be.
fn unit(source: &str) -> Result<Rc<Unit>, String> {
    let program = script(source).map_err(|why| format!("did not parse: {why}"))?;
    compile(&program)
        .map(Rc::new)
        .map_err(|why| format!("did not compile: {why}"))
}

/// Each place the last throw left, as `(offset, which program)` — the index
/// into `units` of the program it is in, or `usize::MAX` for one it is not.
fn places(engine: &Engine, units: &[&Rc<Unit>]) -> Vec<(usize, usize)> {
    engine
        .unwound()
        .places()
        .iter()
        .map(|place| {
            let which = units
                .iter()
                .position(|unit| Rc::ptr_eq(unit, place.unit()))
                .unwrap_or(usize::MAX);
            (place.at(), which)
        })
        .collect()
}

/// Run `source` in a fresh engine, and answer what escaped and where.
fn thrown_from(source: &str) -> (Option<Escape>, Vec<(usize, usize)>, usize) {
    let (Ok(mut engine), Ok(program)) = (Engine::new(), unit(source)) else {
        return (None, Vec::new(), usize::MAX);
    };
    let escaped = engine.run(&program).err();
    let said = places(&engine, &[&program]);
    (escaped, said, engine.unwound().left_out())
}

// --- The closing clauses ------------------------------------------------------

#[test]
fn a_throw_in_the_script_itself_is_placed_at_the_throw() {
    let (escaped, places, left_out) = thrown_from("let a = 1;\n  throw a");
    assert!(matches!(
        escaped,
        Some(Escape::Thrown(Thrown::Value { at: 13, .. }))
    ));
    assert_eq!(places, vec![(13, 0)]);
    assert_eq!(left_out, 0);
}

#[test]
fn every_call_it_left_is_placed_at_the_call_that_was_waiting() {
    // 0         1         2         3         4
    // 0123456789012345678901234567890123456789012345678
    // function f() { throw 1 }
    // function g() { f() }      <- starts at 25, `f()` at 40
    // g()                       <- at 46
    let (escaped, places, left_out) =
        thrown_from("function f() { throw 1 }\nfunction g() { f() }\ng()");
    assert!(matches!(escaped, Some(Escape::Thrown(_))));
    assert_eq!(places, vec![(15, 0), (40, 0), (46, 0)]);
    assert_eq!(left_out, 0);
}

#[test]
fn a_runaway_recursion_keeps_the_innermost_and_counts_the_rest() {
    let (escaped, places, left_out) = thrown_from("function r() { r() } r()");
    assert!(matches!(
        escaped,
        Some(Escape::Thrown(Thrown::Error {
            kind: Kind::RangeError,
            ..
        }))
    ));
    assert_eq!(places.len(), PLACES_IN_A_TRACE);
    // Every one kept is the recursion's own call, which is where it was.
    assert!(places.iter().all(|place| *place == (15, 0)));
    // The script's frame and every call of `r` are all accounted for: the
    // bound is reached with exactly that many calls on the stack.
    assert_eq!(places.len() + left_out, CALLS_ON_THE_STACK);
}

#[test]
fn a_caught_throw_leaves_nothing() {
    let (escaped, places, left_out) =
        thrown_from("function f() { throw 1 } try { f() } catch { } 2");
    assert!(escaped.is_none());
    assert!(places.is_empty());
    assert_eq!(left_out, 0);
}

#[test]
fn the_next_run_forgets_the_last_throw() {
    let (Ok(mut engine), Ok(throws), Ok(answers)) = (Engine::new(), unit("throw 1"), unit("2 + 2"))
    else {
        panic!("an empty heap holds an engine, and both programs compile");
    };
    assert!(engine.run(&throws).is_err());
    assert_eq!(places(&engine, &[&throws]), vec![(0, 0)]);
    assert_eq!(engine.run(&answers), Ok(Value::Number(4.0)));
    assert!(engine.unwound().places().is_empty());
}

// --- Which program -----------------------------------------------------------

#[test]
fn a_function_from_one_program_is_placed_in_that_program() {
    let (Ok(mut engine), Ok(first), Ok(second)) = (
        Engine::new(),
        unit("function f() {\n  return null.x\n}"),
        unit("let a = 0;\nf()"),
    ) else {
        panic!("an empty heap holds an engine, and both programs compile");
    };
    assert!(engine.run(&first).is_ok());
    let escaped = engine.run(&second);
    assert!(matches!(
        escaped,
        Err(Escape::Thrown(Thrown::Error {
            kind: Kind::TypeError,
            ..
        }))
    ));
    // `null.x` at 24 of the first; `f()` at 11 of the second.
    assert_eq!(places(&engine, &[&first, &second]), vec![(24, 0), (11, 1)]);
}

// --- Throws the engine makes, and throws from inside a conversion ------------

#[test]
fn an_error_the_engine_throws_is_placed_where_the_throw_is_said_to_be() {
    for source in ["null.x", "  undefinedName", "let q = 2; q()"] {
        let (Ok(mut engine), Ok(program)) = (Engine::new(), unit(source)) else {
            panic!("an empty heap holds an engine, and {source:?} compiles");
        };
        let Err(Escape::Thrown(thrown)) = engine.run(&program) else {
            panic!("{source:?} throws");
        };
        // The innermost place and the throw's own offset are one answer
        // reached two ways, and they agree.
        assert_eq!(places(&engine, &[&program]), vec![(thrown.at(), 0)]);
    }
}

#[test]
fn a_throw_from_a_value_of_is_placed_inside_it_and_at_the_operator() {
    // The `+` rewinds to itself to run again once `valueOf` answers; the
    // operator's place is still the operator's (item 210's `now`).
    let (escaped, places, _) = thrown_from("({ valueOf() { throw 1 } }) + 1");
    assert!(matches!(escaped, Some(Escape::Thrown(_))));
    assert_eq!(places, vec![(15, 0), (0, 0)]);
}

#[test]
fn a_throw_from_a_getter_is_placed_inside_it_and_at_the_read() {
    let (escaped, places, _) = thrown_from("const o = { get p() { throw 3 } }; o.p");
    assert!(matches!(escaped, Some(Escape::Thrown(_))));
    assert_eq!(places, vec![(22, 0), (35, 0)]);
}

// --- Calls and jobs an embedder makes ----------------------------------------

#[test]
fn a_call_an_embedder_makes_is_placed_like_any_other() {
    let (Ok(mut engine), Ok(program)) = (Engine::new(), unit("function f(a) { throw a } f")) else {
        panic!("an empty heap holds an engine, and the program compiles");
    };
    let Ok(callee) = engine.run(&program) else {
        panic!("the program answers its function");
    };
    assert!(
        engine
            .call(callee, Value::Undefined, &[Value::Number(5.0)])
            .is_err()
    );
    assert_eq!(places(&engine, &[&program]), vec![(16, 0)]);

    // A callee that is not a function is thrown before any call is entered,
    // and nothing is placed rather than something invented.
    assert!(
        engine
            .call(Value::Number(1.0), Value::Undefined, &[])
            .is_err()
    );
    assert!(engine.unwound().places().is_empty());
}

#[test]
fn a_job_that_throws_is_told_where_and_the_next_job_forgets() {
    let (Ok(mut engine), Ok(throws), Ok(answers)) = (
        Engine::new(),
        unit("(function () { throw 1 })"),
        unit("(function () { })"),
    ) else {
        panic!("an empty heap holds an engine, and both programs compile");
    };
    // Each function is queued as soon as its run answers it: the queue holds
    // it from then on, and the engine keeps only the last run's value.
    for program in [&throws, &answers] {
        let Ok(callee) = engine.run(program) else {
            panic!("the program answers its function");
        };
        assert!(engine.queue_job(callee, &[], 0).is_ok());
    }
    let mut told = Vec::new();
    let drained = engine.checkpoint(&mut |_, thrown, unwound| {
        told.push((
            thrown.at(),
            unwound
                .places()
                .iter()
                .map(|place| (place.at(), Rc::ptr_eq(place.unit(), &throws)))
                .collect::<Vec<_>>(),
        ));
    });
    assert!(drained.is_ok_and(|drained| drained.ran == 2 && drained.threw == 1));
    assert_eq!(told, vec![(15, vec![(15, true)])]);
    // The second job ran after the first and started from nothing unwound.
    assert!(engine.unwound().places().is_empty());
}
