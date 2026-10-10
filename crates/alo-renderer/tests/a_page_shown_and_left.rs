/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 373 (ADR 0039 §§ 2–5): a page shown, and a page left.
//!
//! - The last step of a load is `pageshow` at the window: a
//!   `PageTransitionEvent`, trusted, `persisted` `false`, after the page's
//!   own scripts and before `Loaded`.
//! - `Leave` runs the leaving steps as one task — `pagehide`, then
//!   `visibilitychange` if the page was visible, then `unload` — and the
//!   renderer holds no page afterwards. A page already hidden hears no
//!   `visibilitychange`.
//! - A page that loops in `pagehide` is stopped at the deadline, and the
//!   answer says so.
//! - Nothing the page queued as it was left runs afterwards; its microtasks
//!   run in the task's own checkpoints, as in every task.
//! - A `Load` into a renderer holding a page leaves that page first, and
//!   says what it said at the front of `Loaded`, marked as the left page's;
//!   what it asked to fetch is carried apart, and where it asked to go is not
//!   carried at all.
//! - Across the boundary, a leaving page's fetch is refused by name, as its
//!   own document's, whether its tab was closed or a load replaced it.
//!
//! Every task after a load runs ordinarily and with the collector at every
//! allocation.

use std::time::{Duration, Instant};

use alo_layout::Size;
use alo_net::cause::Cause;
use alo_renderer::deadline::LONGEST_LEAVING;
use alo_renderer::fetch::Fetched;
use alo_renderer::fetch_decide::Rule;
use alo_renderer::host::Renderers;
use alo_renderer::message::Failure;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer, Visibility};
use alo_text::FontDatabase;
use alo_url::Url;

/// Where every page here is.
const HERE: &str = "https://shop.example/a/";

/// A page that says, by asking to fetch `heard?…`, every leaving event it
/// hears — its type, then what a listener reads of it and of the document —
/// and writes what it reads of `pageshow` into the root's `data-shown`.
/// `extra` is more script, after that.
fn page(extra: &str) -> String {
    format!(
        "<!DOCTYPE html><html><body><a href=\"/elsewhere\"></a><script>\
         var root = document.documentElement; \
         function say(what) {{ fetch('heard?' + what); }} \
         function read(e) {{ return e.type + '-' + e.persisted + '-' + e.isTrusted + '-' + \
           e.bubbles + '-' + e.cancelable + '-' + e.eventPhase + '-' + (e.target === window) + \
           '-' + document.visibilityState; }} \
         window.addEventListener('pageshow', function (e) {{ \
           root.setAttribute('data-shown', (root.getAttribute('data-shown') || '') + read(e) + ';'); }}); \
         window.addEventListener('pagehide', function (e) {{ say(read(e)); }}); \
         document.addEventListener('visibilitychange', function (e) {{ say(read(e)); }}); \
         window.addEventListener('unload', function (e) {{ say(read(e)); }}); \
         {extra}</script></body></html>"
    )
}

fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// A renderer with `page` loaded, and its load's answer.
fn loaded(page: Page) -> (Renderer, FromRenderer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = renderer.handle(ToRenderer::Load(Box::new(page)));
    (renderer, answer)
}

/// [`page`] with `extra`, loaded at [`HERE`] as `visibility` says, its heap
/// collecting at every allocation from then on if `stress`.
fn shown(extra: &str, visibility: Visibility, stress: bool) -> Result<Renderer, String> {
    let (mut renderer, answer) = loaded(
        Page::new(page(extra), Size::new(400.0, 200.0))
            .at(url(HERE))
            .shown_as(visibility),
    );
    let FromRenderer::Loaded { issues, .. } = answer else {
        return Err(format!("not loaded: {answer:?}"));
    };
    if !issues.is_empty() {
        return Err(format!("the load said {issues:?}"));
    }
    let looping = renderer
        .event_loop()
        .ok_or("none of the page's script ran")?;
    looping.engine().objects().heap_mut().stress(stress);
    Ok(renderer)
}

