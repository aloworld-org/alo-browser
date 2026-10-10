/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 364 (ADR 0039 § 1): the window covered and seen again.
//!
//! The window says so as an order, and the conductor tells the selected
//! tab's page, whose `visibilitychange` listener may change it, and paints
//! it again. A page loaded while the window is covered starts hidden.
//!
//! The renderers are started with no fonts, so the page is a green box,
//! `out`, whose `hidden` its listener turns over each time it hears the
//! event: a frame with the box is a page that has heard an even number of
//! changes, and one without it an odd number.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page, Visibility};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// A conductor over the real renderer binary, sent `before` and then a page
/// running `script` opened in its one tab, at a size.
fn conducting(before: &[Order], script: &str) -> Result<(Conductor, Receiver<News>), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let network = Network::over(Pool::with_trust(Trust::of(&[])?));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[])),
        Fonts::AsStarted,
        network,
        move |said| tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    let url = alo_url::parse("https://shop.example/").map_err(|why| format!("{why:?}"))?;
    let page = Page::new(
        format!(
            "<div id=out></div><script>var out = document.getElementById('out'); {script}</script>"
        ),
        Size::ZERO,
    )
    .at(url.clone())
    .with_sheet("div { height: 20px } #out { background: #2f6f4f }");
    let mut orders = before.to_vec();
    orders.push(Order::Resize(Size::new(200.0, 60.0)));
    orders.push(Order::Open {
        url,
        page: Some(Box::new(page)),
    });
    for order in orders {
        conductor
            .orders()
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    Ok((conductor, news))
}

/// The next frame painted, or what arrived instead.
fn painted(news: &Receiver<News>) -> Result<Frame, String> {
    match news.recv_timeout(AT_MOST) {
        Ok(News::Painted(frame)) => Ok(frame),
        other => Err(format!("not a frame: {other:?}")),
    }
}

/// Send `order`, and answer the frame painted after it.
fn after(conductor: &Conductor, news: &Receiver<News>, order: Order) -> Result<Frame, String> {
    conductor
        .orders()
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())?;
    painted(news)
}

/// Close every tab, and answer whether the conductor said so and finished.
fn close(conductor: Conductor, news: &Receiver<News>) -> Result<(), String> {
    conductor
        .orders()
        .send(Order::CloseEverything)
        .map_err(|_| "the conductor is gone".to_owned())?;
    match news.recv_timeout(AT_MOST) {
        Ok(News::Closed) if conductor.finish() => Ok(()),
        other => Err(format!("not closed: {other:?}")),
    }
}

const TURNS_OVER: &str = "document.addEventListener('visibilitychange', \
                          function () { out.hidden = !out.hidden; });";

#[test]
fn covering_the_window_and_showing_it_again_tells_the_page_each_time() {
    let (conductor, news) = conducting(&[], TURNS_OVER).unwrap_or_else(|why| panic!("{why}"));
    let shown = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    let told =
        |to| after(&conductor, &news, Order::Visibility(to)).unwrap_or_else(|why| panic!("{why}"));

    let covered = told(Visibility::Hidden);
    assert_ne!(shown, covered, "the page heard it was hidden");
    let still = told(Visibility::Hidden);
    assert_eq!(
        covered, still,
        "covered again is no change, and nothing is fired"
    );
    let again = told(Visibility::Visible);
    assert_eq!(shown, again, "the page heard it was seen again");

    close(conductor, &news).unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn a_page_loaded_in_a_covered_window_starts_hidden() {
    let (conductor, news) =
        conducting(&[], "out.hidden = document.hidden;").unwrap_or_else(|why| panic!("{why}"));
    let seen = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    close(conductor, &news).unwrap_or_else(|why| panic!("{why}"));

    let (conductor, news) = conducting(
        &[Order::Visibility(Visibility::Hidden)],
        "out.hidden = document.hidden;",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let unseen = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_ne!(seen, unseen, "the page read that it was hidden");
    let shown = after(&conductor, &news, Order::Visibility(Visibility::Visible))
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(unseen, shown, "it has no listener, so nothing changed it");
    close(conductor, &news).unwrap_or_else(|why| panic!("{why}"));
}
