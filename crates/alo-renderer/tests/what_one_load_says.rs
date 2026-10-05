/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 242, found while building 241: a load says at most so much
//! about its scripts, and then how much more there was.
//!
//! *A page queueing a hundred thousand throwing jobs loads, says the
//! ceiling's worth, and says how many it left out.*
//!
//! Everything a load says crosses to the browser process in one message the
//! wire caps, so a page that throws in a loop could otherwise make its own
//! answer unsendable. Around the closing clause: the ceiling spans every
//! script of a load, counts what is not a throw as well, never changes what
//! runs, and holds inside a single turn of the loop — where a job that throws
//! and requeues itself for ever would otherwise grow without bound until the
//! page was stopped.

use std::thread;
use std::time::Duration;

use alo_js::interpret::Trouble;
use alo_js::{Escape, Value, script};
use alo_layout::Size;
use alo_renderer::event_loop::{MOST_REPORTS, Stopped};
use alo_renderer::scripts::MOST_SAID;
use alo_renderer::wire::{LARGEST_MESSAGE, read_from_renderer, write_from_renderer};
use alo_renderer::{EventLoop, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is drawn.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// A script that queues `jobs` jobs, each adding one to `n` and throwing it.
fn throwing_jobs(jobs: u32) -> String {
    format!(
        "<script>var n = 0; function f() {{ n += 1; throw n; }} \
         for (var i = 0; i < {jobs}; i += 1) {{ queueMicrotask(f); }}</script>"
    )
}

/// Load this markup into a fresh renderer, and answer the renderer and its
/// whole answer.
fn load(markup: &str) -> (Renderer, FromRenderer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        markup.to_owned(),
        WINDOW,
    ))));
    (renderer, answer)
}

/// The issues of a load, or one saying what came back instead.
fn issues(answer: &FromRenderer) -> Vec<String> {
    match answer {
        FromRenderer::Loaded { issues, .. } => issues.clone(),
        other => vec![format!("not loaded: {other:?}")],
    }
}

/// The lines about single scripts, and the line saying how many more there
/// were, if there was one.
fn about_scripts(issues: &[String]) -> (Vec<&str>, Option<&str>) {
    let lines = issues
        .iter()
        .map(String::as_str)
        .filter(|issue| issue.starts_with("script "))
        .collect();
    let more = issues
        .iter()
        .map(String::as_str)
        .find(|issue| issue.contains("about this page's scripts were not said"));
    (lines, more)
}

/// The line a load says when it left `how_many` out.
fn left_out(how_many: usize) -> String {
    format!(
        "{how_many} more things about this page's scripts were not said: one load says at \
         most {MOST_SAID}"
    )
}

/// What a global holds, read from an engine after everything ran.
fn global(looping: &mut EventLoop, name: &str) -> Result<Value, String> {
    let program = script(name).map_err(|why| format!("did not parse: {why}"))?;
    looping
        .engine()
        .evaluate(&program)
        .map_err(|trouble| match trouble {
            Trouble::Escaped(escape) => format!("! {escape}"),
            Trouble::NotCompiled(refusal) => format!("? {refusal}"),
        })
}

/// What the page's global `n` ended as.
fn n_of(renderer: &mut Renderer) -> Result<Value, String> {
    let looping = renderer.event_loop().ok_or("no script ran")?;
    global(looping, "n")
}

// --- The closing clause --------------------------------------------------------

