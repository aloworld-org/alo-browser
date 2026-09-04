/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 218's closing conditions, in the table ADR 0013 § 9 asks for.
//!
//! *A `{}` has a `toString` of its own; a builtin is called by the same
//! machinery a script's function is; a page may replace one; and what is not
//! built refuses by name rather than answering something plausible.*
//!
//! # Every program runs twice, and here that is not a formality
//!
//! Once ordinarily and once with [`Heap::stress`] on — collecting at every
//! allocation. A builtin allocates while its arguments are in a Rust slice and
//! its `this` is in a Rust local, and the argument that this is safe is that
//! both are *also* still on the interpreter's stack. That argument is checked
//! here rather than believed: under stress, a builtin holding the only
//! reference to something would answer differently or fault.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{numeric, script};

/// Run a program in a fresh engine, both ways, and answer what it produced.
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
fn an_object_has_a_to_string_of_its_own() {
    table(&[
        ("({}).toString()", "\"[object Object]\""),
        ("({}) + ''", "\"[object Object]\""),
        ("`${{}}`", "\"[object Object]\""),
        ("'' + { a: 1 }", "\"[object Object]\""),
        ("typeof ({}).toString", "\"function\""),
        // A function is the one tag this engine can tell apart, because
        // `IsCallable` is a question about the cell rather than about a
        // builtin it has not written.
        ("({}).toString.call === undefined", "true"),
        (
            "let f = function () {}; ({}).__proto__.toString.apply === undefined",
            "true",
        ),
    ]);
}

#[test]
fn a_builtin_is_found_the_way_every_other_property_is() {
    table(&[
        // The conversion asks for `valueOf` first, `Object.prototype.valueOf`
        // hands the object back, and the search moves on to `toString` — three
        // calls into a builtin to evaluate one `+`.
        ("1 + {}", "\"1[object Object]\""),
        ("let a = {}; a.valueOf() === a", "true"),
        ("({}).valueOf().valueOf() !== null", "true"),
        // A page's own method shadows the builtin, because it is an own
        // property and the builtin is the prototype's.
        (
            "let o = { toString: function () { return 'mine'; } }; o + ''",
            "\"mine\"",
        ),
        (
            "let o = { valueOf: function () { return 42; } }; o + 1",
            "43",
        ),
        // A script's `valueOf` that answers an object has converted nothing, so
        // the search carries on to the *builtin* `toString`. Both kinds of
        // function in one conversion.
        (
            "let o = { valueOf: function () { return {}; } }; o + ''",
            "\"[object Object]\"",
        ),
        // A property that is not callable is skipped rather than thrown at, and
        // running out of names is the `TypeError` the specification gives.
        (
            "let o = { toString: 1 }; o + ''",
            "! TypeError: this object has no valueOf or toString, so it cannot become a primitive value (at byte 25)",
        ),
        // The prototype is one object, so writing to it is visible everywhere —
        // which is what makes a builtin method writable in the first place.
        (
            "({}).__proto__.toString = function () { return 'x'; }; ({}) + ''",
            "\"x\"",
        ),
    ]);
}

#[test]
fn a_builtin_is_strict_so_its_this_is_what_the_caller_wrote() {
    table(&[
        // A plain call pushes `undefined`, and a builtin is strict code, so
        // `OrdinaryCallBindThis` does not turn it into the global object.
        ("toString()", "\"[object Undefined]\""),
        ("globalThis.toString()", "\"[object Object]\""),
        // The global object inherits from `Object.prototype` like anything
        // else, which is why the bare name resolves at all.
        ("globalThis.__proto__ === ({}).__proto__", "true"),
        // `undefined` and `null` are answered before `ToObject`, which is the
        // one place a primitive `this` is not an error.
        ("({}).toString.bind === undefined", "true"),
    ]);
}

#[test]
fn has_own_property_asks_about_this_object_and_not_its_prototype() {
    table(&[
        ("({ a: 1 }).hasOwnProperty('a')", "true"),
        ("({ a: 1 }).hasOwnProperty('b')", "false"),
        // Inherited is not own, which is the whole reason the method exists.
        ("({}).hasOwnProperty('toString')", "false"),
        ("({}).__proto__.hasOwnProperty('toString')", "true"),
        // No argument is the property named `"undefined"` rather than a
        // refusal: the language has no missing argument.
        ("({}).hasOwnProperty()", "false"),
        ("({ undefined: 1 }).hasOwnProperty()", "true"),
        ("({ 1: 'a' }).hasOwnProperty(1)", "true"),
        ("({ 1: 'a' }).hasOwnProperty('1')", "true"),
        (
            "let a = {}; a.__proto__ = null; a.hasOwnProperty === undefined",
            "true",
        ),
    ]);
}

