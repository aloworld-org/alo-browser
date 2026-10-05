/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 232, cut from item 76: the engine's half of the event loop
//! (ADR 0016 § 1, § 3 and § 7).
//!
//! *A job queued during a task runs after that task and before the next; jobs
//! run oldest first, including the ones jobs queue; a job that throws is
//! reported and the next runs; a stopped checkpoint drops what was waiting;
//! and a function an embedder calls with nothing running is a call like any
//! other.*
//!
//! # The embedder here is the test
//!
//! The loop is the renderer's (ADR 0016 § 1) and is item 233. So each test is
//! a small loop of its own: a **task** is one [`Engine::evaluate`] or one
//! [`Engine::call`], and a checkpoint follows it, exactly as § 3 says a loop
//! must. `queueMicrotask` is the embedder's too — HTML's, not the language's —
//! and is defined here the way an embedder defines it: a builtin that asks the
//! engine to queue a job ([`Want::Job`]).
//!
//! # Every table runs twice
//!
//! Once ordinarily and once with [`Heap::stress`](alo_js::Heap::stress) on,
//! collecting at every allocation, and the two must agree. A job waits in the
//! heap between the task that queued it and the checkpoint that runs it, and
//! that is exactly the span a queue held anywhere else would lose it in.

use std::thread;
use std::time::Duration;

use alo_js::abrupt::{Escape, Kind, Thrown};
use alo_js::interpret::{Drained, Engine, Trouble};
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::object::{Property, Value};
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
    // `Function.prototype`, which a builtin inherits from like any function,
    // and which the realm roots.
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

