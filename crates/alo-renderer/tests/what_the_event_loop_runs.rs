/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 235, cut from 233: the renderer's event loop — tasks, their
//! order, and the checkpoint after every piece of script (ADR 0016 §§ 1–4
//! and 7).
//!
//! *The oldest task runs next; a microtask a task queued runs after that task
//! and before the next; jobs a job queued join the same checkpoint; each
//! listener a dispatch calls is followed by a checkpoint, and two calls made by
//! one script are not; a throw is reported and the loop runs on; anything else
//! stops the page, drops what was waiting and lets go of what it held.*
//!
//! # Every table runs twice
//!
//! Once ordinarily and once with the collector running at every allocation,
//! and the two must agree. A task waits outside the heap holding its script by
//! a root, and a job waits inside it, and a collection between the two is
//! exactly where either would be lost.

use std::thread;
use std::time::Duration;

use alo_js::interpret::Trouble;
use alo_js::{Escape, Ref, Value, script};
use alo_renderer::EventLoop;
use alo_renderer::event_loop::{Report, Stopped, Turn};

/// What one piece of a table queues.
#[derive(Debug, Clone, Copy)]
enum Queue {
    /// A task that runs this script.
    Script(&'static str),
    /// A task that calls these globals in order, with the global named second
    /// as `this` and the third as the one argument — a dispatch to listeners.
    Calls(&'static [&'static str], &'static str, &'static str),
}

/// A loop, collecting at every allocation if `stress`, or [`None`] if one
/// could not be made.
fn a_loop(stress: bool) -> Option<EventLoop> {
    let mut looping = EventLoop::new().ok()?;
    looping.engine().objects().heap_mut().stress(stress);
    Some(looping)
}

/// Run a script outside the loop — reading what the page left behind, or
/// fetching a value to queue — and answer its value.
fn read(looping: &mut EventLoop, source: &str) -> Result<Value, String> {
    let program = script(source).map_err(|why| format!("did not parse: {why}"))?;
    looping
        .engine()
        .evaluate(&program)
        .map_err(|trouble| match trouble {
            Trouble::Escaped(escape) => format!("! {escape}"),
            Trouble::NotCompiled(refusal) => format!("? {refusal}"),
        })
}

/// What `out` holds, as text.
fn out(looping: &mut EventLoop) -> String {
    match read(looping, "out") {
        Ok(Value::Text(held)) => match looping.engine().objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "a string that has gone".to_owned(),
        },
        Ok(other) => format!("{other:?}"),
        Err(why) => why,
    }
}

/// Queue each piece, run every task, and answer what `out` ends as and every
/// report, in order.
fn turns(pieces: &[Queue], stress: bool) -> String {
    let Some(mut looping) = a_loop(stress) else {
        return "no loop".to_owned();
    };
    let mut said = Vec::new();
    for piece in pieces {
        let queued = match piece {
            Queue::Script(source) => looping.queue_script(*source),
            Queue::Calls(callees, this, argument) => {
                let mut values = Vec::new();
                // Each is a global, so it stays reachable between reading it
                // and the task holding it.
                for name in callees.iter().chain([this, argument]) {
                    match read(&mut looping, name) {
                        Ok(value) => values.push(value),
                        Err(why) => return format!("reading {name}: {why}"),
                    }
                }
                let Some((argument, rest)) = values.split_last() else {
                    return "nothing read".to_owned();
                };
                let Some((this, callees)) = rest.split_last() else {
                    return "nothing read".to_owned();
                };
                looping.queue_calls(callees, *this, &[*argument])
            }
        };
        if let Err(stopped) = queued {
            said.push(format!("not queued: {stopped}"));
        }
        // A setup script runs before the next piece reads what it defined.
        if matches!(piece, Queue::Script(_)) {
            while let Some(turn) = looping.run_next() {
                said.extend(turn.reports.iter().map(ToString::to_string));
            }
        }
    }
    while let Some(turn) = looping.run_next() {
        said.extend(turn.reports.iter().map(ToString::to_string));
        if let Some(stopped) = turn.stopped {
            said.push(stopped.to_string());
        }
    }
    let ended = out(&mut looping);
    if said.is_empty() {
        ended
    } else {
        format!("{ended} | {}", said.join(" | "))
    }
}

