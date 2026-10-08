/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 332 (ADR 0031 §§ 1–5): a builtin declares how many values it
//! keeps, the interpreter reserves them on the stack directly above its
//! arguments, and what it keeps there survives every call it asks for and
//! every collection on the way.
//!
//! The builtins here are the test's own, each of the shape a real one will
//! have: `keeper` makes an object, keeps it, asks for a call that allocates,
//! and answers with what it kept — `map` keeping its array; `eight` fills
//! every slot it may have and checks them all after a call; `reporter` and
//! `converter` keep a number across the other two things a builtin may ask for.
//!
//! Every table runs twice, once with the collector running at every
//! allocation, and the two must agree.

use alo_js::abrupt::{Escape, Internal, Kind, Thrown};
use alo_js::bounds;
use alo_js::convert::Hint;
use alo_js::heap::Ref;
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::object::{Property, Value};
use alo_js::{numeric, script};

/// `keeper(f)`: keep a new object with `mark` 7 and the number 41, call `f`,
/// and answer the object if 41 is still there — `null` if it is not.
fn keeper(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        if call.kept(0)? != Value::Undefined || call.kept(1)? != Value::Undefined {
            return Ok(Answer::Value(Value::Null));
        }
        let made = call
            .objects()
            .object(None)
            .map_err(|why| Escape::refused(why, 0))?;
        // Kept before anything else allocates: the second object below is a
        // safepoint, and the slot is what roots the first across it.
        call.keep(0, Value::Object(made))?;
        call.objects()
            .object(None)
            .map_err(|why| Escape::refused(why, 0))?;
        let mark: Vec<u16> = "mark".encode_utf16().collect();
        let defined = call
            .objects()
            .define_named(
                made,
                &mark,
                Property::data(Value::Number(7.0), true, true, true),
            )
            .map_err(|named| Escape::named(named, 0))?;
        if !defined {
            return Ok(Answer::Value(Value::Null));
        }
        call.keep(1, Value::Number(41.0))?;
        return Ok(Answer::want(
            Want::Call {
                callee: call.argument(0),
                receiver: Value::Undefined,
                arguments: Vec::new(),
            },
            1,
        ));
    }
    if call.kept(1)? != Value::Number(41.0) {
        return Ok(Answer::Value(Value::Null));
    }
    Ok(Answer::Value(call.kept(0)?))
}

/// `plain(f)`: call `f` and answer what it did, keeping nothing — the shape
/// every builtin before ADR 0031 had.
fn plain(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        return Ok(Answer::want(
            Want::Call {
                callee: call.argument(0),
                receiver: Value::Undefined,
                arguments: Vec::new(),
            },
            1,
        ));
    }
    Ok(Answer::Value(call.answer()?))
}

/// `eight(f)`: keep 0 to 7 in all eight slots, call `f`, and answer what it
/// answered if every slot still holds its own number — `null` if one does not.
fn eight(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        for which in 0..bounds::KEPT_BY_A_BUILTIN {
            if call.kept(which)? != Value::Undefined {
                return Ok(Answer::Value(Value::Null));
            }
            call.keep(which, Value::Number(f64::from(index(which))))?;
        }
        return Ok(Answer::want(
            Want::Call {
                callee: call.argument(0),
                receiver: Value::Undefined,
                arguments: Vec::new(),
            },
            1,
        ));
    }
    for which in 0..bounds::KEPT_BY_A_BUILTIN {
        if call.kept(which)? != Value::Number(f64::from(index(which))) {
            return Ok(Answer::Value(Value::Null));
        }
    }
    Ok(Answer::Value(call.answer()?))
}

/// A slot number as the number kept in it.
fn index(which: usize) -> u32 {
    u32::try_from(which).unwrap_or(u32::MAX)
}

/// `reporter(f)`: keep 3, call `f` with its throw reported, and answer what
/// was kept plus one if it threw.
fn reporter(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        call.keep(0, Value::Number(3.0))?;
        return Ok(Answer::want(
            Want::Report {
                callee: call.argument(0),
                receiver: Value::Undefined,
                arguments: Vec::new(),
            },
            1,
        ));
    }
    let threw = if call.reported() { 1.0 } else { 0.0 };
    Ok(Answer::Value(Value::Number(call.kept_number(0)? + threw)))
}

/// `converter(o)`: keep 2, ask for `o` as a number, and answer the sum.
fn converter(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        call.keep(0, Value::Number(2.0))?;
        return Ok(Answer::want(
            Want::Primitive {
                of: call.argument(0),
                hint: Hint::Number,
            },
            1,
        ));
    }
    match call.answer()? {
        Value::Number(number) => Ok(Answer::Value(Value::Number(number + call.kept_number(0)?))),
        _ => Ok(Answer::Value(Value::Null)),
    }
}

/// `past()`: keeps one slot and writes the second.
fn past(call: &mut Call<'_>) -> Result<Answer, Escape> {
    call.keep(1, Value::Null)?;
    Ok(Answer::Value(Value::Undefined))
}

