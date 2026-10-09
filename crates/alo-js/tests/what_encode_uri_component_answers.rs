/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 359's closing conditions, as programs and what they evaluate to.
//!
//! *Every code point class — unreserved, reserved, two-, three- and four-byte,
//! a paired and a lone surrogate — is written as the specification says, with
//! a long and a hostile string refused or answered in bounded work.*
//!
//! Every program runs twice, once with the collector running at every
//! allocation ([`Heap::stress`]): the function converts an object argument by
//! running the page's script and then makes a string, and the second is an
//! allocation the first's answer must survive.
//!
//! A string the engine could make but whose encoding it could not — past
//! `LONGEST_STRING` — is `alo_js::uri`'s own unit test, with the bound
//! lowered: reaching the real one takes half a gibibyte.
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

#[test]
fn every_code_point_class_is_written_as_the_specification_says() {
    table(&[
        // Unreserved: `uriAlpha`, `DecimalDigit` and `uriMark`, as themselves.
        (
            "encodeURIComponent(\"AZaz09-_.!~*'()\")",
            "\"AZaz09-_.!~*'()\"",
        ),
        // Reserved, and `#`: escaped, which is what makes it a *component*.
        (
            "encodeURIComponent(';/?:@&=+$,#')",
            "\"%3B%2F%3F%3A%40%26%3D%2B%24%2C%23\"",
        ),
        // The rest of ASCII, the percent sign itself included.
        ("encodeURIComponent(' %\"<>')", "\"%20%25%22%3C%3E\""),
        ("encodeURIComponent('\\u0000\\u007f')", "\"%00%7F\""),
        // Two bytes, three, and the last of the basic plane: uppercase hex.
        ("encodeURIComponent('é')", "\"%C3%A9\""),
        ("encodeURIComponent('€')", "\"%E2%82%AC\""),
        ("encodeURIComponent('\\uffff')", "\"%EF%BF%BF\""),
        // A pair is one code point and four bytes, written or escaped.
        ("encodeURIComponent('😀')", "\"%F0%9F%98%80\""),
        ("encodeURIComponent('\\ud83d\\ude00')", "\"%F0%9F%98%80\""),
        ("encodeURIComponent('\\u{10FFFF}')", "\"%F4%8F%BF%BF\""),
        // A path, as alo Sites' analytics script hands it one.
        (
            "encodeURIComponent('/menu/café au lait')",
            "\"%2Fmenu%2Fcaf%C3%A9%20au%20lait\"",
        ),
        ("encodeURIComponent('')", "\"\""),
    ]);
}

#[test]
fn a_lone_surrogate_is_the_uri_error_a_page_can_catch() {
    let refused = |unit: &str, index: usize| {
        format!(
            "! URIError: encodeURIComponent was given a lone surrogate, U+{unit} at index \
             {index}, which is no character and so has no UTF-8 (at byte 0)"
        )
    };
    assert_eq!(both("encodeURIComponent('\\ud800')"), refused("D800", 0));
    assert_eq!(both("encodeURIComponent('a\\udfff')"), refused("DFFF", 1));
    // Two leading surrogates are not a pair.
    assert_eq!(
        both("encodeURIComponent('\\ud83d\\ud83d\\ude00')"),
        refused("D83D", 0)
    );
    // A pair, then a trailing surrogate alone.
    assert_eq!(
        both("encodeURIComponent('\\ud83d\\ude00\\ude00')"),
        refused("DE00", 2)
    );
    // It is the language's `URIError`, an instance of its constructor.
    table(&[
        (
            "try { encodeURIComponent('\\ud800'); } catch (e) { e.name }",
            "\"URIError\"",
        ),
        (
            "try { encodeURIComponent('\\ud800'); } catch (e) { \
             URIError.prototype.isPrototypeOf(e) }",
            "true",
        ),
    ]);
}

/// A symbol's `TypeError` is `ToString`'s, which this calls, and is not
/// tested here: no page can spell a symbol yet (queue item 73).
#[test]
fn its_argument_is_converted_to_a_string_once() {
    table(&[
        ("encodeURIComponent()", "\"undefined\""),
        ("encodeURIComponent(null)", "\"null\""),
        ("encodeURIComponent(true)", "\"true\""),
        ("encodeURIComponent(-1.5e21)", "\"-1.5e%2B21\""),
        ("encodeURIComponent(1, 2)", "\"1\""),
        // An object's `toString` runs once, and what it answers is encoded.
        (
            "var calls = 0; \
             var o = { toString: function () { calls = calls + 1; return 'a b'; } }; \
             encodeURIComponent(o) + calls",
            "\"a%20b1\"",
        ),
        // `toString` before `valueOf`, because the hint is string.
        (
            "encodeURIComponent({ toString: function () { return 'x'; }, \
             valueOf: function () { return 'y'; } })",
            "\"x\"",
        ),
        // What `toString` throws is what the call throws.
        (
            "try { encodeURIComponent({ toString: function () { throw 'no'; } }); } \
             catch (e) { e }",
            "\"no\"",
        ),
        // A lone surrogate a conversion answers is refused like any other.
        (
            "try { encodeURIComponent({ toString: function () { return '\\udc00'; } }); } \
             catch (e) { e.name }",
            "\"URIError\"",
        ),
    ]);
}

#[test]
fn it_is_a_function_of_the_global_object_as_the_specification_makes_one() {
    table(&[
        ("typeof encodeURIComponent", "\"function\""),
        ("typeof globalThis.encodeURIComponent", "\"function\""),
        // Writable and configurable and not enumerable.
        (
            "globalThis.propertyIsEnumerable('encodeURIComponent')",
            "false",
        ),
        (
            "encodeURIComponent = function () { return 'mine'; }; encodeURIComponent('a b')",
            "\"mine\"",
        ),
        (
            "delete globalThis.encodeURIComponent; typeof encodeURIComponent",
            "\"undefined\"",
        ),
        // It is no constructor.
        (
            "try { new encodeURIComponent('a'); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        // Its siblings are each taken when a page needs one (queue item 73).
        ("typeof encodeURI", "\"undefined\""),
        ("typeof decodeURIComponent", "\"undefined\""),
    ]);
}

#[test]
fn a_long_string_is_answered_in_one_pass() {
    // 2²⁰ `é`s, made by doubling, encode to 2²⁰ `%C3%A9`s made the same way.
    table(&[(
        "var s = 'é'; var t = '%C3%A9'; \
         for (var i = 0; i < 20; i = i + 1) { s = s + s; t = t + t; } \
         encodeURIComponent(s) === t",
        "true",
    )]);
}
