/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 210: *`try`, `catch` and `finally`*. Cut from item 72, which
//! throws and had nowhere for a throw to land.
//!
//! # What opened it
//!
//! A real page's own script. The frozen service worker
//! (`crates/alo-corpus/scripts/alo-service-worker/script.js`, alo's own
//! `sw.js`) was refused at byte 2853, the `try` of its push handler, after
//! item 225 built the array literal on the line before it.
//!
//! # What closes it
//!
//! Each of the five ways out of a `try` — normally, by a throw, by a
//! `return`, by a `break` and by a `continue` — runs its `finally` exactly
//! once, in a test that names which way it left; and a `catch` binds what was
//! thrown, which for an error this engine throws is an instance of the
//! constructor its kind names (item 227).
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with the collector firing at every allocation, and
//! the two must agree. A throw takes a value off the stack and carries it in
//! a Rust local across every frame it unwinds, and an error the engine threw
//! is made into an object on the way into the `catch` — both of which are
//! exactly where a missed root would show.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alo_js::abrupt::Escape;
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

// --- The five ways out ------------------------------------------------------

#[test]
fn leaving_normally_runs_the_finally_once() {
    table(&[
        ("let n = 0; try { n += 10; } finally { n++; } n", "11"),
        (
            "let n = 0; try { } catch { n += 100; } finally { n++; } n",
            "1",
        ),
        // Normally, then on: the statement after the `try` runs.
        (
            "let s = ''; try { s += 't'; } finally { s += 'f'; } s += 'a'",
            "\"tfa\"",
        ),
    ]);
}

#[test]
fn leaving_by_a_throw_runs_the_finally_once_and_throws_on() {
    table(&[
        (
            "let n = 0; try { try { throw 'x'; } finally { n++; } } catch (e) { e + n }",
            "\"x1\"",
        ),
        // Nothing outside catches it: the `finally` still runs, and the script
        // ends with what was thrown, not with what the `finally` did.
        //
        // It is reported at the `try`, because what leaves the `finally` is a
        // throw of its own of the value it was carrying. Where the value was
        // first thrown is a stack trace's business, which is queue item 78.
        (
            "var n = 0; try { throw 1; } finally { n++; }",
            "! the script threw a value (at byte 11)",
        ),
        // A `catch` that throws on is a throw out of the `try` too.
        (
            "let n = 0; try { try { throw 1; } catch (e) { throw e + 1; } finally { n++; } } catch (e) { e * 10 + n }",
            "21",
        ),
    ]);
}

#[test]
fn leaving_by_a_return_runs_the_finally_once_after_the_value_is_made() {
    table(&[
        (
            "let n = 0; function f() { try { return 'r'; } finally { n++; } } f() + n",
            "\"r1\"",
        ),
        // The returned value is evaluated before the `finally` runs, and the
        // `finally` changing what it was made from does not change it.
        (
            "let s = ''; function f() { try { s += 't'; return s; } finally { s += 'f'; } } f() + '|' + s",
            "\"t|tf\"",
        ),
        // From the `catch`, too.
        (
            "let n = 0; function f() { try { throw 0; } catch { return 'c'; } finally { n++; } } f() + n",
            "\"c1\"",
        ),
    ]);
}

#[test]
fn leaving_by_a_break_runs_the_finally_once() {
    table(&[
        (
            "let n = 0; while (true) { try { break; } finally { n++; } } n",
            "1",
        ),
        // A labelled block, which is broken out of and never continued.
        (
            "let n = 0; out: { try { break out; } finally { n++; } n += 10; } n",
            "1",
        ),
        // A `break` that stays inside the `try` is not a way out of it.
        (
            "let n = 0; try { while (true) { break; } n += 10; } finally { n++; } n",
            "11",
        ),
    ]);
}

#[test]
fn leaving_by_a_continue_runs_the_finally_once_per_pass() {
    table(&[
        (
            "let n = 0; for (let i = 0; i < 1; i++) { try { continue; } finally { n++; } n += 10; } n",
            "1",
        ),
        (
            "let n = 0, i = 0; while (i < 3) { i++; try { continue; } finally { n++; } } n",
            "3",
        ),
        // `continue` in a `do … while` goes to the test, through the `finally`.
        (
            "let s = '', i = 0; do { try { i++; continue; } finally { s += i; } } while (i < 3); s",
            "\"123\"",
        ),
    ]);
}

