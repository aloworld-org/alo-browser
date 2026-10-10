/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 255, cut from 81: a dispatch from the browser is a task (ADR
//! 0018 § 3, ADR 0016 §§ 3 and 6).
//!
//! *Two listeners on one target, dispatched from the renderer, each see the
//! other's microtasks run between them, and the same two dispatched by a
//! script's `dispatchEvent` do not.* And around that: the path and its phases,
//! the order inside one listener's turn (the checkpoint before the stepper
//! hears the listener returned), throws reported with the dispatch carrying
//! on, a stop stopping the page, a waiting dispatch holding what it will
//! dispatch through a collection, and a page that never ran script given no
//! heap at all. And queue item 260's browser half: a listener for the
//! browser's dispatch reads `isTrusted` as `true` (ADR 0018 § 4, ADR 0019).
//!
//! # Every case runs twice
//!
//! Once ordinarily and once with the collector running at every allocation,
//! and the two must agree: a waiting dispatch holds its event and target by a
//! root outside the heap, and a collection between queueing it and running it
//! is exactly where either would be lost.

use std::rc::Rc;
use std::thread;
use std::time::Duration;

use alo_bindings::{Fired, Firing, Identity, View};
use alo_dom::{NodeId, parse_document};
use alo_js::interpret::Trouble;
use alo_js::{Clock, Fixed, Value, script};
use alo_renderer::EventLoop;
use alo_renderer::easel::Easel;
use alo_renderer::event_loop::{MOST_REPORTS, Stopped, Unqueued};
use alo_renderer::held::Held;
use alo_renderer::view::PageView;

/// What a page's realm here is told the time by: an instant that never
/// moves, which nothing in this file reads.
fn clock() -> Rc<dyn Clock> {
    Rc::new(Fixed::at(0.0))
}

/// What the page's window is shown by: a viewport of 800 × 600, drawn on an
/// easel with no fonts, as a renderer's would be.
fn view() -> Rc<dyn View> {
    let easel = Easel::new(alo_text::FontDatabase::new());
    Rc::new(PageView::at(
        alo_layout::Size::new(800.0, 600.0),
        Rc::new(core::cell::RefCell::new(easel)),
    ))
}

/// A `div` holding a `button`, nothing between them.
/// What the browser says it is; nothing here reads it.
const IDENTITY: Identity<'static> = Identity {
    user_agent: "Mozilla/5.0 (X11; Linux x86_64) alo/0.0",
    platform: "Linux x86_64",
};

const PAGE: &str = "<!doctype html><body><div><button>Go</button></div></body>";

/// What every setup script starts with: `out`, and the two elements by name.
const NAMES: &str =
    "var out = ''; var outer = document.body.firstChild; var b = outer.firstChild; ";

/// A bubbling, cancelable event of type `ping`.
const PING: Firing<'static> = Firing {
    interface: Fired::Event,
    kind: "ping",
    bubbles: true,
    cancelable: true,
    composed: false,
};

/// A page whose setup script has run, and its two elements.
struct Page {
    held: Held,
    outer: NodeId,
    button: NodeId,
}

impl Page {
    /// [`PAGE`], scripted, its setup — [`NAMES`] then `setup` — run.
    fn new(setup: &str, stress: bool) -> Result<Self, String> {
        let document = parse_document(PAGE);
        let outer = document
            .body()
            .and_then(|body| document.first_child(body))
            .ok_or("no div")?;
        let button = document.first_child(outer).ok_or("no button")?;
        let mut held = Held::Parsed(document);
        let looping = held
            .scripted(&alo_url::Url::about_blank(), IDENTITY, &clock(), &view())
            .map_err(|why| why.to_string())?;
        looping.engine().objects().heap_mut().stress(stress);
        let mut page = Self {
            held,
            outer,
            button,
        };
        let said = page.run(&format!("{NAMES}{setup}"))?;
        if said.is_empty() {
            Ok(page)
        } else {
            Err(format!("the setup said: {said}"))
        }
    }

