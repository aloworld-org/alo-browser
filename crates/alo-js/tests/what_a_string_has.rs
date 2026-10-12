/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 488's closing conditions, as programs and what they evaluate to.
//!
//! *A string's own `length` and units, then `String.prototype` with the
//! string as the receiver; `toString`, `valueOf` and `slice` on it, `slice`'s
//! three conversions once each and in order; a write and a delete as the
//! wrapper the specification makes would answer them; and what is not built
//! refused by name.*
//!
//! Every program runs twice, once with the collector running at every
//! allocation ([`Heap::stress`]): `slice` keeps a string and two numbers
//! across calls into the page's script, and each must survive.
//!
//! No script can reach `String.prototype` itself yet — `"".__proto__` needs
//! `ToObject` (queue item 489) — so the tests that need it are handed it, as
//! `stringPrototype` on the global object, by the embedder.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use alo_js::interpret::{Engine, Trouble};
use alo_js::numeric;
use alo_js::object::{Property, Value};
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

/// An engine with `String.prototype` on its global object as
/// `stringPrototype`, which no script can spell for itself yet.
fn engine(stress: bool) -> Result<Engine, String> {
    let mut engine = Engine::new().map_err(|why| format!("no engine: {why}"))?;
    engine.objects().heap_mut().stress(stress);
    let (intrinsics, objects) = engine.intrinsics();
    let prototype = intrinsics
        .string_prototype(objects)
        .map_err(|why| format!("no prototype: {why}"))?;
    let global = engine.global().map_err(|why| format!("no global: {why}"))?;
    let units: Vec<u16> = "stringPrototype".encode_utf16().collect();
    let key = engine
        .objects()
        .key(&units)
        .map_err(|why| format!("no key: {why:?}"))?;
    engine
        .objects()
        .define(global, key, Property::plain(Value::Object(prototype)))
        .map_err(|why| format!("not defined: {why:?}"))?;
    Ok(engine)
}

/// One run.
fn run(source: &str, stress: bool) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    let mut engine = match engine(stress) {
        Ok(engine) => engine,
        Err(why) => return why,
    };
    evaluated(&mut engine, &program)
}

