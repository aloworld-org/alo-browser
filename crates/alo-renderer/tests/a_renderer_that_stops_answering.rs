/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A renderer that is alive and says nothing, which used to hang the browser.
//!
//! `a_tab_whose_renderer_died.rs` kills a renderer and watches its tab survive.
//! That was always the *easy* half: a process that dies closes its pipe, and a
//! read that ends is an answer. A process that stays alive and stops answering
//! closes nothing, so `pipe::read` waited for ever — and the thing waiting was
//! the **browser** process, with every other tab and everything a person could
//! click behind it. ADR 0005 says that is the one thing which must never
//! happen.
//!
//! # Why the renderer here is stopped rather than stubbed
//!
//! `kill -STOP` is exactly the condition being tested: the process is alive,
//! `kill -0` finds it, its pipe is open, and it will never answer. It is also
//! the **real renderer binary**, confined, spawned by the real lifecycle — a
//! stand-in program that answered nothing would share only the silence, and the
//! part worth checking is that a browser process gives up on a *renderer*.
//!
//! And it is the same mechanism for the other half. A renderer that is merely
//! **slow** must not be killed, which is what decides whether the bound may
//! exist at all; a renderer stopped and then continued is slow by exactly as
//! long as a test says, which nothing about a real page could promise.
//!
//! # What failure looks like here
//!
//! Not a red assertion: a test that never returns. That is the bug itself —
//! with the bound taken out, the browser process waits in a read for as long as
//! the wedged renderer lives, and so does whoever is waiting for the browser
//! process. It is worth saying out loud, because an iteration that broke this
//! would otherwise spend its time wondering why the suite stopped.

use alo_layout::geometry::Size;
use alo_renderer::frame::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::site::Site;
use alo_renderer::tab::{Lost, Tab, TabId, Tabs};
use std::time::{Duration, Instant};

/// How long these tests give a renderer to answer.
///
/// Short, because a test that waited [`alo_renderer::answers::LONGEST_SILENCE`]
/// to find out what happens after it is a test nobody runs — which is the whole
/// reason the bound is a field rather than that constant used in place.
const SOON: Duration = Duration::from_millis(400);

/// Renderers over the renderer binary as cargo built it, waiting [`SOON`].
fn renderers() -> Renderers {
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]).waiting_at_most(SOON)
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

/// A page, and a colour so that two of them are two pictures.
///
/// These renderers are given no fonts, so nothing here draws text: what is
/// being kept across a silence is a picture, and a filled rectangle is one.
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