/// The same, both ways, which must agree.
fn order(pieces: &[Queue]) -> String {
    let ordinary = turns(pieces, false);
    let stressed = turns(pieces, true);
    assert_eq!(
        ordinary, stressed,
        "{pieces:?} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// Check a table of pieces against what `out` ends as.
fn table(cases: &[(&[Queue], &str)]) {
    for (pieces, expected) in cases {
        assert_eq!(&order(pieces), expected, "{pieces:?}");
    }
}

/// Two listeners, each of which writes a digit and queues a letter.
const LISTENERS: &str = "var out = ''; var target = { name: 't' }; var event = { type: 'click' }; \
     function one() { out += '1'; queueMicrotask(() => { out += 'a'; }); } \
     function two() { out += '2'; queueMicrotask(() => { out += 'b'; }); }";

#[test]
fn a_microtask_runs_after_its_task_and_before_the_next() {
    use Queue::Script;
    table(&[
        // A job never runs in the middle of the script that queued it.
        (
            &[Script(
                "var out = 'a'; queueMicrotask(() => { out += 'c'; }); out += 'b';",
            )],
            "abc",
        ),
        // A task's jobs all run before the next task begins.
        (
            &[
                Script("var out = '1'; queueMicrotask(() => { out += '2'; });"),
                Script("out += '3'; queueMicrotask(() => { out += '4'; });"),
            ],
            "1234",
        ),
        // Jobs run oldest first.
        (
            &[Script(
                "var out = ''; for (let i = 0; i < 5; i++) queueMicrotask(() => { out += i; });",
            )],
            "01234",
        ),
        // A job a job queued joins the back of the same checkpoint: after the
        // jobs already waiting, and before the next task.
        (
            &[
                Script(
                    "var out = ''; \
                     queueMicrotask(() => { out += 'a'; queueMicrotask(() => { out += 'c'; }); }); \
                     queueMicrotask(() => { out += 'b'; });",
                ),
                Script("out += 'd';"),
            ],
            "abcd",
        ),
    ]);
}

#[test]
fn queue_microtask_is_a_function_like_the_languages_own() {
    use Queue::Script;
    table(&[(
        &[Script(
            "var out = '' + (queueMicrotask.__proto__ === (function () {}).__proto__);",
        )],
        "true",
    )]);
}

#[test]
fn the_oldest_task_runs_next() {
    // Three tasks queued before any runs, each queueing a job: the order is
    // the order they were queued in, and each one's job runs before the next.
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let mut queued = Vec::new();
    for source in [
        "var out = 'x'; queueMicrotask(() => { out += 'X'; });",
        "out += 'y'; queueMicrotask(() => { out += 'Y'; });",
        "out += 'z'; queueMicrotask(() => { out += 'Z'; });",
    ] {
        match looping.queue_script(source) {
            Ok(seq) => queued.push(seq),
            Err(stopped) => panic!("a fresh page queues: {stopped}"),
        }
    }
    assert_eq!(looping.waiting(), 3);
    let mut ran = Vec::new();
    while let Some(turn) = looping.run_next() {
        assert_eq!(turn.jobs, 1, "each task's own job, in its own checkpoint");
        assert!(
            turn.reports.is_empty() && turn.stopped.is_none(),
            "{turn:?}"
        );
        ran.push(turn.task);
    }
    assert_eq!(ran, queued);
    assert!(queued.windows(2).all(|pair| pair.first() < pair.get(1)));
    assert_eq!(out(&mut looping), "xXyYzZ");
}

#[test]
fn a_checkpoint_follows_each_listener_the_loop_calls_but_not_each_call_a_script_makes() {
    use Queue::{Calls, Script};
    table(&[
        // A person clicked: the browser process's dispatch calls each listener
        // with nothing else running, so each one's microtask runs before the
        // next listener (ADR 0016 § 3).
        (
            &[Script(LISTENERS), Calls(&["one", "two"], "target", "event")],
            "1a2b",
        ),
        // A script called both — `element.click()` — and was still running.
        (&[Script(LISTENERS), Script("one(); two();")], "12ab"),
        // The order of the listeners is the order they were given in.
        (
            &[Script(LISTENERS), Calls(&["two", "one"], "target", "event")],
            "2b1a",
        ),
    ]);
}

#[test]
fn a_listener_is_handed_its_this_and_its_argument() {
    use Queue::{Calls, Script};
    table(&[(
        &[
            Script(
                "var out = ''; var target = { name: 't' }; var event = { type: 'click' }; \
                 function seen(e) { out += this.name + ':' + e.type; }",
            ),
            Calls(&["seen", "seen"], "target", "event"),
        ],
        "t:clickt:click",
    )]);
}

#[test]
fn a_throw_is_reported_and_the_loop_runs_on() {
    use Queue::{Calls, Script};
    table(&[
        // A task that threw: reported, its jobs still run, the next task runs.
        (
            &[
                Script("var out = ''; queueMicrotask(() => { out += 'j'; }); throw 'no';"),
                Script("out += 'n';"),
            ],
            "jn | uncaught: \"no\"",
        ),
        // A job that threw: reported, and the next job runs.
        (
            &[Script(
                "var out = ''; queueMicrotask(() => { null.x; }); \
                 queueMicrotask(() => { out += 'after'; });",
            )],
            "after | uncaught: TypeError: cannot read property 'x' of null",
        ),
        // A listener that threw: reported, and the next listener is called.
        (
            &[
                Script(LISTENERS),
                Script("function bad() { throw 7; }"),
                Calls(&["bad", "two"], "target", "event"),
            ],
            "2b | uncaught: 7",
        ),
        // `queueMicrotask` given something that is not a function.
        (
            &[Script("var out = 'q'; queueMicrotask(1);")],
            "q | uncaught: TypeError: 1 is not a function, and only a function can be queued",
        ),
        // A script that is not one is a SyntaxError, and nothing of it ran.
        (
            &[
                Script("var out = 'ok';"),
                Script("out = 'no'; )"),
                Script("out += '!';"),
            ],
            "ok! | not a script: this begins no expression",
        ),
    ]);
}

#[test]
fn a_callee_that_is_not_a_function_is_reported_when_its_turn_comes() {
    use Queue::{Calls, Script};
    let answered = order(&[
        Script(LISTENERS),
        Script("var nothing = 3;"),
        Calls(&["one", "nothing", "two"], "target", "event"),
    ]);
    assert!(
        answered.starts_with("1a2b | uncaught: TypeError: "),
        "{answered}"
    );
}

/// Queue a call of global `f` with a fresh object `{ v: 'kept' }` as its
/// argument, and answer the object's reference.
fn queue_with_a_fresh_object(looping: &mut EventLoop) -> Result<Ref, String> {
    let callee = read(looping, "f")?;
    // Read last, so it is the value the engine keeps from its last run while
    // the task is made — the root the caller must hold until the task does.
    let Value::Object(object) = read(looping, "({ v: 'kept' })")? else {
        return Err("not an object".to_owned());
    };
    looping
        .queue_calls(&[callee], Value::Undefined, &[Value::Object(object)])
        .map_err(|stopped| stopped.to_string())?;
    Ok(object)
}

#[test]
fn a_waiting_task_holds_what_it_will_call_through_a_collection() {
    for stress in [false, true] {
        let Some(mut looping) = a_loop(stress) else {
            panic!("an empty heap holds a loop");
        };
        let _ = looping.queue_script("var out = ''; function f(x) { out += x.v; }");
        while looping.run_next().is_some() {}
        let Ok(object) = queue_with_a_fresh_object(&mut looping) else {
            panic!("a fresh page queues a call");
        };
        // The engine stops keeping the object, and a collection runs: only the
        // task holds it now.
        let _ = read(&mut looping, "0");
        looping.engine().objects().heap_mut().collect();
        assert!(
            looping.engine().objects().heap().live_at(object),
            "a waiting task's argument survives a collection"
        );
        let Some(turn) = looping.run_next() else {
            panic!("the call was waiting");
        };
        assert!(
            turn.reports.is_empty() && turn.stopped.is_none(),
            "{turn:?}"
        );
        assert_eq!(out(&mut looping), "kept");
        // Run, the task let go of it.
        let _ = read(&mut looping, "0");
        looping.engine().objects().heap_mut().collect();
        assert!(
            !looping.engine().objects().heap().live_at(object),
            "a task that has run holds nothing"
        );
        assert_eq!(looping.engine().objects().heap().check(), Ok(()));
    }
}

#[test]
fn many_tasks_leak_nothing() {
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let _ = looping.queue_script(
        "var out = 0; var target = {}; function count(e) { out += 1; queueMicrotask(() => {}); }",
    );
    while looping.run_next().is_some() {}
    let Ok(count) = read(&mut looping, "count") else {
        panic!("count is defined");
    };
    let mut live = Vec::new();
    for _ in 0..3 {
        for _ in 0..500 {
            let Ok(event) = read(&mut looping, "({ type: 'tick' })") else {
                panic!("an event");
            };
            if let Err(stopped) = looping.queue_calls(&[count, count], Value::Undefined, &[event]) {
                panic!("queued: {stopped}");
            }
            // Each event is held only by its task once the next is read.
        }
        let mut jobs = 0;
        while let Some(turn) = looping.run_next() {
            assert!(
                turn.stopped.is_none() && turn.reports.is_empty(),
                "{turn:?}"
            );
            jobs += turn.jobs;
        }
        assert_eq!(jobs, 1000, "a job for every listener called");
        let _ = read(&mut looping, "0");
        looping.engine().objects().heap_mut().collect();
        live.push(looping.engine().objects().heap().live());
    }
    assert!(
        live.windows(2).all(|pair| pair.first() == pair.get(1)),
        "live cells after each round: {live:?}"
    );
    assert_eq!(read(&mut looping, "out"), Ok(Value::Number(3000.0)));
}

/// Assert a turn stopped the page because it was asked to.
fn stopped_by_the_switch(turn: Option<Turn>) {
    let stopped = turn.map(|turn| turn.stopped);
    assert_eq!(
        stopped,
        Some(Some(Stopped::Escaped(Escape::Interrupted))),
        "a task was waiting, and the switch stopped it"
    );
}

#[test]
fn a_stopped_page_drops_what_was_waiting_and_runs_nothing_more() {
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let _ = looping.queue_script("var out = ''; function f(x) { out += x.v; }");
    while looping.run_next().is_some() {}
    // A straight-line script first: it makes no call and no backward jump, so
    // the engine would never read the switch inside it.
    let _ = looping.queue_script("out += 'never';");
    let Ok(object) = queue_with_a_fresh_object(&mut looping) else {
        panic!("a fresh page queues a call");
    };
    let _ = read(&mut looping, "0");
    // Asked to stop while idle: the next task does not start.
    looping.stop_switch().ask();
    stopped_by_the_switch(looping.run_next());
    assert_eq!(looping.waiting(), 0, "every waiting task dropped");
    assert!(looping.run_next().is_none());
    assert_eq!(
        looping.queue_script("out += 'later';"),
        Err(Stopped::Escaped(Escape::Interrupted))
    );
    assert!(looping.queue_calls(&[], Value::Undefined, &[]).is_err());
    looping.engine().objects().heap_mut().collect();
    assert!(
        !looping.engine().objects().heap().live_at(object),
        "a dropped task's root was released"
    );
    looping.stop_switch().clear();
    assert_eq!(out(&mut looping), "", "neither task ran");
}

#[test]
fn an_endless_task_is_stopped_from_another_thread_and_its_jobs_are_dropped() {
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let _ = looping.queue_script(
        "var out = 'started'; queueMicrotask(() => { out = 'job'; }); while (true) {}",
    );
    let _ = looping.queue_script("out = 'next';");
    let switch = looping.stop_switch();
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        switch.ask();
    });
    stopped_by_the_switch(looping.run_next());
    assert!(stopper.join().is_ok());
    assert_eq!(looping.engine().jobs_waiting(), 0, "its job was dropped");
    assert_eq!(looping.waiting(), 0, "the next task was dropped");
    looping.stop_switch().clear();
    assert_eq!(out(&mut looping), "started");
}

