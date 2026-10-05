/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 225: *an array is an object whose `length` keeps up with its
//! indices* — the exotic object, `Array.prototype`, and the literal that makes
//! one. Cut from item 73 (the exotic object) and item 211 (the literal, without
//! a spread, which reads an iterable and stays there).
//!
//! # What opened it
//!
//! A real page's own script. `crates/alo-corpus/scripts/alo-service-worker/`
//! is alo's offline-shell service worker, frozen with its provenance beside
//! it, and after item 212 the first thing it could not compile was
//! `let changedTypes = [];` at byte 2847. The last test in this file is that
//! script compiling past it. The `try` on the next line was item 210's, which
//! is built now; where the script stops next is pinned in
//! `what_a_catch_catches.rs`.
//!
//! # Every program runs twice
//!
//! Once ordinarily and once with the collector firing at every allocation, and
//! the two must agree. Making an array interns `"length"` and then allocates,
//! and the name is held by nothing but the array once it exists — exactly the
//! window in which a reference held only by Rust would be collected.

use std::path::PathBuf;

use alo_js::compile::{Refusal, What};
use alo_js::heap::Ref;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Found, Key, Objects, Property, Set, Value};
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
    run(&mut engine, source)
}

/// One run, in an engine that may already have run something.
fn run(engine: &mut Engine, source: &str) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
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

#[test]
fn a_literal_makes_an_array_of_its_elements() {
    table(&[
        ("[].length", "0"),
        ("[1, 2, 3].length", "3"),
        ("[1, 2, 3][0] + [1, 2, 3][2]", "4"),
        ("[1, 2, 3][3]", "undefined"),
        // An element is any expression, evaluated left to right.
        (
            "let n = 0; let a = [n = n + 1, n = n * 10, n]; a[0] + a[1] + a[2]",
            "21",
        ),
        ("[1, [2, 3]][1][0]", "2"),
        ("[function () { return 7; }][0]()", "7"),
        // Each evaluation is a different array.
        ("[] !== []", "true"),
        ("typeof []", "\"object\""),
        // A trailing comma is not an element, and every other comma is.
        ("[1, 2,].length", "2"),
    ]);
}

#[test]
fn a_hole_is_not_undefined() {
    table(&[
        // The closing condition of item 211 that this item met: a hole counts
        // towards the length and is not a property.
        ("[1, , 3].length", "3"),
        ("1 in [1, , 3]", "false"),
        ("0 in [1, , 3]", "true"),
        ("[1, , 3][1]", "undefined"),
        ("1 in [1, undefined, 3]", "true"),
        ("[1, , 3].hasOwnProperty(1)", "false"),
        // Holes at the end count too, and so does a literal of nothing else.
        ("[,].length", "1"),
        ("[, ,].length", "2"),
        ("[1, , ].length", "2"),
        ("0 in [,]", "false"),
    ]);
}

#[test]
fn an_index_at_or_past_the_length_grows_it() {
    table(&[
        ("let a = []; a[0] = 1; a.length", "1"),
        ("let a = []; a[5] = 1; a.length", "6"),
        ("let a = []; a[5] = 1; 4 in a", "false"),
        ("let a = [1, 2, 3]; a[1] = 9; a.length", "3"),
        ("let a = []; a['2'] = 1; a.length", "3"),
        // A key that only looks like an index is a name, and grows nothing.
        ("let a = []; a['02'] = 1; a.length", "0"),
        ("let a = []; a[-1] = 1; a.length", "0"),
        ("let a = []; a[1.5] = 1; a.length", "0"),
        // The largest index there is makes the largest length there is, and
        // one past it is a name.
        ("let a = []; a[4294967294] = 1; a.length", "4294967295"),
        ("let a = []; a[4294967295] = 1; a.length", "0"),
    ]);
}

#[test]
fn a_smaller_length_deletes_from_the_end() {
    table(&[
        ("let a = [1, 2, 3]; a.length = 1; a[1]", "undefined"),
        ("let a = [1, 2, 3]; a.length = 1; 1 in a", "false"),
        ("let a = [1, 2, 3]; a.length = 1; a[0]", "1"),
        ("let a = [1, 2, 3]; a.length = 0; 0 in a", "false"),
        // A larger length makes holes, not elements.
        ("let a = [1]; a.length = 3; a.length", "3"),
        ("let a = [1]; a.length = 3; 2 in a", "false"),
        // Growing and shrinking again.
        (
            "let a = [1, 2, 3]; a.length = 1; a.length = 3; (1 in a) + ':' + a.length",
            "\"false:3\"",
        ),
        // Compound assignment reads, converts and writes like anything else.
        ("let a = [1, 2, 3]; a.length -= 1; a.length", "2"),
        ("let a = []; a.length++; a.length", "1"),
        // A length of four billion over two elements, then zero: two deletions,
        // which is why this returns at all.
        (
            "let a = [1, 2]; a.length = 4294967295; a.length = 0; a.length",
            "0",
        ),
        // Names are not elements and a shrinking length leaves them alone.
        ("let a = [1]; a.x = 2; a.length = 0; a.x", "2"),
    ]);
}

