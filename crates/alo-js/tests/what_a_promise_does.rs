/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 333, cut from item 75 by ADR 0032 § 5: a promise.
//!
//! *A table of interleaved `then`, `queueMicrotask` and thenable resolutions
//! runs in the order the specification gives — `Promise.resolve().then(a)`
//! before a `queueMicrotask(b)` queued after it, a thenable adopted one job
//! later than a plain value, `finally` passing the value through —
//! ordinarily and with the collector at every allocation; an executor that
//! throws rejects; a reaction that throws rejects the promise `then` made; an
//! unhandled rejection is reported once and a handled one never; and every
//! prefix cut of a script that makes promises is refused or run, never a
//! panic.*
//!
//! # The embedder here is the test
//!
//! As in `what_a_checkpoint_runs.rs`: a **task** is one script, a checkpoint
//! follows each, and `queueMicrotask` is the embedder's builtin asking for a
//! job. Each table's answer is `out` after the last checkpoint, followed by
//! every report in the order it was made — so an unhandled rejection shows up
//! as `unhandled …` where it was reported, and its absence is checked by the
//! same string.

use alo_js::abrupt::{Escape, Kind, Missing, Thrown};
use alo_js::interpret::{Drained, Engine, Trouble};
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::object::{Objects, Property, Value};
use alo_js::{numeric, script};

/// `queueMicrotask(callback)`, as an embedder writes it.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn queue_microtask(call: &mut Call<'_>) -> Result<Answer, Escape> {
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
}

/// An engine with `queueMicrotask` on its global object, or [`None`] if one
/// could not be made.
fn engine(stress: bool) -> Option<Engine> {
    let mut engine = Engine::new().ok()?;
    let global = engine.global().ok()?;
    let Ok(Value::Object(above)) = task(&mut engine, "(function () {}).__proto__") else {
        return None;
    };
    let objects = engine.objects();
    let function = objects
        .native(Native::new("queueMicrotask", queue_microtask), Some(above))
        .ok()?;
    let name: Vec<u16> = "queueMicrotask".encode_utf16().collect();
    let defined = objects.define_named(
        global,
        &name,
        Property::data(Value::Object(function), true, false, true),
    );
    if defined != Ok(true) {
        return None;
    }
    engine.objects().heap_mut().stress(stress);
    Some(engine)
}

/// One task: a script, run to its end.
fn task(engine: &mut Engine, source: &str) -> Result<Value, String> {
    let program = script(source).map_err(|why| format!("did not parse: {why}"))?;
    engine.evaluate(&program).map_err(|trouble| match trouble {
        Trouble::Escaped(escape) => format!("! {escape}"),
        Trouble::NotCompiled(refusal) => format!("? {refusal}"),
    })
}

/// What a report says was thrown.
fn thrown(objects: &Objects, thrown: &Thrown) -> String {
    match thrown {
        Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
        Thrown::Value { value, .. } => match value {
            Value::Text(held) => match objects.units(*held) {
                Some(units) => format!("\"{}\"", String::from_utf16_lossy(units)),
                None => "a string that has gone".to_owned(),
            },
            Value::Number(number) => numeric::text_of(*number),
            Value::Object(held) if objects.is_error(*held) => "an error".to_owned(),
            Value::Object(held) if objects.heap().live_at(*held) => "an object".to_owned(),
            other => format!("{other:?}"),
        },
    }
}

/// A checkpoint, and what it reported, in order.
fn checkpoint(engine: &mut Engine) -> (Result<Drained, Escape>, Vec<String>) {
    let mut reports = Vec::new();
    let outcome = engine.checkpoint(&mut |objects, what, unwound| {
        // A rejection nobody handled leaves no calls behind; a job's throw
        // does. That is how the two are told apart here.
        let said = if unwound.places().is_empty() {
            "unhandled"
        } else {
            "threw"
        };
        reports.push(format!("{said} {}", thrown(objects, what)));
    });
    (outcome, reports)
}

/// A value, written out.
fn show(engine: &mut Engine, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(is) => is.to_string(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match engine.objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "?".to_owned(),
        },
        Value::Symbol(_) => "a symbol".to_owned(),
        Value::Object(_) => "an object".to_owned(),
    }
}

