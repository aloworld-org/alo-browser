/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Closing the window closes every tab, and every renderer under them stops.
//!
//! ADR 0024 § 6: *"closing the window closes every tab, which item 64's
//! lifecycle already turns into renderers reaped."* Real renderer processes,
//! counted from outside with `pgrep`, so a renderer left behind is seen rather
//! than assumed away.
//!
//! One test in this file on purpose: renderers are counted as this test
//! process's children, and a second test running beside it would start some
//! of its own.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Page;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

fn renderers_running() -> Result<usize, String> {
    let found = std::process::Command::new("pgrep")
        .args(["-P", &std::process::id().to_string(), "-x", "alo-render"])
        .output()
        .map_err(|why| format!("pgrep could not be run: {why}"))?;
    Ok(String::from_utf8_lossy(&found.stdout).lines().count())
}

fn opening(site: &str) -> Result<Order, String> {
    let url = alo_url::parse(site).map_err(|why| format!("not a URL: {why:?}"))?;
    Ok(Order::Open {
        url: url.clone(),
        page: Some(Box::new(
            Page::new("<p>a page</p>", Size::ZERO)
                .at(url)
                .with_sheet("p { height: 20px; background: #2f6f4f }"),
        )),
    })
}

/// A session's network that trusts no certificate. Nothing here fetches; a
/// conductor is started with one because every conductor makes its pages'
/// fetches.
fn offline() -> Result<Network, String> {
    Ok(Network::over(Pool::with_trust(Trust::of(&[])?)))
}

/// Start a conductor with two tabs on two sites, each painted.
fn two_tabs_painted() -> Result<(Conductor, std::sync::mpsc::Receiver<News>), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[])),
        Fonts::AsStarted,
        offline()?,
        move |said| tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    let orders = conductor.orders();
    for order in [
        Order::Resize(Size::new(120.0, 60.0)),
        opening("https://one.example/")?,
        opening("https://two.example/")?,
    ] {
        orders
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    for _ in 0..2 {
        match news.recv_timeout(AT_MOST) {
            Ok(News::Painted(_)) => {}
            other => return Err(format!("a tab was not painted: {other:?}")),
        }
    }
    Ok((conductor, news))
}

#[test]
fn closing_the_window_closes_every_tab_and_stops_every_renderer() {
    let (conductor, news) = two_tabs_painted().unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(renderers_running(), Ok(2), "two sites, two renderers");

    conductor
        .orders()
        .send(Order::CloseEverything)
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    assert_eq!(news.recv_timeout(AT_MOST), Ok(News::Closed));
    assert!(conductor.finish(), "the conductor did not finish");
    assert_eq!(renderers_running(), Ok(0), "a renderer outlived its window");

    // And a window that went without saying so — every sender of orders
    // gone — is closed the same way.
    let (conductor, _news) = two_tabs_painted().unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(renderers_running(), Ok(2));
    assert!(conductor.finish(), "the conductor did not finish");
    assert_eq!(renderers_running(), Ok(0), "a renderer outlived its window");
}