#[test]
fn a_thing_is_not_its_own_prototype() {
    table(&[
        ("let a = {}; a.isPrototypeOf(a)", "false"),
        (
            "let a = {}; let b = {}; b.__proto__ = a; a.isPrototypeOf(b)",
            "true",
        ),
        (
            "let a = {}; let b = {}; b.__proto__ = a; b.isPrototypeOf(a)",
            "false",
        ),
        // Two steps up the chain is still up the chain.
        (
            "let a = {}; let b = {}; let c = {}; b.__proto__ = a; c.__proto__ = b; a.isPrototypeOf(c)",
            "true",
        ),
        ("({}).__proto__.isPrototypeOf({})", "true"),
        // A primitive is not an object, so nothing is its prototype — and that
        // is answered before `this` is looked at.
        ("({}).isPrototypeOf(1)", "false"),
        ("({}).isPrototypeOf()", "false"),
        ("({}).isPrototypeOf(null)", "false"),
    ]);
}

#[test]
fn a_builtin_method_is_not_enumerable_and_a_page_property_is() {
    table(&[
        ("({ a: 1 }).propertyIsEnumerable('a')", "true"),
        // The point of the attribute: a prototype full of methods must not turn
        // up in a page's own loop over its own object.
        ("({}).__proto__.propertyIsEnumerable('toString')", "false"),
        ("({}).__proto__.propertyIsEnumerable('__proto__')", "false"),
        // Not an own property at all, which is `false` rather than an error.
        ("({}).propertyIsEnumerable('toString')", "false"),
        ("({}).propertyIsEnumerable('nothing')", "false"),
    ]);
}

#[test]
fn proto_reads_and_writes_the_prototype_and_ignores_what_it_cannot_use() {
    table(&[
        // One `Object.prototype`, reached from two different objects.
        ("({}).__proto__ === ({}).__proto__", "true"),
        ("({}).__proto__.__proto__", "null"),
        (
            "let a = {}; let b = {}; a.__proto__ = b; a.__proto__ === b",
            "true",
        ),
        // Cutting the chain takes the accessor with it: `__proto__` lives on
        // `Object.prototype`, so an object with no prototype has no such name
        // to read and answers `undefined` rather than `null`.
        ("let a = {}; a.__proto__ = null; a.__proto__", "undefined"),
        (
            "let a = {}; a.__proto__ = null; typeof a.toString",
            "\"undefined\"",
        ),
        // Neither an object nor `null`, so it is quietly nothing — the
        // specification's own answer, and the reason the setter has four cases.
        (
            "let a = {}; a.__proto__ = 1; a.__proto__ === ({}).__proto__",
            "true",
        ),
        (
            "let a = {}; a.__proto__ = 'x'; a.__proto__ === ({}).__proto__",
            "true",
        ),
        // An assignment evaluates to what was assigned whatever the setter did.
        ("let a = {}; a.__proto__ = 1", "1"),
        // A cycle is refused by the object model, and the refusal reaches the
        // page as the `TypeError` the specification gives.
        (
            "let a = {}; let b = {}; a.__proto__ = b; b.__proto__ = a",
            "! TypeError: this object's prototype cannot be changed: it is not extensible, or the new prototype is already below it (at byte 41)",
        ),
        // Reading a property of what a null prototype leaves behind.
        (
            "({}).__proto__.__proto__.toString",
            "! TypeError: cannot read property 'toString' of null (at byte 0)",
        ),
    ]);
}