/// The root element's attribute `name`, if the page's script wrote one.
fn root(renderer: &Renderer, name: &str) -> Option<String> {
    let document = renderer.document()?;
    let html = document
        .descendants(document.root())
        .find(|id| document.element(*id).is_some())?;
    document.element(html)?.attr(name).map(ToOwned::to_owned)
}

/// What each fetch asked for, with [`HERE`] taken off the front.
fn heard(fetches: &[alo_renderer::fetch::FetchAsk]) -> Vec<String> {
    fetches
        .iter()
        .map(|ask| ask.url.strip_prefix(HERE).unwrap_or(&ask.url).to_owned())
        .collect()
}

/// Leave the page, and answer what it said and the fetches it asked for.
fn leave(renderer: &mut Renderer) -> (Vec<String>, Vec<String>) {
    match renderer.handle(ToRenderer::Leave) {
        FromRenderer::Left { issues, fetches } => (issues, heard(&fetches)),
        other => (vec![format!("not left: {other:?}")], Vec::new()),
    }
}

/// The three leaving events, as [`page`]'s listeners say them, for a page
/// left `visible`: `pagehide` a `PageTransitionEvent`, bubbling and
/// cancelable as HTML says; then two plain `Event`s, which have no
/// `persisted`, and the state `hidden` from the second on.
const LEFT_VISIBLE: [&str; 3] = [
    "heard?pagehide-false-true-true-true-2-true-visible",
    "heard?visibilitychange-undefined-true-true-false-2-false-hidden",
    "heard?unload-undefined-true-false-false-2-true-hidden",
];

#[test]
fn the_last_step_of_a_load_is_pageshow_at_the_window() {
    for stress in [false, true] {
        let renderer = shown(
            // A listener a later script adds hears it too: it comes after
            // every script, not after the first.
            "</script><script>window.addEventListener('pageshow', function (e) { \
               root.setAttribute('data-later', e.type); });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        // Once: a PageTransitionEvent, not persisted, trusted, bubbling and
        // cancelable as HTML says, at its target, the window.
        assert_eq!(
            root(&renderer, "data-shown").as_deref(),
            Some("pageshow-false-true-true-true-2-true-visible;"),
            "stress: {stress}"
        );
        assert_eq!(root(&renderer, "data-later").as_deref(), Some("pageshow"));
    }
}

#[test]
fn persisted_is_read_from_a_page_transition_event_and_nothing_else() {
    let renderer = shown(
        "window.addEventListener('pageshow', function (e) { \
           var plain = new Event('pageshow'); \
           var fake = new Event('pageshow'); fake.__proto__ = e.__proto__; \
           var refused = 'nothing'; \
           try { fake.persisted; } catch (x) { refused = x.name; } \
           root.setAttribute('data-persisted', (e.__proto__.__proto__ === plain.__proto__) + \
             ' ' + typeof PageTransitionEvent + ' ' + e.hasOwnProperty('persisted') + ' ' + \
             ('persisted' in e) + ' ' + ('persisted' in plain) + ' ' + refused); });",
        Visibility::Visible,
        false,
    )
    .unwrap_or_else(|why| panic!("{why}"));
    // Its prototype inherits Event's; no interface object (only the browser
    // makes one); `persisted` is its prototype's, not the event's own, and
    // no plain event's; and the getter refuses any other event.
    assert_eq!(
        root(&renderer, "data-persisted").as_deref(),
        Some("true undefined false true false TypeError")
    );
}

#[test]
fn a_page_left_hears_pagehide_then_visibilitychange_then_unload() {
    for stress in [false, true] {
        let mut renderer =
            shown("", Visibility::Visible, stress).unwrap_or_else(|why| panic!("{why}"));
        let (issues, fetches) = leave(&mut renderer);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(fetches, LEFT_VISIBLE, "stress: {stress}");

        // The renderer holds no page now: nothing to paint, read or leave.
        assert!(renderer.event_loop().is_none());
        assert!(renderer.document().is_none());
        assert_eq!(
            renderer.handle(ToRenderer::Paint),
            FromRenderer::Failed(Failure::NothingLoaded)
        );
        assert_eq!(
            renderer.handle(ToRenderer::Leave),
            FromRenderer::Failed(Failure::NothingLoaded)
        );
    }
}

#[test]
fn a_page_already_hidden_hears_no_visibilitychange() {
    for stress in [false, true] {
        let mut renderer =
            shown("", Visibility::Hidden, stress).unwrap_or_else(|why| panic!("{why}"));
        let (issues, fetches) = leave(&mut renderer);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            fetches,
            [
                "heard?pagehide-false-true-true-true-2-true-hidden",
                "heard?unload-undefined-true-false-false-2-true-hidden",
            ],
            "stress: {stress}"
        );
    }
}

