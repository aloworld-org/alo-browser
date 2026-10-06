/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 254, the engine's half (ADR 0018 § 3): a builtin may ask for a
//! call whose throw is **reported rather than propagated**, and a builtin
//! constructor may be given an embedder's instance.
//!
//! *A listener that throws must not throw out of `dispatchEvent`: the standard
//! reports it and carries on with the next listener.* The embedder here is
//! the test, and `each(f, g, …)` is a builtin of `dispatchEvent`'s shape: it
//! calls each argument in turn, asking for each call with [`Want::Report`],
//! and answers how many of them threw. What it set aside is handed to the
//! test after the run, as an embedder is handed it.
//!
//! Every table runs twice, once with the collector running at every
//! allocation, and the two must agree: a thrown object waits for its report
//! across everything the rest of the run allocates.

use alo_js::abrupt::{Escape, Thrown};
use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::interpret::{Engine, Trouble, Unwound};
use alo_js::object::native::{Answer, Call, Instance, Native, Want};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property, Value};
use alo_js::{numeric, script};

/// `each(...callees)`: call each with its index, reporting what throws, and
/// answer how many threw.
///
/// A step is the next callee's index, plus how many have thrown so far
/// shifted above it — a builtin keeps nothing else across a call.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn each(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let step = call.step();
    let (next, threw) = (step & 0xffff, (step >> 16) + u32::from(call.reported()));
    let Ok(index) = usize::try_from(next) else {
        return Ok(Answer::Value(Value::Undefined));
    };
    if index >= call.count() {
        return Ok(Answer::Value(Value::Number(f64::from(threw))));
    }
    Ok(Answer::want(
        Want::Report {
            callee: call.argument(index),
            receiver: Value::Undefined,
            arguments: vec![Value::Number(f64::from(next))],
        },
        (threw << 16) | (next + 1),
    ))
}

/// A builtin that throws a `TypeError` of its own.
fn thrower(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Err(Escape::type_error("a builtin threw", call.at()))
}

/// An embedder's object: nothing but an ordinary object's part.
#[derive(Debug)]
struct Thing {
    own: Ordinary,
}

impl Internal for Thing {
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

impl Trace for Thing {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
    }
    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for Thing {
    fn describe(&self) -> &'static str {
        "a thing"
    }
}

/// How `Thing` makes its instance.
fn make_thing(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(Thing {
        own: Ordinary::with_prototype(prototype),
    })
}

