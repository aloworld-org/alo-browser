/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 373 (ADR 0039 §§ 2 and 4): closing the window leaves its page.
//!
//! Closing the window closes every tab, and each tab's page is left as it
//! goes: its `pagehide` listener runs, in the real renderer process, before
//! that process is reaped. What the listener asks to fetch without asking to
//! outlive the page is refused by name, and written into the session's
//! record as the page's own document's, which the conductor hands back when
//! it finishes.
//!
//! Queue item 369 (ADR 0040 § 3): a beacon it sends as it goes is decided,
//! and would be made after the page has gone — but the browser is closing,
//! and a closed browser makes nothing more. The beacon is written into the
//! record as not made, because the browser closed.

use alo_layout::Size;
use alo_net::activity::Happened;
use alo_net::cause::Cause;
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

#[test]
fn closing_the_window_leaves_the_page_and_records_what_it_asked_for() {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    assert!(renderer.is_file(), "no renderer at {}", renderer.display());
    let Ok(trust) = Trust::of(&[]) else {
        panic!("an empty trust store");
    };
    let (tell, news) = channel();
    let Ok(conductor) = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[])),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(trust)),
        move |said| tell.send(said).is_ok(),
    ) else {
        panic!("no conductor");
    };
    let Ok(url) = alo_url::parse("https://shop.example/") else {
        panic!("a URL");
    };
    let page = Page::new(
        "<div></div><script>window.addEventListener('pagehide', function (e) { \
           fetch('/bye?' + e.type + '-' + document.visibilityState); \
           navigator.sendBeacon('/last', 't=0'); });</script>",
        Size::ZERO,
    )
    .at(url.clone())
    .with_sheet("div { height: 20px; background: #2f6f4f }");
    for order in [
        Order::Resize(Size::new(120.0, 60.0)),
        Order::Open {
            url,
            page: Some(Box::new(page)),
        },
    ] {
        assert!(
            conductor.orders().send(order).is_ok(),
            "the conductor is gone"
        );
    }
    assert!(
        matches!(news.recv_timeout(AT_MOST), Ok(News::Painted(_))),
        "the page was not shown"
    );

    assert!(conductor.orders().send(Order::CloseEverything).is_ok());
    assert_eq!(news.recv_timeout(AT_MOST), Ok(News::Closed));
    let Some(network) = conductor.hand_back() else {
        panic!("the conductor did not finish");
    };

    let left: Vec<_> = network
        .pool
        .activity()
        .entries()
        .filter(|entry| entry.url().serialised.contains("/bye"))
        .collect();
    let [entry] = left.as_slice() else {
        panic!("one ask recorded: {left:?}");
    };
    assert_eq!(
        entry.url().serialised,
        "https://shop.example/bye?pagehide-visible"
    );
    assert!(
        matches!(entry.happened(), Happened::Refused { rule } if rule.contains("being left")),
        "{entry:?}"
    );
    assert!(
        matches!(entry.cause(), Cause::Document { .. }),
        "the page's own: {entry:?}"
    );

    // The beacon: decided, as the page's own, and never made.
    let beacons: Vec<_> = network
        .pool
        .activity()
        .entries()
        .filter(|entry| entry.url().serialised.contains("/last"))
        .collect();
    let [beacon] = beacons.as_slice() else {
        panic!("one beacon recorded: {beacons:?}");
    };
    assert_eq!(beacon.purpose(), &alo_net::Purpose::Beacon);
    assert!(
        matches!(beacon.happened(), Happened::Refused { rule } if rule.contains("the browser closed")),
        "{beacon:?}"
    );
    assert_eq!(beacon.cause(), entry.cause(), "the same document's");
}
