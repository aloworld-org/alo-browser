/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 528: the tab strip's renderer stops answering, and the window
//! keeps answering (ADR 0024 §§ 2, 4).
//!
//! ADR 0024 § 4: *"its renderer dying is a strip that keeps its last frame
//! and says so, as a tab does"*; § 2: the event loop never waits on a
//! renderer. The strip's renderer is stopped with `kill -STOP` — alive, its
//! pipe open, never answering — and the window's picture is checked while
//! the conductor waits on it, and every tab is checked to go on working
//! after it has been given up on.
//!
//! The real thing on every side but the screen: the `alo-render` binary built
//! beside `alo`, spawned and confined by the real lifecycle, one for the
//! strip at its own site and one for each page's, and the real conductor on
//! its own thread. What the window does is played by what its event loop
//! does — hear [`News`] into a [`Showing`] and compose from it.
//!
//! Each strip the conductor sends is compared with the same strip drawn here,
//! in this process, by the engine: so a frame that matches is a strip that
//! said exactly those titles, in that order, with that tab selected.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Strip, StripTab};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use alo_window::beside::renderer_beside;
use alo_window::compose::{Scene, compose};
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use alo_window::notice::Lettering;
use alo_window::place::{STRIP_HEIGHT, strip_viewport};
use alo_window::showing::Showing;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long a renderer is given before it is given up on.
const PATIENCE: Duration = Duration::from_secs(4);

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

/// The size the page is laid out at, below the strip.
const PAGE: Size = Size {
    width: 480.0,
    height: 400.0,
};

fn offline() -> Result<Network, String> {
    Ok(Network::over(Pool::with_trust(Trust::of(&[])?)))
}

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// A conductor over real renderers with the corpus's fonts, telling every
/// piece of news — the strip's and the tab's — to the receiver.
fn conducting() -> Result<(Conductor, Receiver<News>), String> {
    let tabs = Tabs::over(
        Renderers::running(renderer()?, &[])
            .with_machine(alo_window::fonts::frozen())
            .waiting_at_most(PATIENCE),
    );
    let (tell, news) = channel();
    let conductor = Conductor::start(tabs, Fonts::AsStarted, offline()?, move |said| {
        tell.send(said).is_ok()
    })
    .map_err(|why| format!("no conductor: {why}"))?;
    Ok((conductor, news))
}

