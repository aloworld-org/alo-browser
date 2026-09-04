/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 72's closing condition: *a suite of small programs produces the
//! values the specification says, run as a table rather than as prose.*
//!
//! ADR 0013 § 9 is where that rule comes from, and it says why a language is
//! different from the rest of this repository: *there is no honest way to claim
//! an interpreter is correct from a handful of examples*, so the semantics
//! argument is settled in a table with values in it rather than in a paragraph.
//!
//! # Every program runs twice
//!
//! Once ordinarily, and once with [`Heap::stress`] on — collecting at **every**
//! allocation. ADR 0014 § 10 says that mode is not optional, and this is the
//! file that spends it: a rooting bug in the interpreter is invisible in the
//! first run and fails in the second, because the only reference to a value the
//! engine is in the middle of using was a stack slot it had already given up.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// Run a program in a fresh engine, both ways, and answer what it produced.
///
/// The two runs must agree: a value that differs under stress is a reference
/// the collector took while the interpreter was holding it.
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
fn literals_and_the_completion_value() {
    table(&[
        ("1", "1"),
        ("1.5", "1.5"),
        ("0x10", "16"),
        ("'a'", "\"a\""),
        ("true", "true"),
        ("null", "null"),
        ("undefined", "undefined"),
        // A program of nothing but declarations evaluates to `undefined`.
        ("let a = 1;", "undefined"),
        // The last statement that produced a value wins, and a block that
        // produced none leaves the one before it alone.
        ("1; 2;", "2"),
        ("2; {}", "2"),
        ("2; ;", "2"),
        // An `if` is different from a block, and this is the difference: an
        // empty branch makes the completion `undefined` rather than leaving it.
        ("2; if (true) {}", "undefined"),
        ("2; while (false) {}", "undefined"),
        ("2; if (true) { 3; }", "3"),
    ]);
}

#[test]
fn arithmetic_is_the_languages_rather_than_the_hardwares() {
    table(&[
        ("1 + 1", "2"),
        ("7 / 2", "3.5"),
        ("1 / 0", "Infinity"),
        ("-1 / 0", "-Infinity"),
        ("0 / 0", "NaN"),
        ("-1 % 2", "-1"),
        ("2 ** 10", "1024"),
        ("(-1) ** Infinity", "NaN"),
        ("2 ** Infinity", "Infinity"),
        ("1 << 31", "-2147483648"),
        ("1 << 32", "1"),
        ("-1 >>> 0", "4294967295"),
        ("-1 >> 1", "-1"),
        ("~5", "-6"),
        ("5 & 3", "1"),
        ("5 | 3", "7"),
        ("5 ^ 3", "6"),
        ("+'12'", "12"),
        ("+'1_0'", "NaN"),
        ("-'0x10'", "-16"),
        ("+''", "0"),
        ("+' '", "0"),
        ("+'abc'", "NaN"),
    ]);
}

#[test]
fn plus_is_two_operators_sharing_a_spelling() {
    table(&[
        ("'a' + 1", "\"a1\""),
        ("1 + 'a'", "\"1a\""),
        ("1 + null", "1"),
        ("1 + undefined", "NaN"),
        ("'' + true", "\"true\""),
        ("'' + 1e21", "\"1e+21\""),
        ("'' + 0.1", "\"0.1\""),
        ("'' + -0", "\"0\""),
        ("'a' + 'b' + 'c'", "\"abc\""),
    ]);
}

#[test]
fn equality_and_comparison() {
    table(&[
        ("1 == '1'", "true"),
        ("1 === '1'", "false"),
        ("null == undefined", "true"),
        ("null === undefined", "false"),
        ("null == 0", "false"),
        ("NaN == NaN", "false"),
        ("NaN !== NaN", "true"),
        ("0 === -0", "true"),
        ("true == 1", "true"),
        ("'' == 0", "true"),
        ("'a' < 'b'", "true"),
        ("'Z' < 'a'", "true"),
        ("'10' < '9'", "true"),
        ("10 < 9", "false"),
        ("'10' < 9", "false"),
        ("1 <= 1", "true"),
        ("1 > 2", "false"),
        ("2 >= 2", "true"),
    ]);
}

#[test]
fn the_operators_that_do_not_evaluate_both_sides() {
    table(&[
        ("1 && 2", "2"),
        ("0 && 2", "0"),
        ("1 || 2", "1"),
        ("0 || 2", "2"),
        ("null ?? 2", "2"),
        ("0 ?? 2", "0"),
        ("false ?? 2", "false"),
        ("1 ? 'a' : 'b'", "\"a\""),
        ("0 ? 'a' : 'b'", "\"b\""),
        ("(1, 2, 3)", "3"),
        // The right side really is not evaluated: a name nothing declares
        // would be a `ReferenceError` if it were.
        ("0 && nobody", "0"),
        ("1 || nobody", "1"),
    ]);
}