/// A checkpoint, collecting what the jobs that threw threw.
fn checkpoint(engine: &mut Engine) -> (Result<Drained, Escape>, Vec<String>) {
    let mut reports = Vec::new();
    let outcome = engine.checkpoint(&mut |objects, thrown| {
        reports.push(match thrown {
            Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
            Thrown::Value { value, .. } => match value {
                Value::Text(held) => match objects.units(*held) {
                    Some(units) => format!("threw \"{}\"", String::from_utf16_lossy(units)),
                    None => "threw a string that has gone".to_owned(),
                },
                Value::Object(held) if objects.heap().live_at(*held) => {
                    "threw an object".to_owned()
                }
                other => format!("threw {other:?}"),
            },
        });
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
        if let Err(escape) = outcome {
            said.push(format!("checkpoint: {escape}"));
        }
    }
    let out = match task(&mut engine, "out") {
        Ok(value) => show(&mut engine, value),
        Err(why) => why,
    };
    if said.is_empty() {
        out
    } else {
        format!("{out} | {}", said.join(" | "))
    }
}

/// The same, both ways, which must agree.
fn order(tasks: &[&str]) -> String {
    let ordinary = turns(tasks, false);
    let stressed = turns(tasks, true);
    assert_eq!(
        ordinary, stressed,
        "{tasks:?} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// Check a table of task lists against what `out` ends as.
fn table(cases: &[(&[&str], &str)]) {
    for (tasks, expected) in cases {
        assert_eq!(&order(tasks), expected, "{tasks:?}");
    }
}

#[test]
fn a_job_runs_after_the_task_that_queued_it_and_before_the_next() {
    table(&[
        // The script finishes first: a job is never run in the middle of one.
        (
            &["var out = 'a'; queueMicrotask(() => { out += 'c'; }); out += 'b';"],
            "abc",
        ),
        // A task's jobs all run before the next task begins.
        (
            &[
                "var out = '1'; queueMicrotask(() => { out += '2'; });",
                "out += '3'; queueMicrotask(() => { out += '4'; });",
            ],
            "1234",
        ),
        // Oldest first.
        (
            &["var out = ''; for (let i = 0; i < 5; i++) queueMicrotask(() => { out += i; });"],
            "01234",
        ),
        // A job a job queued joins the back of the same checkpoint: it runs
        // after the jobs already waiting, and before the next task.
        (
            &[
                "var out = ''; \
                 queueMicrotask(() => { out += 'a'; queueMicrotask(() => { out += 'd'; }); }); \
                 queueMicrotask(() => { out += 'b'; queueMicrotask(() => { out += 'e'; }); }); \
                 queueMicrotask(() => { out += 'c'; });",
                "out += '|';",
            ],
            "abcde|",
        ),
        // A chain a thousand long is still one checkpoint.
        (
            &[
                "var out = 0; function f() { out++; if (out < 1000) queueMicrotask(f); } \
                 queueMicrotask(f);",
                "out += 0.5;",
            ],
            "1000.5",
        ),
        // A job's `this` is `undefined`, which sloppy code makes the global
        // object and strict code keeps.
        (
            &["var out = ''; \
               queueMicrotask(function () { out += (this === globalThis); }); \
               queueMicrotask(function () { 'use strict'; out += (this === undefined); });"],
            "truetrue",
        ),
    ]);
}

#[test]
fn a_job_that_throws_is_reported_and_the_next_one_runs() {
    table(&[
        (
            &["var out = ''; queueMicrotask(() => { throw 'x'; }); \
               queueMicrotask(() => { out += 'after'; });"],
            "after | threw \"x\"",
        ),
        (
            &["var out = ''; queueMicrotask(() => { null.a; }); \
               queueMicrotask(() => { out += 'on'; });"],
            "on | TypeError: cannot read property 'a' of null",
        ),
        // A thrown object is still there to be described when it is reported.
        (
            &["var out = 'k'; queueMicrotask(() => { throw { big: [1, 2, 3] }; });"],
            "k | threw an object",
        ),
        // A job's own `catch` is the page's, and nothing is reported.
        (
            &["var out = ''; queueMicrotask(() => { try { throw 1; } catch (e) { out += e; } });"],
            "1",
        ),
    ]);
}

#[test]
fn queueing_something_that_is_not_a_function_throws_where_it_was_queued() {
    table(&[
        (
            &["var out = ''; try { queueMicrotask(1); } catch (e) { out = e.name; }"],
            "TypeError",
        ),
        (
            &["var out = ''; try { queueMicrotask({}); } catch (e) { out = e.name; }"],
            "TypeError",
        ),
        // Nothing was queued, so nothing runs.
        (
            &["var out = 'x'; try { queueMicrotask(); } catch (e) {}"],
            "x",
        ),
    ]);
    // The refusal is the language's error, thrown at the call.
    let Some(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    match task(&mut engine, "queueMicrotask(null)") {
        Err(why) => assert!(why.contains("TypeError"), "{why}"),
        Ok(_) => panic!("queueing null is refused"),
    }
    assert_eq!(engine.jobs_waiting(), 0);
}

#[test]
fn a_checkpoint_follows_each_callback_a_loop_calls_but_not_each_a_script_calls() {
    // ADR 0016 § 3: two listeners a person's click dispatched are two calls
    // the loop makes, each followed by a checkpoint — so each sees the
    // other's jobs run between them.
    for stress in [false, true] {
        let Some(mut engine) = engine(stress) else {
            panic!("an empty heap holds an engine");
        };
        let made = task(
            &mut engine,
            "var out = ''; \
             function one() { out += '1'; queueMicrotask(() => { out += 'a'; }); } \
             function two() { out += '2'; queueMicrotask(() => { out += 'b'; }); } \
             function both() { one(); two(); }",
        );
        assert!(made.is_ok(), "{made:?}");
        for name in ["one", "two"] {
            let Some(callee) = global_named(&mut engine, name) else {
                panic!("{name} is a global function");
            };
            let answer = engine.call(callee, Value::Undefined, &[]);
            assert_eq!(answer, Ok(Value::Undefined));
            let (outcome, reports) = checkpoint(&mut engine);
            assert_eq!(outcome, Ok(Drained { ran: 1, threw: 0 }));
            assert!(reports.is_empty());
        }
        // The same two from a script — `element.click()` — are one call, and
        // the script is still running between them.
        let Some(callee) = global_named(&mut engine, "both") else {
            panic!("both is a global function");
        };
        assert_eq!(
            engine.call(callee, Value::Undefined, &[]),
            Ok(Value::Undefined)
        );
        let (outcome, _) = checkpoint(&mut engine);
        assert_eq!(outcome, Ok(Drained { ran: 2, threw: 0 }));
        let out = task(&mut engine, "out").map(|value| show(&mut engine, value));
        assert_eq!(out, Ok("1a2b12ab".to_owned()));
    }
}

/// A global, by name, if it is an object. It is rooted by the global object.
fn global_named(engine: &mut Engine, name: &str) -> Option<Value> {
    match task(engine, name) {
        Ok(value @ Value::Object(_)) => Some(value),
        _ => None,
    }
}

#[test]
fn a_call_with_nothing_running_is_a_call_like_any_other() {
    let Some(mut engine) = engine(true) else {
        panic!("an empty heap holds an engine");
    };
    let made = task(
        &mut engine,
        "var o = { n: 3 }; \
         function add(a, b) { return this.n + a + b; } \
         function strict() { 'use strict'; return this; } \
         function thrower() { throw new RangeError('no'); } \
         function deep(n) { return n == 0 ? 0 : 1 + deep(n - 1); }",
    );
    assert!(made.is_ok(), "{made:?}");

    let Some(add) = global_named(&mut engine, "add") else {
        panic!("add is a global function");
    };
    let Some(o) = global_named(&mut engine, "o") else {
        panic!("o is a global object");
    };
    assert_eq!(
        engine.call(add, o, &[Value::Number(1.0), Value::Number(2.0)]),
        Ok(Value::Number(6.0))
    );
    // A parameter nobody passed is `undefined`, as in any call.
    let sum = engine.call(add, o, &[Value::Number(1.0)]);
    assert!(matches!(sum, Ok(Value::Number(n)) if n.is_nan()), "{sum:?}");

    let Some(strict) = global_named(&mut engine, "strict") else {
        panic!("strict is a global function");
    };
    assert_eq!(engine.call(strict, Value::Null, &[]), Ok(Value::Null));

    let Some(thrower) = global_named(&mut engine, "thrower") else {
        panic!("thrower is a global function");
    };
    assert!(
        matches!(
            engine.call(thrower, Value::Undefined, &[]),
            Err(Escape::Thrown(Thrown::Value {
                value: Value::Object(_),
                ..
            }))
        ),
        "a throw comes back to the embedder"
    );

    // Calls made from a call nest on the same stack, and the bound is the
    // language's `RangeError`, not this process's stack.
    let Some(deep) = global_named(&mut engine, "deep") else {
        panic!("deep is a global function");
    };
    assert_eq!(
        engine.call(deep, Value::Undefined, &[Value::Number(200.0)]),
        Ok(Value::Number(200.0))
    );
    // Not under stress: a collection at each of ten thousand frames' worth of
    // allocations proves nothing the two hundred above did not.
    engine.objects().heap_mut().stress(false);
    let runaway = engine.call(deep, Value::Undefined, &[Value::Number(1.0e9)]);
    engine.objects().heap_mut().stress(true);
    match runaway {
        Err(Escape::Thrown(Thrown::Error {
            kind: Kind::RangeError,
            ..
        })) => {}
        other => panic!("a runaway recursion is a RangeError: {other:?}"),
    }

    // A builtin, called with nothing running.
    let to_string = match task(&mut engine, "({}).toString") {
        Ok(value) => value,
        Err(why) => panic!("{why}"),
    };
    let answer = engine.call(to_string, Value::Null, &[]);
    let answer = answer.map(|value| show(&mut engine, value));
    assert_eq!(answer, Ok("[object Null]".to_owned()));

    // Something that is not a function is the `TypeError` a call on one is.
    match engine.call(Value::Number(1.0), Value::Undefined, &[]) {
        Err(Escape::Thrown(Thrown::Error {
            kind: Kind::TypeError,
            ..
        })) => {}
        other => panic!("calling a number is a TypeError: {other:?}"),
    }
    assert_eq!(engine.objects().heap().check(), Ok(()));
}

#[test]
fn an_embedder_queues_a_job_with_arguments_and_the_queue_keeps_them() {
    let Some(mut engine) = engine(true) else {
        panic!("an empty heap holds an engine");
    };
    let made = task(
        &mut engine,
        "var out = ''; function take(a, b) { out += a.label + b; }",
    );
    assert!(made.is_ok(), "{made:?}");
    let Some(take) = global_named(&mut engine, "take") else {
        panic!("take is a global function");
    };
    // An object nothing but the queue will hold once its root is released.
    let label = match task(&mut engine, "({ label: 'kept' })") {
        Ok(Value::Object(held)) => held,
        other => panic!("an object: {other:?}"),
    };
    let root = engine.objects().heap_mut().root(label);
    assert_eq!(
        engine.queue_job(take, &[Value::Object(label), Value::Number(7.0)], 0),
        Ok(())
    );
    engine.objects().heap_mut().release(root);
    // Two runs and a collection between the queueing and the running.
    assert!(task(&mut engine, "({}); [1, 2, 3]").is_ok());
    engine.objects().heap_mut().collect();
    assert_eq!(engine.jobs_waiting(), 1);
    let (outcome, reports) = checkpoint(&mut engine);
    assert_eq!(outcome, Ok(Drained { ran: 1, threw: 0 }), "{reports:?}");
    let out = task(&mut engine, "out").map(|value| show(&mut engine, value));
    assert_eq!(out, Ok("kept7".to_owned()));
    assert_eq!(engine.objects().heap().check(), Ok(()));

    // And the embedder is refused a callee that is not a function, as a
    // script is.
    assert!(matches!(
        engine.queue_job(Value::Bool(true), &[], 0),
        Err(Escape::Thrown(Thrown::Error {
            kind: Kind::TypeError,
            ..
        }))
    ));
}

#[test]
fn a_checkpoint_keeps_what_the_last_run_answered() {
    // An embedder holding the value a script answered may hold it until its
    // next run (`Engine::run`); a checkpoint is not one of its runs, and the
    // jobs' answers are nobody's.
    let Some(mut engine) = engine(true) else {
        panic!("an empty heap holds an engine");
    };
    let answered = match task(
        &mut engine,
        "for (let i = 0; i < 50; i++) queueMicrotask(() => ({ i })); ({ last: 1 })",
    ) {
        Ok(Value::Object(held)) => held,
        other => panic!("an object: {other:?}"),
    };
    let (outcome, _) = checkpoint(&mut engine);
    assert_eq!(outcome, Ok(Drained { ran: 50, threw: 0 }));
    engine.objects().heap_mut().collect();
    assert!(engine.objects().heap().live_at(answered));
}

#[test]
fn a_checkpoint_ends_the_job() {
    // ADR 0014 § 7: what a dereferenced `WeakRef` keeps alive is kept for the
    // rest of the job, and the checkpoint is where the job ends.
    let Some(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    let held = match task(&mut engine, "({})") {
        Ok(Value::Object(held)) => held,
        other => panic!("an object: {other:?}"),
    };
    engine.objects().heap_mut().keep_alive(held);
    assert_eq!(engine.objects().heap().kept(), 1);
    let (outcome, _) = checkpoint(&mut engine);
    assert_eq!(outcome, Ok(Drained::default()));
    assert_eq!(engine.objects().heap().kept(), 0);

    // Even when the checkpoint did not finish.
    engine.objects().heap_mut().keep_alive(held);
    assert!(task(&mut engine, "queueMicrotask(() => {})").is_ok());
    engine.stop().ask();
    let (outcome, _) = checkpoint(&mut engine);
    assert_eq!(outcome, Err(Escape::Interrupted));
    assert_eq!(engine.objects().heap().kept(), 0);
}

#[test]
fn a_stopped_checkpoint_drops_what_was_waiting() {
    for stress in [false, true] {
        // Stopped before it starts: nothing runs, not even a builtin job.
        let Some(mut engine) = engine(stress) else {
            panic!("an empty heap holds an engine");
        };
        let queued = task(
            &mut engine,
            "var out = ''; queueMicrotask(() => { out += 'no'; }); \
             queueMicrotask(queueMicrotask);",
        );
        assert!(queued.is_ok(), "{queued:?}");
        assert_eq!(engine.jobs_waiting(), 2);
        engine.stop().ask();
        let (outcome, reports) = checkpoint(&mut engine);
        assert_eq!(outcome, Err(Escape::Interrupted));
        assert!(reports.is_empty());
        assert_eq!(
            engine.jobs_waiting(),
            0,
            "a stopped page's jobs are dropped"
        );
        engine.stop().clear();
        let out = task(&mut engine, "out").map(|value| show(&mut engine, value));
        assert_eq!(out, Ok(String::new()), "and none of them ran");
        let (outcome, _) = checkpoint(&mut engine);
        assert_eq!(outcome, Ok(Drained::default()));
    }
}

#[test]
fn a_job_that_never_ends_is_stopped_from_another_thread() {
    let Some(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    let queued = task(
        &mut engine,
        "var out = ''; queueMicrotask(() => { while (true) {} }); \
         queueMicrotask(() => { out += 'never'; });",
    );
    assert!(queued.is_ok(), "{queued:?}");
    let stop = engine.stop();
    let asking = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        stop.ask();
    });
    let (outcome, _) = checkpoint(&mut engine);
    assert_eq!(outcome, Err(Escape::Interrupted));
    let Ok(()) = asking.join() else {
        panic!("the thread that asked has finished");
    };
    assert_eq!(engine.jobs_waiting(), 0);
}

#[test]
fn jobs_that_queue_jobs_for_ever_are_stopped_rather_than_run_for_ever() {
    // The language lets a microtask starve everything, as `while (true)`
    // does, and it is answered the same way: the embedder's switch, read at
    // every job (ADR 0016 § 3). Both a script's function and a builtin as the
    // job that requeues.
    for source in [
        "function f() { queueMicrotask(f); } queueMicrotask(f);",
        "function g() { queueMicrotask(g); queueMicrotask(g); } queueMicrotask(g);",
    ] {
        let Some(mut engine) = engine(false) else {
            panic!("an empty heap holds an engine");
        };
        assert!(task(&mut engine, source).is_ok());
        let stop = engine.stop();
        let asking = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            stop.ask();
        });
        let (outcome, _) = checkpoint(&mut engine);
        let Ok(()) = asking.join() else {
            panic!("the thread that asked has finished");
        };
        // The doubling queue may meet the heap's ceiling first, which is the
        // other answer ADR 0016 § 3 allows; either way it ends and is dropped.
        assert!(
            matches!(outcome, Err(Escape::Interrupted | Escape::Full(_))),
            "{source}: {outcome:?}"
        );
        assert_eq!(engine.jobs_waiting(), 0);
        engine.stop().clear();
        engine.objects().heap_mut().collect();
        assert_eq!(engine.objects().heap().check(), Ok(()));
    }
}

#[test]
fn checkpoints_leave_nothing_behind_that_a_collection_cannot_take() {
    let Some(mut engine) = engine(false) else {
        panic!("an empty heap holds an engine");
    };
    let mut live = Vec::new();
    for _ in 0..3 {
        let queued = task(
            &mut engine,
            "var out = 0; for (let i = 0; i < 2000; i++) \
             queueMicrotask(() => { out += [i, { i }].length; if (i % 7 == 0) throw i; });",
        );
        assert!(queued.is_ok(), "{queued:?}");
        let (outcome, reports) = checkpoint(&mut engine);
        assert_eq!(
            outcome,
            Ok(Drained {
                ran: 2000,
                threw: 286
            })
        );
        assert_eq!(reports.len(), 286);
        engine.objects().heap_mut().collect();
        live.push(engine.objects().heap().live());
    }
    assert_eq!(
        live.first(),
        live.last(),
        "live cells after each run: {live:?}"
    );
}

// --- Hostile input -----------------------------------------------------------

#[test]
fn every_cut_of_a_program_that_queues_jobs_is_a_result_rather_than_a_crash() {
    let source = "var out = ''; function f(n) { out += n; if (n > 0) queueMicrotask(() => f(n - 1)); } \
                  queueMicrotask(() => { throw { n: 1 }; }); queueMicrotask(f.bind ? f : () => f(3)); \
                  try { queueMicrotask(out); } catch (e) { out += e.name; } \
                  queueMicrotask(function () { 'use strict'; out += typeof this; null(); });";
    let mut cuts = 0_usize;
    for (end, _) in source.char_indices() {
        let Some(prefix) = source.get(..end) else {
            continue;
        };
        let Some(mut engine) = engine(false) else {
            panic!("an empty heap holds an engine");
        };
        let _ = task(&mut engine, prefix);
        let (outcome, _) = checkpoint(&mut engine);
        assert!(
            outcome.is_ok() || matches!(outcome, Err(Escape::NotBuiltYet(_))),
            "{prefix}: {outcome:?}"
        );
        assert_eq!(engine.jobs_waiting(), 0, "{prefix}");
        assert_eq!(engine.objects().heap().check(), Ok(()), "{prefix}");
        cuts += 1;
    }
    assert!(cuts > 300);
}
