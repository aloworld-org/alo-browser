/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The end of a renderer's life, which is the word the lifecycle was missing.
//!
//! `two_sites_two_processes.rs` asserts that a process **starts** per site, is
//! reused, and is evicted when there are too many. None of that is reaping: a
//! renderer whose last tab had closed used to run until the ceiling happened to
//! take it, so a person who closed sixteen tabs one by one still had sixteen
//! processes holding sixteen pages they had finished with.
//!
//! Real processes, and the assertions are about the **operating system's** view
//! of them rather than about a map in this program: a bookkeeping entry that
//! disappeared while the process kept running is precisely the bug that would
//! not show up in a test of the map.

use alo_layout::geometry::Size;
use alo_net::Cause;
use alo_renderer::host::Renderers;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::site::Site;
use alo_renderer::tab::{Lost, TabId, Tabs};
use std::collections::HashSet;

/// The renderer binary, as cargo built it for this test.
fn renderers() -> Renderers {
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
}

fn tabs() -> Tabs {
    Tabs::over(renderers())
}

/// A URL, or something that is plainly not one of these sites if the text was
/// wrong — so a mistake shows up as the assertion it broke rather than as a
/// panic in a helper.
fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn site(text: &str) -> Site {
    Site::of(&url(text))
}

fn a_page(text: &str) -> Page {
    Page::new(
        format!("<p>{text}</p>"),
        Size {
            width: 200.0,
            height: 100.0,
        },
    )
    .with_sheet("p { margin: 8px; height: 40px; background: #2f6f4f }")
}

/// Whether the operating system still has this process.
///
/// `kill -0` asks exactly that and sends nothing. It would also succeed for a
/// **zombie** — a process that has exited and not been waited for — and that is
/// why [`Renderers::stop`] waiting matters: a reaped renderer is waited for, so
/// a `false` here is the process genuinely gone rather than tidied up later.
fn still_running(process: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(process.to_string())
        // Its complaint about a process that is not there is the answer, not
        // something to print in the middle of a passing test run.
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The process behind a tab, if it has one.
///
/// [`None`] rather than a panic — for the lints' sake, and because a caller
/// that says which tab it was asking about is a better failure than a helper
/// saying a tab had no process.
fn process_behind(tabs: &Tabs, id: TabId) -> Option<u32> {
    let site = tabs.tab(id).map(|tab| tab.site().clone())?;
    tabs.renderers().process_of(&site)
}

/// Load a tab, so that its site really has a process running.
fn load(tabs: &mut Tabs, id: TabId, text: &str) {
    let loaded = tabs.load(id, a_page(text), Cause::Person { tab: id });
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{id} did not load: {loaded:?}",
    );
}

// --- The closing condition ---------------------------------------------------

/// The first half of what closes queue item 64: closing the last tab on a site
/// stops that site's process, watched going.
#[test]
fn closing_the_last_tab_on_a_site_stops_its_process() {
    let mut tabs = tabs();
    let bank = tabs.open(url("https://bank.example/statement"));
    let news = tabs.open(url("https://news.example/today"));
    load(&mut tabs, bank, "your balance is four pounds");
    load(&mut tabs, news, "nothing happened today");

    let (Some(doomed), Some(kept)) = (process_behind(&tabs, news), process_behind(&tabs, bank))
    else {
        panic!("a loaded tab has no renderer process");
    };
    assert_ne!(doomed, kept, "two sites shared a process");
    assert!(still_running(doomed));

    assert!(tabs.close(news));

    assert!(
        !still_running(doomed),
        "the last tab on a site closed and its process is still running",
    );
    assert_eq!(tabs.renderers().len(), 1, "the reaped renderer was kept");

    // And the other site is untouched, which is what makes this reaping rather
    // than a browser tidying up whatever it felt like.
    assert!(
        still_running(kept),
        "closing one tab took another site's process",
    );
    assert_eq!(
        process_behind(&tabs, bank),
        Some(kept),
        "the bank's renderer was restarted",
    );
    assert!(
        matches!(tabs.paint(bank), Ok(FromRenderer::Painted(_))),
        "another site's tab stopped working when a renderer was reaped",
    );
}