/// Run each task followed by a checkpoint, then answer what `out` holds and
/// every report, in order.
fn turns(tasks: &[&str], stress: bool) -> String {
    let Some(mut engine) = engine(stress) else {
        return "no engine".to_owned();
    };
    let mut said = Vec::new();
    for source in tasks {
        if let Err(why) = task(&mut engine, source) {
            said.push(why);
        }
        let (outcome, reports) = checkpoint(&mut engine);
        said.extend(reports);
        match outcome {
            Ok(drained) if drained.unhandled > 1 => {
                said.push(format!("{} unhandled", drained.unhandled));
            }
            Ok(_) => {}
            Err(escape) => said.push(format!("checkpoint: {escape}")),
        }
    }
    let out = match task(&mut engine, "out") {
        Ok(value) => show(&mut engine, value),
        Err(why) => why,
    };
    if engine.objects().heap().check().is_err() {
        said.push("the heap is inconsistent".to_owned());
    }
    if said.is_empty() {
        out
    } else {
        format!("{out} | {}", said.join(" | "))
    }
}

/// The same, both ways, which must agree.
fn order(tasks: &[&str]) -> (String, String) {
    (turns(tasks, false), turns(tasks, true))
}

/// Check a table of task lists against what `out` ends as, ordinarily and
/// with the collector running at every allocation.
fn table(cases: &[(&[&str], &str)]) {
    for (tasks, expected) in cases {
        let (ordinary, stressed) = order(tasks);
        assert_eq!(&ordinary, expected, "{tasks:?}");
        assert_eq!(
            &stressed, expected,
            "{tasks:?} with the collector at every allocation"
        );
    }
}

#[test]
fn reactions_and_microtasks_run_in_the_order_the_specification_gives() {
    table(&[
        // A reaction to a settled promise is queued when `then` is called, so
        // it runs before a microtask queued after it.
        (
            &[
                "var out = ''; Promise.resolve().then(() => { out += 'a'; }); \
               queueMicrotask(() => { out += 'b'; });",
            ],
            "ab",
        ),
        (
            &["var out = ''; queueMicrotask(() => { out += 'b'; }); \
               Promise.resolve().then(() => { out += 'a'; });"],
            "ba",
        ),
        // Two chains interleave a link at a time.
        (
            &["var out = ''; \
               Promise.resolve().then(() => { out += 1; }).then(() => { out += 3; }); \
               Promise.resolve().then(() => { out += 2; }).then(() => { out += 4; });"],
            "1234",
        ),
        // A reaction to a pending promise waits for it, and every reaction
        // runs in the order its `then` was called.
        (
            &[
                "var out = ''; var settle; var p = new Promise((resolve) => { settle = resolve; }); \
               p.then((v) => { out += 'a' + v; }); p.then((v) => { out += 'b' + v; }); \
               queueMicrotask(() => { out += 'q'; settle(1); }); out += 's';",
            ],
            "sqa1b1",
        ),
        // A thenable is adopted one job later than a plain value: its `then`
        // runs in a job of its own, and only then is the promise fulfilled.
        (
            &["var out = ''; \
               var thenable = { then: function (resolve) { out += 't'; resolve('T'); } }; \
               new Promise((resolve) => { resolve(thenable); }).then((v) => { out += v; }); \
               new Promise((resolve) => { resolve('P'); }).then((v) => { out += v; }); \
               queueMicrotask(() => { out += 'q'; });"],
            "tPqT",
        ),
        // Resolving with a promise costs two jobs: one to call its `then`,
        // and one for that `then`'s reaction.
        (
            &["var out = ''; var p = Promise.resolve(); \
               new Promise((resolve) => { resolve(p); }).then(() => { out += 'a'; }); \
               p.then(() => { out += 'b'; }).then(() => { out += 'c'; }).then(() => { out += 'd'; });"],
            "bcad",
        ),
    ]);
}

