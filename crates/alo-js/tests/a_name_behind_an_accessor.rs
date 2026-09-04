/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The half of queue item 214 a **script** cannot reach: a bare name whose
//! value is behind a getter.
//!
//! `document` is the example that matters. ADR 0013 § 6 says an embedder puts
//! its own things on the global object, and the DOM's are accessors rather than
//! values — a page can see that `document` is not writable and that reading it
//! runs something. Nothing in a script can *make* one until
//! `Object.defineProperty` arrives (queue item 73), so this is the only place
//! that path is exercised, and leaving it out would leave the engine able to
//! see such a name and unable to read it.

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Property, Value};
use alo_js::{numeric, script};

/// An engine whose global object has `answer` behind a getter and `taken`
/// behind a setter, both written in the language rather than in Rust.
fn engine_with_accessors() -> Result<Engine, String> {
    let mut engine = Engine::new().map_err(|why| format!("no engine: {why}"))?;
    define(&mut engine, "answer", "(function () { return 42; })", false)?;
    // The setter writes to a `var`, so a test can ask what it was given.
    define(
        &mut engine,
        "taken",
        "var seen; (function (v) { seen = v; })",
        true,
    )?;
    Ok(engine)
}

/// Evaluate `source` to a function and make it one half of an accessor property
/// of the global object called `name`.
fn define(engine: &mut Engine, name: &str, source: &str, setter: bool) -> Result<(), String> {
    let program = script(source).map_err(|why| format!("{source} did not parse: {why}"))?;
    let Ok(Value::Object(function)) = engine.evaluate(&program) else {
        return Err(format!("{source} is not a function"));
    };
    let global = engine.global().map_err(|why| why.to_string())?;
    let units: Vec<u16> = name.encode_utf16().collect();
    let property = if setter {
        Property::accessor(Value::Undefined, Value::Object(function), false, true)
    } else {
        Property::accessor(Value::Object(function), Value::Undefined, false, true)
    };
    match engine.objects().define_named(global, &units, property) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!("the global object refused {name}")),
        Err(why) => Err(why.to_string()),
    }
}

/// What a program evaluates to in an engine that already has those.
fn answer(engine: &mut Engine, source: &str) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match engine.evaluate(&program) {
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(Value::Undefined) => "undefined".to_owned(),
        Ok(Value::Text(held)) => match engine.objects().units(held) {
            Some(units) => format!("\"{}\"", String::from_utf16_lossy(units)),
            None => "\"?\"".to_owned(),
        },
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

#[test]
fn reading_such_a_name_calls_the_getter() {
    let Ok(mut engine) = engine_with_accessors() else {
        panic!("an empty heap holds an engine with two accessors on it");
    };
    assert_eq!(answer(&mut engine, "answer"), "42");
    // In an expression rather than alone, so the call's answer has to land
    // where an operand belongs.
    assert_eq!(answer(&mut engine, "answer + 1"), "43");
    // And `typeof`, which is the one instruction whose value is a question
    // about what the getter returned rather than the thing itself.
    assert_eq!(answer(&mut engine, "typeof answer"), "\"number\"");
}

#[test]
fn writing_such_a_name_calls_the_setter_and_evaluates_to_the_value() {
    let Ok(mut engine) = engine_with_accessors() else {
        panic!("an empty heap holds an engine with two accessors on it");
    };
    assert_eq!(answer(&mut engine, "taken = 7"), "7");
    assert_eq!(answer(&mut engine, "seen"), "7");
    // An assignment evaluates to what was assigned, never to what the setter
    // answered — which is why leaving that call drops its value.
    assert_eq!(answer(&mut engine, "(taken = 8) + 1"), "9");
    assert_eq!(answer(&mut engine, "seen"), "8");
}

#[test]
fn a_name_with_a_getter_and_no_setter_refuses_the_way_every_other_write_does() {
    let Ok(mut engine) = engine_with_accessors() else {
        panic!("an empty heap holds an engine with two accessors on it");
    };
    // Sloppy code is told nothing, which is the language's rule for every
    // failed write and not one this engine gets to improve on.
    assert_eq!(answer(&mut engine, "answer = 1; answer"), "42");
    let refused = answer(&mut engine, "'use strict'; answer = 1;");
    assert!(refused.contains("TypeError"), "{refused}");
    assert!(refused.contains("getter and no setter"), "{refused}");
}