    /// The page's loop.
    fn looping(&mut self) -> Result<&mut EventLoop, String> {
        self.held.event_loop().ok_or_else(|| "no loop".to_owned())
    }

    /// Queue a script as a task and run every task, answering what they said.
    fn run(&mut self, source: &str) -> Result<String, String> {
        self.looping()?
            .queue_script("piece", source)
            .map_err(|stopped| stopped.to_string())?;
        self.drain()
    }

    /// Queue the browser's dispatch of `firing` to the button and run every
    /// task, answering what they said.
    fn fire(&mut self, firing: &Firing<'_>) -> Result<String, String> {
        self.fire_at(self.button, firing)
    }

    /// The same, to `node`.
    fn fire_at(&mut self, node: NodeId, firing: &Firing<'_>) -> Result<String, String> {
        match self.held.dispatch(node, firing) {
            Ok(Some(_)) => self.drain(),
            Ok(None) => Err("nobody was dispatched to".to_owned()),
            Err(unqueued) => Err(unqueued.to_string()),
        }
    }

    /// Run every task waiting, answering every report and stop, in order.
    fn drain(&mut self) -> Result<String, String> {
        let looping = self.looping()?;
        let mut said = Vec::new();
        while let Some(turn) = looping.run_next() {
            said.extend(turn.reports.iter().map(ToString::to_string));
            if turn.unreported > 0 {
                said.push(format!("{} unreported", turn.unreported));
            }
            if let Some(stopped) = turn.stopped {
                said.push(stopped.to_string());
            }
        }
        Ok(said.join(" | "))
    }

    /// Evaluate `source` outside the loop, answering its value as text.
    fn read(&mut self, source: &str) -> String {
        let Ok(looping) = self.looping() else {
            return "no loop".to_owned();
        };
        let Ok(program) = script(source) else {
            return "did not parse".to_owned();
        };
        match looping.engine().evaluate(&program) {
            Ok(Value::Text(held)) => match looping.engine().objects().units(held) {
                Some(units) => String::from_utf16_lossy(units),
                None => "a string that has gone".to_owned(),
            },
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }
}

/// Set up the page with `setup`, fire `firing` at the button, then run
/// `after` if there is one, and answer `out` and everything said.
fn case_once(setup: &str, firing: &Firing<'_>, after: &str, stress: bool) -> String {
    let mut page = match Page::new(setup, stress) {
        Ok(page) => page,
        Err(why) => return why,
    };
    let mut said = match page.fire(firing) {
        Ok(said) => said,
        Err(why) => return why,
    };
    if !after.is_empty() {
        match page.run(after) {
            Ok(more) if !more.is_empty() => {
                said = [said, more]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" | ");
            }
            Ok(_) => {}
            Err(why) => return why,
        }
    }
    let out = page.read("out");
    if said.is_empty() {
        out
    } else {
        format!("{out} | {said}")
    }
}