#[test]
fn a_length_is_converted_and_a_bad_one_is_a_range_error() {
    table(&[
        ("let a = [1]; a.length = '3'; a.length", "3"),
        ("let a = [1, 2]; a.length = true; a.length", "1"),
        ("let a = [1, 2]; a.length = null; a.length", "0"),
        ("let a = [1, 2]; a.length = -0; a.length", "0"),
        ("let a = [1, 2]; a.length = ''; a.length", "0"),
        // The assignment is the value as written, not the length it became.
        ("let a = [1, 2]; (a.length = '1')", "\"1\""),
    ]);
    for (source, says) in [
        (
            "let a = []; a.length = -1",
            "-1 is not a valid array length",
        ),
        (
            "let a = []; a.length = 1.5",
            "1.5 is not a valid array length",
        ),
        (
            "let a = []; a.length = NaN",
            "NaN is not a valid array length",
        ),
        (
            "let a = []; a.length = Infinity",
            "Infinity is not a valid array length",
        ),
        (
            "let a = []; a.length = 4294967296",
            "4294967296 is not a valid array length",
        ),
        (
            "let a = []; a.length = 'x'",
            "NaN is not a valid array length",
        ),
        (
            "let a = []; a.length = undefined",
            "NaN is not a valid array length",
        ),
    ] {
        let answered = value(source);
        assert!(
            answered.starts_with("! RangeError") && answered.contains(says),
            "{source} is a RangeError saying {says:?}: {answered}"
        );
    }
    // An object with no prototype could not be converted at all, and is still
    // refused by name rather than reported as a TypeError it might not be.
    let answered = value("let a = []; a.length = { __proto__: null }");
    assert!(
        answered.contains("226"),
        "an object assigned to a length names the item that builds it: {answered}"
    );
}

#[test]
fn length_has_the_attributes_the_specification_gives() {
    table(&[
        ("[].hasOwnProperty('length')", "true"),
        ("[].propertyIsEnumerable('length')", "false"),
        // Not configurable: deleting it is refused, silently and then loudly.
        ("let a = [1]; delete a.length", "false"),
        ("let a = [1]; delete a.length; a.length", "1"),
        // Elements are ordinary properties.
        ("[1].propertyIsEnumerable(0)", "true"),
        ("let a = [1, 2]; delete a[1]; a.length", "2"),
        ("let a = [1, 2]; delete a[1]; 1 in a", "false"),
    ]);
    let answered = value("'use strict'; let a = [1]; delete a.length");
    assert!(
        answered.starts_with("! TypeError"),
        "strict code is told: {answered}"
    );
}

#[test]
fn array_prototype_is_an_array_with_nothing_on_it_yet() {
    table(&[
        ("[].__proto__ === [1].__proto__", "true"),
        ("[].__proto__ === ({}).__proto__", "false"),
        ("[].__proto__.__proto__ === ({}).__proto__", "true"),
        ("[].__proto__.length", "0"),
        // IsArray, as `Object.prototype.toString` reports it — for an array, for
        // `Array.prototype`, and not for an object that only inherits from one.
        ("({}).toString.call([1, 2])", "\"[object Array]\""),
        ("({}).toString.call([].__proto__)", "\"[object Array]\""),
        (
            "({}).toString.call({ __proto__: [] })",
            "\"[object Object]\"",
        ),
        ("'' + { __proto__: [] }", "\"[object Object]\""),
        // Absent rather than approximate: no method is there until it is
        // built, which a page's own feature test reads correctly.
        ("typeof [].push", "\"undefined\""),
        ("typeof [].map", "\"undefined\""),
        ("typeof Array", "\"undefined\""),
        // What an array does inherit is `Object.prototype`'s.
        ("[1].hasOwnProperty(0)", "true"),
        ("[].toString === ({}).toString", "true"),
    ]);
}

#[test]
fn an_object_that_inherits_from_an_array_has_no_length_of_its_own() {
    table(&[
        // `length` is found on the prototype and is a writable data property,
        // so assigning shadows it with an ordinary property — no conversion and
        // no truncation, because the receiver is not an array.
        (
            "let o = { __proto__: [1, 2] }; o.length = 'x'; o.length",
            "\"x\"",
        ),
        (
            "let p = [1, 2]; let o = { __proto__: p }; o.length = 0; p.length",
            "2",
        ),
        ("let o = { __proto__: [1, 2] }; o[5] = 1; o.length", "2"),
    ]);
}