#[test]
fn a_job_that_requeues_itself_for_ever_is_stopped_from_another_thread() {
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let _ = looping.queue_script(
        "var out = 0; function again() { out += 1; queueMicrotask(again); } again();",
    );
    let switch = looping.stop_switch();
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        switch.ask();
    });
    stopped_by_the_switch(looping.run_next());
    assert!(stopper.join().is_ok());
    assert_eq!(looping.engine().jobs_waiting(), 0);
}

#[test]
fn a_quiet_point_that_is_not_quiet_stops_the_page() {
    let Some(mut looping) = a_loop(false) else {
        panic!("an empty heap holds a loop");
    };
    let _ = looping.queue_script("var out = {};");
    let Ok(Value::Object(held)) = read(&mut looping, "({})") else {
        panic!("an object");
    };
    // Something left a scope open across the end of a task — which only a bug
    // in the loop or the engine can do, so a test does it by hand.
    let scope = looping.engine().objects().heap_mut().open();
    looping.engine().objects().heap_mut().hold(held);
    let Some(turn) = looping.run_next() else {
        panic!("a task was waiting");
    };
    assert_eq!(turn.stopped, Some(Stopped::NotQuiet { scoped: 1, kept: 0 }));
    looping.engine().objects().heap_mut().close(scope);
    assert!(looping.queue_script("0").is_err());
}