/// The corpus's sans-serif, and what `system-ui` and `sans-serif` mean.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (weight, bytes) in [
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ] {
        if let Some(font) = Font::load("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// `tabs` as the engine draws them, here, in a strip above [`PAGE`].
fn drawn(tabs: Vec<StripTab>) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    let strip = Strip {
        tabs,
        viewport: strip_viewport(PAGE),
    };
    match renderer.handle(ToRenderer::Strip(Box::new(strip))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the strip was not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the strip was not painted: {other:?}")),
    }
}

/// A page of `html` at a file address named `name`, as `alo PAGE.html`
/// would open it.
fn page_at(name: &str, html: &str) -> Result<(Url, Order), String> {
    let url = alo_url::parse(&format!("file:///frozen/{name}"))
        .map_err(|why| format!("not a URL: {why:?}"))?;
    let order = Order::Open {
        url: url.clone(),
        page: Some(Box::new(Page::new(html, Size::ZERO).at(url.clone()))),
    };
    Ok((url, order))
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

fn send(orders: &alo_window::inbox::Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

/// The renderers this test process started, by process id.
fn renderers_running() -> Result<Vec<String>, String> {
    let found = std::process::Command::new("pgrep")
        .args(["-P", &std::process::id().to_string(), "-x", "alo-render"])
        .output()
        .map_err(|why| format!("pgrep could not be run: {why}"))?;
    Ok(String::from_utf8_lossy(&found.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

fn signal(what: &str, process: &str) -> bool {
    std::process::Command::new("kill")
        .args([what, process])
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn a_strip_whose_renderer_stops_leaves_the_window_composing_and_the_tabs_working() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let lettering = Lettering::compiled_in().unwrap_or_else(|| panic!("no lettering"));
    let mut showing = Showing::default();

    // An empty tab, which starts no renderer of its own: the strip's is the
    // only one running.
    send(
        &orders,
        Order::Open {
            url: Url::about_blank(),
            page: None,
        },
    )
    .unwrap_or_else(|why| panic!("{why}"));
    send(&orders, Order::Resize(PAGE)).unwrap_or_else(|why| panic!("{why}"));
    let first = next(&news).unwrap_or_else(|why| panic!("{why}"));
    let News::Strip(strip) = &first else {
        panic!("the strip was not drawn: {first:?}");
    };
    assert_eq!(
        Ok(strip),
        drawn(vec![StripTab::new("", "about:blank").selected()]).as_ref()
    );
    showing.hear(first.clone());
    let running = renderers_running().unwrap_or_else(|why| panic!("{why}"));
    let [silent] = running.as_slice() else {
        panic!("expected the strip's renderer alone, found {running:?}");
    };
    assert!(
        signal("-STOP", silent),
        "the strip's renderer could not be stopped"
    );

    // The window grows. The conductor is now stuck waiting on the strip…
    send(&orders, Order::Resize(Size::new(560.0, 460.0))).unwrap_or_else(|why| panic!("{why}"));
    std::thread::sleep(Duration::from_millis(200));
    // …and the window still composes, at once, with the strip it had.
    let window = (560, 460 + STRIP_HEIGHT);
    let started = Instant::now();
    let composed = compose(&showing.scene(window, 1), &lettering);
    assert!(
        started.elapsed() < PATIENCE,
        "composing waited on the strip"
    );
    let before = compose(
        &Scene {
            window,
            replication: 1,
            strip: Some(strip),
            strip_sentence: None,
            frame: None,
            sentence: None,
        },
        &lettering,
    );
    assert!(composed == before, "the window did not show its last strip");
    assert!(news.try_recv().is_err(), "something was said too soon");

    // At the bound, the strip says what happened and keeps its frame.
    let said = next(&news).unwrap_or_else(|why| panic!("{why}"));
    let News::StripSaid(sentence) = &said else {
        panic!("the strip did not say what happened: {said:?}");
    };
    assert!(
        sentence.contains("is gone") && sentence.contains("said nothing"),
        "{sentence}"
    );
    assert!(showing.hear(said.clone()));
    assert_eq!(
        showing.strip(),
        Some(strip),
        "the strip's frame was not kept"
    );
    let told = compose(&showing.scene(window, 1), &lettering);
    assert_eq!(told.at(10, 10), before.at(10, 10), "the strip was not kept");
    assert_eq!(
        told.at(559, 495).map(alo_value::Rgba::to_rgba8),
        Some(alo_window::colours::notice_ground().to_rgba8()),
        "the strip's sentence is not along the bottom"
    );
    // The lifecycle stopped it rather than leaving it behind.
    assert_eq!(renderers_running().map(|running| running.len()), Ok(0));

    // A page still loads and paints: every tab is untouched, and the strip
    // is not drawn again by a fresh process behind the person's back.
    let (_, opening) = page_at("after.html", "<title>After</title><p>still here</p>")
        .unwrap_or_else(|why| panic!("{why}"));
    send(&orders, opening).unwrap_or_else(|why| panic!("{why}"));
    // Opened over the tab shown, which is no longer shown under it.
    assert_eq!(next(&news), Ok(News::Chosen(None)));
    let painted = next(&news).unwrap_or_else(|why| panic!("{why}"));
    assert!(matches!(painted, News::Painted(_)), "{painted:?}");
    assert_eq!(
        renderers_running().map(|running| running.len()),
        Ok(1),
        "only the page's renderer runs"
    );

    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Closed));
    assert!(conductor.finish(), "the conductor did not finish");
}
