/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The page's tab in the frame around Chrome's window follows the page's
//! title, and is drawn by a tab strip's renderer, outside this process
//! (queue item 681, ADR 0024 § 4).
//!
//! A shown Chrome — its window placed on the screen as `alo --chrome`
//! places it — opens a page titled "First" that retitles itself, a moment
//! after it loads, to a title holding markup. The frame's side is this
//! test's: everything the thread holding Chrome tells the frame is kept,
//! and it is told what Chrome calls the page each time that changes — the
//! first title, then the second, at the page's address.
//!
//! Each name it was told is then handed, as the frame hands it, to the
//! frame's strip ([`FrameStrip`]): a thread holding the `alo-render` binary
//! built beside `alo`, spawned and confined as every renderer is, at the
//! strip's own site. What it paints for each is compared with the same
//! strip drawn here, in this process, by the engine — so a frame that
//! matches is a tab that says exactly that title, drawn by the strip's
//! renderer and not by the browser process. What the frame then composes
//! of it is `the_window_composed.rs`'s reference renders.

use alo_renderer::host::Renderers;
use alo_renderer::strip::Strip;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::agent_sites::AgentSites;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::chrome_protocol::Named;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::frame_strip::{FrameStrip, strip_of};
use alo_window::frame_told::ToFrame;
use alo_window::message::Order;
use alo_window::opening::CHROME_AT;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Where the page says it was served from. Frozen: nothing reaches it.
const ADDRESS: &str = "https://example.com/retitled";

/// What the page retitles itself to: markup, which a tab shows as the
/// characters it is.
const SECOND: &str = "Second <b>&amp;</b>";

/// A page titled "First", which retitles itself to [`SECOND`] a moment
/// after it has loaded — late enough that the frame has been told the
/// first title first.
const RETITLING: &str = "<!doctype html><title>First</title><h1>Retitled</h1><script>\
     addEventListener('load', () => setTimeout(() => { \
     document.title = 'Second <b>&amp;</b>'; }, 1500));</script>";

/// How long the test waits for anything it expects before saying so.
const AT_MOST: Duration = Duration::from_secs(60);

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

/// The corpus's sans-serif, and what `system-ui` and `sans-serif` mean: the
/// strip's stylesheet asks for them in that order.
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

/// `strip` as the engine draws it here, in this process.
fn drawn_here(strip: Strip) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    match renderer.handle(ToRenderer::Strip(Box::new(strip))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the strip was not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the strip was not painted: {other:?}")),
    }
}

/// Every name the frame is told, in order, until it is told one titled
/// [`SECOND`] or [`AT_MOST`] has passed.
fn names_until_retitled(told: &Receiver<ToFrame>) -> Vec<Named> {
    let until = Instant::now() + AT_MOST;
    let mut told_names = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Named(named)) => {
                let retitled = named.title == SECOND;
                told_names.push(named);
                if retitled {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    told_names
}

/// What the strip's thread tells the frame next, within [`AT_MOST`].
fn painted(told: &Receiver<ToFrame>) -> Result<Frame, String> {
    match told.recv_timeout(AT_MOST) {
        Ok(ToFrame::Strip(frame)) => Ok(frame),
        Ok(other) => Err(format!("the strip's thread told {other:?}")),
        Err(why) => Err(format!("the strip's thread told nothing: {why}")),
    }
}

#[test]
fn the_frames_tab_follows_the_pages_title_drawn_by_the_strips_renderer() -> Result<(), String> {
    let (tell, told) = channel();
    let serving = ChromeServing::start_shown(
        Chromium::Machines,
        Opening::Frozen {
            url: ADDRESS.to_owned(),
            page: RETITLING.as_bytes().to_vec(),
        },
        (600, 400),
        CHROME_AT,
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no tab in a shown Chrome: {why}"))?;
    let names = names_until_retitled(&told);
    // Chrome's window closed with the thread holding it.
    let _ = serving.orders().send(Order::CloseEverything);
    serving
        .records()
        .ok_or("the tab in Chrome did not finish")?;

    let first = Named {
        title: "First".to_owned(),
        address: ADDRESS.to_owned(),
    };
    let second = Named {
        title: SECOND.to_owned(),
        address: ADDRESS.to_owned(),
    };
    // Told the first title as the page loaded, then the second once it
    // retitled itself, and nothing between: a name is told only as it
    // changes.
    assert_eq!(names, [first.clone(), second.clone()], "{names:#?}");

    let (tell, painting) = channel();
    let mut strip = FrameStrip::start(
        Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen()),
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no thread for the frame's strip: {why}"))?;
    let mut tabs = Vec::new();
    for named in &names {
        strip.draw(strip_of(named, 1.0));
        tabs.push(painted(&painting)?);
    }
    // The same strip again is not drawn again.
    strip.draw(strip_of(&second, 1.0));
    assert!(
        painting.recv_timeout(Duration::from_secs(2)).is_err(),
        "the same strip was painted twice"
    );
    assert!(strip.finish(), "the frame's strip did not finish");

    let [first_tab, second_tab] = tabs.as_slice() else {
        panic!("one tab painted for each name: {}", tabs.len());
    };
    assert_eq!((first_tab.width, first_tab.height), (216, 36));
    assert_eq!(*first_tab, drawn_here(strip_of(&first, 1.0))?);
    assert_eq!(*second_tab, drawn_here(strip_of(&second, 1.0))?);
    assert_ne!(first_tab, second_tab, "the tab follows the title");
    Ok(())
}