#[test]
fn a_page_that_will_not_leave_is_stopped_at_the_deadline_and_says_so() {
    for stress in [false, true] {
        let mut renderer = shown(
            "window.addEventListener('pagehide', function () { for (;;) {} });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        let began = Instant::now();
        let (issues, fetches) = leave(&mut renderer);
        let took = began.elapsed();
        assert!(took >= LONGEST_LEAVING, "stopped early, after {took:?}");
        assert!(took < Duration::from_secs(30), "not stopped: {took:?}");
        // The first `pagehide` listener said its word; the loop stopped the
        // page, so nothing after it ran.
        assert_eq!(fetches, LEFT_VISIBLE[..1], "stress: {stress}");
        assert!(
            issues
                .iter()
                .all(|line| line.starts_with("the page that was left: ")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("the page stopped")),
            "{issues:?}"
        );
        assert!(
            issues
                .iter()
                .any(|line| line.contains("still running 1000 ms after it began to leave")),
            "{issues:?}"
        );
        assert!(renderer.event_loop().is_none(), "it was let go of");
    }
}

#[test]
fn nothing_the_page_queued_as_it_was_left_runs_afterwards() {
    for stress in [false, true] {
        let mut renderer = shown(
            "window.addEventListener('pagehide', function () { \
               queueMicrotask(function () { say('microtask'); }); \
               fetch('asked').then(function () { say('reaction'); }, \
                 function () { say('reaction'); }); \
               Promise.resolve().then(function () { say('job'); }); });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        let (issues, fetches) = leave(&mut renderer);
        assert!(issues.is_empty(), "{issues:?}");
        // The listener's own jobs run in the checkpoint after it, as in every
        // task — before `visibilitychange` — and the fetch is asked.
        assert_eq!(
            fetches,
            [
                LEFT_VISIBLE[0],
                "asked",
                "heard?microtask",
                "heard?job",
                LEFT_VISIBLE[1],
                LEFT_VISIBLE[2],
            ],
            "stress: {stress}"
        );

        // The answer to its fetch arrives at a renderer holding no page, and
        // the reaction waiting for it never runs.
        assert_eq!(
            renderer.handle(ToRenderer::Fetched(Box::new(Fetched::failed(1)))),
            FromRenderer::Failed(Failure::NothingLoaded)
        );
    }
}

#[test]
fn a_load_leaves_the_page_it_replaces_and_says_so_first() {
    for stress in [false, true] {
        let mut renderer = shown(
            "window.addEventListener('pagehide', function () { \
               document.querySelectorAll('a')[0].click(); \
               throw new TypeError('not leaving quietly'); });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        let next = Page::new(
            "<!DOCTYPE html><p></p><script>fetch('mine');</script>",
            Size::new(400.0, 200.0),
        )
        .at(url("https://shop.example/b/"));
        let FromRenderer::Loaded {
            issues,
            fetches,
            left,
            navigation,
            ..
        } = renderer.handle(ToRenderer::Load(Box::new(next)))
        else {
            panic!("not loaded");
        };
        // What the left page said is first, and marked as its own.
        let [said, rest @ ..] = issues.as_slice() else {
            panic!("nothing was said: {issues:?}");
        };
        assert!(
            said.starts_with("the page that was left: ") && said.contains("not leaving quietly"),
            "{issues:?}"
        );
        assert!(rest.is_empty(), "{issues:?}");
        // Its fetches are carried apart from the new page's, and where it
        // asked to go is not carried at all.
        assert_eq!(heard(&left), LEFT_VISIBLE, "stress: {stress}");
        assert_eq!(heard(&fetches), ["https://shop.example/b/mine"]);
        assert_eq!(navigation, None);
        // The new page is the one held.
        assert!(root(&renderer, "data-shown").is_none());
    }
}

#[test]
fn a_page_that_never_ran_script_is_left_with_nothing_said() {
    let (mut renderer, _) = loaded(Page::new("<p>hi</p>", Size::new(100.0, 100.0)));
    assert_eq!(
        renderer.handle(ToRenderer::Leave),
        FromRenderer::Left {
            issues: Vec::new(),
            fetches: Vec::new(),
        }
    );
    assert!(renderer.document().is_none());
    let mut nothing = Renderer::new(FontDatabase::new());
    assert_eq!(
        nothing.handle(ToRenderer::Leave),
        FromRenderer::Failed(Failure::NothingLoaded)
    );
}

// --- What the browser process decides -----------------------------------------

/// Tabs over the real renderer binary.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
}

#[test]
fn a_closed_tabs_page_is_left_and_its_fetches_refused_by_name() {
    let mut tabs = tabs();
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        Page::new(page(""), Size::new(400.0, 200.0)).at(url(HERE)),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let document = tabs.tab(tab).and_then(Tab::document).expect("a document");
    assert!(tabs.left().is_empty(), "nothing left yet");

    assert!(tabs.close(tab));
    let left = tabs.left();
    let [leaving] = left.as_slice() else {
        panic!("one page left: {left:?}");
    };
    assert_eq!(leaving.tab, tab);
    let refused: Vec<String> = leaving
        .refused
        .iter()
        .map(|refusal| {
            assert_eq!(refusal.rule, Rule::Leaving);
            assert_eq!(refusal.cause, Cause::Document { document });
            refusal
                .asked
                .strip_prefix(HERE)
                .unwrap_or(&refusal.asked)
                .to_owned()
        })
        .collect();
    assert_eq!(refused, LEFT_VISIBLE);
    // Each refusal is said, in words.
    assert_eq!(leaving.said.len(), 3, "{:?}", leaving.said);
    assert!(
        leaving
            .said
            .iter()
            .all(|line| line.contains("as the page was being left")),
        "{:?}",
        leaving.said
    );
    assert!(tabs.left().is_empty(), "taken once");
    assert!(
        tabs.renderers().is_empty(),
        "the site's renderer was reaped"
    );
}

#[test]
fn a_load_replacing_a_page_refuses_what_that_page_asked_as_its_own() {
    let mut tabs = tabs();
    let tab = tabs.open(url(HERE));
    let first = tabs.load(
        tab,
        Page::new(page(""), Size::new(400.0, 200.0)).at(url(HERE)),
        Cause::Person { tab },
    );
    assert!(
        matches!(first, Ok(FromRenderer::Loaded { .. })),
        "{first:?}"
    );
    let replaced = tabs.tab(tab).and_then(Tab::document).expect("a document");

    let next = tabs.load(
        tab,
        Page::new("<!DOCTYPE html><p>next</p>", Size::new(400.0, 200.0)).at(url(HERE)),
        Cause::Person { tab },
    );
    let Ok(FromRenderer::Loaded { issues, left, .. }) = next else {
        panic!("not loaded: {next:?}");
    };
    assert!(left.is_empty(), "handed to the tabs, not to whoever loaded");
    assert_eq!(
        issues
            .iter()
            .filter(|line| line.contains("as the page was being left"))
            .count(),
        3,
        "{issues:?}"
    );
    let left = tabs.left();
    let [leaving] = left.as_slice() else {
        panic!("one page left: {left:?}");
    };
    assert!(leaving.said.is_empty(), "the load said it");
    assert_eq!(leaving.refused.len(), 3);
    for refusal in &leaving.refused {
        assert_eq!(refusal.rule, Rule::Leaving);
        assert_eq!(
            refusal.cause,
            Cause::Document { document: replaced },
            "the left page's, not the new one's"
        );
    }
    assert_ne!(tabs.tab(tab).and_then(Tab::document), Some(replaced));
}