/// What a program evaluates to in an engine, written out.
fn evaluated(engine: &mut Engine, program: &alo_js::ast::Program) -> String {
    match engine.evaluate(program) {
        Ok(value) => show(engine, value),
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
fn a_string_has_its_length_and_its_units() {
    table(&[
        ("'abc'.length", "3"),
        ("''.length", "0"),
        ("'abc'[0] + 'abc'[2]", "\"ac\""),
        ("'abc'['1']", "\"b\""),
        ("let s = 'abc'; let i = 1; s[i + 1]", "\"c\""),
        // Past either end, and a key that only looks like an index, is not
        // its own, and nothing along the chain has it.
        ("'abc'[3]", "undefined"),
        ("'abc'[-1]", "undefined"),
        ("'abc'['1.0']", "undefined"),
        // A unit, not a character: a pair is two, and each half is a string
        // of its own.
        ("'\\uD83D\\uDE00'.length", "2"),
        ("'\\uD83D\\uDE00'[0] === '\\uD83D'", "true"),
        // The units of a string a script made, not only of a literal.
        (
            "let s = ''; for (let i = 0; i < 20; i++) { s = s + 'ab'; } s.length",
            "40",
        ),
        ("`a${1 + 1}c`[1]", "\"2\""),
    ]);
}

#[test]
fn a_string_reads_string_prototype_and_what_is_behind_it() {
    table(&[
        ("typeof 'a'.slice", "\"function\""),
        ("'a'.slice === 'b'.slice", "true"),
        ("'a'.slice === stringPrototype.slice", "true"),
        // Behind it is `Object.prototype`, whatever a page put there.
        ("({}).__proto__.shared = 7; 'a'.shared", "7"),
        // Absent rather than approximate: a page's feature test reads it.
        ("typeof 'a'.trim", "\"undefined\""),
        // No `String` constructor (queue item 489), and no `Object` either
        // for the walk to find instead.
        ("typeof 'a'.constructor", "\"undefined\""),
    ]);
}

#[test]
fn string_prototype_is_a_string_object_of_nothing() {
    table(&[
        ("stringPrototype.length", "0"),
        ("stringPrototype[0]", "undefined"),
        ("stringPrototype.toString()", "\"\""),
        ("stringPrototype.valueOf()", "\"\""),
        // `ToPrimitive` finds its `valueOf`, so it is the empty string.
        ("stringPrototype + 'x'", "\"x\""),
        ("({}).toString.call(stringPrototype)", "\"[object String]\""),
        ("''.slice.call(stringPrototype)", "\"\""),
        // Its own properties are fixed, as the specification makes them.
        ("stringPrototype.length = 3; stringPrototype.length", "0"),
        ("delete stringPrototype.length", "false"),
        ("stringPrototype.propertyIsEnumerable('length')", "false"),
        ("stringPrototype.hasOwnProperty('slice')", "true"),
        ("stringPrototype.propertyIsEnumerable('slice')", "false"),
    ]);
}

#[test]
fn to_string_and_value_of_answer_the_string() {
    table(&[
        ("'abc'.toString()", "\"abc\""),
        ("'abc'.valueOf()", "\"abc\""),
        ("let f = ''.toString; f.call('x')", "\"x\""),
        // Anything that is neither a string nor a `String` object is the
        // `TypeError` `thisStringValue` gives — a number included.
        (
            "try { ''.toString.call({}); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        (
            "try { ''.valueOf.call(1); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        (
            "try { ''.toString.call(null); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn slice_cuts_as_the_specification_counts() {
    table(&[
        // alo Sites' analytics script, on a form's action.
        (
            "'/f/f4K9sL2wN7qR5tYx8vB1cA'.slice(3)",
            "\"f4K9sL2wN7qR5tYx8vB1cA\"",
        ),
        ("'abcdef'.slice()", "\"abcdef\""),
        ("'abcdef'.slice(2)", "\"cdef\""),
        ("'abcdef'.slice(1, 3)", "\"bc\""),
        ("'abcdef'.slice(-2)", "\"ef\""),
        ("'abcdef'.slice(0, -1)", "\"abcde\""),
        ("'abcdef'.slice(-3, -1)", "\"de\""),
        ("'abcdef'.slice(4, 2)", "\"\""),
        ("'abcdef'.slice(9)", "\"\""),
        ("'abcdef'.slice(-9)", "\"abcdef\""),
        ("'abcdef'.slice(2, 99)", "\"cdef\""),
        // `ToIntegerOrInfinity`: towards zero, `NaN` is zero, the
        // infinities are held at the ends.
        ("'abcdef'.slice(1.9, 3.9)", "\"bc\""),
        ("'abcdef'.slice(-1.5)", "\"f\""),
        ("'abcdef'.slice(NaN)", "\"abcdef\""),
        ("'abcdef'.slice(0, NaN)", "\"\""),
        ("'abcdef'.slice(-Infinity, Infinity)", "\"abcdef\""),
        ("'abcdef'.slice(Infinity)", "\"\""),
        // `undefined` for `end` is the whole length, not zero; `null` is
        // zero.
        ("'abcdef'.slice(2, undefined)", "\"cdef\""),
        ("'abcdef'.slice(2, null)", "\"\""),
        ("'abcdef'.slice('1', '2')", "\"b\""),
        ("'abcdef'.slice(true)", "\"bcdef\""),
        // A cut of units may leave half a pair, which is still a string.
        ("'\\uD83D\\uDE00'.slice(1) === '\\uDE00'", "true"),
        ("''.slice(0)", "\"\""),
    ]);
}

#[test]
fn slice_makes_a_string_of_any_this_but_null_and_undefined() {
    table(&[
        ("''.slice.call(12345, 1, 3)", "\"23\""),
        ("''.slice.call(true, 1)", "\"rue\""),
        (
            "''.slice.call({ toString() { return 'hello'; } }, 1)",
            "\"ello\"",
        ),
        (
            "try { ''.slice.call(null); } catch (e) { e.name + ': ' + e.message }",
            "\"TypeError: String.prototype.slice was called on null or undefined\"",
        ),
        (
            "try { ''.slice.call(undefined); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        // `ToString` of an object whose conversion answers nothing it can use
        // is the `TypeError` it always is.
        (
            "try { ''.slice.call({ toString() { return {}; }, valueOf() { return {}; } }); } \
             catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn slice_converts_this_then_start_then_end_once_each() {
    table(&[
        (
            "var log = ''; \
             var s = { toString() { log += 's'; return 'hello'; } }; \
             var a = { valueOf() { log += 'a'; return 1; } }; \
             var b = { valueOf() { log += 'b'; return 3; } }; \
             ''.slice.call(s, a, b) + ' ' + log",
            "\"el sab\"",
        ),
        // `end` absent is never converted, and `undefined` is not either.
        (
            "var log = ''; \
             var a = { valueOf() { log += 'a'; return 2; } }; \
             'hello'.slice(a) + ' ' + log + ' ' + 'hello'.slice(a, undefined) + ' ' + log",
            "\"llo a llo aa\"",
        ),
        // A conversion that throws stops `slice` there, and nothing after it
        // runs.
        (
            "var log = ''; \
             var a = { valueOf() { log += 'a'; throw 'stop'; } }; \
             var b = { valueOf() { log += 'b'; return 1; } }; \
             try { 'abc'.slice(a, b); } catch (e) { e + ' ' + log }",
            "\"stop a\"",
        ),
        // What `this` came to is kept while the page's script runs and
        // allocates around it.
        (
            "var s = { toString() { let t = ''; for (let i = 0; i < 50; i++) t = t + 'x'; return t + 'end'; } }; \
             var a = { valueOf() { let u = []; for (let i = 0; i < 50; i++) u = [u, 'y' + i]; return -3; } }; \
             ''.slice.call(s, a)",
            "\"end\"",
        ),
    ]);
}

#[test]
fn a_string_is_not_written_to_and_keeps_what_it_was_made_with() {
    table(&[
        // Sloppy code: the write is silence, and nothing is kept.
        ("let s = 'abc'; s.x = 1; s.x", "undefined"),
        ("let s = 'abc'; s[0] = 'z'; s", "\"abc\""),
        ("let s = 'abc'; s.length = 1; s.length", "3"),
        // The assignment is still the value written.
        ("'abc'.x = 5", "5"),
        // Strict code: each is the `TypeError` `PutValue` gives.
        (
            "(function () { 'use strict'; try { 'abc'.x = 1; } catch (e) { return e.name; } })()",
            "\"TypeError\"",
        ),
        (
            "(function () { 'use strict'; try { 'abc'.length = 1; } catch (e) { return e.name; } })()",
            "\"TypeError\"",
        ),
        (
            "(function () { 'use strict'; try { 'abc'[1] = 'z'; } catch (e) { return e.name; } })()",
            "\"TypeError\"",
        ),
        // A writable data property along the chain is not written through.
        ("({}).__proto__.w = 1; 'abc'.w = 2; ({}).w", "1"),
        // Delete: what the wrapper is made with stays, and anything else was
        // never there.
        ("delete 'abc'.x", "true"),
        ("delete 'abc'.slice", "true"),
        ("typeof 'abc'.slice", "\"function\""),
        ("delete 'abc'[0]", "false"),
        ("delete 'abc'.length", "false"),
        ("delete 'abc'[5]", "true"),
        (
            "(function () { 'use strict'; try { delete 'abc'.length; } catch (e) { return e.name; } })()",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn an_accessor_along_the_chain_is_called_with_the_string_as_this() {
    for stress in [false, true] {
        let mut engine = match engine(stress) {
            Ok(engine) => engine,
            Err(why) => panic!("{why}"),
        };
        // Script functions for each half, written by the page and put on
        // `String.prototype` by the embedder, since no script can define an
        // accessor on an object it did not make (`Object.defineProperty` is
        // queue item 73's).
        let Ok(halves) = script(
            "var seen = ''; [\
               function () { 'use strict'; return typeof this + ':' + this; }, \
               function (v) { 'use strict'; seen = typeof this + ':' + this + '=' + v; }]",
        ) else {
            panic!("the halves parse");
        };
        let Ok(Value::Object(array)) = engine.evaluate(&halves) else {
            panic!("the halves are made");
        };
        let held = engine.objects().heap_mut().root(array);
        let half = |engine: &mut Engine, at: u32| {
            let Some(key) = alo_js::object::Key::index(at) else {
                panic!("an index");
            };
            match engine.objects().get(array, key) {
                Ok(alo_js::object::Found::Value(value)) => value,
                other => panic!("{other:?}"),
            }
        };
        let get = half(&mut engine, 0);
        let set = half(&mut engine, 1);
        let (intrinsics, objects) = engine.intrinsics();
        let Ok(prototype) = intrinsics.string_prototype(objects) else {
            panic!("String.prototype is rooted");
        };
        let units: Vec<u16> = "both".encode_utf16().collect();
        let Ok(key) = engine.objects().key(&units) else {
            panic!("the name is interned");
        };
        assert_eq!(
            engine
                .objects()
                .define(prototype, key, Property::accessor(get, set, false, true)),
            Ok(true)
        );
        // And under an index: a string long enough has that unit of its own,
        // which shadows the accessor for a read and refuses a write.
        let Some(second) = alo_js::object::Key::index(1) else {
            panic!("one is an index");
        };
        assert_eq!(
            engine
                .objects()
                .define(prototype, second, Property::accessor(get, set, false, true)),
            Ok(true)
        );
        engine.objects().heap_mut().release(held);

        for (source, expected) in [
            ("'abc'[1]", "\"b\""),
            ("'a'[1]", "\"string:a\""),
            ("seen = ''; 'abc'[1] = 'z'; seen", "\"\""),
            ("'a'[1] = 'z'; seen", "\"string:a=z\""),
            ("'abc'.both", "\"string:abc\""),
            ("'abc'.both = 1; seen", "\"string:abc=1\""),
            (
                "(function () { 'use strict'; 'xy'.both = 2; return seen; })()",
                "\"string:xy=2\"",
            ),
        ] {
            let Ok(program) = script(source) else {
                panic!("{source} parses");
            };
            assert_eq!(evaluated(&mut engine, &program), expected, "{source}");
        }
    }
}

#[test]
fn what_is_not_built_is_refused_by_name() {
    for (source, item) in [
        // Iterating a string reads a method `String.prototype` does not
        // have yet; calling the string not iterable would be wrong.
        ("for (const c of 'ab') {}", "queue item 490"),
        // A number's, a boolean's and a symbol's properties, and a string
        // made an object, need `ToObject`.
        ("(1).toString()", "queue item 489"),
        ("true.x", "queue item 489"),
        ("''.__proto__", "queue item 489"),
        ("'a'.hasOwnProperty('length')", "queue item 489"),
        ("(function () { return this; }).call('a')", "queue item 489"),
    ] {
        let answer = both(source);
        assert!(answer.starts_with('!'), "{source}: {answer}");
        assert!(answer.contains(item), "{source}: {answer}");
    }
}