/// `nothing(...)`: answers `undefined` at once.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn nothing(_call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Undefined))
}

/// Put a native on the global object under `name`.
fn define(engine: &mut Engine, name: &str, native: Native) -> Option<Ref> {
    let global = engine.global().ok()?;
    let function = engine.function(native).ok()?;
    let units: Vec<u16> = name.encode_utf16().collect();
    // Held while the name is interned, which allocates.
    let scope = engine.objects().heap_mut().open();
    engine.objects().heap_mut().hold(function);
    let defined = engine.objects().define_named(
        global,
        &units,
        Property::data(Value::Object(function), true, false, true),
    );
    engine.objects().heap_mut().close(scope);
    (defined == Ok(true)).then_some(function)
}

/// An engine with this file's builtins on its global object.
fn engine(stress: bool) -> Option<Engine> {
    let mut engine = Engine::new().ok()?;
    define(
        &mut engine,
        "keeper",
        Native::new("keeper", keeper).keeping(2),
    )?;
    define(&mut engine, "plain", Native::new("plain", plain))?;
    define(
        &mut engine,
        "eight",
        Native::new("eight", eight).keeping(bounds::KEPT_BY_A_BUILTIN),
    )?;
    define(
        &mut engine,
        "reporter",
        Native::new("reporter", reporter).keeping(1),
    )?;
    define(
        &mut engine,
        "converter",
        Native::new("converter", converter).keeping(1),
    )?;
    define(&mut engine, "past", Native::new("past", past).keeping(1))?;
    define(&mut engine, "none", Native::new("none", nothing))?;
    define(
        &mut engine,
        "full",
        Native::new("full", nothing).keeping(bounds::KEPT_BY_A_BUILTIN),
    )?;
    engine.objects().heap_mut().stress(stress);
    Some(engine)
}

/// Run `source` and answer its global `out`, or how the run ended.
fn run(source: &str, stress: bool) -> String {
    let Some(mut engine) = engine(stress) else {
        return "no engine".to_owned();
    };
    let Ok(program) = script(source) else {
        return "did not parse".to_owned();
    };
    if let Err(trouble) = engine.evaluate(&program) {
        return format!("escaped: {trouble}");
    }
    if engine.objects().heap().check().is_err() {
        return "the heap lost something".to_owned();
    }
    let Ok(read) = script("out") else {
        return "did not parse".to_owned();
    };
    match engine.evaluate(&read) {
        Ok(Value::Text(held)) => engine
            .objects()
            .units(held)
            .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(other) => format!("{other:?}"),
        Err(_) => "no out".to_owned(),
    }
}

/// Check a table both ways.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        let ordinary = run(source, false);
        let stressed = run(source, true);
        assert_eq!(
            ordinary, stressed,
            "{source} answered differently when the collector ran at every allocation"
        );
        assert_eq!(&ordinary, expected, "{source}");
    }
}

#[test]
fn an_object_a_builtin_keeps_survives_a_call_that_allocates() {
    table(&[
        // The callee allocates enough to collect many times over; the object
        // the builtin made and kept is the one it answers.
        (
            "var o = keeper(function () { \
               var junk = []; \
               for (var i = 0; i < 64; i++) { junk = [junk, { i: i }]; } \
               return junk; \
             }); \
             var out = typeof o + '/' + o.mark;",
            "object/7",
        ),
        // Fresh slots every call: a second call does not see the first's.
        (
            "var a = keeper(() => 0); var b = keeper(() => 0); \
             var out = (a === b) + '/' + a.mark + b.mark;",
            "false/77",
        ),
        // A builtin that keeps values, called from inside the call another
        // one asked for: the inner one's region is above the outer's answer
        // slot, and neither disturbs the other.
        (
            "var out = keeper(() => keeper(() => eight(() => 1))).mark;",
            "7",
        ),
    ]);
}

#[test]
fn a_call_a_builtin_asks_for_never_disturbs_a_slot() {
    table(&[
        // A callee with locals, operands and a recursion of its own.
        (
            "var out = eight(function () { \
               var a = 1, b = 2, c = 3, d = 4; \
               function g(x) { return x > 0 ? g(x - 1) + a : b; } \
               return g(20) + c + d + [a, b, c, d].length; \
             });",
            "33",
        ),
        // Two in a row, and one inside another.
        (
            "var out = eight(() => 5) + eight(() => eight(() => 6));",
            "11",
        ),
    ]);
}

#[test]
fn a_number_kept_before_a_report_or_a_conversion_reads_back_after_it() {
    table(&[
        ("var out = reporter(() => 0);", "3"),
        // The throw is reported, the builtin carries on, and its slot is
        // still its own.
        ("var out = reporter(() => { throw 'x'; });", "4"),
        (
            "var out = converter({ valueOf: function () { var t = [1, 2, 3]; return 40; } });",
            "42",
        ),
    ]);
}

