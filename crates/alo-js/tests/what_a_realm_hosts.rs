/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 260, the engine's half (ADR 0019 § 1): a realm's
//! `[[HostDefined]]`.
//!
//! *A native whose `this` reaches nothing — a constructor's fresh instance —
//! must still find its page.* The embedder here is the test: it sets the
//! realm's host to an object of its own, holding a `name`, and `host()` is a
//! native that answers what it was handed. The engine roots the host, so the
//! test keeps no root of its own, and every script runs with the collector at
//! every allocation as well as without.

use alo_js::abrupt::Escape;
use alo_js::heap::Ref;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::native::{Answer, Call, Native};
use alo_js::object::{Property, Value};
use alo_js::{numeric, script};

/// `host()`: the realm's host, or `null` when its embedder set none.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn host(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(
        call.host_defined().map_or(Value::Null, Value::Object),
    ))
}

/// Put `host` on the global object.
fn with_host_function(engine: &mut Engine) -> Option<()> {
    let global = engine.global().ok()?;
    let function = engine.function(Native::new("host", host)).ok()?;
    let scope = engine.objects().heap_mut().open();
    engine.objects().heap_mut().hold(function);
    let name: Vec<u16> = "host".encode_utf16().collect();
    let defined = engine.objects().define_named(
        global,
        &name,
        Property::data(Value::Object(function), true, false, true),
    );
    engine.objects().heap_mut().close(scope);
    matches!(defined, Ok(true)).then_some(())
}

/// An object with `name` set to `called`, held by nothing.
fn named_object(engine: &mut Engine, called: &str) -> Option<Ref> {
    let object = engine.objects().object(None).ok()?;
    let scope = engine.objects().heap_mut().open();
    engine.objects().heap_mut().hold(object);
    let units: Vec<u16> = called.encode_utf16().collect();
    let text = engine.objects().text(units);
    let defined = text.ok().map(|text| {
        let name: Vec<u16> = "name".encode_utf16().collect();
        engine
            .objects()
            .define_named(object, &name, Property::plain(Value::Text(text)))
    });
    engine.objects().heap_mut().close(scope);
    matches!(defined, Some(Ok(true))).then_some(object)
}

/// What `source` evaluates to, as a string.
fn evaluate(engine: &mut Engine, source: &str) -> String {
    let Ok(program) = script(source) else {
        return "? did not parse".to_owned();
    };
    match engine.evaluate(&program) {
        Ok(Value::Text(held)) => engine
            .objects()
            .units(held)
            .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
        Ok(Value::Bool(answer)) => answer.to_string(),
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(Value::Null) => "null".to_owned(),
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// An engine with `host` on its global object and, if `called` is given, a
/// host of that name — both ways, collecting or not.
fn engine(called: Option<&str>, stress: bool) -> Option<Engine> {
    let mut engine = Engine::new().ok()?;
    with_host_function(&mut engine)?;
    if let Some(called) = called {
        let object = named_object(&mut engine, called)?;
        engine.host_defined(object).ok()?;
    }
    engine.objects().heap_mut().stress(stress);
    Some(engine)
}

/// Run `source` both ways and answer what it answered.
fn run(called: Option<&str>, source: &str) -> String {
    let mut answers = Vec::new();
    for stress in [false, true] {
        let Some(mut engine) = engine(called, stress) else {
            return "the engine was not made".to_owned();
        };
        answers.push(evaluate(&mut engine, source));
    }
    let stressed = answers.pop().unwrap_or_default();
    let ordinary = answers.pop().unwrap_or_default();
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

#[test]
fn a_native_is_handed_the_host_its_embedder_set() {
    assert_eq!(run(Some("page"), "host().name"), "page");
    // The same object every time: one reference, not a copy per call.
    assert_eq!(run(Some("page"), "host() === host()"), "true");
}

#[test]
fn a_realm_whose_embedder_set_nothing_hands_over_nothing() {
    assert_eq!(run(None, "host()"), "null");
}

#[test]
fn the_realm_roots_its_host() {
    // Nothing but the realm holds the host, and a script that allocates a
    // great deal still finds it — with the collector at every allocation.
    assert_eq!(
        run(
            Some("kept"),
            "var a = null; for (var i = 0; i < 200; i++) { a = { i: i, next: a }; } host().name",
        ),
        "kept"
    );
    let Some(mut engine) = engine(Some("kept"), false) else {
        panic!("the engine was not made");
    };
    engine.objects().heap_mut().collect();
    assert_eq!(evaluate(&mut engine, "host().name"), "kept");
}

#[test]
fn a_host_is_defined_once_and_the_first_stands() {
    let Some(mut engine) = engine(Some("first"), false) else {
        panic!("the engine was not made");
    };
    let Some(second) = named_object(&mut engine, "second") else {
        panic!("the second host was not made");
    };
    let refused = engine.host_defined(second);
    assert!(
        matches!(&refused, Err(escape) if escape.to_string().contains("already defined")),
        "a second host was not refused: {refused:?}"
    );
    assert_eq!(evaluate(&mut engine, "host().name"), "first");
}