/// `new Thing()`: the instance it was given, and a `TypeError` without `new`.
fn thing(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if !call.constructing() {
        return Err(Escape::type_error("Thing needs 'new'", call.at()));
    }
    Ok(Answer::Value(call.this()))
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

/// An engine with `each`, `thrower` and `Thing` on its global object.
fn engine(stress: bool) -> Option<Engine> {
    let mut engine = Engine::new().ok()?;
    define(&mut engine, "each", Native::new("each", each))?;
    define(&mut engine, "thrower", Native::new("thrower", thrower))?;
    let constructor = define(
        &mut engine,
        "Thing",
        Native::constructor("Thing", thing, Instance::Made(make_thing)),
    )?;
    let (intrinsics, objects) = engine.intrinsics();
    let above = intrinsics.object_prototype(objects).ok()?;
    let prototype = engine.objects().object(Some(above)).ok()?;
    let name: Vec<u16> = "prototype".encode_utf16().collect();
    let defined = engine.objects().define_named(
        constructor,
        &name,
        Property::data(Value::Object(prototype), false, false, false),
    );
    if defined != Ok(true) {
        return None;
    }
    engine.objects().heap_mut().stress(stress);
    Some(engine)
}

/// What was thrown, in words.
fn said(objects: &Objects, thrown: &Thrown, unwound: &Unwound) -> String {
    let what = match thrown {
        Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
        Thrown::Value { value, .. } => match value {
            Value::Text(held) => match objects.units(*held) {
                Some(units) => format!("threw \"{}\"", String::from_utf16_lossy(units)),
                None => "threw a string that has gone".to_owned(),
            },
            Value::Number(number) => format!("threw {}", numeric::text_of(*number)),
            Value::Object(held) if objects.heap().live_at(*held) => "threw an object".to_owned(),
            Value::Object(_) => "threw an object that has gone".to_owned(),
            other => format!("threw {other:?}"),
        },
    };
    format!("{what} [{} calls]", unwound.places().len())
}

/// Run `source`, then hand over what was reported; answer `out`, then each
/// report, then the run's own escape if it had one.
fn run(source: &str, stress: bool) -> String {
    let Some(mut engine) = engine(stress) else {
        return "no engine".to_owned();
    };
    let Ok(program) = script(source) else {
        return "did not parse".to_owned();
    };
    let outcome = engine.evaluate(&program);
    let mut reports = Vec::new();
    let unreported = engine.hand_over_reported(&mut |objects, thrown, unwound| {
        reports.push(said(objects, thrown, unwound));
    });
    if unreported > 0 {
        reports.push(format!("and {unreported} more"));
    }
    if let Err(trouble) = outcome {
        reports.push(match trouble {
            Trouble::Escaped(escape) => format!("escaped: {escape}"),
            Trouble::NotCompiled(refusal) => format!("not compiled: {refusal}"),
        });
    }
    assert_eq!(engine.reported_waiting(), 0, "everything was handed over");
    let Ok(read) = script("out") else {
        return "did not parse".to_owned();
    };
    let out = match engine.evaluate(&read) {
        Ok(Value::Text(held)) => engine
            .objects()
            .units(held)
            .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
        Ok(Value::Number(number)) => numeric::text_of(number),
        Ok(other) => format!("{other:?}"),
        Err(_) => "no out".to_owned(),
    };
    if reports.is_empty() {
        out
    } else {
        format!("{out} | {}", reports.join(" | "))
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
fn a_reported_throw_is_set_aside_and_the_builtin_carries_on() {
    table(&[
        // Each callee runs, the one that throws is reported, and the builtin
        // is told how many did.
        (
            "var out = ''; \
             var n = each(i => { out += i; }, i => { out += i; throw 'x'; }, i => { out += i; }); \
             out += '/' + n;",
            "012/1 | threw \"x\" [1 calls]",
        ),
        // The order of the reports is the order of the throws.
        (
            "var out = each(() => { throw 1; }, () => { throw 2; }, () => { throw 3; });",
            "3 | threw 1 [1 calls] | threw 2 [1 calls] | threw 3 [1 calls]",
        ),
        // A `try` inside the callee is the callee's own, and nothing is
        // reported.
        (
            "var out = ''; each(() => { try { throw 'c'; } catch (e) { out += e; } }); \
             out += '!';",
            "c!",
        ),
        // A `try` around the builtin is not reached: the throw stops at the
        // reported call, as a listener's stops at `dispatchEvent`.
        (
            "var out = 'not caught'; \
             try { each(() => { throw 'w'; }); } catch (e) { out = 'caught'; }",
            "not caught | threw \"w\" [1 calls]",
        ),
        // The trace is the calls inside the reported one, innermost first,
        // and stops there.
        (
            "function deep() { throw 'd'; } function mid() { deep(); } \
             var out = each(() => { mid(); });",
            "1 | threw \"d\" [3 calls]",
        ),
        // An error the engine throws is reported like a value.
        (
            "var out = each(() => null.a);",
            "1 | TypeError: cannot read property 'a' of null [1 calls]",
        ),
        // A thrown object is still there to be described after the run went
        // on allocating.
        (
            "var out = each(() => { throw { big: [1, 2, 3] }; }); \
             for (let i = 0; i < 200; i++) { out += [i].length; }",
            "201 | threw an object [1 calls]",
        ),
    ]);
}

#[test]
fn what_is_not_a_function_and_a_builtin_that_throws_are_reported_too() {
    table(&[
        (
            "var out = each(1, () => {});",
            "1 | TypeError: 1 is not a function [0 calls]",
        ),
        (
            "var out = each(thrower, () => {});",
            "1 | TypeError: a builtin threw [0 calls]",
        ),
    ]);
}

#[test]
fn reported_calls_nest_and_each_throw_stops_at_its_own() {
    table(&[
        // The inner `each` reports its callee's throw, and the outer callee
        // carries on and returns normally.
        (
            "var out = ''; \
             var n = each(() => { out += each(() => { throw 'in'; }); out += 'after'; }); \
             out += '/' + n;",
            "1after/0 | threw \"in\" [1 calls]",
        ),
        // A throw from the outer callee after its inner `each` is reported
        // by the outer one.
        (
            "var out = each(() => { each(() => {}); throw 'out'; });",
            "1 | threw \"out\" [1 calls]",
        ),
        // A builtin's own throw inside a reported call is caught by a `try`
        // inside that call.
        (
            "var out = ''; \
             each(() => { try { new Thing().x.y; } catch (e) { out += e.name; } });",
            "TypeError",
        ),
    ]);
}

#[test]
fn only_the_pages_own_escapes_are_reported() {
    // A stop is the embedder's, and ends the run rather than being reported.
    let Some(mut engine) = engine(false) else {
        panic!("an engine");
    };
    engine.stop().ask();
    let program = script("each(() => { for (;;) {} });").expect("it parses");
    assert!(matches!(
        engine.evaluate(&program),
        Err(Trouble::Escaped(Escape::Interrupted))
    ));
    assert_eq!(engine.reported_waiting(), 0, "a stop is not a report");
}

#[test]
fn past_the_bound_a_reported_throw_is_counted_rather_than_kept() {
    let kept = alo_js::bounds::REPORTS_SET_ASIDE;
    let Some(mut engine) = engine(false) else {
        panic!("an engine");
    };
    let source = format!(
        "for (let i = 0; i < {}; i++) each(() => {{ throw i; }});",
        kept + 44
    );
    assert!(
        engine
            .evaluate(&script(&source).expect("it parses"))
            .is_ok()
    );
    assert_eq!(engine.reported_waiting(), kept + 44);
    let mut reports = Vec::new();
    let unreported = engine.hand_over_reported(&mut |objects, thrown, unwound| {
        reports.push(said(objects, thrown, unwound));
    });
    assert_eq!(reports.len(), kept);
    assert_eq!(unreported, 44);
    assert_eq!(
        reports.first().map(String::as_str),
        Some("threw 0 [1 calls]")
    );
    assert_eq!(engine.reported_waiting(), 0);
    assert_eq!(
        engine.hand_over_reported(&mut |_, _, _| {}),
        0,
        "a hand-over takes the count with it"
    );
}

#[test]
fn a_checkpoint_hands_a_jobs_reported_throws_to_its_own_report() {
    let Some(mut engine) = engine(true) else {
        panic!("an engine");
    };
    let Some(queue) = define(
        &mut engine,
        "queueMicrotask",
        Native::new("queueMicrotask", |call| {
            if call.step() == 0 {
                return Ok(Answer::want(
                    Want::Job {
                        callee: call.argument(0),
                        arguments: Vec::new(),
                    },
                    1,
                ));
            }
            Ok(Answer::Value(Value::Undefined))
        }),
    ) else {
        panic!("queueMicrotask");
    };
    let _ = queue;
    let source = "queueMicrotask(() => { each(() => { throw 'r'; }); throw 'own'; }); \
                  queueMicrotask(() => { each(() => { throw 's'; }); });";
    let ran = engine.evaluate(&script(source).expect("it parses"));
    assert!(ran.is_ok(), "{ran:?}");
    let mut reports = Vec::new();
    let drained = engine.checkpoint(&mut |objects, thrown, unwound| {
        reports.push(said(objects, thrown, unwound));
    });
    let Ok(drained) = drained else {
        panic!("the checkpoint ran");
    };
    assert_eq!(
        reports,
        [
            "threw \"r\" [1 calls]",
            "threw \"own\" [1 calls]",
            "threw \"s\" [1 calls]"
        ],
        "a job's reported throws come before its own"
    );
    assert_eq!((drained.ran, drained.threw, drained.reported), (2, 1, 2));
    assert_eq!(drained.unreported, 0);
    assert_eq!(engine.reported_waiting(), 0);
}

#[test]
fn an_embedders_constructor_is_given_its_instance_only_by_new() {
    table(&[
        (
            "var t = new Thing(); \
             var out = '' + (t.__proto__ === Thing.prototype) + (typeof t); \
             t.x = 3; out += t.x;",
            "trueobject3",
        ),
        (
            "var out = ''; try { Thing(); } catch (e) { out = e.name + ': ' + e.message; }",
            "TypeError: Thing needs 'new'",
        ),
        ("var out = '' + (new Thing() !== new Thing());", "true"),
    ]);
}