#[test]
fn a_chain_passes_values_and_rejections_along_and_finally_passes_them_through() {
    table(&[
        // What a handler returns is what the next link is handed, and a
        // promise it returns is waited for.
        (
            &["var out = ''; Promise.resolve(1).then((v) => v + 1) \
               .then((v) => Promise.resolve(v * 10)).then((v) => { out += v; });"],
            "20",
        ),
        // A missing handler passes the outcome through.
        (
            &[
                "var out = ''; Promise.resolve('v').then().then(undefined, 3) \
               .then((v) => { out += v; });",
            ],
            "v",
        ),
        // `finally` passes the value through, waits for what its callback
        // returns, and ignores its value.
        (
            &[
                "var out = ''; Promise.resolve(5).finally(() => { out += 'f'; return 9; }) \
               .then((v) => { out += v; });",
            ],
            "f5",
        ),
        (
            &[
                "var out = ''; Promise.reject(7).finally(() => { out += 'f'; }) \
               .catch((e) => { out += e; });",
            ],
            "f7",
        ),
        (
            &[
                "var out = ''; Promise.resolve(1).finally(() => { throw 2; }) \
               .catch((e) => { out += e; });",
            ],
            "2",
        ),
        (
            &["var out = ''; var late; \
               Promise.resolve(1).finally(() => new Promise((r) => { late = r; })) \
               .then((v) => { out += 'then' + v; }); \
               queueMicrotask(() => { out += 'q'; late(); });"],
            "qthen1",
        ),
        // `catch` is `then(undefined, f)`, and a fulfilment passes it by.
        (
            &[
                "var out = ''; Promise.resolve(1).catch(() => { out += 'x'; }) \
               .then((v) => { out += v; });",
            ],
            "1",
        ),
        // A rejection passes down the chain to the first handler for it.
        (
            &[
                "var out = ''; Promise.reject('r').then(() => { out += 'x'; }) \
               .then(() => { out += 'y'; }).catch((e) => { out += e; });",
            ],
            "r",
        ),
        // A chain finishes inside the checkpoint after the task that began
        // it, before the next task.
        (
            &[
                "var out = ''; Promise.resolve().then(() => { out += 1; }).then(() => { out += 2; });",
                "out += 3;",
            ],
            "123",
        ),
    ]);
}

#[test]
fn an_executor_runs_at_once_and_its_throw_rejects() {
    table(&[
        (
            &["var out = 'a'; new Promise(() => { out += 'b'; }); out += 'c';"],
            "abc",
        ),
        (
            &["var out = ''; new Promise(() => { throw 'e'; }).catch((e) => { out += e; });"],
            "e",
        ),
        // An error the engine throws arrives as the object a `catch` binds.
        (
            &["var out = ''; new Promise(() => { null(); }).catch((e) => { out += e.name; });"],
            "TypeError",
        ),
        // Whichever of the pair runs first wins: a throw after `resolve`
        // changes nothing, and so does a second call.
        (
            &[
                "var out = ''; new Promise((resolve, reject) => { resolve(1); reject(2); resolve(3); throw 4; }) \
               .then((v) => { out += v; }, (e) => { out += 'x' + e; });",
            ],
            "1",
        ),
        (
            &[
                "var out = ''; new Promise((resolve, reject) => { reject(2); resolve(1); }) \
               .then((v) => { out += v; }, (e) => { out += 'x' + e; });",
            ],
            "x2",
        ),
        // A `try` inside the executor catches its own throw first.
        (
            &[
                "var out = ''; new Promise((resolve) => { try { null(); } catch (e) { out += 'c'; } resolve(1); }) \
               .then((v) => { out += v; });",
            ],
            "c1",
        ),
    ]);
}

#[test]
fn a_reaction_that_throws_rejects_the_promise_then_made() {
    table(&[
        (
            &[
                "var out = ''; Promise.resolve().then(() => { throw 'x'; }) \
               .catch((e) => { out += e; });",
            ],
            "x",
        ),
        (
            &["var out = ''; Promise.resolve().then(() => null()) \
               .catch((e) => { out += e.name; });"],
            "TypeError",
        ),
        (
            &[
                "var out = ''; Promise.reject(1).catch(() => { throw 'again'; }) \
               .then(() => { out += 'x'; }, (e) => { out += e; });",
            ],
            "again",
        ),
        // A thenable whose `then` throws rejects what adopted it — unless it
        // had resolved it first.
        (
            &[
                "var out = ''; Promise.resolve({ then: function () { throw 't'; } }) \
               .catch((e) => { out += e; });",
            ],
            "t",
        ),
        (
            &[
                "var out = ''; Promise.resolve({ then: function (r) { r('ok'); throw 't'; } }) \
               .then((v) => { out += v; }, (e) => { out += 'x' + e; });",
            ],
            "ok",
        ),
        // A promise resolved with itself rejects with a `TypeError`.
        (
            &[
                "var out = ''; var settle; var p = new Promise((resolve) => { settle = resolve; }); \
               settle(p); p.catch((e) => { out += e.name; });",
            ],
            "TypeError",
        ),
        (
            &["var out = ''; var p = Promise.resolve().then(() => p); \
               p.catch((e) => { out += e.name; });"],
            "TypeError",
        ),
    ]);
}