/// [`case_once`] both ways, which must agree.
fn case(setup: &str, firing: &Firing<'_>, after: &str) -> String {
    let ordinary = case_once(setup, firing, after, false);
    let stressed = case_once(setup, firing, after, true);
    assert_eq!(
        ordinary, stressed,
        "{setup} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// Two listeners on the button, each writing a digit and queueing a letter.
const TWO: &str = "\
    function one() { out += '1'; queueMicrotask(() => { out += 'a'; }); } \
    function two() { out += '2'; queueMicrotask(() => { out += 'b'; }); } \
    b.addEventListener('ping', one); b.addEventListener('ping', two);";

#[test]
fn listeners_the_browser_calls_see_each_others_microtasks_and_a_scripts_do_not() {
    // The closing condition. From the browser: a checkpoint after each.
    assert_eq!(case(TWO, &PING, ""), "1a2b");
    // From a script: the script is still running, so both jobs wait for it.
    assert_eq!(
        case(
            TWO,
            &PING,
            "out += '|'; b.dispatchEvent(new Event('ping'));"
        ),
        "1a2b|12ab"
    );
}

#[test]
fn the_dispatch_walks_the_path_and_each_phase_is_what_the_listener_reads() {
    let setup = "\
        function seen(e) { \
            out += e.eventPhase \
                + (e.currentTarget === outer ? 'o' : e.currentTarget === b ? 'b' : 'd') \
                + (e.target === b ? 't' : '?') + ' '; \
            queueMicrotask(() => { out += 'm '; }); \
        } \
        outer.addEventListener('ping', seen, true); \
        outer.addEventListener('ping', seen); \
        b.addEventListener('ping', seen); \
        b.addEventListener('ping', seen, { capture: true }); \
        document.addEventListener('ping', seen, true);";
    // The document's two capture listeners, the div's, the button's at its
    // target — capture first — and the div's on the way up; a checkpoint
    // after each `seen`, and none to run after `first`.
    let document_too = "function first(e) { out += 'd' + e.eventPhase + ' '; } \
                        document.addEventListener('ping', first, true);";
    assert_eq!(
        case(&format!("{document_too} {setup}"), &PING, ""),
        "d1 1dt m 1ot m 2bt m 2bt m 3ot m "
    );
    // Not bubbling: no ancestor is reached on the way up.
    let quiet = Firing {
        interface: Fired::Event,
        bubbles: false,
        ..PING
    };
    assert_eq!(case(setup, &quiet, ""), "1dt m 1ot m 2bt m 2bt m ");
}

#[test]
fn the_event_is_what_the_browser_made_and_is_let_go_of_when_it_ends() {
    let setup = "var kept; \
        b.addEventListener('ping', (e) => { kept = e; \
            out += e.type + ',' + e.bubbles + ',' + e.cancelable + ',' + e.composed + ',' \
                + (e.__proto__ === Event.prototype) + ',' + e.defaultPrevented; });";
    assert_eq!(
        case(
            setup,
            &PING,
            "out += '|' + kept.eventPhase + ',' + kept.currentTarget + ',' \
             + (kept.target === b) + ',' + kept.composedPath().length;"
        ),
        "ping,true,true,false,true,false|0,null,true,0"
    );
    let other = Firing {
        interface: Fired::Event,
        kind: "pong",
        bubbles: false,
        cancelable: false,
        composed: true,
    };
    let pong = "b.addEventListener('pong', (e) => { e.preventDefault(); \
                out += e.type + ',' + e.bubbles + ',' + e.cancelable + ',' + e.composed + ',' \
                    + e.defaultPrevented; });";
    assert_eq!(case(pong, &other, ""), "pong,false,false,true,false");
}

#[test]
fn the_browser_s_event_is_trusted_until_a_script_dispatches_it() {
    let setup = "var kept; \
        b.addEventListener('ping', (e) => { kept = e; out += e.isTrusted + ','; }); \
        outer.addEventListener('ping', (e) => { out += (e === kept) + '' + e.isTrusted + ','; });";
    // Trusted at the target and on the way up, still trusted once the
    // dispatch has ended, and untrusted once a script dispatches it again —
    // `dispatchEvent` says so, whoever made the event. A page cannot delete
    // the property or assign to it.
    assert_eq!(
        case(
            setup,
            &PING,
            "out += '|' + kept.isTrusted + '|'; \
             kept.isTrusted = false; out += (delete kept.isTrusted) + '' + kept.isTrusted + '|'; \
             b.dispatchEvent(kept); out += kept.isTrusted;"
        ),
        "true,truetrue,|true|falsetrue|false,truefalse,false"
    );
}

#[test]
fn a_microtask_between_listeners_runs_before_the_dispatch_decides_who_is_next() {
    // `stopImmediatePropagation` in the first listener's microtask: the
    // second is not called, because the checkpoint ran before the stepper
    // was told the first had returned.
    let immediate = "\
        b.addEventListener('ping', (e) => { out += '1'; \
            queueMicrotask(() => { e.stopImmediatePropagation(); out += 's'; }); }); \
        b.addEventListener('ping', () => { out += '2'; }); \
        outer.addEventListener('ping', () => { out += 'o'; });";
    assert_eq!(case(immediate, &PING, ""), "1s");
    // `stopPropagation`: the rest of this target's listeners, and no more.
    let propagation = "\
        b.addEventListener('ping', (e) => { out += '1'; \
            queueMicrotask(() => { e.stopPropagation(); out += 's'; }); }); \
        b.addEventListener('ping', () => { out += '2'; }); \
        outer.addEventListener('ping', () => { out += 'o'; });";
    assert_eq!(case(propagation, &PING, ""), "1s2");
    // `preventDefault` from a passive listener's microtask finds the passive
    // flag still set and does nothing; from an ordinary one's, it cancels.
    let passive = "\
        b.addEventListener('ping', (e) => { \
            queueMicrotask(() => { e.preventDefault(); out += e.defaultPrevented; }); \
        }, { passive: true }); \
        b.addEventListener('ping', (e) => { \
            queueMicrotask(() => { e.preventDefault(); out += ',' + e.defaultPrevented; }); \
        });";
    assert_eq!(case(passive, &PING, ""), "false,true");
}

#[test]
fn a_listener_that_throws_is_reported_and_the_dispatch_carries_on() {
    let setup = "\
        b.addEventListener('ping', () => { queueMicrotask(() => { out += 'j'; }); \
            throw new Error('first'); }); \
        b.addEventListener('ping', () => { out += '2'; }); \
        outer.addEventListener('ping', () => { out += 'o'; });";
    let answered = case(setup, &PING, "");
    assert!(
        answered.starts_with("j2o | uncaught: Error: first (at piece, line 1, column "),
        "{answered}"
    );
    // A listener that dispatches the same event again: `InvalidStateError`,
    // caught by the listener — and from a microtask between listeners too.
    let again = "\
        b.addEventListener('ping', (e) => { \
            try { b.dispatchEvent(e); } catch (x) { out += x.name + ' '; } \
            queueMicrotask(() => { \
                try { b.dispatchEvent(e); } catch (x) { out += x.name; } }); });";
    assert_eq!(
        case(again, &PING, ""),
        "InvalidStateError InvalidStateError"
    );
}

#[test]
fn a_callback_object_is_called_by_its_handle_event_as_a_scripts_dispatch_calls_it() {
    let setup = "\
        var plain = { name: 'p', handleEvent(e) { out += this.name + e.type + ' '; } }; \
        var got = { name: 'g', get handleEvent() { out += 'get '; \
            return function (e) { out += this.name + ' '; }; } }; \
        var throws = { get handleEvent() { throw new Error('getter'); } }; \
        var none = {}; \
        b.addEventListener('ping', plain); b.addEventListener('ping', got); \
        b.addEventListener('ping', throws); b.addEventListener('ping', none); \
        b.addEventListener('ping', () => { out += 'last'; });";
    let answered = case(setup, &PING, "");
    let mut parts = answered.split(" | ");
    assert_eq!(parts.next(), Some("pping get g last"), "{answered}");
    let Some(getter) = parts.next() else {
        panic!("the getter's throw is reported: {answered}");
    };
    assert!(
        getter.starts_with("uncaught: Error: getter (at piece, line 1, column "),
        "{answered}"
    );
    let Some(not_callable) = parts.next() else {
        panic!("a handleEvent that is not there is reported: {answered}");
    };
    assert!(
        not_callable.starts_with("uncaught: TypeError: "),
        "{answered}"
    );
    assert_eq!(parts.next(), None, "{answered}");
    // The same objects dispatched by a script are called the same way.
    let scripted = case(
        setup,
        &Firing {
            interface: Fired::Event,
            kind: "other",
            ..PING
        },
        "out = ''; b.dispatchEvent(new Event('ping'));",
    );
    assert!(scripted.starts_with("pping get g last | "), "{scripted}");
}

#[test]
fn listeners_changed_during_the_dispatch_are_the_standards() {
    let setup = "\
        function late() { out += 'late'; } \
        function second() { out += '2'; } \
        b.addEventListener('ping', () => { out += '1'; \
            b.removeEventListener('ping', second); b.addEventListener('ping', late); }); \
        b.addEventListener('ping', second); \
        b.addEventListener('ping', () => { out += 'o'; }, { once: true });";
    // The removed one is not called, the added one waits for the next
    // dispatch, and the `once` one is called once.
    assert_eq!(case(setup, &PING, ""), "1o");
    let mut page = match Page::new(setup, false) {
        Ok(page) => page,
        Err(why) => panic!("{why}"),
    };
    // The first dispatch, then a second in which `late` is there.
    for _ in 0..2 {
        assert_eq!(page.fire(&PING).as_deref(), Ok(""));
    }
    assert_eq!(page.read("out"), "1o1late");
}

#[test]
fn a_waiting_dispatch_holds_its_event_and_target_through_a_collection() {
    for stress in [false, true] {
        let setup = "b.addEventListener('ping', (e) => { \
            out += (e.target.parentNode === null) + ':' + e.target.textContent; });";
        let mut page = match Page::new(setup, stress) {
            Ok(page) => page,
            Err(why) => panic!("{why}"),
        };
        let button = page.button;
        assert!(matches!(page.held.dispatch(button, &PING), Ok(Some(_))));
        // Before the task runs, the page drops every reference it had to the
        // button and takes it out of the document; a collection runs.
        assert_eq!(page.read("outer.removeChild(b); b = null; 'gone'"), "gone");
        let Ok(looping) = page.looping() else {
            panic!("the page has a loop");
        };
        looping.engine().objects().heap_mut().collect();
        assert_eq!(looping.engine().objects().heap().check(), Ok(()));
        assert_eq!(page.drain().as_deref(), Ok(""));
        assert_eq!(page.read("out"), "true:Go");
        // Run, the task let go: a second collection is as sound.
        let Ok(looping) = page.looping() else {
            panic!("the page has a loop");
        };
        looping.engine().objects().heap_mut().collect();
        assert_eq!(looping.engine().objects().heap().check(), Ok(()));
    }
}

#[test]
fn a_target_with_no_wrapper_is_given_one_and_its_ancestors_listeners_hear() {
    // Nothing in script ever touched the button: its wrapper is made by the
    // dispatch, and the div's listener reads it as the target.
    let setup = "var outer = document.body.firstChild; \
        outer.addEventListener('ping', (e) => { \
            out += e.target.textContent + (e.target === outer.firstChild); });";
    for stress in [false, true] {
        let document = parse_document(PAGE);
        let Some(outer) = document.body().and_then(|body| document.first_child(body)) else {
            panic!("the page has a div");
        };
        let Some(button) = document.first_child(outer) else {
            panic!("the div has a button");
        };
        let mut held = Held::Parsed(document);
        let Ok(looping) = held.scripted(&alo_url::Url::about_blank(), IDENTITY, &clock(), &view())
        else {
            panic!("an empty heap takes the page");
        };
        looping.engine().objects().heap_mut().stress(stress);
        let _ = looping.queue_script("setup", format!("var out = ''; {setup}"));
        while looping.run_next().is_some() {}
        let mut page = Page {
            held,
            outer,
            button,
        };
        assert_eq!(page.fire(&PING).as_deref(), Ok(""));
        assert_eq!(page.read("out"), "Gotrue");
        // Fired at the div, the button's listeners are nobody's.
        assert_eq!(page.fire_at(page.outer, &PING).as_deref(), Ok(""));
        assert_eq!(page.read("out"), "GotrueGofalse");
    }
}

#[test]
fn a_page_that_never_ran_script_is_dispatched_to_by_nobody_and_given_no_heap() {
    let document = parse_document(PAGE);
    let Some(body) = document.body() else {
        panic!("the page has a body");
    };
    let mut held = Held::Parsed(document);
    assert!(matches!(held.dispatch(body, &PING), Ok(None)));
    assert!(held.event_loop().is_none(), "no heap was built");
    assert!(matches!(held, Held::Parsed(_)));
}

#[test]
fn a_node_the_page_does_not_have_is_refused_and_the_page_runs_on() {
    let mut page = match Page::new("b.addEventListener('ping', () => { out += 'b'; });", false) {
        Ok(page) => page,
        Err(why) => panic!("{why}"),
    };
    // An id another, larger document minted: not this page's.
    let other = parse_document(
        "<!doctype html><body><p>1</p><p>2</p><p>3</p><p>4</p><p>5</p><p>6</p></body>",
    );
    let Some(stranger) = other
        .body()
        .and_then(|body| other.last_child(body))
        .and_then(|p| other.last_child(p))
    else {
        panic!("the other page has a text node");
    };
    assert!(matches!(
        page.held.dispatch(stranger, &PING),
        Err(Unqueued::NoSuchNode(node)) if node == stranger
    ));
    assert_eq!(page.fire(&PING).as_deref(), Ok(""));
    assert_eq!(page.read("out"), "b");
}

#[test]
fn a_stop_during_a_dispatch_stops_the_page() {
    let setup = "\
        b.addEventListener('ping', () => { out += '1'; queueMicrotask(() => { out += 'a'; }); }); \
        b.addEventListener('ping', () => { while (true) {} }); \
        b.addEventListener('ping', () => { out += 'never'; });";
    let mut page = match Page::new(setup, false) {
        Ok(page) => page,
        Err(why) => panic!("{why}"),
    };
    let Ok(looping) = page.looping() else {
        panic!("the page has a loop");
    };
    let switch = looping.stop_switch();
    let button = page.button;
    assert!(matches!(page.held.dispatch(button, &PING), Ok(Some(_))));
    // A task queued behind the dispatch, which a stopped page never runs.
    let Ok(looping) = page.looping() else {
        panic!("the page has a loop");
    };
    assert!(looping.queue_script("after", "out += 'after';").is_ok());
    let asker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        switch.ask();
    });
    let turn = looping.run_next();
    assert!(asker.join().is_ok());
    let Some(turn) = turn else {
        panic!("the dispatch was waiting");
    };
    assert!(
        matches!(turn.stopped, Some(Stopped::Escaped(_))),
        "{turn:?}"
    );
    assert_eq!(looping.waiting(), 0, "what was waiting is dropped");
    assert!(looping.run_next().is_none());
    assert!(matches!(
        page.held.dispatch(button, &PING),
        Err(Unqueued::Stopped(_))
    ));
    assert_eq!(page.read("out"), "1a");
}

#[test]
fn a_thousand_listeners_that_throw_cost_a_counter_past_the_turns_room() {
    let setup = "var called = 0; for (let i = 0; i < 1000; i++) \
        b.addEventListener('ping', () => { called += 1; throw i; });";
    let mut page = match Page::new(setup, false) {
        Ok(page) => page,
        Err(why) => panic!("{why}"),
    };
    let button = page.button;
    assert!(matches!(page.held.dispatch(button, &PING), Ok(Some(_))));
    let Ok(looping) = page.looping() else {
        panic!("the page has a loop");
    };
    let Some(turn) = looping.run_next() else {
        panic!("the dispatch was waiting");
    };
    assert_eq!(turn.reports.len(), MOST_REPORTS);
    assert_eq!(turn.unreported, 1000 - MOST_REPORTS);
    assert!(turn.stopped.is_none(), "{turn:?}");
    assert_eq!(page.read("called"), "Number(1000.0)");
}
