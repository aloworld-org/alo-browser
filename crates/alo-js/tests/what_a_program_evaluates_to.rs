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
        ("new f()", "212"),
        ("class A {}", "212"),
        ("function f(a = 1) {}", "213"),
        ("function f(...a) {}", "213"),
        ("function f(a, a) {}", "213"),
        ("function f() { return arguments; }", "213"),
        ("f`a`", "215"),
        ("try { 1; } catch {}", "210"),
        ("[1]", "211"),
        ("let [a] = b;", "211"),
        ("for (const a in b) {}", "211"),
        ("for (const a of b) {}", "211"),
        ("f(...a)", "211"),
        ("/a/.test", "74"),
        ("1n", "207"),
        ("function* g() {}", "75"),
        ("async function g() {}", "75"),
    ] {
        let answered = value(source);
        assert!(
            answered.starts_with('?') && answered.contains(item),
            "{source} should name queue item {item}: {answered}"
        );
    }
}

#[test]
fn calling_things() {
    table(&[
        // A function is a value, and calling it is what it is for.
        ("function f() { return 1; } f()", "1"),
        ("(function () { return 1; })()", "1"),
        ("(function () {})()", "undefined"),
        // A function declaration is readable above the line that declares it,
        // which a `let` is not — that is the whole of what hoisting means.
        ("let n = f(); function f() { return 2; } n", "2"),
        // Arguments, in order, with the missing and the spare ones both
        // specified rather than an error.
        ("function f(a, b) { return a - b; } f(5, 2)", "3"),
        ("function f(a, b) { return b; } f(1)", "undefined"),
        ("function f(a) { return a; } f(1, 2, 3)", "1"),
        // An arrow, with a body and with an expression.
        ("((a, b) => a + b)(1, 2)", "3"),
        ("((a) => { return a * 2; })(21)", "42"),
        ("(() => 1)()", "1"),
        // Recursion, which needs the name to be readable inside the body.
        (
            "function fact(n) { return n < 2 ? 1 : n * fact(n - 1); } fact(10)",
            "3628800",
        ),
        // A function expression can see itself under its own name, and that
        // name is nowhere else.
        (
            "let f = function me(n) { return n < 1 ? 0 : n + me(n - 1); }; f(3)",
            "6",
        ),
        ("typeof (function me() {})", "\"function\""),
        ("let f = function me() {}; typeof me", "\"undefined\""),
        // `typeof` is the one place a function is not just an object.
        ("typeof (function () {})", "\"function\""),
        ("typeof (() => 1)", "\"function\""),
        ("typeof {}", "\"object\""),
        // Calling what is not a function is the page's own mistake, and it is
        // the commonest one there is.
        (
            "let a = 1; a()",
            "! TypeError: 1 is not a function (at byte 11)",
        ),
        (
            "({}).nothing()",
            "! TypeError: undefined is not a function (at byte 0)",
        ),
    ]);
}

#[test]
fn a_function_body_has_its_own_names() {
    table(&[
        // A `var` in a function is the function's, not the global object's.
        (
            "function f() { var a = 1; return a; } f(); typeof a",
            "\"undefined\"",
        ),
        // And it is readable above its line, as `undefined`.
        (
            "function f() { let n = typeof a; var a = 1; return n; } f()",
            "\"undefined\"",
        ),
        // A `let` in a function body has a dead zone like anywhere else.
        (
            "function f() { return a; let a = 1; } f()",
            "! ReferenceError: 'a' is used before it is declared (at byte 22)",
        ),
        // A parameter and a `var` of the same name are one place.
        ("function f(a) { var a; return a; } f(7)", "7"),
        // A block inside a function still has its own slots.
        (
            "function f() { let a = 1; { let a = 2; } return a; } f()",
            "1",
        ),
        // A name nothing declares is still the realm's.
        ("var outside = 3; function f() { return outside; } f()", "3"),
        // A function declared inside a function is that function's.
        (
            "function f() { function g() { return 4; } return g(); } f(); typeof g",
            "\"undefined\"",
        ),
    ]);
}