// --- What a `catch` binds ---------------------------------------------------

#[test]
fn a_catch_binds_exactly_what_was_thrown() {
    table(&[
        ("try { throw 42; } catch (e) { e }", "42"),
        ("let o = {}; try { throw o; } catch (e) { e === o }", "true"),
        (
            "try { throw undefined; } catch (e) { typeof e }",
            "\"undefined\"",
        ),
        ("try { throw null; } catch (e) { e }", "null"),
        // Without a name, nothing looks at it and the block still runs.
        ("let n = 0; try { throw 1; } catch { n = 7; } n", "7"),
        // The name is the catch's own, and the one outside is untouched.
        (
            "let e = 'out'; try { throw 'in'; } catch (e) { } e",
            "\"out\"",
        ),
        (
            "let e = 'out'; try { throw 'in'; } catch (e) { e = 'changed'; } e",
            "\"out\"",
        ),
        // A closure made in the block keeps the binding it was given.
        (
            "let f; try { throw 5; } catch (e) { f = () => e; } f()",
            "5",
        ),
        // Each pass's throw is a binding of its own.
        (
            "let fs = []; for (let i = 0; i < 3; i++) { try { throw i; } catch (e) { fs[i] = () => e; } } '' + fs[0]() + fs[1]() + fs[2]()",
            "\"012\"",
        ),
        // A block of its own inside the catch may shadow the name.
        ("try { throw 1; } catch (e) { { let e = 2; } e }", "1"),
    ]);
}

#[test]
fn an_error_this_engine_throws_is_caught_as_an_instance_of_its_constructor() {
    for (program, family) in [
        ("null.a", "TypeError"),
        ("nobody", "ReferenceError"),
        ("const a = 1; a = 2;", "TypeError"),
        ("(1)()", "TypeError"),
        ("let a = []; a.length = -1;", "RangeError"),
    ] {
        let wrap = |inside: &str| format!("try {{ {program} }} catch (e) {{ {inside} }}");
        table(&[
            (&wrap("e.name"), &format!("\"{family}\"")),
            (&wrap(&format!("e.constructor === {family}")), "true"),
            (
                &wrap(&format!("e.__proto__ === {family}.prototype")),
                "true",
            ),
            (&wrap("typeof e.message"), "\"string\""),
            (&wrap("e.hasOwnProperty('message')"), "true"),
            (&wrap("e.propertyIsEnumerable('message')"), "false"),
            (&wrap("'' + e === e.name + ': ' + e.message"), "true"),
            (
                &wrap("let t = ({}).toString; e.t = t; e.t()"),
                "\"[object Error]\"",
            ),
        ]);
    }
    // And the message is the one the escape carried.
    table(&[(
        "try { nobody; } catch (e) { '' + e }",
        "\"ReferenceError: 'nobody' is not defined\"",
    )]);
}

// --- Where a throw comes from -----------------------------------------------

#[test]
fn a_throw_lands_from_any_depth_of_call() {
    table(&[
        (
            "function a() { throw 'x'; } function b() { return a() + 1; } try { b(); } catch (e) { e }",
            "\"x\"",
        ),
        // A getter is a call nothing in the source spells.
        (
            "let o = { get g() { throw 'g'; } }; try { o.g; } catch (e) { e }",
            "\"g\"",
        ),
        // A setter.
        (
            "let o = { set s(v) { throw v + 1; } }; try { o.s = 1; } catch (e) { e }",
            "2",
        ),
        // A conversion, which rewinds the instruction that wanted it.
        (
            "let o = { valueOf() { throw 'v'; } }; try { o + 1; } catch (e) { e }",
            "\"v\"",
        ),
        (
            "let o = { toString() { throw 's'; } }; try { `${o}`; } catch (e) { e }",
            "\"s\"",
        ),
        // `new`, whose body throws.
        (
            "function F() { throw 'n'; } try { new F(); } catch (e) { e }",
            "\"n\"",
        ),
        // The frame that catches is the caller, and the callee's `finally`
        // runs on the way.
        (
            "let s = ''; function f() { try { throw 1; } finally { s += 'f'; } } try { f(); } catch (e) { s += 'c' + e; } s",
            "\"fc1\"",
        ),
    ]);
}

