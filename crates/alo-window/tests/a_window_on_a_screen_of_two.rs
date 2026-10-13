/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 562 (ADR 0057 § 3): the window tells both its renderers the
//! scale of the screen it is on, with its size, and each paints its frame in
//! device pixels at that scale.
//!
//! The real thing on every side but the screen: the `alo-render` binary
//! built beside `alo`, spawned and confined by the real lifecycle, and the
//! real conductor on its own thread. Each frame the conductor sends is
//! compared with the same page, or the same strip, drawn here in this
//! process by the engine at the scale the window said — so a frame that
//! matches is one laid out and painted at that scale, not at one and
//! enlarged.
//!
//! The page is a row of boxes at widths and offsets in tenths of a pixel,
//! which are rounded onto a finer grid at two (ADR 0057 § 2), so its frame
//! at two is not its frame at one grown.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Strip, StripTab};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, Screen, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use alo_window::place::{STRIP_HEIGHT, strip_viewport};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// How long a renderer is given before it is given up on.
const PATIENCE: Duration = Duration::from_secs(4);

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

/// The size the page is laid out at, below the strip.
const PAGE: Size = Size {
    width: 240.0,
    height: 60.0,
};

/// Boxes a tenth of a pixel off the grid, in three colours, and a title.
const HTML: &str = "<!doctype html><title>boxes</title>\
    <body style=\"margin:0\">\
    <div style=\"display:flex;margin-left:0.3px;margin-top:10.7px\">\
    <div style=\"width:20.3px;height:13.7px;background:#c0392b\"></div>\
    <div style=\"width:0.6px;height:9.1px;background:#102a43\"></div>\
    <div style=\"width:31.45px;height:20.2px;background:#e76f51\"></div>\
    </div>";

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
/// piece of news to the receiver.
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

/// What a renderer here paints after `work`, or why it did not.
fn painted_after(work: ToRenderer) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    match renderer.handle(work) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("not drawn: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("not painted: {other:?}")),
    }
}

/// The page as the engine paints it here at [`PAGE`] on a screen of `scale`.
fn page_drawn(scale: f32) -> Result<Frame, String> {
    painted_after(ToRenderer::Load(Box::new(
        Page::new(HTML, PAGE).at_scale(scale),
    )))
}

/// The strip of the page's one tab, selected, as the engine paints it here
/// above [`PAGE`] on a screen of `scale`.
fn strip_drawn(url: &Url, scale: f32) -> Result<Frame, String> {
    painted_after(ToRenderer::Strip(Box::new(Strip {
        tabs: vec![StripTab::new("boxes", url.to_string()).selected()],
        viewport: strip_viewport(PAGE),
        scale,
    })))
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

/// The page's frame, and the strip drawn after it once the page has loaded
/// — past the strips said while it was loading.
fn page_and_strip(news: &Receiver<News>) -> Result<(Frame, Frame), String> {
    let page = loop {
        match next(news)? {
            News::Painted(frame) => break frame,
            News::Strip(_) => {}
            other => return Err(format!("unexpected while loading: {other:?}")),
        }
    };
    match next(news)? {
        News::Strip(strip) => Ok((page, strip)),
        other => Err(format!("no strip after the paint: {other:?}")),
    }
}

/// What the conductor says after a resize: the page painted again, then
/// the strip drawn again for the new scale.
fn after_resize(news: &Receiver<News>) -> Result<(Frame, Frame), String> {
    let page = match next(news)? {
        News::Painted(frame) => frame,
        other => return Err(format!("no paint after the resize: {other:?}")),
    };
    match next(news)? {
        News::Strip(strip) => Ok((page, strip)),
        other => Err(format!("no strip after the resize: {other:?}")),
    }
}

fn send(orders: &alo_window::inbox::Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

#[test]
fn a_page_and_the_strip_are_painted_at_the_screens_scale_and_again_when_it_changes() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    send(&orders, Order::Resize(Screen::new(PAGE).at_scale(2.0)))
        .unwrap_or_else(|why| panic!("{why}"));
    let url = alo_url::parse("file:///frozen/boxes.html").unwrap_or_else(|why| panic!("{why:?}"));
    send(
        &orders,
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(HTML, Size::ZERO).at(url.clone()))),
        },
    )
    .unwrap_or_else(|why| panic!("{why}"));

    let at_two = page_drawn(2.0).unwrap_or_else(|why| panic!("{why}"));
    let at_one = page_drawn(1.0).unwrap_or_else(|why| panic!("{why}"));
    let strip_at_two = strip_drawn(&url, 2.0).unwrap_or_else(|why| panic!("{why}"));
    let strip_at_one = strip_drawn(&url, 1.0).unwrap_or_else(|why| panic!("{why}"));
    // The scale is visible in the picture, not only in its size: the frame
    // at two is not the frame at one with each pixel doubled.
    let grown = |x: u32, y: u32| at_one.at(x / 2, y / 2);
    let doubled = (0..at_two.height)
        .flat_map(|y| (0..at_two.width).map(move |x| (x, y)))
        .all(|(x, y)| at_two.at(x, y) == grown(x, y));
    assert!(!doubled, "the page at two is the page at one, enlarged");

    // On a screen of two: both frames in device pixels, as drawn at two.
    let (page, strip) = page_and_strip(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((page.width, page.height), (480, 120));
    assert_eq!((strip.width, strip.height), (480, 2 * STRIP_HEIGHT));
    assert!(page == at_two, "the page was not painted at two");
    assert!(strip == strip_at_two, "the strip was not painted at two");

    // Moved to a screen of one, the same size in CSS pixels: the page it
    // has is laid out and painted again at one, and so is the strip.
    send(&orders, Order::Resize(Screen::new(PAGE))).unwrap_or_else(|why| panic!("{why}"));
    let (page, strip) = after_resize(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((page.width, page.height), (240, 60));
    assert!(page == at_one, "the page was not painted at one");
    assert!(strip == strip_at_one, "the strip was not painted at one");

    // And back.
    send(&orders, Order::Resize(Screen::new(PAGE).at_scale(2.0)))
        .unwrap_or_else(|why| panic!("{why}"));
    let (page, strip) = after_resize(&news).unwrap_or_else(|why| panic!("{why}"));
    assert!(page == at_two, "the page was not painted at two again");
    assert!(
        strip == strip_at_two,
        "the strip was not painted at two again"
    );
}