/// Whether the operating system still has this process.
///
/// `kill -0` asks exactly that and sends nothing. It succeeds for a **stopped**
/// process, which is what makes it the right question here: a wedged renderer
/// is alive, and a browser process that treated silence as death would be
/// guessing.
fn still_running(process: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(process.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Send a signal, and say whether it was sent.
fn signal(process: u32, which: &str) -> bool {
    std::process::Command::new("kill")
        .arg(which)
        .arg(process.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Stop a process without killing it: alive, and never going to answer.
///
/// The short sleep is for the same reason `a_tab_whose_renderer_died.rs` sleeps
/// after `kill -9` — the signal is delivered by the operating system rather
/// than by the call returning.
fn wedge(process: u32) -> bool {
    let sent = signal(process, "-STOP");
    std::thread::sleep(Duration::from_millis(100));
    sent
}

// --- The closing condition ---------------------------------------------------

/// A renderer that never answers is given up on after a bound this test names,
/// and it is stopped — because an answer arriving after we stopped waiting for
/// it would be handed back as the answer to the next question.
#[test]
fn a_renderer_that_stops_answering_is_given_up_on_and_stopped() {
    let mut renderers = renderers();
    let news = site("https://news.example/");
    let loaded = renderers.ask(
        &news,
        &ToRenderer::Load(Box::new(a_page("nothing happened today", "#8a3324"))),
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "the renderer was not working before it was wedged: {loaded:?}",
    );
    let Some(wedged) = renderers.process_of(&news) else {
        panic!("a loaded site has no renderer process");
    };
    assert!(wedge(wedged), "the renderer could not be stopped");
    assert!(
        still_running(wedged),
        "a stopped process is meant to be alive, so this test is testing the wrong thing",
    );

    let began = Instant::now();
    let asked = renderers.ask(&news, &ToRenderer::Paint);
    let waited = began.elapsed();

    let Err(gone) = asked else {
        panic!("a renderer that cannot answer answered: {asked:?}");
    };
    assert!(
        waited >= SOON,
        "it gave up after {waited:?}, before the {SOON:?} it was given had passed",
    );
    assert!(
        waited < SOON * 8,
        "it waited {waited:?} for a bound of {SOON:?}",
    );
    assert!(
        gone.why.contains("said nothing"),
        "it did not say that the renderer had gone quiet: {gone:?}",
    );
    assert!(
        gone.to_string().contains("news.example"),
        "it did not say whose renderer it was: {gone}",
    );

    // And the process is gone rather than left wedged for ever — which matters
    // twice over: a stopped process holds its memory, and a renderer nobody is
    // listening to any more must not still be holding somebody's page.
    assert!(
        !still_running(wedged),
        "the renderer was given up on and left running",
    );
    assert!(
        renderers.is_empty(),
        "a renderer that could not answer was kept",
    );
}

/// The half that decides what the bound may be: a renderer that is **slow** is
/// waited for, not killed. A bound too eager to fire would lose pages that were
/// about to arrive, and that is a worse browser than a slow one.
#[test]
fn a_renderer_that_is_merely_slow_is_waited_for() {
    let mut renderers = Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .waiting_at_most(Duration::from_secs(20));
    let news = site("https://news.example/");
    assert!(
        renderers
            .ask(
                &news,
                &ToRenderer::Load(Box::new(a_page("nothing happened today", "#8a3324"))),
            )
            .is_ok(),
    );
    let Some(slow) = renderers.process_of(&news) else {
        panic!("a loaded site has no renderer process");
    };
    assert!(wedge(slow), "the renderer could not be stopped");

    // Silent for a while, and then answering — which is what "slow" is from
    // this side of a pipe, and is indistinguishable from a renderer working
    // hard on a large page.
    let slowly = Duration::from_millis(700);
    std::thread::spawn(move || {
        std::thread::sleep(slowly);
        signal(slow, "-CONT");
    });

    let began = Instant::now();
    let painted = renderers.ask(&news, &ToRenderer::Paint);
    let waited = began.elapsed();

    assert!(
        matches!(painted, Ok(FromRenderer::Painted(_))),
        "a renderer that was only slow was not waited for: {painted:?}",
    );
    assert!(
        waited >= slowly,
        "it answered in {waited:?}, before it had been let run again",
    );
    assert!(
        still_running(slow),
        "a renderer that answered was killed anyway",
    );
    assert!(renderers.holds(&news), "a slow renderer was given up on");
    assert!(
        matches!(
            renderers.ask(&news, &ToRenderer::Paint),
            Ok(FromRenderer::Painted(_))
        ),
        "a renderer that was slow once was not asked again",
    );
}

/// The second clause: the tab says what happened in the same shape as a tab
/// whose renderer died. From a person's side those *are* the same event — the
/// page stopped answering — and a browser that told them apart would be
/// explaining its own bookkeeping rather than what happened.
#[test]
fn a_tab_whose_renderer_stopped_answering_says_so_and_keeps_its_picture() {
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

    let Some(wedged) = tabs.renderers().process_of(&site("https://news.example/")) else {
        panic!("a loaded tab has no renderer process");
    };
    assert!(wedge(wedged), "the news renderer could not be stopped");

    let repaint = tabs.paint(news);
    assert!(matches!(repaint, Err(Lost::Gone(_))), "{repaint:?}");

    let Some(quiet) = tabs.tab(news) else {
        panic!("the tab went with its renderer");
    };
    assert!(!quiet.is_live());
    let said = quiet
        .what_happened()
        .unwrap_or_else(|| panic!("a tab whose renderer went quiet said nothing"));
    assert!(
        said.contains("news.example") && said.contains("said nothing"),
        "the tab did not say what happened to which renderer: {said:?}",
    );
    assert_eq!(
        quiet.frame(),
        Some(&headline),
        "the tab lost the picture it was showing, which is the blank rectangle",
    );

    // Every other tab is untouched, which is the entire point of the split —
    // and is the assertion that would fail if the browser process had simply
    // been hung by the wedged renderer rather than giving up on it.
    assert!(tabs.tab(bank).is_some_and(Tab::is_live));
    assert_eq!(tabs.tab(bank).and_then(Tab::frame), Some(&statement));
    let again = tabs.paint(bank);
    assert!(
        matches!(again, Ok(FromRenderer::Painted(_))),
        "one renderer going quiet stopped another site's tab from painting: {again:?}",
    );
    assert!(
        !still_running(wedged),
        "the wedged renderer was left running"
    );
}

/// And it comes back the same way a dead one does: when a person asks for the
/// page again, and never on its own. ADR 0005's rule does not have an exception
/// for a renderer that went quiet rather than dying.
#[test]
fn a_tab_whose_renderer_went_quiet_comes_back_only_when_somebody_asks() {
    let mut tabs = tabs();
    let news = tabs.open(url("https://news.example/"));
    assert!(showing(&mut tabs, news, "nothing happened today", "#8a3324").is_some());
    assert_eq!(tabs.renderers().started(), 1);

    let Some(wedged) = tabs.renderers().process_of(&site("https://news.example/")) else {
        panic!("a loaded tab has no renderer process");
    };
    assert!(wedge(wedged), "the news renderer could not be stopped");
    assert!(matches!(tabs.paint(news), Err(Lost::Gone(_))));

    // Nothing that is not a person asking for the page again gets a process,
    // and neither of these waits for the bound a second time: the renderer is
    // gone, and the tab answers from what it was told.
    let began = Instant::now();
    assert!(matches!(tabs.paint(news), Err(Lost::Gone(_))));
    assert!(matches!(tabs.read(news), Err(Lost::Gone(_))));
    assert!(
        began.elapsed() < SOON,
        "a tab that had already been told waited for the bound all over again",
    );
    assert_eq!(
        tabs.renderers().started(),
        1,
        "a tab whose renderer went quiet was given another one without being asked",
    );

    let reloaded = tabs.load(news, a_page("something happened after all", "#8a3324"));
    assert!(
        matches!(reloaded, Ok(FromRenderer::Loaded { .. })),
        "{reloaded:?}",
    );
    assert_eq!(tabs.renderers().started(), 2);
    assert!(tabs.tab(news).is_some_and(Tab::is_live));
    assert!(matches!(tabs.paint(news), Ok(FromRenderer::Painted(_))));
}