#[test]
fn a_hundred_thousand_throwing_jobs_say_the_ceilings_worth_and_how_many_more() {
    let (mut renderer, answer) = load(&throwing_jobs(100_000));
    let said = issues(&answer);
    let (lines, more) = about_scripts(&said);
    assert_eq!(lines.len(), MOST_SAID);
    // The first of them, in the order they were thrown, each placed.
    for (which, line) in lines.iter().enumerate() {
        let thrown = which + 1;
        assert!(
            line.starts_with(&format!(
                "script 1: uncaught: {thrown} (at script 1, line 1, "
            )),
            "line {which}: {line}"
        );
    }
    assert_eq!(more, Some(left_out(100_000 - MOST_SAID).as_str()));
    // Every job ran: the ceiling is on what is said, not on what runs.
    assert_eq!(n_of(&mut renderer), Ok(Value::Number(100_000.0)));
    // And the answer goes where it has to go.
    let bytes = write_from_renderer(&answer);
    assert!(bytes.len() < LARGEST_MESSAGE, "{} bytes", bytes.len());
    assert_eq!(read_from_renderer(&bytes).as_ref(), Ok(&answer));
}

// --- Around it -----------------------------------------------------------------

#[test]
fn the_ceiling_spans_every_script_of_a_load() {
    let markup =
        "<script>var n = 0;</script>".to_owned() + &"<script>n += 1; throw n;</script>".repeat(300);
    let (mut renderer, answer) = load(&markup);
    let said = issues(&answer);
    let (lines, more) = about_scripts(&said);
    assert_eq!(lines.len(), MOST_SAID);
    assert_eq!(
        lines.first().copied(),
        Some("script 2: uncaught: 1 (at script 2, line 1, column 9)")
    );
    assert_eq!(
        lines.last().copied(),
        Some("script 257: uncaught: 256 (at script 257, line 1, column 9)")
    );
    assert_eq!(more, Some(left_out(300 - MOST_SAID).as_str()));
    assert_eq!(n_of(&mut renderer), Ok(Value::Number(300.0)));
}

#[test]
fn what_is_not_a_throw_counts_against_it_too() {
    let markup = "<script src=\"a.js\"></script>".repeat(300);
    let (_, answer) = load(&markup);
    let said = issues(&answer);
    let (lines, more) = about_scripts(&said);
    assert_eq!(lines.len(), MOST_SAID);
    assert!(
        lines
            .iter()
            .all(|line| line.contains("not run: it is fetched")),
        "{lines:?}"
    );
    assert_eq!(more, Some(left_out(300 - MOST_SAID).as_str()));
}

#[test]
fn a_load_under_the_ceiling_says_everything_and_no_count() {
    let (_, answer) = load(&throwing_jobs(u32::try_from(MOST_SAID).unwrap_or(0)));
    let said = issues(&answer);
    let (lines, more) = about_scripts(&said);
    assert_eq!(lines.len(), MOST_SAID);
    assert_eq!(more, None);
}

#[test]
fn every_prefix_of_a_page_that_throws_in_a_loop_loads_within_the_ceiling() {
    let markup = throwing_jobs(400) + "<script>throw 'last'</script>";
    for cut in 0..=markup.len() {
        let Some(prefix) = markup.get(..cut) else {
            continue;
        };
        let (_, answer) = load(prefix);
        let said = issues(&answer);
        let (lines, more) = about_scripts(&said);
        assert!(lines.len() <= MOST_SAID, "cut at {cut}: {}", lines.len());
        if let Some(more) = more {
            assert_eq!(lines.len(), MOST_SAID, "cut at {cut}: a count only past it");
            assert!(more.ends_with(&format!("at most {MOST_SAID}")), "{more}");
        }
        assert!(write_from_renderer(&answer).len() < LARGEST_MESSAGE);
    }
}

// --- One turn of the loop --------------------------------------------------------

/// A loop with this script queued, or [`None`] if one could not be made.
fn a_loop_with(source: &str) -> Option<EventLoop> {
    let mut looping = EventLoop::new().ok()?;
    looping.queue_script("a script", source).ok()?;
    Some(looping)
}

