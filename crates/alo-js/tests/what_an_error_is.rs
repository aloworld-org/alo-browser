/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 227: *`Error` and the six native errors* — the constructors,
//! their prototypes, `Error.prototype.toString`, and the first builtin with a
//! `[[Construct]]`. Cut from item 73.
//!
//! # What opened it
//!
//! A real page's own script, at one remove. The frozen service worker
//! (`crates/alo-corpus/scripts/alo-service-worker/`) was refused at byte 2853,
//! the `try` of its push handler, which is item 210 — and item 210 waited on
//! the `Error` objects a `catch` binds, which are these. Item 210 is built
//! now, and `what_a_catch_catches.rs` is where a caught error is tested.
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with the collector firing at every allocation, and
//! the two must agree. An error constructor may run the page's script twice —
//! a `toString` on its message, a getter for its `cause` — and the instance it
//! is filling in is held only by the `this` slot while it does.

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

const FAMILIES: [&str; 7] = [
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
];

#[test]
fn each_of_the_seven_is_a_constructor_on_the_global_object() {
    for name in FAMILIES {
        table(&[
            (&format!("typeof {name}"), "\"function\""),
            (&format!("typeof new {name}()"), "\"object\""),
            (&format!("new {name}('x').message"), "\"x\""),
            (&format!("'' + new {name}('x')"), &format!("\"{name}: x\"")),
            (&format!("{name}.prototype.name"), &format!("\"{name}\"")),
            (&format!("{name}.prototype.message"), "\"\""),
            (&format!("{name}.prototype.constructor === {name}"), "true"),
            (&format!("new {name}().constructor === {name}"), "true"),
            (
                &format!("new {name}().__proto__ === {name}.prototype"),
                "true",
            ),
            // Bound like every constructor on the global object: there, but not
            // listed by a `for…in` over it.
            (
                &format!("globalThis.propertyIsEnumerable('{name}')"),
                "false",
            ),
            (&format!("globalThis.hasOwnProperty('{name}')"), "true"),
        ]);
    }
}

#[test]
fn called_and_constructed_are_the_same_object() {
    table(&[
        ("TypeError('x').message", "\"x\""),
        ("TypeError('x').__proto__ === TypeError.prototype", "true"),
        ("({}).toString.call(TypeError('x'))", "\"[object Error]\""),
        // Reached through a builtin that asked for the call, rather than by a
        // call the source spells: the instance is still made.
        ("TypeError.call({}, 'y').message", "\"y\""),
        (
            "TypeError.call({}, 'y').__proto__ === TypeError.prototype",
            "true",
        ),
        // Each call is a different error.
        ("Error() !== Error()", "true"),
    ]);
}

#[test]
fn the_six_inherit_from_error_and_their_constructors_from_error_itself() {
    table(&[
        ("TypeError.prototype.__proto__ === Error.prototype", "true"),
        ("URIError.prototype.__proto__ === Error.prototype", "true"),
        ("TypeError.__proto__ === Error", "true"),
        ("EvalError.__proto__ === Error", "true"),
        ("Error.__proto__ === (function () {}).__proto__", "true"),
        ("Error.prototype.__proto__ === ({}).__proto__", "true"),
        // `toString` is on `Error.prototype` once, and the six find it there.
        ("Error.prototype.hasOwnProperty('toString')", "true"),
        ("TypeError.prototype.hasOwnProperty('toString')", "false"),
        (
            "TypeError.prototype.toString === Error.prototype.toString",
            "true",
        ),
        // A prototype is an ordinary object, not an error.
        ("({}).toString.call(Error.prototype)", "\"[object Object]\""),
        (
            "({}).toString.call(TypeError.prototype)",
            "\"[object Object]\"",
        ),
        ("({}).toString.call(new RangeError())", "\"[object Error]\""),
    ]);
}

#[test]
fn a_message_is_converted_and_own_and_not_enumerable() {
    table(&[
        ("new Error().hasOwnProperty('message')", "false"),
        ("new Error(undefined).hasOwnProperty('message')", "false"),
        ("new Error('').hasOwnProperty('message')", "true"),
        ("new Error(1).message", "\"1\""),
        ("new Error(null).message", "\"null\""),
        ("new Error(true).message", "\"true\""),
        ("new Error(1.5e300).message", "\"1.5e+300\""),
        ("new Error('x').propertyIsEnumerable('message')", "false"),
        // An object is `ToString`ed, which tries `toString` before `valueOf`.
        (
            "new Error({ toString() { return 'mine'; } }).message",
            "\"mine\"",
        ),
        (
            "new Error({ valueOf() { return 1; }, toString() { return 2; } }).message",
            "\"2\"",
        ),
        (
            "new Error({ toString() { return {}; }, valueOf() { return 3; } }).message",
            "\"3\"",
        ),
        ("new Error([]).message", "\"[object Array]\""),
        // Writable and configurable, so a page may change or remove it.
        (
            "let e = new Error('a'); e.message = 'b'; e.message",
            "\"b\"",
        ),
        (
            "let e = new Error('a'); delete e.message; e.message === Error.prototype.message",
            "true",
        ),
    ]);
}