#[test]
fn closures_keep_the_names_they_were_written_beside() {
    table(&[
        // The counter, which is the whole reason an environment outlives its
        // call: `n` is gone from the stack and still readable.
        (
            "function make() { var n = 0; return function () { n = n + 1; return n; }; } \
             let c = make(); c(); c(); c()",
            "3",
        ),
        // Two closures over two calls are two `n`s.
        (
            "function make() { var n = 0; return function () { n = n + 1; return n; }; } \
             let a = make(); let b = make(); a(); a(); b()",
            "1",
        ),
        // Two closures over **one** call share it.
        (
            "function make() { var n = 0; \
               return { up: function () { n = n + 1; return n; }, read: function () { return n; } }; } \
             let m = make(); m.up(); m.up(); m.read()",
            "2",
        ),
        // Two environments out, which is what `hops` counts.
        (
            "function a() { var x = 5; return function b() { return function c() { return x; }; }; } \
             a()()()",
            "5",
        ),
        // A parameter is a binding like any other, so it is captured too.
        (
            "function adder(by) { return function (n) { return n + by; }; } adder(3)(4)",
            "7",
        ),
    ]);
}

#[test]
fn this_is_decided_where_the_call_is_made_and_an_arrow_has_none() {
    table(&[
        // A method call passes the object it was reached through.
        (
            "let o = { x: 5, get: function () { return this.x; } }; o.get()",
            "5",
        ),
        ("let o = { x: 5, get() { return this.x; } }; o.get()", "5"),
        (
            "let o = { x: 5, get: function () { return this.x; } }; let f = o.get; f()",
            "undefined",
        ),
        // A computed member call passes it too.
        (
            "let o = { x: 6, m: function () { return this.x; } }; o['m']()",
            "6",
        ),
        // Sloppy code with no receiver gets the global object, and strict code
        // gets `undefined` — which is most of what `\"use strict\"` is for.
        ("function f() { return this === globalThis; } f()", "true"),
        (
            "'use strict'; function f() { return typeof this; } f()",
            "\"undefined\"",
        ),
        // An arrow has no `this` of its own, so it uses the one where it was
        // written — and that is what makes it usable inside a method.
        (
            "let o = { x: 7, m: function () { let g = () => this.x; return g(); } }; o.m()",
            "7",
        ),
        // Even when it is called as a method of something else.
        (
            "let o = { x: 8, m: function () { return () => this.x; } }; \
             let other = { x: 9, f: o.m() }; other.f()",
            "8",
        ),
        // A script's own `this` is the global object.
        ("this === globalThis", "true"),
    ]);
}

#[test]
fn an_optional_call_short_circuits_the_whole_chain() {
    table(&[
        ("let o = { m: function () { return 1; } }; o.m?.()", "1"),
        ("let o = {}; o.m?.()", "undefined"),
        ("let o = {}; o.m?.().a.b", "undefined"),
        ("let f = null; f?.()", "undefined"),
        ("let o = null; o?.m()", "undefined"),
        // The receiver is still the object it was reached through.
        (
            "let o = { x: 2, m: function () { return this.x; } }; o?.m()",
            "2",
        ),
    ]);
}