/// A script that queues and requeues jobs, to be cut at every place.
const CUT: &str = "var out = ''; function f(x) { out += x; queueMicrotask(() => { out += 'j'; }); } \
     queueMicrotask(() => f('a')); try { queueMicrotask(f.bind); } catch (e) { out += 'c'; } \
     for (const n of [1, 2]) queueMicrotask(() => { out += n; });";

#[test]
fn every_prefix_of_a_script_is_reported_or_run_and_never_panics() {
    let mut cuts = 0;
    for (at, _) in CUT.char_indices().chain([(CUT.len(), ' ')]) {
        let Some(prefix) = CUT.get(..at) else {
            continue;
        };
        let Some(mut looping) = a_loop(false) else {
            panic!("an empty heap holds a loop");
        };
        let _ = looping.queue_script(prefix.to_owned());
        let _ = looping.queue_script("var after = 'ran';");
        while let Some(turn) = looping.run_next() {
            assert!(
                turn.stopped.is_none()
                    || matches!(turn.stopped, Some(Stopped::Escaped(Escape::NotBuiltYet(_)))),
                "{prefix:?} stopped the page: {turn:?}"
            );
            for report in &turn.reports {
                assert!(
                    matches!(
                        report,
                        Report::NotParsed(_) | Report::NotCompiled(_) | Report::Threw(_)
                    ),
                    "{report:?}"
                );
            }
        }
        if looping.stopped().is_none() {
            assert_eq!(
                read(&mut looping, "after"),
                read(&mut looping, "'ran'"),
                "the task after {prefix:?} ran"
            );
        }
        cuts += 1;
    }
    assert!(cuts > 100, "{cuts} cuts");
}