#[test]
fn a_turn_keeps_at_most_its_ceiling_and_counts_the_rest() {
    let Some(mut looping) = a_loop_with(
        "var n = 0; function f() { n += 1; throw n; } \
         for (var i = 0; i < 1000; i += 1) { queueMicrotask(f); }",
    ) else {
        panic!("an empty heap holds a loop");
    };
    let Some(turn) = looping.run_next() else {
        panic!("a task was waiting");
    };
    assert_eq!(turn.reports.len(), MOST_REPORTS);
    assert_eq!(turn.unreported, 1000 - MOST_REPORTS);
    assert_eq!(turn.jobs, 1000);
    assert_eq!(turn.stopped, None);
    assert_eq!(global(&mut looping, "n"), Ok(Value::Number(1000.0)));
}

#[test]
fn a_turn_given_less_room_keeps_less_and_given_none_keeps_nothing() {
    let source = "function f() { throw 1; } \
                  for (var i = 0; i < 10; i += 1) { queueMicrotask(f); } throw 0;";
    let Some(mut looping) = a_loop_with(source) else {
        panic!("an empty heap holds a loop");
    };
    let Some(turn) = looping.run_next_within(3) else {
        panic!("a task was waiting");
    };
    assert_eq!(turn.reports.len(), 3);
    assert_eq!(turn.unreported, 8);
    // The script's own throw first, then the jobs': `throw 0` follows
    // 26 + 55 characters, so column 82.
    assert_eq!(
        turn.reports.first().map(ToString::to_string).as_deref(),
        Some("uncaught: 0 (at a script, line 1, column 82)")
    );

    let Some(mut looping) = a_loop_with(source) else {
        panic!("an empty heap holds a loop");
    };
    let Some(turn) = looping.run_next_within(0) else {
        panic!("a task was waiting");
    };
    assert!(turn.reports.is_empty());
    assert_eq!(turn.unreported, 11);
    assert_eq!(turn.jobs, 10, "every job ran with nothing kept");

    // More room than a turn keeps is a turn's worth.
    let Some(mut looping) = a_loop_with(
        "function f() { throw 1; } for (var i = 0; i < 300; i += 1) { queueMicrotask(f); }",
    ) else {
        panic!("an empty heap holds a loop");
    };
    let Some(turn) = looping.run_next_within(usize::MAX) else {
        panic!("a task was waiting");
    };
    assert_eq!(turn.reports.len(), MOST_REPORTS);
    assert_eq!(turn.unreported, 300 - MOST_REPORTS);
}

#[test]
fn a_job_that_throws_and_requeues_itself_for_ever_costs_a_count() {
    let Some(mut looping) = a_loop_with(
        "var n = 0; function again() { n += 1; queueMicrotask(again); throw n; } again();",
    ) else {
        panic!("an empty heap holds a loop");
    };
    let switch = looping.stop_switch();
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        switch.ask();
    });
    let Some(turn) = looping.run_next() else {
        panic!("a task was waiting");
    };
    assert!(stopper.join().is_ok());
    assert_eq!(turn.stopped, Some(Stopped::Escaped(Escape::Interrupted)));
    assert_eq!(turn.reports.len(), MOST_REPORTS);
    assert!(
        turn.unreported > 0,
        "two hundred milliseconds of throwing is more than a turn keeps"
    );
    // `throw` follows 11 + 19 + 8 + 23 characters: column 62.
    assert_eq!(
        turn.reports.get(1).map(ToString::to_string).as_deref(),
        Some("uncaught: 2 (at a script, line 1, column 62)")
    );
    // Stopped mid-job, so `n` counts one job more than threw — or exactly
    // as many — and never fewer than were kept and counted together.
    looping.stop_switch().clear();
    let thrown = turn.reports.len() + turn.unreported;
    let Ok(thrown) = u32::try_from(thrown).map(f64::from) else {
        panic!("{thrown} throws in two hundred milliseconds");
    };
    let Ok(Value::Number(n)) = global(&mut looping, "n") else {
        panic!("n is a number");
    };
    assert!(n >= thrown && n <= thrown + 1.0, "{n} against {thrown}");
}