/// The second half, and the one that says reaping is not just stopping things:
/// the other tab is still showing a page out of that process.
#[test]
fn closing_one_of_two_tabs_on_a_site_stops_nothing() {
    let mut tabs = tabs();
    let statement = tabs.open(url("https://a.bank.example/statement"));
    let settings = tabs.open(url("https://b.bank.example/settings"));
    load(&mut tabs, statement, "your balance is four pounds");
    load(&mut tabs, settings, "your address");

    let Some(one_process) = process_behind(&tabs, settings) else {
        panic!("a loaded tab has no renderer process");
    };
    assert_eq!(
        process_behind(&tabs, statement),
        Some(one_process),
        "two subdomains of one site are two processes",
    );

    assert!(tabs.close(statement));

    assert!(
        still_running(one_process),
        "closing one of two tabs on a site stopped its process",
    );
    assert_eq!(tabs.renderers().len(), 1);
    assert_eq!(
        process_behind(&tabs, settings),
        Some(one_process),
        "the renderer was restarted",
    );
    assert!(
        matches!(tabs.paint(settings), Ok(FromRenderer::Painted(_))),
        "the tab still open lost the renderer it was showing a page out of",
    );
}

/// Closing a tab whose renderer had already died reaps nothing, because there
/// is nothing to reap — and does not fail on the way.
#[test]
fn closing_a_tab_whose_renderer_already_went_is_nothing_to_reap() {
    let mut tabs = tabs();
    let news = tabs.open(url("https://news.example/"));
    load(&mut tabs, news, "nothing happened today");

    let Some(victim) = process_behind(&tabs, news) else {
        panic!("a loaded tab has no renderer process");
    };
    let killed = std::process::Command::new("kill")
        .arg("-9")
        .arg(victim.to_string())
        .status();
    assert!(killed.is_ok_and(|status| status.success()), "not killed");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(matches!(tabs.paint(news), Err(Lost::Gone(_))));
    assert!(tabs.renderers().is_empty(), "a dead renderer was kept");

    assert!(tabs.close(news));
    assert!(tabs.is_empty());
    assert!(!still_running(victim));
    assert_eq!(
        tabs.renderers().started(),
        1,
        "closing a tab started a renderer",
    );
}

// --- What the bookkeeping owes after a process ends --------------------------

/// The page went with the process, so nothing is holding it. A `held` entry
/// left behind would refuse the next tab on the site on behalf of a renderer
/// that no longer exists — and that refusal names a tab which by then has been
/// closed, so it would be unanswerable as well as wrong.
#[test]
fn a_site_opened_again_after_reaping_is_not_refused_on_a_dead_renderers_behalf() {
    let mut tabs = tabs();
    let first = tabs.open(url("https://news.example/today"));
    load(&mut tabs, first, "nothing happened today");
    let Some(reaped) = process_behind(&tabs, first) else {
        panic!("a loaded tab has no renderer process");
    };
    assert!(tabs.close(first));
    assert!(!still_running(reaped));

    let second = tabs.open(url("https://news.example/tomorrow"));
    let before_loading = tabs.paint(second);
    assert!(
        !matches!(before_loading, Err(Lost::HoldsAnotherPage { .. })),
        "a new tab was refused for a page a stopped process was holding: \
         {before_loading:?}",
    );

    load(&mut tabs, second, "something happened after all");
    assert!(matches!(tabs.paint(second), Ok(FromRenderer::Painted(_))));
    assert_ne!(
        process_behind(&tabs, second),
        Some(reaped),
        "the reaped process came back to life",
    );
}

// --- The lifecycle on its own ------------------------------------------------

/// [`Renderers::reap`] taking what is **wanted** rather than what to stop, with
/// no tabs anywhere near it.
#[test]
fn reaping_stops_what_nothing_wants_and_leaves_the_rest_running() {
    let mut renderers = renderers();
    let bank = site("https://bank.example/");
    let news = site("https://news.example/");
    let page = ToRenderer::Load(Box::new(a_page("a page")));
    assert!(renderers.ask(&bank, &page).is_ok());
    assert!(renderers.ask(&news, &page).is_ok());

    let doomed = renderers
        .process_of(&news)
        .unwrap_or_else(|| panic!("the news renderer has no process"));
    let kept = renderers
        .process_of(&bank)
        .unwrap_or_else(|| panic!("the bank renderer has no process"));

    let stopped = renderers.reap(&HashSet::from([bank.clone()]));

    assert_eq!(stopped, vec![news.clone()], "it stopped the wrong thing");
    assert!(!renderers.holds(&news));
    assert!(!still_running(doomed));
    assert!(renderers.holds(&bank));
    assert!(still_running(kept));
    assert!(matches!(
        renderers.ask(&bank, &ToRenderer::Paint),
        Ok(FromRenderer::Painted(_))
    ));
    assert_eq!(renderers.started(), 2, "reaping started something");
}

/// It only ever ends things: a site that is wanted and has no renderer does not
/// get one out of this.
#[test]
fn reaping_never_starts_anything() {
    let mut renderers = renderers();
    let stopped = renderers.reap(&HashSet::from([site("https://never.example/")]));
    assert!(stopped.is_empty());
    assert!(renderers.is_empty());
    assert_eq!(renderers.started(), 0, "reaping started a renderer");
}