#[test]
fn an_accessor_is_a_call() {
    // Queue item 214's first two closing conditions. A getter and a setter are
    // the two places the language turns reading and writing a *property* into
    // running a page's own code, and every case here is a program that could
    // not run at all before.
    table(&[
        ("({ get a() { return 1; } }).a", "1"),
        // The setter sees what was assigned, and the assignment still evaluates
        // to that value rather than to what the setter answered.
        (
            "let seen; const o = { set a(v) { seen = v; } }; o.a = 5; seen",
            "5",
        ),
        ("const o = { set a(v) { return 9; } }; o.a = 5", "5"),
        // Both halves of one name are one property, which is what makes the
        // second definition complete the first rather than replace it.
        (
            "let n = 0; const o = { get a() { return n; }, set a(v) { n = v + 1; } }; o.a = 1; o.a",
            "2",
        ),
        (
            "let n = 0; const o = { set a(v) { n = v + 1; }, get a() { return n; } }; o.a = 1; o.a",
            "2",
        ),
        // A half that is missing is not an error: reading a set-only property
        // is `undefined`, and writing a get-only one is sloppy mode's silence.
        ("({ set a(v) {} }).a", "undefined"),
        ("const o = { get a() { return 1; } }; o.a = 2; o.a", "1"),
        (
            "'use strict'; const o = { get a() { return 1; } }; o.a = 2;",
            "! TypeError: property 'a' has a getter and no setter, so it cannot be written (at byte 51)",
        ),
        // `this` is the object it was reached through, which is the whole
        // reason an accessor is not a value in a property.
        ("({ x: 3, get a() { return this.x; } }).a", "3"),
        // Inherited, which is where a getter differs from a data property most:
        // it runs with the *child* as its `this`.
        (
            "const p = { get a() { return this.x; } }; ({ __proto__: p, x: 4 }).a",
            "4",
        ),
        (
            "let seen; const p = { set a(v) { seen = v; } }; const o = { __proto__: p }; o.a = 6; seen",
            "6",
        ),
        // A prototype's setter is called rather than shadowed, so the child has
        // no own property afterwards.
        (
            "const p = { set a(v) {} }; const o = { __proto__: p }; o.a = 6; o.a",
            "undefined",
        ),
        // Every spelling of a key reaches the same property.
        ("const o = { get a() { return 1; } }; o['a']", "1"),
        (
            "const k = 'a'; const o = { get [k]() { return 2; } }; o.a",
            "2",
        ),
        ("const o = { get 1() { return 'i'; } }; o[1]", "\"i\""),
        ("const o = { get 'a b'() { return 3; } }; o['a b']", "3"),
        // Reading and writing in one expression, which is two calls.
        (
            "let n = 1; const o = { get a() { return n; }, set a(v) { n = v; } }; o.a += 4; n",
            "5",
        ),
        (
            "let n = 1; const o = { get a() { return n; }, set a(v) { n = v; } }; o.a++; n",
            "2",
        ),
        (
            "let n = 1; const o = { get a() { return n; }, set a(v) { n = v; } }; o.a++",
            "1",
        ),
        // What a getter answers can be called, and it keeps the receiver the
        // *call* was written with rather than the getter's.
        (
            "const o = { x: 8, get f() { return function () { return this.x; }; } }; o.f()",
            "8",
        ),
        // An accessor is an ordinary property in every other respect.
        (
            "const o = { get a() { return 1; } }; delete o.a; o.a",
            "undefined",
        ),
        ("const o = { get a() { return 1; } }; 'a' in o", "true"),
        ("typeof ({ get a() { return 1; } }).a", "\"number\""),
        // A getter that throws throws out of the read.
        (
            "({ get a() { throw 'no'; } }).a",
            "! the script threw a value (at byte 13)",
        ),
    ]);
}