#[test]
fn an_unhandled_rejection_is_reported_once_and_a_handled_one_never() {
    table(&[
        // Nobody handles it: reported at the end of the checkpoint.
        (
            &["var out = ''; Promise.reject('no');"],
            " | unhandled \"no\"",
        ),
        // Handled in the same task, or by a job in the same checkpoint: never.
        (
            &["var out = ''; var p = Promise.reject(1); p.catch(() => { out += 'c'; });"],
            "c",
        ),
        (
            &["var out = ''; var p = Promise.reject(1); \
               queueMicrotask(() => { p.catch(() => { out += 'c'; }); });"],
            "c",
        ),
        // Handled only in the next task: it was reported once, at the end of
        // the first checkpoint, and is not reported again.
        (
            &[
                "var out = ''; var p = Promise.reject(1);",
                "p.catch(() => { out += 'c'; });",
                "out += 'd';",
            ],
            "cd | unhandled 1",
        ),
        // `then` with no rejection handler handles the first promise, and the
        // one it made is the rejection nobody handled.
        (
            &["var out = ''; Promise.reject(2).then(() => { out += 'x'; });"],
            " | unhandled 2",
        ),
        // A reaction's throw is a rejection, not a job's throw.
        (
            &["var out = ''; Promise.resolve().then(() => { throw 'r'; });"],
            " | unhandled \"r\"",
        ),
        // An executor's throw too, with the error object it made.
        (
            &["var out = ''; new Promise(() => { null(); });"],
            " | unhandled an error",
        ),
        // Two, in the order they were rejected.
        (
            &["var out = ''; Promise.reject(1); Promise.reject(2);"],
            " | unhandled 1 | unhandled 2 | 2 unhandled",
        ),
        // A fulfilled promise is never reported.
        (&["var out = 'o'; Promise.resolve(1);"], "o"),
    ]);
}

#[test]
fn the_constructor_and_its_statics_answer_as_the_specification_says() {
    table(&[
        (
            &["var out = typeof Promise + ' ' + typeof Promise.prototype.then;"],
            "function function",
        ),
        (
            &["var out = ({}).__proto__.toString.call(Promise.resolve());"],
            "[object Promise]",
        ),
        (
            &["var out = Promise.prototype.constructor === Promise;"],
            "true",
        ),
        // `Promise.resolve` answers a promise `Promise` made as it is.
        (
            &["var p = Promise.resolve(1); var out = Promise.resolve(p) === p;"],
            "true",
        ),
        // ...and wraps one whose `constructor` says otherwise.
        (
            &["var p = Promise.resolve(1); p.constructor = undefined; \
               var out = Promise.resolve(p) === p;"],
            "false",
        ),
        // Adopting it calls its `then`, which reads that `constructor` too:
        // `0` is neither `undefined` nor an object, so the wrapper rejects.
        (
            &["var p = Promise.resolve(1); p.constructor = 0; var out = Promise.resolve(p) === p;"],
            "false | unhandled an error",
        ),
    ]);
}