#[test]
fn a_cause_is_given_only_when_options_has_one() {
    table(&[
        ("new Error('x', { cause: 1 }).cause", "1"),
        ("'cause' in new Error('x', {})", "false"),
        ("'cause' in new Error('x')", "false"),
        ("'cause' in new Error('x', 1)", "false"),
        ("'cause' in new Error('x', { cause: undefined })", "true"),
        (
            "new Error('x', { cause: undefined }).hasOwnProperty('cause')",
            "true",
        ),
        // `HasProperty`, so an inherited cause counts.
        ("new Error('x', { __proto__: { cause: 2 } }).cause", "2"),
        (
            "new Error('x', { cause: 1 }).propertyIsEnumerable('cause')",
            "false",
        ),
        // A cause is any value and is not converted.
        (
            "let o = {}; new Error('x', { cause: o }).cause === o",
            "true",
        ),
        // A getter runs once, with the options as its `this`.
        (
            "let n = 0; let e = new Error('x', { get cause() { n = n + 1; return 3; } }); n + e.cause",
            "4",
        ),
        (
            "let o = { get cause() { return this; } }; new Error('x', o).cause === o",
            "true",
        ),
        (
            "new Error('x', { get cause() { return undefined; } }).hasOwnProperty('cause')",
            "true",
        ),
    ]);
}

#[test]
fn the_message_is_converted_before_the_cause_is_read() {
    table(&[(
        "let log = ''; \
         new Error({ toString() { log = log + 'm'; return ''; } }, \
                   { get cause() { log = log + 'c'; } }); \
         log",
        "\"mc\"",
    )]);
}

#[test]
fn error_prototype_to_string_answers_what_the_specification_says() {
    table(&[
        ("'' + new Error()", "\"Error\""),
        ("'' + new Error('')", "\"Error\""),
        ("'' + new TypeError('bad')", "\"TypeError: bad\""),
        (
            "Error.prototype.toString.call({ message: 'm' })",
            "\"Error: m\"",
        ),
        (
            "Error.prototype.toString.call({ name: '', message: 'm' })",
            "\"m\"",
        ),
        (
            "Error.prototype.toString.call({ name: 'N', message: '' })",
            "\"N\"",
        ),
        (
            "Error.prototype.toString.call({ name: '', message: '' })",
            "\"\"",
        ),
        ("Error.prototype.toString.call({})", "\"Error\""),
        (
            "Error.prototype.toString.call({ name: undefined, message: undefined })",
            "\"Error\"",
        ),
        (
            "Error.prototype.toString.call({ name: 1, message: 2 })",
            "\"1: 2\"",
        ),
        // `name` may be a getter, and may be an object that converts.
        (
            "Error.prototype.toString.call({ get name() { return 'N'; }, message: 'm' })",
            "\"N: m\"",
        ),
        (
            "Error.prototype.toString.call({ name: { toString() { return 'Q'; } } })",
            "\"Q\"",
        ),
        (
            "let n = 0; Error.prototype.toString.call({ get name() { n = n + 1; return 'N'; } }); n",
            "1",
        ),
        // A page's own `name` and a page's own `toString` both win.
        (
            "let e = new Error('m'); e.name = 'Mine'; '' + e",
            "\"Mine: m\"",
        ),
        (
            "let e = new Error('m'); e.toString = function () { return 'own'; }; '' + e",
            "\"own\"",
        ),
    ]);
}

#[test]
fn to_string_on_something_that_is_not_an_object_is_a_type_error() {
    for this in ["1", "undefined", "null", "'s'", "true"] {
        let answered = value(&format!("Error.prototype.toString.call({this})"));
        assert!(
            answered.starts_with("! TypeError"),
            "{this} as this: {answered}"
        );
    }
}

#[test]
fn a_constructors_prototype_is_fixed() {
    table(&[
        (
            "let p = Error.prototype; Error.prototype = 1; Error.prototype === p",
            "true",
        ),
        ("delete TypeError.prototype", "false"),
        ("Error.propertyIsEnumerable('prototype')", "false"),
    ]);
    let answered = value("'use strict'; Error.prototype = 1;");
    assert!(answered.starts_with("! TypeError"), "{answered}");
}