#[test]
fn typeof_and_void_and_delete() {
    table(&[
        ("typeof 1", "\"number\""),
        ("typeof 'a'", "\"string\""),
        ("typeof true", "\"boolean\""),
        ("typeof undefined", "\"undefined\""),
        // The oldest wart in the language, and it is specified.
        ("typeof null", "\"object\""),
        ("typeof {}", "\"object\""),
        // A name nothing declares: a string rather than a `ReferenceError`,
        // which is the whole reason `typeof` has an instruction of its own.
        ("typeof nobodyDeclaredThis", "\"undefined\""),
        ("void 1", "undefined"),
        ("var a = 1; delete a", "false"),
        ("b = 1; delete b", "true"),
        ("delete ({ a: 1 }).a", "true"),
    ]);
}

#[test]
fn bindings_scopes_and_the_dead_zone() {
    table(&[
        ("var a = 1; a", "1"),
        ("var a; a", "undefined"),
        ("let a = 1; a", "1"),
        ("const a = 1; a", "1"),
        ("a = 1; a", "1"),
        ("let a = 1; { let a = 2; } a", "1"),
        ("let a = 1; { a = 2; } a", "2"),
        ("{ let a = 1; } typeof a", "\"undefined\""),
        ("var a = 1; { var a = 2; } a", "2"),
        // The dead zone is a `ReferenceError` rather than `undefined`, and it
        // is what makes `let` different from `var`.
        (
            "{ a; let a = 1; }",
            "! ReferenceError: 'a' is used before it is declared (at byte 2)",
        ),
        (
            "const a = 1; a = 2;",
            "! TypeError: 'a' is a constant and cannot be assigned to (at byte 13)",
        ),
        (
            "{ const a = 1; a = 2; }",
            "! TypeError: 'a' is a constant and cannot be assigned to (at byte 15)",
        ),
        (
            "nobody",
            "! ReferenceError: 'nobody' is not defined (at byte 0)",
        ),
        (
            "'use strict'; nobody = 1;",
            "! ReferenceError: 'nobody' is not defined (at byte 14)",
        ),
        // The three value properties of the global object are the only way to
        // write three of the language's own values, and they refuse to be
        // written over.
        ("undefined = 1; typeof undefined", "\"undefined\""),
        ("typeof globalThis", "\"object\""),
        ("var a = 1; globalThis.a", "1"),
    ]);
}

#[test]
fn assignment_and_the_operators_that_assign() {
    table(&[
        ("let a = 1; a = 2", "2"),
        ("let a = 1; a += 2", "3"),
        ("let a = 'a'; a += 1", "\"a1\""),
        ("let a = 8; a >>= 2", "2"),
        ("let a = 1; a **= 3", "1"),
        ("let a = 0; a ||= 5", "5"),
        ("let a = 1; a ||= 5", "1"),
        ("let a = null; a ??= 5", "5"),
        ("let a = 0; a ??= 5", "0"),
        ("let a = 1; a &&= 5", "5"),
        ("let a = 1; let b = 2; a = b = 3; a", "3"),
        ("let a = 1; a++", "1"),
        ("let a = 1; a++; a", "2"),
        ("let a = 1; ++a", "2"),
        ("let a = '1'; a++; a", "2"),
        ("let a = 1; a--", "1"),
        ("let a = {}; a.b = 1; a.b", "1"),
        ("let a = { b: 1 }; a.b += 2; a.b", "3"),
        ("let a = { b: 1 }; a.b++", "1"),
        ("let a = { b: 1 }; a.b++; a.b", "2"),
        ("let a = { b: 1 }; ++a.b", "2"),
        ("let a = { b: 1 }; a['b'] += 2; a.b", "3"),
        ("let a = { b: 1 }; a['b']++; a.b", "2"),
        ("let a = { b: 0 }; a.b ||= 7; a.b", "7"),
        ("let a = { b: 1 }; a.b ||= 7; a.b", "1"),
        ("let a = { b: null }; a['b'] ??= 7; a.b", "7"),
    ]);
}

#[test]
fn objects_and_their_properties() {
    table(&[
        ("({ a: 1 }).a", "1"),
        ("({ a: 1 })['a']", "1"),
        ("({}).a", "undefined"),
        ("({ 'a b': 1 })['a b']", "1"),
        ("({ 1: 'a' })[1]", "\"a\""),
        ("({ 1: 'a' })['1']", "\"a\""),
        ("let k = 'a'; ({ [k]: 1 }).a", "1"),
        ("let a = 1; ({ a }).a", "1"),
        ("'a' in { a: 1 }", "true"),
        ("'b' in { a: 1 }", "false"),
        ("let a = { b: { c: 1 } }; a.b.c", "1"),
        ("let a = { __proto__: { b: 1 } }; a.b", "1"),
        ("let a = { ['__proto__']: 1 }; a.__proto__", "1"),
        ("let a = { b: 1 }; delete a.b; 'b' in a", "false"),
        (
            "null.a",
            "! TypeError: cannot read property 'a' of null (at byte 0)",
        ),
        (
            "undefined.a",
            "! TypeError: cannot read property 'a' of undefined (at byte 0)",
        ),
        ("({ a: 1 })?.a", "1"),
        ("let a = null; a?.b", "undefined"),
        ("let a = null; a?.b.c.d", "undefined"),
        ("let a = { b: null }; a?.b?.c", "undefined"),
    ]);
}

