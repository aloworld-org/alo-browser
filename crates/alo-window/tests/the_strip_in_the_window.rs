/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 528: the tab strip in the window (ADR 0024 §§ 2, 4, 6).
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
//!
//! The strip's renderer stopping is `a_strip_that_never_answers.rs`, alone in
//! its file because it counts this process's renderers.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Screen;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{Condition, LONGEST_TITLE, Strip, StripTab};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use alo_window::place::strip_viewport;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

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
        scale: 1.0,
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

/// Every strip frame said until the tab's page is painted, and the one
/// after it — the order a load says them in.
fn strips_until_painted(news: &Receiver<News>) -> Result<Vec<Frame>, String> {
    let mut strips = Vec::new();
    loop {
        match next(news)? {
            News::Strip(frame) => strips.push(frame),
            News::Painted(_) => break,
            // A tab opened over another: the one left is not shown under it
            // (queue item 529).
            News::Chosen(None) => {}
            other => return Err(format!("unexpected while loading: {other:?}")),
        }
    }
    match next(news)? {
        News::Strip(frame) => strips.push(frame),
        other => return Err(format!("no strip after the paint: {other:?}")),
    }
    Ok(strips)
}

fn send(orders: &alo_window::inbox::Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

#[test]
fn a_pages_title_reaches_the_strip_as_text_hostile_or_not() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    send(&orders, Order::Resize(Screen::new(PAGE))).unwrap_or_else(|why| panic!("{why}"));

    // Markup, an escaped end tag, a right-to-left override and white space
    // that collapses: the title is the text, as HTML reads it.
    let (first, opening) = page_at(
        "hostile.html",
        "<title>\n  <b>Invoices</b> &lt;/title&gt;\t\u{202e}gnp.exe  </title><p>a page</p>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    send(&orders, opening).unwrap_or_else(|why| panic!("{why}"));
    let title = "<b>Invoices</b> </title> \u{202e}gnp.exe";
    let strips = strips_until_painted(&news).unwrap_or_else(|why| panic!("{why}"));
    // As the load starts: the tab, selected, loading, called by its address.
    let loading = drawn(vec![
        StripTab::new("", first.to_string())
            .selected()
            .in_condition(Condition::Loading),
    ])
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        strips.first(),
        Some(&loading),
        "the strip did not say loading"
    );
    // And once it has loaded: called by its title.
    let shown = drawn(vec![StripTab::new(title, first.to_string()).selected()])
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        strips.last(),
        Some(&shown),
        "the title did not reach the strip"
    );
    assert_ne!(loading, shown);

    // A second tab with a title of a hundred thousand characters: drawn to
    // the strip's bound, and the new tab selected.
    let long = "x".repeat(100_000);
    let (second, opening) = page_at("long.html", &format!("<title>{long}</title>"))
        .unwrap_or_else(|why| panic!("{why}"));
    send(&orders, opening).unwrap_or_else(|why| panic!("{why}"));
    let strips = strips_until_painted(&news).unwrap_or_else(|why| panic!("{why}"));
    let both = drawn(vec![
        StripTab::new(title, first.to_string()),
        StripTab::new("x".repeat(LONGEST_TITLE), second.to_string()).selected(),
    ])
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        strips.last(),
        Some(&both),
        "the long title is not the strip's"
    );
    // The whole title, cut by the strip rather than by the wire, draws the
    // same: what the strip shows of it is the same whichever side cut it.
    let whole = drawn(vec![
        StripTab::new(title, first.to_string()),
        StripTab::new(long, second.to_string()).selected(),
    ])
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(whole, both);

    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(next(&news), Ok(News::Closed));
    assert!(conductor.finish(), "the conductor did not finish");
}