#[test]
fn what_it_refuses_it_refuses_as_the_specification_says() {
    let refusals: &[(&str, &str)] = &[
        (
            "Promise()",
            "Promise is a constructor, and is called with new",
        ),
        ("new Promise()", "a promise's executor is not a function"),
        ("new Promise(1)", "a promise's executor is not a function"),
        (
            "Promise.prototype.then.call({})",
            "Promise.prototype.then was called on something that is not a promise",
        ),
        (
            "Promise.prototype.catch.call(null)",
            "Promise.prototype.catch was called on null or undefined",
        ),
        (
            "Promise.prototype.finally.call(1)",
            "Promise.prototype.finally was called on something that is not an object",
        ),
        (
            "Promise.resolve.call(1)",
            "Promise.resolve was called on something that is not an object",
        ),
        (
            "Promise.reject.call({})",
            "Promise.reject was called on something that is not a constructor",
        ),
        (
            "var p = Promise.resolve(); p.constructor = 1; p.then()",
            "a promise's constructor is neither undefined nor an object",
        ),
    ];
    for (source, message) in refusals {
        let Some(mut engine) = engine(false) else {
            panic!("an empty heap holds an engine");
        };
        let program = script(source).unwrap_or_else(|why| panic!("{source}: {why}"));
        match engine.evaluate(&program) {
            Err(Trouble::Escaped(Escape::Thrown(Thrown::Error {
                kind: Kind::TypeError,
                message: said,
                ..
            }))) => assert_eq!(&said, message, "{source}"),
            other => panic!("{source}: {other:?}"),
        }
    }

    // Another constructor is refused by name rather than answered with a
    // `Promise`, and the constructor a promise names is read to find out.
    for source in [
        "Promise.resolve.call(function () {}, 1)",
        "Promise.reject.call(function () {}, 1)",
    ] {
        let Some(mut engine) = engine(false) else {
            panic!("an empty heap holds an engine");
        };
        let program = script(source).unwrap_or_else(|why| panic!("{source}: {why}"));
        assert_eq!(
            engine.evaluate(&program),
            Err(Trouble::Escaped(Escape::NotBuiltYet(
                Missing::APromiseOfAnotherConstructor
            ))),
            "{source}"
        );
    }

    // A `constructor` that is an object with no `Symbol.species` is
    // `Promise`, as the specification says.
    table(&[(
        &[
            "var out = ''; var p = Promise.resolve(3); p.constructor = function () {}; \
           p.then((v) => { out += v; });",
        ],
        "3",
    )]);
}

#[test]
fn a_stopped_checkpoint_drops_the_rejections_waiting_with_the_jobs() {
    let Some(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    assert!(
        task(
            &mut engine,
            "Promise.reject(1); Promise.resolve().then(() => {});"
        )
        .is_ok()
    );
    assert_eq!(engine.rejections_waiting(), 1);
    assert_eq!(engine.jobs_waiting(), 1);
    assert_eq!(engine.abandon(), Ok(()));
    assert_eq!(engine.rejections_waiting(), 0);
    assert_eq!(engine.jobs_waiting(), 0);
    let (outcome, reports) = checkpoint(&mut engine);
    assert_eq!(outcome.map(|drained| drained.unhandled), Ok(0));
    assert!(reports.is_empty(), "{reports:?}");
}

#[test]
fn every_prefix_of_a_script_that_makes_promises_is_refused_or_run() {
    let source = "var out = ''; var settle; var p = new Promise((resolve, reject) => { settle = resolve; }); \
                  p.then((v) => { out += v; }, (e) => { out += e; }).finally(() => { out += 'f'; }); \
                  Promise.resolve({ then: function (r) { r(1); throw 2; } }).catch(() => {}); \
                  Promise.reject(3).then().catch((e) => { throw e; }); \
                  queueMicrotask(() => settle(Promise.resolve('x')));";
    let mut cuts = 0_usize;
    for (end, _) in source.char_indices() {
        let Some(prefix) = source.get(..end) else {
            continue;
        };
        for stress in [false, true] {
            let Some(mut engine) = engine(stress) else {
                panic!("an empty heap holds an engine");
            };
            let _ = task(&mut engine, prefix);
            let (outcome, _) = checkpoint(&mut engine);
            assert!(
                outcome.is_ok() || matches!(outcome, Err(Escape::NotBuiltYet(_))),
                "{prefix}: {outcome:?}"
            );
            assert_eq!(engine.jobs_waiting(), 0, "{prefix}");
            assert_eq!(engine.rejections_waiting(), 0, "{prefix}");
            assert_eq!(engine.objects().heap().check(), Ok(()), "{prefix}");
            assert_eq!(engine.objects().heap().scoped(), 0, "{prefix}");
        }
        cuts += 1;
    }
    assert!(cuts > 300);
    // And the whole of it, which runs to the end.
    assert_eq!(
        order(&[source]),
        ("xf | unhandled 3".to_owned(), "xf | unhandled 3".to_owned())
    );
}