#[test]
fn a_throw_takes_the_slots_down_as_it_takes_down_a_builtin_with_none() {
    table(&[
        // The operands below the call are intact after the throw is caught,
        // whichever builtin it came through.
        (
            "function through(b) { \
               try { return b(() => { throw 'x'; }); } catch (e) { return e; } \
             } \
             var out = 'a' + through(keeper) + 'b' + through(plain) + 'c' + through(eight);",
            "axbxcx",
        ),
        // And the next builtin to keep values is given fresh slots on the
        // same ground.
        (
            "try { eight(() => { throw 1; }); } catch (e) {} \
             var out = keeper(() => 0).mark + eight(() => 2);",
            "9",
        ),
    ]);
}

#[test]
fn a_slot_past_the_count_is_this_engines_mistake() {
    let Some(mut engine) = engine(false) else {
        panic!("an engine with the test's builtins");
    };
    let Ok(program) = script("past()") else {
        panic!("it parses");
    };
    assert!(
        matches!(
            engine.evaluate(&program),
            Err(Trouble::Escaped(Escape::Broken(Internal::BuiltinIsWrong)))
        ),
        "slot 1 of a builtin that keeps one is never the answer slot"
    );
}

#[test]
fn a_builtin_that_keeps_more_than_eight_is_refused_when_it_is_made() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an engine");
    };
    assert_eq!(
        engine.function(Native::new("nine", nothing).keeping(bounds::KEPT_BY_A_BUILTIN + 1)),
        Err(Escape::Broken(Internal::BuiltinIsWrong))
    );
    assert!(
        engine
            .function(Native::new("eight", nothing).keeping(bounds::KEPT_BY_A_BUILTIN))
            .is_ok()
    );
}

#[test]
fn every_builtin_the_engine_furnishes_a_realm_with_keeps_no_more_than_eight() {
    let Ok(mut engine) = Engine::new() else {
        panic!("an engine");
    };
    let mut builtins = 0_usize;
    for native in engine.objects().natives() {
        builtins += 1;
        assert!(
            native.kept() <= bounds::KEPT_BY_A_BUILTIN,
            "{} keeps {}",
            native.name(),
            native.kept()
        );
    }
    // `Object.prototype`'s, `Function.prototype`'s, the iterators', the
    // arrays', the regular expressions' and the error constructors: the walk
    // found the realm rather than nothing.
    assert!(builtins > 20, "the walk found {builtins} builtins");
}

/// Whether `f(depth)` runs to its end with `last(0, …)` and `room` more
/// arguments at the bottom, or is the `RangeError` a stack too deep is.
///
/// Each level passes forty arguments of its own, so the stack's value bound
/// is reached at a depth well under the bound on calls.
fn reaches(depth: usize, last: &str, room: usize) -> Option<bool> {
    let wide = vec!["0"; 40].join(", ");
    let extra = vec![", 0"; room].concat();
    let source = format!(
        "function f(d) {{ return d === 0 ? {last}(0{extra}) : f(d - 1, {wide}); }} f({depth});"
    );
    let mut engine = engine(false)?;
    let program = script(&source).ok()?;
    match engine.evaluate(&program) {
        Ok(_) => Some(true),
        Err(Trouble::Escaped(Escape::Thrown(Thrown::Error {
            kind: Kind::RangeError,
            ..
        }))) => Some(false),
        Err(_) => None,
    }
}

/// The largest `n` in `0..=high` for which `works(n)` holds, where it holds
/// for every number below one that does.
fn largest(high: usize, works: impl Fn(usize) -> Option<bool>) -> Option<usize> {
    if !works(0)? {
        return None;
    }
    let (mut low, mut high) = (0, high);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if works(middle)? {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Some(low)
}

#[test]
fn reserving_slots_past_the_stacks_bound_is_the_range_error_a_deep_recursion_is() {
    // How deep `f` can go before the stack is full, less one level so there
    // is room at the bottom for a call with arguments of its own.
    let Some(deepest) = largest(bounds::CALLS_ON_THE_STACK, |depth| {
        reaches(depth, "none", 0)
    }) else {
        panic!("a shallow recursion runs");
    };
    assert!(
        deepest < bounds::CALLS_ON_THE_STACK - 1,
        "the bound on values, not on calls, is what stopped it"
    );
    let depth = deepest - 1;
    // The most arguments the bottom call can be given: a builtin that keeps
    // nothing, and one that keeps eight.
    let Some(none) = largest(400, |room| reaches(depth, "none", room)) else {
        panic!("a builtin that keeps nothing is entered");
    };
    let Some(full) = largest(400, |room| reaches(depth, "full", room)) else {
        panic!("a builtin that keeps eight is entered");
    };
    assert!(none < 400, "the search found the bound");
    assert_eq!(
        none - full,
        bounds::KEPT_BY_A_BUILTIN,
        "the eight slots are counted against the stack's bound, and past it \
         they are a RangeError rather than a stack that grows"
    );
}