#[test]
fn what_is_not_built_is_refused_by_name() {
    for (source, item) in [
        ("[...a]", "211"),
        ("[1, ...a, 2]", "211"),
        ("let a = []; a.length = {}", "226"),
        ("let a = []; a.length = { valueOf() { return 1; } }", "226"),
    ] {
        let answered = value(source);
        assert!(
            answered.contains(item),
            "{source} should name queue item {item}: {answered}"
        );
    }
}

/// Unwrap what a test cannot go on without, saying what it was.
macro_rules! some {
    ($asked:expr) => {
        match $asked {
            Some(answer) => answer,
            None => panic!("{} answered nothing", stringify!($asked)),
        }
    };
}

/// The key `"length"` is, which a heap has interned once an array exists.
fn length_key(objects: &Objects) -> Option<Key> {
    let units: Vec<u16> = "length".encode_utf16().collect();
    objects.existing_key(&units)
}

/// An array of three elements in a fresh heap, rooted.
fn three() -> Option<(Objects, Ref)> {
    let mut objects = Objects::new();
    let array = objects.array(None, 0).ok()?;
    let _ = objects.heap_mut().root(array);
    for at in 0..3 {
        let property = Property::plain(Value::Number(f64::from(at)));
        if objects.define(array, Key::index(at)?, property) != Ok(true) {
            return None;
        }
    }
    Some((objects, array))
}

/// What an array's `length` is, read through the object model.
fn length(objects: &Objects, array: Ref) -> Option<Found> {
    objects.get(array, length_key(objects)?).ok()
}

#[test]
fn an_element_that_will_not_go_stops_a_shrinking_length() {
    // No script can make a property that is not configurable until
    // `Object.defineProperty` is built (item 73), so this is the object model's
    // own test of `ArraySetLength`'s last rule.
    let (mut objects, array) = some!(three());
    let pinned = objects.define(
        array,
        some!(Key::index(1)),
        Property::data(Value::Number(1.0), true, true, false),
    );
    assert_eq!(pinned, Ok(true));
    let key = some!(length_key(&objects));
    assert_eq!(
        objects.set(array, key, Value::Number(0.0)),
        Ok(Set::Refused),
        "the definition answers false"
    );
    assert_eq!(
        some!(length(&objects, array)),
        Found::Value(Value::Number(2.0)),
        "and the length is one past the element that stayed"
    );
    assert_eq!(
        objects.has(array, some!(Key::index(2))),
        Ok(false),
        "the one above went"
    );
    assert_eq!(objects.has(array, some!(Key::index(1))), Ok(true));
    assert_eq!(
        objects.has(array, some!(Key::index(0))),
        Ok(true),
        "the one below stayed"
    );
}

#[test]
fn a_length_that_is_not_writable_refuses_to_grow_or_shrink() {
    let (mut objects, array) = some!(three());
    let key = some!(length_key(&objects));
    let frozen = objects.define(
        array,
        key,
        Property::data(Value::Number(3.0), false, false, false),
    );
    assert_eq!(frozen, Ok(true), "the same length, made read-only");
    assert_eq!(
        objects.set(array, some!(Key::index(3)), Value::Number(9.0)),
        Ok(Set::Refused),
        "an index at the length would grow it"
    );
    assert_eq!(
        objects.has(array, some!(Key::index(3))),
        Ok(false),
        "and nothing was stored"
    );
    assert_eq!(
        objects.set(array, some!(Key::index(1)), Value::Number(9.0)),
        Ok(Set::Done),
        "an index below it is an ordinary write"
    );
    assert_eq!(
        objects.set(array, key, Value::Number(1.0)),
        Ok(Set::Refused)
    );
    assert_eq!(
        some!(length(&objects, array)),
        Found::Value(Value::Number(3.0))
    );
    assert_eq!(objects.has(array, some!(Key::index(2))), Ok(true));
}

#[test]
fn a_length_nobody_converted_is_refused_rather_than_guessed_at() {
    // Converting is the interpreter's, because it can call a script and throw.
    // The object model must not turn anything else into a number.
    let (mut objects, array) = some!(three());
    let key = some!(length_key(&objects));
    for wrong in [
        Value::Number(1.5),
        Value::Number(-1.0),
        Value::Number(f64::NAN),
        Value::Bool(true),
        Value::Null,
        Value::Undefined,
    ] {
        assert_eq!(
            objects.set(array, key, wrong),
            Ok(Set::Refused),
            "{wrong:?}"
        );
        assert_eq!(
            some!(length(&objects, array)),
            Found::Value(Value::Number(3.0))
        );
    }
    let accessor = Property::accessor(Value::Undefined, Value::Undefined, false, false);
    assert_eq!(objects.define(array, key, accessor), Ok(false));
}

