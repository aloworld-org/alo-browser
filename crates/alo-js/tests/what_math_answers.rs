/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 365's closing conditions, as programs and what they evaluate to.
//!
//! *Each function answers ECMA-262's values for the edge cases — `-0`, `NaN`,
//! the infinities, no arguments — under `Heap::stress`.*
//!
//! A `-0` is told from a `+0` by dividing one by it, since both are written
//! `0`. Every program runs twice, once with the collector running at every
//! allocation ([`Heap::stress`]): an argument may be an object whose
//! `valueOf` is script, and what each function kept across it must survive.
//!
//! The transcendental functions are *implementation-approximated*, so an
//! ordinary value of one is checked only where the answer is exact on every
//! correct library (`Math.log2(1024)`, `Math.sqrt(9)`) or by a relation
//! rather than by its last digit.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use alo_js::interpret::{Engine, Trouble};
use alo_js::numeric;
use alo_js::object::Value;
use alo_js::script;

/// Run a program both ways, and answer what it produced.
fn both(source: &str) -> String {
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
        assert_eq!(&both(source), expected, "{source}");
    }
}

/// A function that makes an object whose `valueOf` writes its number down.
const COUNTED: &str = "var log = ''; \
                       function o(n) { return { valueOf: function () { log += n; return n; } }; }";

#[test]
fn math_is_an_ordinary_object_on_the_global_object() {
    table(&[
        ("typeof Math", "\"object\""),
        ("Math === globalThis.Math", "true"),
        ("({}).__proto__.isPrototypeOf(Math)", "true"),
        ("Math.__proto__ === ({}).__proto__", "true"),
        // Its tag is what `Object.prototype.toString` names it by.
        ("'' + Math", "\"[object Math]\""),
        // Writable and configurable, and not enumerable.
        ("globalThis.propertyIsEnumerable('Math')", "false"),
        ("Math = 1; Math", "1"),
        ("delete globalThis.Math; typeof Math", "\"undefined\""),
        // It is neither a function nor a constructor, and nor are its
        // functions.
        ("try { Math(); } catch (e) { e.name }", "\"TypeError\""),
        (
            "try { new Math.max(1); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        ("typeof Math.max", "\"function\""),
        // A function of it reads nothing of `this`.
        ("var max = Math.max; max(1, 2)", "2"),
    ]);
}