#[test]
fn an_object_becomes_a_primitive_by_being_asked() {
    // Queue item 214's third closing condition. `ToPrimitive` is what every
    // operator is written in terms of, so this is the same re-entry as a getter
    // seen from the other side: the instruction is half way through.
    table(&[
        ("({ valueOf() { return 2; } }) + 1", "3"),
        ("({ toString() { return 'x'; } }) + 'y'", "\"xy\""),
        ("+{ valueOf() { return '3'; } }", "3"),
        ("-{ valueOf() { return 3; } }", "-3"),
        ("~{ valueOf() { return 5; } }", "-6"),
        ("({ valueOf() { return 2; } }) * 3", "6"),
        ("({ valueOf() { return 1; } }) == 1", "true"),
        ("({ valueOf() { return 2; } }) < 3", "true"),
        // Both sides, which is three runs of one instruction and one call each.
        (
            "({ valueOf() { return 2; } }) + ({ valueOf() { return 3; } })",
            "5",
        ),
        // `valueOf` first for `+`, `toString` first for a template — which is
        // the difference `Op::ToText` exists for, and a page can see it.
        (
            "'' + { toString() { return 't'; }, valueOf() { return 'v'; } }",
            "\"v\"",
        ),
        (
            "`${ { toString() { return 't'; }, valueOf() { return 'v'; } } }`",
            "\"t\"",
        ),
        // A method that answers with an object has converted nothing, so the
        // *other* name is tried — and when both do, it is a `TypeError`.
        (
            "({ valueOf() { return {}; }, toString() { return 'z'; } }) + ''",
            "\"z\"",
        ),
        (
            "({ valueOf() { return {}; }, toString() { return {}; } }) + ''",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 0)",
        ),
        // A name that is there and is not callable is skipped rather than
        // thrown at, which is the specification's `IsCallable` check.
        ("({ valueOf: 1, toString() { return 'q'; } }) + ''", "\"q\""),
        // `valueOf` may itself be behind a getter, so finding the method is a
        // call before calling it is.
        (
            "({ get valueOf() { return function () { return 5; }; } }) + 1",
            "6",
        ),
        // The order is left then right, and a `valueOf` with a side effect can
        // see it.
        (
            "let log = ''; const a = { valueOf() { log += 'a'; return 1; } }; const b = { valueOf() { log += 'b'; return 2; } }; a + b; log",
            "\"ab\"",
        ),
        (
            "let log = ''; const a = { valueOf() { log += 'a'; return 1; } }; const b = { valueOf() { log += 'b'; return 2; } }; a <= b; log",
            "\"ab\"",
        ),
        // A property key is a conversion too.
        (
            "const o = {}; o[{ toString() { return 'k'; } }] = 1; o.k",
            "1",
        ),
        (
            "const o = { get [{ toString() { return 'k'; } }]() { return 2; } }; o.k",
            "2",
        ),
        // And an inherited `toString` is found by the walk, which is what
        // `({}) + ''` will use once there is an `Object.prototype` to inherit
        // it from (queue item 73).
        (
            "({ __proto__: { toString() { return 'p'; } } }) + ''",
            "\"p\"",
        ),
    ]);
}

#[test]
fn instanceof_answers_the_two_questions_it_can() {
    // Not this item's work and it is in this item's file, because item 209 made
    // the old answer a lie: `instanceof` refused everything as *not callable*
    // on the grounds that nothing in the heap was, and now things are.
    table(&[
        // The right-hand side has to be an object, and then it has to be
        // callable — both `TypeError`s the language specifies.
        (
            "1 instanceof 2",
            "! TypeError: the right-hand side of 'instanceof' must be an object (at byte 0)",
        ),
        (
            "1 instanceof {}",
            "! TypeError: the right-hand side of 'instanceof' is not callable (at byte 0)",
        ),
        // A primitive is not an instance of anything, and the specification
        // answers that *before* it reads `prototype`.
        ("function f() {} 1 instanceof f", "false"),
        ("function f() {} 'a' instanceof f", "false"),
        ("function f() {} null instanceof f", "false"),
        // What is left needs the `prototype` a constructor has, which is queue
        // item 212 — and saying so is not the same as answering `false`.
        (
            "function f() {} ({}) instanceof f",
            "! 'instanceof' needs the `prototype` property a constructor has, which is queue item 212",
        ),
    ]);
}

#[test]
fn an_object_with_no_prototype_cannot_become_a_primitive() {
    // Not a gap: an object in this engine has no `Object.prototype` (queue item
    // 73), so it has no `valueOf` and no `toString` to *find* — and a
    // `TypeError` is exactly what a real engine gives for
    // `Object.create(null) + ""`. Calling one it does have is queue item 214
    // and is the test above.
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
