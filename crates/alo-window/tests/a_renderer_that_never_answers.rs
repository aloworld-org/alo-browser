/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A renderer stops answering, and the window keeps answering.
//!
//! ADR 0024 § 2: *"The event loop never calls a renderer"*, and *"a renderer
//! that never answers leaves its last frame and, at `LONGEST_SILENCE`, the
//! sentence `tab.rs` already says."* Item 296 closes on a test that shows it,
//! and this is that test.
//!
//! The real thing on every side but the screen: the `alo-render` binary built
//! beside `alo`, spawned and confined by the real lifecycle, stopped with
//! `kill -STOP` — alive, its pipe open, never answering — and the real
//! conductor on its own thread. What the window does is played by the two
//! things the event loop does: hear [`News`] into a [`Showing`] and compose
//! from it. Neither has any way to reach a renderer, which is the design; what
//! this checks is that the window's picture is ready **while** the conductor
//! is stuck waiting, and is the frame it had.

use alo_layout::Size;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::beside::renderer_beside;
use alo_window::compose::{Scene, compose};
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use alo_window::notice::Lettering;
use alo_window::showing::Showing;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long a renderer is given before it is given up on. Long enough for a
/// debug build to lay out and paint the page, and short enough that a test
/// that waits for it to pass is a test somebody runs.
const PATIENCE: Duration = Duration::from_secs(4);

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

fn case() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-offline")
}

/// The renderer as `alo` finds it: beside `alo`.
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

/// The frozen page, as `alo PAGE.html` would open it.
fn opening() -> Result<Order, String> {
    let page = std::fs::read_to_string(case().join("page.html"))
        .map_err(|why| format!("the frozen page could not be read: {why}"))?;
    let url = alo_url::parse(&format!("file://{}", case().join("page.html").display()))
        .map_err(|why| format!("not a URL: {why:?}"))?;
    Ok(Order::Open {
        url: url.clone(),
        page: Some(Box::new(Page::new(page, Size::ZERO).at(url))),
    })
}

/// The corpus's committed render of the same page.
fn committed() -> Result<Frame, String> {
    let bytes = std::fs::read(case().join("render.png"))
        .map_err(|why| format!("the committed render could not be read: {why}"))?;
    let canvas = alo_paint::from_png(&bytes).map_err(|why| format!("not a picture: {why}"))?;
    Ok(Frame::from_canvas(&canvas))
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

/// Send `process` a signal. Whether it was sent.
fn signal(what: &str, process: &str) -> bool {
    std::process::Command::new("kill")
        .args([what, process])
        .status()
        .is_ok_and(|status| status.success())
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

#[test]
fn a_renderer_that_never_answers_leaves_the_window_answering_with_its_last_frame() {
    let tabs = Tabs::over(
        Renderers::running(renderer().unwrap_or_else(|why| panic!("{why}")), &[])
            .with_machine(alo_window::fonts::frozen())
            .waiting_at_most(PATIENCE),
    );
    let (tell, news) = channel();
    let conductor = Conductor::start(tabs, Fonts::AsStarted, move |said| tell.send(said).is_ok())
        .unwrap_or_else(|why| panic!("no conductor: {why}"));
    let orders = conductor.orders();
    let lettering = Lettering::compiled_in().unwrap_or_else(|| panic!("no lettering"));
    let mut showing = Showing::default();

    // The page opens before the window has a size, as `alo` sends it, and is
    // painted once the size arrives.
    orders
        .send(opening().unwrap_or_else(|why| panic!("{why}")))
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    orders
        .send(Order::Resize(Size::new(480.0, 400.0)))
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    let painted = next(&news).unwrap_or_else(|why| panic!("{why}"));
    let News::Painted(frame) = &painted else {
        panic!("the page was not painted: {painted:?}");
    };
    // `--frozen-fonts` is a claim that the window shows the corpus's page
    // pixel for pixel; this is where it is held to it.
    assert!(
        Ok(frame) == committed().as_ref(),
        "the frozen page, rendered by a renderer process with the corpus's fonts, is not the \
         corpus's committed render"
    );
    showing.hear(painted.clone());

    // Stop the renderer: alive, its pipe open, and never answering again.
    let running = renderers_running().unwrap_or_else(|why| panic!("{why}"));
    let [silent] = running.as_slice() else {
        panic!("expected one renderer, found {running:?}");
    };
    assert!(signal("-STOP", silent), "the renderer could not be stopped");

    // The window grows. The conductor is sent the size and is now stuck
    // waiting on a renderer that will not answer…
    orders
        .send(Order::Resize(Size::new(560.0, 460.0)))
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    std::thread::sleep(Duration::from_millis(200));
    // …and the window still answers, at once, with the frame it had, at the
    // size it was painted: the resize reference, pixel for pixel.
    let started = Instant::now();
    let composed = compose(&showing.scene((560, 460), 1), &lettering);
    assert!(
        started.elapsed() < PATIENCE,
        "composing took {:?}, as long as the renderer is given",
        started.elapsed()
    );
    let before = compose(
        &Scene {
            window: (560, 460),
            replication: 1,
            frame: Some(&committed().unwrap_or_else(|why| panic!("{why}"))),
            sentence: None,
        },
        &lettering,
    );
    assert!(
        composed == before,
        "the window did not show its last frame while waiting"
    );
    assert!(
        news.try_recv().is_err(),
        "something was said before the renderer was given up on"
    );

    // At the bound, the tab says what happened, and keeps its frame.
    let said = next(&news).unwrap_or_else(|why| panic!("{why}"));
    let News::Said(sentence) = &said else {
        panic!("the tab did not say what happened: {said:?}");
    };
    assert!(
        sentence.contains("is gone") && sentence.contains("said nothing"),
        "{sentence}"
    );
    assert!(showing.hear(said.clone()));
    assert_eq!(showing.frame(), Some(frame), "the frame was not kept");
    let told = compose(&showing.scene((560, 460), 1), &lettering);
    // The notice is along the bottom, over the frame's background, and the
    // frame above it is untouched.
    assert_eq!(
        told.at(559, 459).map(alo_value::Rgba::to_rgba8),
        Some(alo_window::colours::notice_ground().to_rgba8())
    );
    assert_eq!(told.at(240, 150), before.at(240, 150));

    // The renderer was stopped by the lifecycle, not left behind.
    assert_eq!(
        renderers_running().map(|running| running.len()),
        Ok(0),
        "the silent renderer was left running"
    );

    orders
        .send(Order::CloseEverything)
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    assert_eq!(next(&news), Ok(News::Closed));
    assert!(conductor.finish(), "the conductor did not finish");
}
