/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0005's promise to the person looking at the screen.
//!
//! *"The tab keeps the last frame it painted and says what happened. Every
//! other tab is untouched, because no other tab was in that process. Reloading
//! is the user's to ask for."*
//!
//! `two_sites_two_processes.rs` asserts the first half of that sentence about
//! **renderers** — one dies, the others keep working. These assert it about
//! **tabs**, which is where a person meets it: the picture is still there, the
//! tab says why nothing is answering, and getting it back is something somebody
//! asked for rather than something that happened by itself.
//!
//! Real processes, spawned and confined, and one of them killed from outside
//! with `kill -9` — the only way to check a thing that is about a process
//! dying.

use alo_layout::geometry::Size;
use alo_renderer::frame::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::message::FromRenderer;
use alo_renderer::page::Page;
use alo_renderer::tab::{Lost, Tab, TabId, Tabs};

/// Tabs over the renderer binary as cargo built it for this test.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
}

/// A URL, or something that is plainly not one of these sites if the text was
/// wrong — the failure then shows up as the assertion it broke rather than as
/// a panic in a helper, which is where the lints want it.
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

/// A page, and a colour so that two of them are two pictures.
///
/// The colour rather than the words: these renderers are given no fonts, so
/// nothing here draws text at all. What is being asserted is that a picture
/// survives the process that made it, and a filled rectangle is a picture.
fn a_page(text: &str, colour: &str) -> Page {
    Page::new(
        format!("<p>{text}</p>"),
        Size {
            width: 200.0,
            height: 100.0,
        },
    )
    .with_sheet(format!(
        "p {{ margin: 8px; height: 40px; background: {colour} }}"
    ))
}

/// Load a tab and paint it, and hand back the picture that came out.
///
/// [`None`] rather than a panic if either step did not happen, so that the
/// test says which tab it was about.
fn showing(tabs: &mut Tabs, id: TabId, text: &str, colour: &str) -> Option<Frame> {
    match tabs.load(id, a_page(text, colour)) {
        Ok(FromRenderer::Loaded { .. }) => {}
        _ => return None,
    }
    match tabs.paint(id) {
        Ok(FromRenderer::Painted(frame)) => Some(frame),
        _ => None,
    }
}