#[test]
fn a_throw_from_inside_a_builtin_takes_the_builtin_down_with_it() {
    table(&[
        // The error constructor asks for the message's `toString`, and the page
        // throws from inside it: the builtin waiting for the answer goes.
        (
            "try { new Error({ toString() { throw 't'; } }); } catch (e) { e }",
            "\"t\"",
        ),
        // And the engine is still whole afterwards — the builtin is not left
        // waiting for an answer that will never come.
        (
            "try { new Error({ toString() { throw 't'; } }); } catch (e) { } new Error('ok').message",
            "\"ok\"",
        ),
        // A cause behind a getter, the builtin's second kind of call.
        (
            "try { new TypeError('m', { get cause() { throw 'c'; } }); } catch (e) { e }",
            "\"c\"",
        ),
        // A builtin that throws itself.
        (
            "let t = Error.prototype.toString; try { t(); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn a_throw_leaves_every_block_it_was_in() {
    table(&[
        (
            "let a = 'outer'; try { let a = 'inner'; { let b = 1; throw 0; } } catch { } a",
            "\"outer\"",
        ),
        // A throw from three blocks down inside a loop's per-pass environment,
        // and the loop goes on.
        (
            "let s = ''; for (let i = 0; i < 3; i++) { try { let x = i; { let y = x; throw y; } } catch (e) { s += e + i; } } s",
            "\"024\"",
        ),
        // The name read in the `catch` and after it is one binding out, and a
        // throw that left its blocks standing would answer from one of them
        // instead: every binding here holds a different letter so that the
        // wrong one cannot read as the right one.
        (
            "let s = ''; { let k = 'k'; try { let x = 'x'; { let y = 'y'; throw 0; } } catch { s += k; } s += k; } s",
            "\"kk\"",
        ),
        (
            "function f() { let k = 'k'; try { let x = 'x'; { let y = 'y'; throw 0; } } catch (e) { return k + e; } } f()",
            "\"k0\"",
        ),
        (
            "let s = ''; { let k = 'k'; try { try { let x = 'x'; { let y = 'y'; throw 0; } } finally { s += k; } } catch { } } s",
            "\"k\"",
        ),
        // A function's own environment is not one the `try` went into.
        (
            "function f(a) { try { { let b = a; throw b; } } catch (e) { return a + e; } } f(2)",
            "4",
        ),
    ]);
}

// --- `finally` and the order of things --------------------------------------

#[test]
fn nested_finallys_run_innermost_first() {
    table(&[
        (
            "let s = ''; out: for (;;) { try { try { break out; } finally { s += 'a'; } } finally { s += 'b'; } } s",
            "\"ab\"",
        ),
        (
            "let s = ''; function f() { try { try { return 'r'; } finally { s += 'a'; } } finally { s += 'b'; } } f() + s",
            "\"rab\"",
        ),
        (
            "let s = ''; try { try { try { throw 1; } finally { s += 'a'; } } finally { s += 'b'; } } catch (e) { s += e; } s",
            "\"ab1\"",
        ),
        // A `continue` past two `finally`s to an outer loop.
        (
            "let s = ''; outer: for (let i = 0; i < 2; i++) { for (;;) { try { try { continue outer; } finally { s += 'a'; } } finally { s += 'b'; } } } s",
            "\"abab\"",
        ),
    ]);
}

#[test]
fn a_finally_that_leaves_its_own_way_wins() {
    table(&[
        (
            "function f() { try { return 1; } finally { return 2; } } f()",
            "2",
        ),
        (
            "function f() { try { throw 1; } finally { return 2; } } f()",
            "2",
        ),
        (
            "function f() { try { return 1; } finally { throw 2; } } try { f(); } catch (e) { e }",
            "2",
        ),
        (
            "let n = 0; out: { try { throw 1; } finally { break out; } } 'kept going'",
            "\"kept going\"",
        ),
        (
            "let n = 0; while (n < 5) { n++; try { break; } finally { continue; } } n",
            "5",
        ),
    ]);
}

#[test]
fn a_return_in_a_function_written_inside_a_try_leaves_the_function_only() {
    table(&[(
        "let n = 0; try { function f() { return 1; } n = f() + f(); } finally { n += 10; } n",
        "12",
    )]);
}

// --- What a script evaluates to ---------------------------------------------

#[test]
fn a_try_completes_as_the_specification_says() {
    table(&[
        ("try { 1; } catch { 2; }", "1"),
        ("try { throw 0; } catch { 3; }", "3"),
        ("try { 1; throw 0; } catch { }", "undefined"),
        ("1; try { } catch { }", "undefined"),
        ("try { 1; } finally { 2; }", "1"),
        ("try { throw 0; } catch { 1; } finally { 2; }", "1"),
    ]);
}

// --- What is not caught -----------------------------------------------------

#[test]
fn what_is_not_the_pages_is_not_caught() {
    // Something not built yet is a sentence for a person, not a value.
    let answer = value("try { 'a'.length; } catch { 'caught'; }");
    assert!(answer.contains("queue item 73"), "{answer}");

    // And a script the embedder stopped stays stopped, `finally` or not.
    let Ok(program) = script("let n = 0; while (true) { try { n++; } catch { } finally { } }")
    else {
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
        other => panic!("a loop inside a try is stopped all the same: {other:?}"),
    }
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };
}

#[test]
fn a_recursion_that_catches_its_own_range_error_can_still_be_stopped() {
    // Before a `catch` existed, a recursion ended at the depth bound whether
    // the page liked it or not. Now it can catch that and recurse again, and
    // this one does twice per level — no backward jump anywhere — so the
    // embedder's switch has to be read on a call as well.
    let Ok(program) = script("function f() { try { f(); } finally { f(); } } f()") else {
        panic!("that parses");
    };
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let stop = engine.stop();
    let asking = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        stop.ask();
    });
    match engine.evaluate(&program) {
        Err(Trouble::Escaped(Escape::Interrupted)) => {}
        other => panic!("a recursion with no loop in it is stopped: {other:?}"),
    }
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };

    // And a recursion that catches the bound once comes back with an answer,
    // and so does one that catches it at the top. Run ordinarily only, as
    // `an_engine_that_is_hostile.rs` runs its runaway recursions: under stress
    // ten thousand frames cost time quadratic in their number, and the
    // rooting they exercise is the same as a shallow throw across frames,
    // which `a_throw_lands_from_any_depth_of_call` runs both ways.
    for (source, expected) in [
        (
            "function f() { try { return f(); } catch (e) { return e.name; } } f()",
            "\"RangeError\"",
        ),
        (
            "function f() { return f(); } try { f(); } catch (e) { e.constructor === RangeError }",
            "true",
        ),
        // The frames it unwound gave their environments back: it can do it
        // again.
        (
            "function f() { return f(); } let n = 0; for (let i = 0; i < 3; i++) { try { f(); } catch { n++; } } n",
            "3",
        ),
    ] {
        assert_eq!(fresh(source, false), expected, "{source}");
    }
}