#[test]
fn its_values_are_the_specifications_and_fixed() {
    table(&[
        ("Math.E", "2.718281828459045"),
        ("Math.LN10", "2.302585092994046"),
        ("Math.LN2", "0.6931471805599453"),
        ("Math.LOG10E", "0.4342944819032518"),
        ("Math.LOG2E", "1.4426950408889634"),
        ("Math.PI", "3.141592653589793"),
        ("Math.SQRT1_2", "0.7071067811865476"),
        ("Math.SQRT2", "1.4142135623730951"),
        // Not writable, not configurable, not enumerable.
        ("Math.PI = 3; Math.PI", "3.141592653589793"),
        (
            "'use strict'; try { Math.PI = 3; } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        ("delete Math.PI", "false"),
        ("Math.propertyIsEnumerable('PI')", "false"),
        ("Math.hasOwnProperty('SQRT2')", "true"),
        ("Math.propertyIsEnumerable('max')", "false"),
    ]);
}

#[test]
fn what_is_not_built_is_absent_rather_than_approximate() {
    table(&[
        // Queue item 367: a source of randomness is a decision.
        ("typeof Math.random", "\"undefined\""),
        ("Math.hasOwnProperty('random')", "false"),
        // Queue item 368.
        ("typeof Math.f16round", "\"undefined\""),
        ("typeof Math.sumPrecise", "\"undefined\""),
    ]);
}

#[test]
fn max_and_min_order_the_zeros_and_let_nan_win() {
    table(&[
        ("Math.max()", "-Infinity"),
        ("Math.min()", "Infinity"),
        ("Math.max(1, 3, 2)", "3"),
        ("Math.min(1, 3, 2)", "1"),
        ("Math.max(-Infinity, -1e308)", "-1e+308"),
        ("1 / Math.max(-0, 0)", "Infinity"),
        ("1 / Math.max(0, -0)", "Infinity"),
        ("1 / Math.max(-0)", "-Infinity"),
        ("1 / Math.max(-0, -0)", "-Infinity"),
        ("1 / Math.min(0, -0)", "-Infinity"),
        ("1 / Math.min(-0, 0)", "-Infinity"),
        ("1 / Math.min(0)", "Infinity"),
        ("Math.max(1, NaN, 3)", "NaN"),
        ("Math.max(NaN, Infinity)", "NaN"),
        ("Math.min(-Infinity, NaN)", "NaN"),
        ("Math.max(undefined)", "NaN"),
        ("Math.max(null, -1)", "0"),
        ("Math.max('7', true)", "7"),
        ("Math.min('', '-2')", "-2"),
        ("Math.max('x')", "NaN"),
    ]);
}

#[test]
fn hypot_lets_an_infinity_beat_a_nan_and_does_not_overflow() {
    table(&[
        ("Math.hypot()", "0"),
        ("Math.hypot(3, 4)", "5"),
        ("Math.hypot(-3)", "3"),
        ("1 / Math.hypot(-0)", "Infinity"),
        ("1 / Math.hypot(-0, -0)", "Infinity"),
        ("Math.hypot(NaN, Infinity)", "Infinity"),
        ("Math.hypot(-Infinity, NaN)", "Infinity"),
        ("Math.hypot(NaN, 1)", "NaN"),
        ("Math.hypot(1, undefined)", "NaN"),
        (
            "var h = Math.hypot(1e200, 1e200); h > 1.4e200 && h < 1.5e200",
            "true",
        ),
        ("Math.hypot(1e-200, 1e-200) > 0", "true"),
    ]);
}

#[test]
fn every_argument_is_converted_once_and_in_order_even_after_a_nan() {
    let cases = [
        (
            "var r = Math.max(o(1), NaN, o(3), o(2)); log + ':' + r",
            "\"132:NaN\"",
        ),
        ("var r = Math.min(o(5), o(2)); log + ':' + r", "\"52:2\""),
        ("var r = Math.hypot(o(3), o(4)); log + ':' + r", "\"34:5\""),
        (
            "var r = Math.pow(o(2), o(10)); log + ':' + r",
            "\"210:1024\"",
        ),
        (
            "var r = Math.atan2(o(1), o(0)); log + ':' + (r === Math.PI / 2)",
            "\"10:true\"",
        ),
        (
            "var r = Math.imul(o(3), o(-4)); log + ':' + r",
            "\"3-4:-12\"",
        ),
        ("var r = Math.round(o(2.5)); log + ':' + r", "\"2.5:3\""),
        // Arguments past those a function reads are never converted.
        ("var r = Math.abs(o(-1), o(9)); log + ':' + r", "\"-1:1\""),
        // A throw ends the call at the argument that threw.
        (
            "try { Math.max(o(1), { valueOf: function () { throw new TypeError('no'); } }, o(3)); } \
             catch (e) { log += '!' + e.message; } log",
            "\"1!no\"",
        ),
        (
            "try { Math.pow(o(1), { valueOf: function () { throw 'x'; } }); } \
             catch (e) { log += '!' + e; } log",
            "\"1!x\"",
        ),
        // `valueOf` before `toString`, because the hint is number.
        (
            "Math.abs({ valueOf: function () { return -4; }, toString: function () { return '9'; } })",
            "4",
        ),
        ("Math.abs({ toString: function () { return '-4'; } })", "4"),
    ];
    for (source, expected) in cases {
        assert_eq!(both(&format!("{COUNTED} {source}")), expected, "{source}");
    }
}

#[test]
fn a_fold_over_many_arguments_keeps_its_place_across_every_conversion() {
    // 2 000 arguments, every seventh an object whose `valueOf` is script, so
    // the fold suspends and resumes far past any index a step could hold.
    let mut arguments = Vec::new();
    for index in 0..2_000_u32 {
        if index % 7 == 0 {
            arguments.push(format!("o({index})"));
        } else {
            arguments.push(format!("{}", 2_000 - index));
        }
    }
    let list = arguments.join(", ");
    // Each conversion is counted and checked to come after the one before it.
    let counted = "var count = 0; var last = -1; var ordered = true; \
                   function o(n) { return { valueOf: function () { \
                   if (n <= last) { ordered = false; } last = n; count += 1; return n; } }; }";
    assert_eq!(
        both(&format!(
            "{counted} var r = Math.max({list}); count + ':' + ordered + ':' + r"
        )),
        // Objects at 0, 7, … 1995: 286 of them, converted in order; the
        // largest argument is the 1 999 at index 1.
        "\"286:true:1999\""
    );
    assert_eq!(
        both(&format!("{counted} Math.min({list})")),
        "0",
        "the object at index 0 is the smallest"
    );
}

#[test]
fn the_rounding_functions_keep_the_sign_of_a_zero() {
    table(&[
        ("Math.round(2.5)", "3"),
        ("Math.round(-2.5)", "-2"),
        ("Math.round(2.4)", "2"),
        ("1 / Math.round(-0.5)", "-Infinity"),
        ("1 / Math.round(-0.2)", "-Infinity"),
        ("1 / Math.round(0.2)", "Infinity"),
        ("1 / Math.round(-0)", "-Infinity"),
        ("Math.round(0.49999999999999994)", "0"),
        ("Math.round(-0.5000000000000001)", "-1"),
        ("Math.round(4503599627370497)", "4503599627370497"),
        ("Math.round(NaN)", "NaN"),
        ("Math.round(-Infinity)", "-Infinity"),
        ("Math.round()", "NaN"),
        ("Math.round('2.5')", "3"),
        ("1 / Math.ceil(-0.5)", "-Infinity"),
        ("Math.ceil(1.1)", "2"),
        ("Math.ceil(-Infinity)", "-Infinity"),
        ("Math.floor(-0.5)", "-1"),
        ("1 / Math.floor(-0)", "-Infinity"),
        ("1 / Math.floor(0.5)", "Infinity"),
        ("1 / Math.trunc(-0.9)", "-Infinity"),
        ("Math.trunc(4.7)", "4"),
        ("Math.trunc(-Infinity)", "-Infinity"),
        ("Math.trunc()", "NaN"),
        ("Math.fround(5.5)", "5.5"),
        ("Math.fround(5.05)", "5.050000190734863"),
        ("Math.fround(1.0000000596046448)", "1"),
        ("Math.fround(2 ** 128)", "Infinity"),
        ("Math.fround(NaN)", "NaN"),
        ("1 / Math.fround(-0)", "-Infinity"),
    ]);
}

#[test]
fn sign_abs_and_the_integer_functions() {
    table(&[
        ("Math.sign(-3)", "-1"),
        ("Math.sign(3)", "1"),
        ("Math.sign(5e-324)", "1"),
        ("1 / Math.sign(-0)", "-Infinity"),
        ("1 / Math.sign(0)", "Infinity"),
        ("Math.sign(NaN)", "NaN"),
        ("Math.sign(-Infinity)", "-1"),
        ("Math.abs(-Infinity)", "Infinity"),
        ("1 / Math.abs(-0)", "Infinity"),
        ("Math.abs()", "NaN"),
        ("Math.abs('-2')", "2"),
        ("Math.abs(null)", "0"),
        ("Math.clz32(1)", "31"),
        ("Math.clz32(0)", "32"),
        ("Math.clz32()", "32"),
        ("Math.clz32(-1)", "0"),
        ("Math.clz32(0.5)", "32"),
        ("Math.clz32(2 ** 32)", "32"),
        ("Math.clz32(2 ** 32 + 1)", "31"),
        ("Math.clz32(NaN)", "32"),
        ("Math.clz32(Infinity)", "32"),
        ("Math.imul(2, 4)", "8"),
        ("Math.imul(-1, 8)", "-8"),
        ("Math.imul(0xffffffff, 5)", "-5"),
        ("Math.imul(0x7fffffff, 2)", "-2"),
        ("Math.imul(2 ** 32 + 3, 2)", "6"),
        ("Math.imul()", "0"),
        ("Math.imul(NaN, 3)", "0"),
    ]);
}

#[test]
fn the_powers_and_logarithms_answer_their_edge_cases() {
    table(&[
        ("Math.pow(2, 10)", "1024"),
        ("Math.pow(-1, Infinity)", "NaN"),
        ("Math.pow(1, NaN)", "NaN"),
        ("Math.pow(NaN, 0)", "1"),
        ("Math.pow(NaN, -0)", "1"),
        ("Math.pow(-0, -1)", "-Infinity"),
        ("Math.pow(0, -1)", "Infinity"),
        ("Math.pow(-8, 1 / 3)", "NaN"),
        ("Math.pow(2)", "NaN"),
        ("Math.pow(2, 0.5) === Math.SQRT2", "true"),
        ("Math.sqrt(9)", "3"),
        ("Math.sqrt(-1)", "NaN"),
        ("1 / Math.sqrt(-0)", "-Infinity"),
        ("Math.sqrt(Infinity)", "Infinity"),
        ("Math.cbrt(-8)", "-2"),
        ("1 / Math.cbrt(-0)", "-Infinity"),
        ("Math.cbrt(-Infinity)", "-Infinity"),
        ("Math.exp(0)", "1"),
        ("Math.exp(-Infinity)", "0"),
        ("Math.exp(Infinity)", "Infinity"),
        ("Math.exp(NaN)", "NaN"),
        ("1 / Math.expm1(-0)", "-Infinity"),
        ("Math.expm1(-Infinity)", "-1"),
        ("Math.expm1(Infinity)", "Infinity"),
        ("Math.log(-1)", "NaN"),
        ("Math.log(0)", "-Infinity"),
        ("Math.log(-0)", "-Infinity"),
        ("1 / Math.log(1)", "Infinity"),
        ("Math.log(Infinity)", "Infinity"),
        ("Math.log1p(-1)", "-Infinity"),
        ("Math.log1p(-2)", "NaN"),
        ("1 / Math.log1p(-0)", "-Infinity"),
        ("Math.log10(0)", "-Infinity"),
        ("Math.log10(-1)", "NaN"),
        ("1 / Math.log10(1)", "Infinity"),
        ("Math.log2(1024)", "10"),
        ("Math.log2(-0)", "-Infinity"),
        ("Math.log2(Infinity)", "Infinity"),
    ]);
}

#[test]
fn the_trigonometric_functions_answer_their_edge_cases() {
    table(&[
        ("1 / Math.sin(-0)", "-Infinity"),
        ("Math.sin(Infinity)", "NaN"),
        ("Math.cos(0)", "1"),
        ("Math.cos(-Infinity)", "NaN"),
        ("1 / Math.tan(-0)", "-Infinity"),
        ("Math.tan(NaN)", "NaN"),
        ("Math.acos(2)", "NaN"),
        ("1 / Math.acos(1)", "Infinity"),
        ("1 / Math.asin(-0)", "-Infinity"),
        ("Math.asin(1.5)", "NaN"),
        ("1 / Math.atan(-0)", "-Infinity"),
        ("Math.atan(Infinity) === Math.PI / 2", "true"),
        ("Math.atan(-Infinity) === -Math.PI / 2", "true"),
        ("1 / Math.atan2(0, 0)", "Infinity"),
        ("1 / Math.atan2(-0, 0)", "-Infinity"),
        ("1 / Math.atan2(-0, 1)", "-Infinity"),
        ("Math.atan2(0, -0) === Math.PI", "true"),
        ("Math.atan2(-0, -0) === -Math.PI", "true"),
        ("Math.atan2(1, 0) === Math.PI / 2", "true"),
        ("Math.atan2(Infinity, Infinity) === Math.PI / 4", "true"),
        ("Math.atan2(1, -Infinity) === Math.PI", "true"),
        ("Math.atan2(NaN, 1)", "NaN"),
        ("Math.atan2(1)", "NaN"),
    ]);
}

#[test]
fn the_hyperbolic_functions_answer_their_edge_cases() {
    table(&[
        ("1 / Math.sinh(-0)", "-Infinity"),
        ("Math.sinh(-Infinity)", "-Infinity"),
        ("Math.cosh(0)", "1"),
        ("Math.cosh(-Infinity)", "Infinity"),
        ("1 / Math.tanh(-0)", "-Infinity"),
        ("Math.tanh(Infinity)", "1"),
        ("Math.tanh(-Infinity)", "-1"),
        ("Math.acosh(0.5)", "NaN"),
        ("1 / Math.acosh(1)", "Infinity"),
        ("Math.acosh(Infinity)", "Infinity"),
        ("Math.acosh(1e308) < 710", "true"),
        ("1 / Math.asinh(-0)", "-Infinity"),
        ("Math.asinh(-Infinity)", "-Infinity"),
        ("Math.asinh(-1e308) > -710", "true"),
        ("Math.atanh(1)", "Infinity"),
        ("Math.atanh(-1)", "-Infinity"),
        ("1 / Math.atanh(-0)", "-Infinity"),
        ("Math.atanh(2)", "NaN"),
    ]);
}