#[test]
fn a_function_inherits_from_function_prototype_which_inherits_from_object_prototype() {
    table(&[
        (
            "let f = function () {}; let g = function () {}; f.__proto__ === g.__proto__",
            "true",
        ),
        (
            "let f = function () {}; f.__proto__.__proto__ === ({}).__proto__",
            "true",
        ),
        // `Function.prototype` is itself a function, and calling it answers
        // `undefined` for any arguments at all.
        ("let f = function () {}; typeof f.__proto__", "\"function\""),
        ("let f = function () {}; f.__proto__()", "undefined"),
        ("let f = function () {}; f.__proto__(1, 2, 3)", "undefined"),
        ("typeof ({}).toString.__proto__", "\"function\""),
        // A builtin is an ordinary object, so a page may hang a property on one.
        ("let t = ({}).toString; t.mine = 1; t.mine", "1"),
        // `Object.prototype.toString` tells a function apart from an object,
        // because `IsCallable` asks about the cell rather than about a builtin.
        (
            "let f = function () {}; ({}).__proto__.toString.mine = 1; typeof f",
            "\"function\"",
        ),
    ]);
}

#[test]
fn what_this_engine_has_not_built_refuses_by_name() {
    table(&[
        // A function's source text is queue item 220. Left to
        // `Object.prototype.toString` it would answer `"[object Function]"` —
        // a sentence no engine produces, handed over as though it were right.
        (
            "(function () {}) + ''",
            "! a function's own source text, which Function.prototype.toString answers with, is queue item 220",
        ),
        (
            "let f = function () {}; f.toString()",
            "! a function's own source text, which Function.prototype.toString answers with, is queue item 220",
        ),
        // Turning an argument into a property key means calling the script's
        // own `valueOf`, which a builtin cannot do until queue item 219. Only
        // the object case: every primitive converts here and now.
        (
            "({}).hasOwnProperty({})",
            "! a builtin was given an object where a property key was wanted, and turning one into a primitive from inside a builtin is queue item 219",
        ),
        (
            "({}).propertyIsEnumerable({})",
            "! a builtin was given an object where a property key was wanted, and turning one into a primitive from inside a builtin is queue item 219",
        ),
        // A primitive `this` needs the wrapper object the builtins bring, and
        // the property read says so before the builtin is even reached.
        (
            "'a'.toString()",
            "! a property of a primitive needs a wrapper object, which is queue item 73",
        ),
    ]);
}

#[test]
fn a_builtin_called_by_a_hostile_program_refuses_rather_than_breaking() {
    table(&[
        // `undefined` and `null` are the two the specification names outright,
        // and they are a `TypeError` a page can catch rather than a fault.
        (
            "let f = ({}).valueOf; f()",
            "! TypeError: Object.prototype.valueOf was called on undefined or null (at byte 22)",
        ),
        (
            "let f = ({}).hasOwnProperty; f('a')",
            "! TypeError: Object.prototype.hasOwnProperty was called on undefined or null (at byte 29)",
        ),
        // The argument is an object, so `isPrototypeOf` does not answer `false`
        // early and reaches `ToObject(this)` — which is the order the
        // specification gives and the only way to see it.
        (
            "let f = ({}).isPrototypeOf; f({})",
            "! TypeError: Object.prototype.isPrototypeOf was called on undefined or null (at byte 28)",
        ),
        ("let f = ({}).isPrototypeOf; f(1)", "false"),
        // More arguments than a builtin reads is not an error: the extra ones
        // are on the stack and go with the call.
        ("({ a: 1 }).hasOwnProperty('a', 2, 3, 4, 5)", "true"),
        // A builtin whose `this` is the object it is defining a property on,
        // reached through a chain a script built.
        (
            "let a = {}; let b = {}; b.__proto__ = a; a.mine = 1; b.hasOwnProperty('mine')",
            "false",
        ),
        // A method taken off the prototype and called with an explicit
        // receiver, which is the shape a real page writes.
        (
            "let a = {}; let has = ({}).__proto__.hasOwnProperty; a.x = 1; a.hasOwnProperty('x')",
            "true",
        ),
        // And once the chain is cut, `__proto__` is an ordinary name: the
        // assignment makes an own data property rather than reaching a setter,
        // so the object's prototype stays null and the name reads back an
        // object. A page that wanted the prototype has to be given the accessor
        // again, which is what `Object.setPrototypeOf` is for.
        (
            "let a = {}; a.__proto__ = null; a.__proto__ = {}; a.__proto__",
            "an object",
        ),
        (
            "let a = {}; a.__proto__ = null; a.__proto__ = {}; typeof a.toString",
            "\"undefined\"",
        ),
    ]);
}