#[test]
fn the_frames_a_throw_unwinds_give_their_environments_back() {
    // A frame holds its environment by a root, and a root nobody releases
    // keeps it — and everything it reaches — for the life of the engine. Ten
    // thousand frames unwound into a `catch` must leave nothing behind that a
    // collection cannot take.
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let mut live = Vec::new();
    for _ in 0..3 {
        let Ok(program) =
            script("function f(a) { let b = [a]; return f(b); } try { f(0); } catch { }")
        else {
            panic!("that parses");
        };
        assert!(engine.evaluate(&program).is_ok());
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
fn every_cut_of_a_try_program_is_a_result_rather_than_a_crash() {
    let source = "let s = ''; function f(n) { try { if (n) throw n; return 'r'; } \
                  catch (e) { s += e; return f(n - 1); } finally { s += '.'; } } \
                  out: for (let i = 0; i < 2; i++) { try { s += f(2); continue out; } \
                  finally { s += i; } } s";
    // `s += f(2)` reads `s` before the call writes it, which is why what
    // the calls appended is lost and only `r` and the passes survive.
    assert_eq!(value(source), "\"r0r1\"");
    for end in 0..=source.len() {
        let Some(cut) = source.get(..end) else {
            continue;
        };
        let _ = value(cut);
    }
}

#[test]
fn a_tower_of_trys_is_refused_by_the_parser_or_run_and_never_more() {
    // Nested past the parser's bound: a refusal, not an overflow.
    let deep = format!(
        "{}1{}",
        "try {".repeat(20_000),
        "} finally {}".repeat(20_000)
    );
    assert!(
        value(&deep).starts_with("did not parse"),
        "twenty thousand nested trys are refused by the parser's bound"
    );
    // Within it, every `finally` runs, innermost first, on every way out.
    let nested = |depth: usize, inside: &str| {
        let mut source = String::from("let n = 0; function f() { ");
        source.push_str(&"try { ".repeat(depth));
        source.push_str(inside);
        source.push_str(&" } finally { n++; }".repeat(depth));
        source.push_str(" } try { f(); } catch { } n");
        source
    };
    for inside in ["return 1;", "throw 1;", "1;"] {
        assert_eq!(value(&nested(200, inside)), "200", "{inside}");
    }
    // And many in a row, each taking slots of its own.
    let many = format!(
        "let n = 0; {} n",
        "try { n++; } finally { n++; } ".repeat(2_000)
    );
    assert_eq!(value(&many), "4000");
}

// --- What is not a program --------------------------------------------------

#[test]
fn a_second_declaration_of_the_catchs_name_is_not_a_program() {
    for source in [
        "try {} catch (e) { let e; }",
        "try {} catch (e) { const e = 1; }",
        "try {} catch (e) { function e() {} }",
        // Annex B allows this one for old pages; that is the legacy tail
        // (queue item 142).
        "try {} catch (e) { var e; }",
        "try {} catch (e) { { var e; } }",
    ] {
        let Ok(program) = script(source) else {
            panic!("{source} parses");
        };
        match alo_js::compile(&program) {
            Err(Refusal::NotAProgram { why, .. }) => {
                assert!(why.contains("catch's own name"), "{source}: {why}");
            }
            other => panic!("{source} is not a program: {other:?}"),
        }
    }
    // A pattern is item 211's, as every pattern is.
    let Ok(program) = script("try {} catch ([a]) {}") else {
        panic!("that parses");
    };
    match alo_js::compile(&program) {
        Err(Refusal::NotBuiltYet { what, .. }) => assert_eq!(what, What::TakingAValueApart),
        other => panic!("a pattern names item 211: {other:?}"),
    }
}

// --- The page that opened it ------------------------------------------------

/// The frozen service worker, or nothing if it is not where it should be.
fn service_worker() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/scripts/alo-service-worker/script.js");
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_frozen_service_worker_compiles_past_the_try_that_stopped_it() {
    let source = service_worker();
    assert!(!source.is_empty(), "the frozen script is in the corpus");
    assert_eq!(
        source.get(2853..2856),
        Some("try"),
        "the frozen bytes are the ones the item was opened against"
    );
    let Ok(program) = script(&source) else {
        panic!("the frozen script parses");
    };
    match alo_js::compile(&program) {
        Err(Refusal::NotBuiltYet { what, at }) => {
            assert!(at > 2853, "it gets past the try at 2853, to {at}");
            // What stops it next is the `for … of` inside the `try` — queue
            // item 211, and a different item.
            assert_eq!(what, What::TakingAValueApart);
            assert_eq!(at, 2922);
            assert_eq!(source.get(at..at.saturating_add(3)), Some("for"));
        }
        other => panic!("expected the next unbuilt item, got {other:?}"),
    }
}