#[test]
fn a_global_a_page_replaces_or_deletes_leaves_the_intrinsic_alone() {
    table(&[
        (
            "delete globalThis.TypeError; typeof TypeError",
            "\"undefined\"",
        ),
        ("TypeError = 1; TypeError", "1"),
        (
            "let T = TypeError; delete globalThis.TypeError; new T('x').message",
            "\"x\"",
        ),
        (
            "let T = TypeError; TypeError = null; '' + new T('x')",
            "\"TypeError: x\"",
        ),
    ]);
}

#[test]
fn only_a_builtin_made_as_a_constructor_constructs() {
    for source in [
        "new (({}).toString)()",
        "new (({}).hasOwnProperty)('a')",
        "new (Error.prototype.toString)()",
    ] {
        let answered = value(source);
        assert!(
            answered.contains("is not a constructor"),
            "{source}: {answered}"
        );
    }
}

#[test]
fn an_error_the_engine_throws_is_still_the_engines_until_something_catches_it() {
    // Item 210 turns these into instances of the constructors above; until a
    // `catch` exists nothing can see the difference, and the escape is
    // unchanged.
    let answered = value("null.a");
    assert!(answered.starts_with("! TypeError:"), "{answered}");
    let answered = value("nothing");
    assert!(answered.starts_with("! ReferenceError:"), "{answered}");
}

#[test]
fn what_is_not_built_is_refused_by_name() {
    for source in [
        "Error.prototype.toString.call({ get message() { return 'x'; } })",
        "Error.prototype.toString.call({ message: { toString() { return 'x'; } } })",
        "let e = new Error(); e.message = {}; '' + e",
    ] {
        let answered = value(source);
        assert!(answered.contains("228"), "{source}: {answered}");
    }
    let answered = value("typeof AggregateError");
    assert_eq!(answered, "\"undefined\"", "AggregateError is item 229");
}

#[test]
fn a_conversion_or_a_getter_that_throws_ends_the_construction() {
    for source in [
        "new Error({ toString() { throw 7; } })",
        "new Error('x', { get cause() { throw 7; } })",
        "Error.prototype.toString.call({ get name() { throw 7; } })",
    ] {
        let answered = value(source);
        assert!(answered.contains("threw a value"), "{source}: {answered}");
    }
    let answered = value("new Error({ toString() { return {}; }, valueOf() { return {}; } })");
    assert!(answered.starts_with("! TypeError"), "{answered}");
}

#[test]
fn an_error_made_for_ever_is_a_range_error_rather_than_a_process_that_stops() {
    for source in [
        "function f() { return new Error('', { get cause() { return f(); } }); } f()",
        "let o = { toString() { return new Error(o); } }; new Error(o)",
        "let o = { get name() { return Error.prototype.toString.call(o); } }; '' + Error.prototype.toString.call(o)",
    ] {
        let answered = value(source);
        assert!(answered.starts_with("! RangeError"), "{source}: {answered}");
    }
}

#[test]
fn many_errors_with_causes_survive_the_collector() {
    table(&[(
        "let last = null; \
         for (let i = 0; i < 60; i = i + 1) { last = new TypeError('m' + i, { cause: last }); } \
         let n = 0; while (last !== undefined && last !== null) { n = n + 1; last = last.cause; } n",
        "60",
    )]);
    table(&[(
        "let e = new Error('a', { cause: new RangeError('b', { cause: 'c' }) }); \
         '' + e + '/' + e.cause + '/' + e.cause.cause",
        "\"Error: a/RangeError: b/c\"",
    )]);
}

#[test]
fn every_prefix_of_an_error_program_is_an_answer_rather_than_a_crash() {
    let source = "let e = new TypeError({ toString() { return 'm'; } }, \
                  { get cause() { return new RangeError('c'); } }); \
                  Error.prototype.toString.call(e) + e.cause";
    assert_eq!(value(source), "\"TypeError: mRangeError: c\"");
    for (end, _) in source.char_indices() {
        let Some(prefix) = source.get(..end) else {
            continue;
        };
        // Whatever it answers, it answers — and both ways agree.
        let _ = value(prefix);
    }
}

#[test]
fn a_long_message_is_kept_whole() {
    let answered = value(
        "let s = 'x'; for (let i = 0; i < 20; i = i + 1) { s = s + s; } \
         new Error(s).message === s && ('' + new Error(s)) === 'Error: ' + s",
    );
    // A string's own `length` needs a wrapper object (item 73), so the
    // message is compared rather than measured.
    assert_eq!(answered, "true");
}