/// Kill a tab's renderer the way the operating system would, and wait for the
/// operating system to actually reap it. Whether it was killed.
fn kill_the_renderer_of(tabs: &Tabs, id: TabId) -> bool {
    let Some(site) = tabs.tab(id).map(|tab| tab.site().clone()) else {
        return false;
    };
    let Some(victim) = tabs.renderers().process_of(&site) else {
        return false;
    };
    let killed = std::process::Command::new("kill")
        .arg("-9")
        .arg(victim.to_string())
        .status();
    if !killed.is_ok_and(|status| status.success()) {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    true
}

// --- The closing condition ---------------------------------------------------

/// The sentence this whole file exists for: killed from outside, and the tab
/// says what happened while every other tab keeps working.
#[test]
fn a_killed_renderer_leaves_its_tab_saying_what_happened_and_showing_its_picture() {
    let mut tabs = tabs();
    let bank = tabs.open(url("https://bank.example/statement"));
    let news = tabs.open(url("https://news.example/today"));

    let statement = showing(&mut tabs, bank, "your balance is four pounds", "#2f6f4f")
        .unwrap_or_else(|| panic!("the bank tab never painted anything"));
    let headline = showing(&mut tabs, news, "nothing happened today", "#8a3324")
        .unwrap_or_else(|| panic!("the news tab never painted anything"));
    assert_ne!(
        statement.pixels, headline.pixels,
        "two different pages painted the same picture, so this test proves nothing",
    );

    assert!(
        kill_the_renderer_of(&tabs, news),
        "the news renderer could not be killed"
    );

    // The tab finds out at the moment it next needs its renderer, which is
    // also the moment a person would notice.
    let repaint = tabs.paint(news);
    assert!(matches!(repaint, Err(Lost::Gone(_))), "{repaint:?}");

    let Some(dead) = tabs.tab(news) else {
        panic!("the tab went with its renderer");
    };
    assert!(!dead.is_live());
    let said = dead
        .what_happened()
        .unwrap_or_else(|| panic!("a dead tab said nothing"));
    assert!(
        said.contains("news.example"),
        "the tab did not say whose renderer it was: {said:?}",
    );
    assert_eq!(
        dead.frame(),
        Some(&headline),
        "the tab lost the picture it was showing, which is the blank rectangle",
    );

    // And every other tab is untouched, which is the entire point of the
    // process split.
    assert!(tabs.tab(bank).is_some_and(Tab::is_live));
    assert_eq!(
        tabs.tab(bank).and_then(Tab::frame),
        Some(&statement),
        "another site's tab lost its picture",
    );
    let again = tabs.paint(bank);
    assert!(
        matches!(again, Ok(FromRenderer::Painted(_))),
        "one renderer dying stopped another site's tab from painting: {again:?}",
    );
}

/// One process holds one site, so it takes **every** tab on that site — and,
/// because it holds nothing else, no other tab at all.
#[test]
fn a_dead_renderer_takes_every_tab_on_its_site_and_no_other() {
    let mut tabs = tabs();
    let statement = tabs.open(url("https://a.bank.example/statement"));
    let settings = tabs.open(url("https://b.bank.example/settings"));
    let news = tabs.open(url("https://news.example/"));

    let first = showing(
        &mut tabs,
        statement,
        "your balance is four pounds",
        "#2f6f4f",
    )
    .unwrap_or_else(|| panic!("the statement tab never painted anything"));
    let second = showing(&mut tabs, settings, "your address", "#1c3f94")
        .unwrap_or_else(|| panic!("the settings tab never painted anything"));
    let headline = showing(&mut tabs, news, "nothing happened today", "#8a3324")
        .unwrap_or_else(|| panic!("the news tab never painted anything"));

    // Two subdomains of one site are one process (ADR 0005), and a renderer
    // holds one page — so the second tab to load displaced the first, and
    // asking about the first would have answered about the second's page.
    let displaced = tabs.paint(statement);
    assert_eq!(displaced, Err(Lost::HoldsAnotherPage { holder: settings }));

    assert!(
        kill_the_renderer_of(&tabs, settings),
        "the bank renderer could not be killed"
    );
    let repaint = tabs.paint(settings);
    assert!(matches!(repaint, Err(Lost::Gone(_))), "{repaint:?}");

    for (id, painted) in [(statement, &first), (settings, &second)] {
        let Some(tab) = tabs.tab(id) else {
            panic!("{id} went with its renderer");
        };
        assert!(!tab.is_live(), "{id} was in the dead process and is live");
        assert_eq!(tab.frame(), Some(painted), "{id} lost its picture");
    }
    assert!(
        tabs.tab(news).is_some_and(Tab::is_live),
        "a tab in another process was told it had died",
    );
    assert_eq!(tabs.tab(news).and_then(Tab::frame), Some(&headline));
    assert!(matches!(tabs.paint(news), Ok(FromRenderer::Painted(_))));
}

/// *"Reloading is the user's to ask for; a browser that silently restarts a
/// renderer hides a bug that somebody needs to see."*
#[test]
fn a_dead_tab_comes_back_only_when_somebody_asks_it_to() {
    let mut tabs = tabs();
    let news = tabs.open(url("https://news.example/"));
    assert!(showing(&mut tabs, news, "nothing happened today", "#8a3324").is_some());
    assert_eq!(tabs.renderers().started(), 1);

    assert!(
        kill_the_renderer_of(&tabs, news),
        "the news renderer could not be killed"
    );

    // Everything that is not a person asking for this page again is refused,
    // and none of it starts a process.
    assert!(matches!(tabs.paint(news), Err(Lost::Gone(_))));
    assert!(matches!(tabs.read(news), Err(Lost::Gone(_))));
    assert_eq!(
        tabs.renderers().started(),
        1,
        "a dead tab was quietly given a new renderer",
    );

    // A deliberate load is the person asking, and it gets one.
    let reloaded = tabs.load(news, a_page("something happened after all", "#8a3324"));
    assert!(
        matches!(reloaded, Ok(FromRenderer::Loaded { .. })),
        "{reloaded:?}",
    );
    assert_eq!(tabs.renderers().started(), 2);
    assert!(tabs.tab(news).is_some_and(Tab::is_live));
    assert!(matches!(tabs.paint(news), Ok(FromRenderer::Painted(_))));
}