#[test]
fn length_comes_after_the_indices_and_before_every_other_name() {
    let mut objects = Objects::new();
    let Ok(array) = objects.array(None, 0) else {
        panic!("an empty heap holds an array");
    };
    let _ = objects.heap_mut().root(array);
    let named: Vec<u16> = "b".encode_utf16().collect();
    let Ok(defined) = objects.define_named(array, &named, Property::plain(Value::Null)) else {
        panic!("a name can be interned");
    };
    assert!(defined);
    for at in [1, 0] {
        assert_eq!(
            objects.define(array, some!(Key::index(at)), Property::plain(Value::Null)),
            Ok(true)
        );
    }
    let Some(b) = objects.existing_key(&named) else {
        panic!("b was interned");
    };
    assert_eq!(
        objects.own_keys(array),
        Ok(vec![
            some!(Key::index(0)),
            some!(Key::index(1)),
            some!(length_key(&objects)),
            b
        ])
    );
}

#[test]
fn an_array_keeps_its_length_alive_through_a_collection_at_every_allocation() {
    let mut objects = Objects::new();
    objects.heap_mut().stress(true);
    let Ok(array) = objects.array(None, 4) else {
        panic!("an empty heap holds an array however often it collects");
    };
    let _ = objects.heap_mut().root(array);
    // A second array interns nothing new and allocates, which collects again.
    let Ok(other) = objects.array(None, 0) else {
        panic!("and a second");
    };
    let _ = other;
    objects.heap_mut().stress(false);
    objects.heap_mut().collect();
    assert!(objects.heap().check().is_ok(), "the heap is well formed");
    assert_eq!(objects.heap().scoped(), 0, "every scope was closed");
    assert_eq!(
        some!(length(&objects, array)),
        Found::Value(Value::Number(4.0))
    );
}

#[test]
fn every_cut_of_an_array_program_is_a_result_rather_than_a_crash() {
    let source = "let a = [1, , [2, 3], 'x']; a[9] = a; a.length = 3; \
                  a.length + a[2][1] + (1 in a)";
    assert_eq!(value(source), "6");
    for end in 0..=source.len() {
        let Some(cut) = source.get(..end) else {
            continue;
        };
        let _ = value(cut);
    }
    // Brackets nested past the parser's bound are refused by it.
    let deep = format!("{}{}", "[".repeat(20_000), "]".repeat(20_000));
    assert!(
        value(&deep).starts_with("did not parse"),
        "a tower of brackets is refused by the parser's bound"
    );
}

#[test]
fn a_long_literal_and_many_arrays_leave_nothing_behind_that_nobody_holds() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an empty heap holds an engine");
    };
    let long = format!("[{}].length", "0,".repeat(50_000));
    assert_eq!(run(&mut engine, &long), "50000");
    let wide = format!("[{}].length", ",".repeat(50_000));
    assert_eq!(run(&mut engine, &wide), "50000", "fifty thousand holes");

    assert_eq!(
        run(
            &mut engine,
            "var kept = null; for (let i = 0; i < 2000; i = i + 1) { kept = [i, [i]]; } kept[1][0]",
        ),
        "1999"
    );
    engine.objects().heap_mut().collect();
    assert!(engine.objects().heap().check().is_ok());
    let after_one = engine.objects().heap().live();
    assert_eq!(
        run(
            &mut engine,
            "for (let i = 0; i < 2000; i = i + 1) { kept = [i, [i]]; } kept[1][0]"
        ),
        "1999"
    );
    engine.objects().heap_mut().collect();
    let after_two = engine.objects().heap().live();
    assert!(
        after_two <= after_one.saturating_add(16),
        "{after_one} cells after one loop and {after_two} after two"
    );
}

/// The frozen service worker, or nothing if it is not where it should be.
fn service_worker() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/scripts/alo-service-worker/script.js");
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_frozen_service_worker_compiles_past_the_array_that_stopped_it() {
    let source = service_worker();
    assert!(!source.is_empty(), "the frozen script is in the corpus");
    assert_eq!(
        source.get(2847..2849),
        Some("[]"),
        "the frozen bytes are the ones the item was opened against"
    );
    let Ok(program) = script(&source) else {
        panic!("the frozen script parses");
    };
    match alo_js::compile(&program) {
        Err(Refusal::NotBuiltYet { what, at }) => {
            assert!(at > 2847, "it gets past the array at 2847, to {at}");
            // And past the `try` on the following line, which item 210
            // built; what stops it now is that item's test's business, and
            // is not an array.
            assert!(at > 2853, "and past the try at 2853, to {at}");
            assert_ne!(what, What::AClass);
        }
        // Queue item 230 built the last thing it was refused at.
        Ok(_) => {}
        other => panic!("expected the next unbuilt item, got {other:?}"),
    }
}
