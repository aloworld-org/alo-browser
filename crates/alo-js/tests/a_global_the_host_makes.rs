/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0037 § 1, for queue item 362: a realm whose global object **the
//! embedder makes**, as ECMAScript's `InitializeHostDefinedRealm` lets a host
//! do and as a page's `Window` needs.
//!
//! The engine furnishes the embedder's object with the language's values and
//! builtins exactly as it furnishes an ordinary one, and every way a script
//! reaches the global — a name resolved, a `var` or a function declared,
//! `globalThis`, the top-level `this`, an assignment in sloppy and in strict
//! code, `delete` and `typeof` — goes through that object's own internal
//! methods. The embedder's object here is the test's own, so nothing about a
//! window is assumed: it holds an ordinary object's part, and one more edge
//! of its own that the collector must walk.
//!
//! Every program runs twice, once ordinarily and once with the collector at
//! every allocation, and the two must agree.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Exotic, Found, Internal, Key, Ordinary, Property, Value};
use alo_js::{numeric, script};

/// The test's own global object: an ordinary part, and an edge to an object
/// only it holds.
#[derive(Debug)]
struct Mine {
    own: Ordinary,
    kept: Field,
}

impl Internal for Mine {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Mine {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.kept.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for Mine {
    fn describe(&self) -> &'static str {
        "the test's global object"
    }
}

/// How the engine makes the global: from `Object.prototype`.
fn make(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(Mine {
        own: Ordinary::with_prototype(prototype),
        kept: Field::default(),
    })
}

/// An engine whose global is [`Mine`], collecting at every allocation when
/// `stress` says so.
fn engine(stress: bool) -> Result<Engine, String> {
    let mut engine = Engine::with_global(make, None).map_err(|why| why.to_string())?;
    engine.objects().heap_mut().stress(stress);
    Ok(engine)
}

/// What `source` evaluates to, in `engine`.
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

/// Each program, in a fresh engine of each kind, against what it answers.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        let answers: Vec<String> = [false, true]
            .into_iter()
            .map(|stress| match engine(stress) {
                Ok(mut engine) => run(&mut engine, source),
                Err(why) => format!("no engine: {why}"),
            })
            .collect();
        assert_eq!(
            answers.first(),
            answers.get(1),
            "{source} answered differently when the collector ran at every allocation"
        );
        assert_eq!(
            answers.first().map(String::as_str),
            Some(*expected),
            "{source}"
        );
    }
}

#[test]
fn the_global_object_is_the_embedders_and_the_language_is_on_it() {
    let Ok(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    let Ok(global) = engine.global() else {
        panic!("the engine has a global object");
    };
    assert!(
        engine.objects().embedded::<Mine>(global).is_some(),
        "the global object is the one the embedder made"
    );
    let Ok(Ok(Value::Object(this))) = script("globalThis").map(|program| engine.evaluate(&program))
    else {
        panic!("globalThis is an object");
    };
    assert_eq!(this, global, "globalThis is that object");
    table(&[
        (
            "typeof undefined + ' ' + NaN + ' ' + Infinity",
            "\"undefined NaN Infinity\"",
        ),
        ("globalThis === this", "true"),
        ("globalThis.globalThis === globalThis", "true"),
        (
            "typeof TypeError + typeof Promise + typeof Date + typeof encodeURIComponent",
            "\"functionfunctionfunctionfunction\"",
        ),
        ("encodeURIComponent('a b')", "\"a%20b\""),
        ("new TypeError('x').message", "\"x\""),
        // It inherits from the realm's `Object.prototype`.
        ("typeof globalThis.hasOwnProperty", "\"function\""),
        ("globalThis.hasOwnProperty('Date')", "true"),
        // `undefined` is as fixed on it as on an ordinary one.
        ("undefined = 1; typeof undefined", "\"undefined\""),
        (
            "'use strict'; try { undefined = 1; 'assigned' } catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn every_way_a_script_reaches_the_global_reaches_the_embedders_object() {
    table(&[
        // A `var` is a property of it, and not deletable.
        ("var a = 1; globalThis.a", "1"),
        ("var a = 1; delete a", "false"),
        ("var a = 1; globalThis.hasOwnProperty('a')", "true"),
        // A function declaration too.
        ("function f() { return 2; } globalThis.f()", "2"),
        ("function f() {} typeof globalThis.f", "\"function\""),
        // A property put on it is a name.
        ("globalThis.b = 3; b", "3"),
        ("this.c = 4; c", "4"),
        // Sloppy assignment to nothing makes a property, which `delete` takes.
        ("d = 5; globalThis.d", "5"),
        ("d = 5; delete d; typeof d", "\"undefined\""),
        // Strict assignment to nothing is a `ReferenceError`.
        (
            "'use strict'; try { e = 6; 'assigned' } catch (x) { x.name }",
            "\"ReferenceError\"",
        ),
        // Strict assignment to a name that is there goes through.
        ("'use strict'; globalThis.g = 1; g = 7; globalThis.g", "7"),
        // A `let` is not on it, and shadows a property of the same name.
        ("let h = 8; globalThis.h", "undefined"),
        ("globalThis.i = 1; let j = 2; i + j", "3"),
        // Reading a name nothing declares is a `ReferenceError`, and
        // `typeof` of one is not.
        (
            "try { nothing; 'read' } catch (x) { x.name }",
            "\"ReferenceError\"",
        ),
        ("typeof nothing", "\"undefined\""),
        // Inside a function, a free name is still the global's.
        ("var k = 9; (function () { return k; })()", "9"),
        ("(function () { l = 10; })(); globalThis.l", "10"),
    ]);
}

#[test]
fn what_the_embedders_object_holds_and_what_is_put_on_it_survive_collections() {
    let Ok(mut engine) = engine(true) else {
        panic!("an empty heap holds an engine");
    };
    let Ok(global) = engine.global() else {
        panic!("the engine has a global object");
    };
    // An object only the global's own edge holds.
    let Ok(kept) = engine.objects().object(None) else {
        panic!("an empty heap holds an object");
    };
    let Some(()) = engine
        .objects()
        .write_embedded::<Mine, _>(global, |held, barrier| {
            held.kept.set(barrier, Some(kept));
        })
    else {
        panic!("the global object is the embedder's");
    };
    // Marked once the global holds it, so a slot swept and used again for
    // something else does not read as it.
    let marker: Vec<u16> = "marker".encode_utf16().collect();
    let Ok(true) =
        engine
            .objects()
            .define_named(kept, &marker, Property::plain(Value::Number(7.0)))
    else {
        panic!("an empty object takes a property");
    };
    assert_eq!(
        run(
            &mut engine,
            "var held = { n: 1 }; function f() { return held.n; } \
             for (var i = 0; i < 200; i = i + 1) { var junk = { i: i }; } f()",
        ),
        "1"
    );
    assert_eq!(run(&mut engine, "held.n + f()"), "2");
    assert!(
        engine
            .objects()
            .embedded::<Mine>(global)
            .and_then(|held| held.kept.get())
            .is_some_and(|still| still == kept),
        "the embedder's edge is kept"
    );
    let read = engine
        .objects()
        .existing_key(&marker)
        .map(|key| engine.objects().get(kept, key));
    assert!(
        matches!(read, Some(Ok(Found::Value(Value::Number(seven)))) if seven.to_bits() == 7.0_f64.to_bits()),
        "what the embedder's edge holds is the object it was given: {read:?}"
    );
    assert!(engine.objects().heap().check().is_ok(), "the heap is whole");
}