#[test]
fn templates_ask_an_object_for_its_string() {
    table(&[
        ("`a`", "\"a\""),
        ("`a${1}b`", "\"a1b\""),
        ("`${1}${2}`", "\"12\""),
        ("let a = 'x'; `<${a}>`", "\"<x>\""),
        ("`${null}`", "\"null\""),
        (
            "`a\\nb`.length",
            "! a property of a primitive needs a wrapper object, which is queue item 73",
        ),
    ]);
}

#[test]
fn control_flow() {
    table(&[
        ("let a = 0; if (1) { a = 1; } else { a = 2; } a", "1"),
        ("let a = 0; if (0) { a = 1; } else { a = 2; } a", "2"),
        ("let a = 0; while (a < 3) { a++; } a", "3"),
        ("let a = 0; do { a++; } while (a < 3); a", "3"),
        ("let a = 0; do { a++; } while (false); a", "1"),
        ("let a = 0; for (let i = 0; i < 4; i++) { a += i; } a", "6"),
        ("let a = 0; for (;;) { a++; if (a > 2) break; } a", "3"),
        (
            "let a = 0; for (let i = 0; i < 5; i++) { if (i > 2) continue; a++; } a",
            "3",
        ),
        (
            "let a = 0; outer: for (let i = 0; i < 3; i++) { for (let j = 0; j < 3; j++) { if (j > i) continue outer; a++; } } a",
            "6",
        ),
        (
            "let a = 0; outer: for (let i = 0; i < 3; i++) { for (let j = 0; j < 3; j++) { a++; break outer; } } a",
            "1",
        ),
        ("let a = 0; here: { a = 1; break here; a = 2; } a", "1"),
        // A `let` in a loop body is in its dead zone again on every pass, which
        // is what makes this a `ReferenceError` rather than a value.
        (
            "let n = 0; while (n < 2) { n++; if (n === 2) { b; } let b = 1; } n",
            "! ReferenceError: 'b' is used before it is declared (at byte 47)",
        ),
    ]);
}

#[test]
fn a_switch_tests_in_order_and_falls_through() {
    table(&[
        (
            "let a = 0; switch (1) { case 1: a = 1; break; case 2: a = 2; } a",
            "1",
        ),
        (
            "let a = 0; switch (2) { case 1: a = 1; case 2: a = 2; case 3: a += 10; } a",
            "12",
        ),
        (
            "let a = 0; switch (9) { case 1: a = 1; break; default: a = 3; } a",
            "3",
        ),
        // The `default` is tested last however early it is written.
        (
            "let a = 0; switch (2) { default: a = 3; break; case 2: a = 2; } a",
            "2",
        ),
        // `===` rather than `==`, which is the one thing everybody gets wrong
        // about `switch`.
        (
            "let a = 0; switch ('1') { case 1: a = 1; break; default: a = 3; } a",
            "3",
        ),
        (
            "let a = 0; switch (1) { case 1: { let b = 5; a = b; } } a",
            "5",
        ),
    ]);
}

#[test]
fn what_the_language_says_and_this_engine_has_not_built() {
    // ADR 0013 § 3: absent beats approximate. Each of these names the queue
    // item that builds it rather than producing something plausible.
    for (source, item) in [
        ("f()", "209"),
        ("function f() {}", "209"),
        ("(() => 1)", "209"),
        ("class A {}", "209"),
        ("this", "209"),
        ("try { 1; } catch {}", "210"),
        ("[1]", "211"),
        ("let [a] = b;", "211"),
        ("for (const a in b) {}", "211"),
        ("for (const a of b) {}", "211"),
        ("/a/.test", "74"),
        ("1n", "207"),
        ("function* g() {}", "209"),
    ] {
        let answered = value(source);
        assert!(
            answered.starts_with('?') && answered.contains(item),
            "{source} should name queue item {item}: {answered}"
        );
    }
}

#[test]
fn an_object_with_no_prototype_cannot_become_a_primitive() {
    // Not a gap: an object in this engine has no `Object.prototype` (queue item
    // 73), so it has no `valueOf` and no `toString` to call — and a `TypeError`
    // is exactly what a real engine gives for `Object.create(null) + ""`.
    table(&[
        (
            "({}) + ''",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 0)",
        ),
        (
            "({}) == 1",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 0)",
        ),
        // These do not convert at all, so they answer.
        ("({}) === ({})", "false"),
        ("let a = {}; a === a", "true"),
        ("!{}", "false"),
        ("typeof {}", "\"object\""),
    ]);
}

#[test]
fn a_script_may_throw_and_nothing_catches_it_yet() {
    // `try`/`catch` is queue item 210. What a `throw` does today is end the
    // script, which is what an uncaught one does anyway.
    assert!(value("throw 1;").starts_with("! the script threw a value"));
    assert!(value("let a = 0; while (1) { a++; if (a > 2) throw 'stop'; }").contains("threw"));
}
