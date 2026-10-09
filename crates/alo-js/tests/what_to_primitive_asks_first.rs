/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ToPrimitive` asks for `Symbol.toPrimitive` before `valueOf` and
//! `toString` (queue item 356).
//!
//! No page can spell the symbol yet — the `Symbol` function is item 73's — so
//! `Date.prototype` is the only object a script meets with one, and
//! `what_a_date_is.rs` covers it. What a page cannot make, an embedder can:
//! these tests put a `Symbol.toPrimitive` of every shape on an object
//! through the engine's own interface — a method, a value that is not one,
//! `undefined` and `null`, and a getter answering each — and check that the
//! conversion does what ECMA-262's `GetMethod` and `ToPrimitive` say.
//!
//! The objects are made with the collector at rest, and every program then
//! runs with it collecting at every allocation as well, as the suite's other
//! files do.

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::native::{Answer, Call, Native};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Property, Value};
use alo_js::{Escape, numeric, script};

/// A `Symbol.toPrimitive` that answers the hint it was handed.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn echo(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(call.argument(0)))
}

/// A `Symbol.toPrimitive` that answers an object: its `this`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn itself(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(call.this()))
}

/// What `Symbol.toPrimitive` is on the object a program converts.
#[derive(Clone, Copy)]
enum Shape {
    /// A method answering the hint.
    Echo,
    /// A method answering an object.
    Itself,
    /// A value that is not callable.
    Number,
    /// `undefined`, which is no method.
    Undefined,
    /// `null`, which is no method either.
    Null,
    /// A getter answering the method that answers the hint.
    GetterOfEcho,
    /// A getter answering `undefined`.
    GetterOfUndefined,
    /// A getter answering a number.
    GetterOfNumber,
}

/// A getter answering `echo`'s function, which the test hangs on the global
/// object as `echo` so that it is reachable.
fn getter_of_echo(call: &mut Call<'_>) -> Result<Answer, Escape> {
    global_named(call, "echo")
}

/// A getter answering `undefined`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn getter_of_undefined(_: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Undefined))
}

/// A getter answering a number.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn getter_of_number(_: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Number(1.0)))
}

/// What the global object's property `name` holds, read through the object
/// the getter was called on — the converted object, whose `holder` is the
/// global object.
fn global_named(call: &mut Call<'_>, name: &str) -> Result<Answer, Escape> {
    let Value::Object(this) = call.this() else {
        return Ok(Answer::Value(Value::Undefined));
    };
    let objects = call.objects();
    let holder: Vec<u16> = "holder".encode_utf16().collect();
    let Some(key) = objects.existing_key(&holder) else {
        return Ok(Answer::Value(Value::Undefined));
    };
    let Ok(alo_js::Found::Value(Value::Object(global))) = objects.get(this, key) else {
        return Ok(Answer::Value(Value::Undefined));
    };
    let units: Vec<u16> = name.encode_utf16().collect();
    let Some(key) = objects.existing_key(&units) else {
        return Ok(Answer::Value(Value::Undefined));
    };
    match objects.get(global, key)? {
        alo_js::Found::Value(value) => Ok(Answer::Value(value)),
        _ => Ok(Answer::Value(Value::Undefined)),
    }
}

/// An engine with `o` on its global object, whose `Symbol.toPrimitive` has
/// this shape, and `echo` beside it.
fn engine_with(shape: Shape) -> Result<Engine, String> {
    let mut engine = Engine::new().map_err(|why| why.to_string())?;
    let global = engine.global().map_err(|why| why.to_string())?;
    let echo_function = engine
        .function(Native::new("echo", echo))
        .map_err(|why| why.to_string())?;
    let units: Vec<u16> = "echo".encode_utf16().collect();
    engine
        .objects()
        .define_named(
            global,
            &units,
            Property::plain(Value::Object(echo_function)),
        )
        .map_err(|why| why.to_string())?;
    let above = engine
        .intrinsics()
        .0
        .object_prototype(engine.intrinsics().1)
        .map_err(|why| why.to_string())?;
    let object = engine
        .objects()
        .object(Some(above))
        .map_err(|why| why.to_string())?;
    let units: Vec<u16> = "o".encode_utf16().collect();
    engine
        .objects()
        .define_named(global, &units, Property::plain(Value::Object(object)))
        .map_err(|why| why.to_string())?;
    let units: Vec<u16> = "holder".encode_utf16().collect();
    engine
        .objects()
        .define_named(object, &units, Property::plain(Value::Object(global)))
        .map_err(|why| why.to_string())?;
    let symbol = engine
        .well_known(WellKnown::ToPrimitive)
        .map_err(|why| why.to_string())?;
    let key = engine
        .objects()
        .symbol_key(symbol)
        .map_err(|why| why.to_string())?;
    let property = match shape {
        Shape::Echo => Property::plain(Value::Object(echo_function)),
        Shape::Itself => {
            let function = engine
                .function(Native::new("itself", itself))
                .map_err(|why| why.to_string())?;
            Property::plain(Value::Object(function))
        }
        Shape::Number => Property::plain(Value::Number(1.0)),
        Shape::Undefined => Property::plain(Value::Undefined),
        Shape::Null => Property::plain(Value::Null),
        Shape::GetterOfEcho | Shape::GetterOfUndefined | Shape::GetterOfNumber => {
            let body = match shape {
                Shape::GetterOfEcho => getter_of_echo,
                Shape::GetterOfUndefined => getter_of_undefined,
                _ => getter_of_number,
            };
            let getter = engine
                .function(Native::new("get", body))
                .map_err(|why| why.to_string())?;
            Property::accessor(Value::Object(getter), Value::Undefined, false, true)
        }
    };
    engine
        .objects()
        .define(object, key, property)
        .map_err(|why| why.to_string())?;
    Ok(engine)
}

/// Run a program over `o` of this shape, both ways, and answer what it
/// produced.
fn converted(shape: Shape, source: &str) -> String {
    let ordinary = run(shape, source, false);
    let stressed = run(shape, source, true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// One run.
fn run(shape: Shape, source: &str, stress: bool) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    let mut engine = match engine_with(shape) {
        Ok(engine) => engine,
        Err(why) => return format!("no engine: {why}"),
    };
    engine.objects().heap_mut().stress(stress);
    match engine.evaluate(&program) {
        Ok(Value::Text(held)) => match engine.objects().units(held) {
            Some(units) => format!("\"{}\"", String::from_utf16_lossy(units)),
            None => "\"?\"".to_owned(),
        },
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

#[test]
fn a_method_is_handed_the_hint_as_a_string_and_its_answer_is_final() {
    assert_eq!(converted(Shape::Echo, "o + ''"), "\"default\"");
    assert_eq!(converted(Shape::Echo, "`${o}`"), "\"string\"");
    assert_eq!(converted(Shape::Echo, "o * 1"), "NaN");
    assert_eq!(
        converted(
            Shape::Echo,
            "var r = ''; o.valueOf = function () { r = 'ran'; return 1; }; o + r"
        ),
        "\"default\"",
        "valueOf is never asked when there is a method"
    );
}

#[test]
fn a_method_that_answers_an_object_is_a_type_error_with_no_second_name_to_try() {
    assert_eq!(
        converted(
            Shape::Itself,
            "try { o + ''; } catch (e) { e.name + ': ' + e.message }"
        ),
        "\"TypeError: Symbol.toPrimitive answered an object, which is not a primitive value\""
    );
}

#[test]
fn a_value_that_is_not_callable_is_a_type_error() {
    assert_eq!(
        converted(
            Shape::Number,
            "try { o + ''; } catch (e) { e.name + ': ' + e.message }"
        ),
        "\"TypeError: this object's Symbol.toPrimitive is not a function\""
    );
}

#[test]
fn undefined_and_null_are_no_method_and_the_ordinary_search_runs() {
    for shape in [Shape::Undefined, Shape::Null] {
        assert_eq!(converted(shape, "o + ''"), "\"[object Object]\"");
        assert_eq!(
            converted(shape, "o.valueOf = function () { return 3; }; o * 2"),
            "6"
        );
    }
}

#[test]
fn a_getter_is_asked_for_the_method_and_what_it_answers_is_judged_the_same() {
    assert_eq!(converted(Shape::GetterOfEcho, "o + ''"), "\"default\"");
    assert_eq!(converted(Shape::GetterOfEcho, "o - 0"), "NaN");
    assert_eq!(
        converted(Shape::GetterOfUndefined, "o + ''"),
        "\"[object Object]\""
    );
    assert_eq!(
        converted(
            Shape::GetterOfNumber,
            "try { o + ''; } catch (e) { e.name + ': ' + e.message }"
        ),
        "\"TypeError: the getter for Symbol.toPrimitive answered something that is not a function\""
    );
}
