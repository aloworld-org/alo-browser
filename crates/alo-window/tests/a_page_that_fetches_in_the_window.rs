/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's fetches, made by the conductor (queue item 338, ADR 0032 § 3).
//!
//! The conductor holds the tabs and the session's network, so it is what
//! makes a page's fetches: between orders, one at a time, painting the page
//! again after each answer and saying why one failed. A page that never
//! stops fetching costs its own tab's answers and never the window's ability
//! to close.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, mpsc};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// A server on this machine: its origin, and how many requests it answered.
/// `/text` is `fetched`; `/hop` redirects to it.
fn serve() -> Option<(String, Arc<AtomicUsize>)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().ok()?.port());
    let answered = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&answered);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            if socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .is_err()
            {
                continue;
            }
            let mut asked = Vec::new();
            let mut block = [0u8; 1024];
            while let Ok(got) = socket.read(&mut block) {
                asked.extend_from_slice(block.get(..got).unwrap_or_default());
                if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                    break;
                }
            }
            let reply: &[u8] = if asked.starts_with(b"GET /hop ") {
                b"HTTP/1.1 302 Found\r\nLocation: /text\r\nContent-Length: 0\r\n\
                  Cache-Control: no-store\r\nConnection: close\r\n\r\n"
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 7\r\n\
                  Cache-Control: no-store\r\nConnection: close\r\n\r\nfetched"
            };
            let _ = socket.write_all(reply);
            counting.fetch_add(1, Ordering::SeqCst);
        }
    });
    Some((origin, answered))
}

/// A conductor over the real renderer binary and a network that trusts no
/// certificate — the server here speaks plain HTTP — with a page at `origin`
/// running `script` open in its one tab, at a size.
///
/// The renderers are started with no fonts, so what a reaction changes has
/// to be seen without text: the page is a green box, `out`, and a red one,
/// `bad`, that starts hidden.
fn conducting(origin: &str, script: &str) -> Result<(Conductor, Receiver<News>), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let network =
        Network::over(Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(5)));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[])),
        Fonts::AsStarted,
        network,
        move |said| tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    let url = alo_url::parse(&format!("{origin}/page")).map_err(|why| format!("{why:?}"))?;
    let page = Page::new(
        format!(
            "<div id=out></div><div id=bad hidden></div><script>\
             var out = document.getElementById('out'); \
             var bad = document.getElementById('bad'); {script}</script>"
        ),
        Size::ZERO,
    )
    .at(url.clone())
    .with_sheet("div { height: 20px } #out { background: #2f6f4f } #bad { background: #b03a2e }");
    for order in [
        Order::Resize(Size::new(200.0, 60.0)),
        Order::Open {
            url,
            page: Some(Box::new(page)),
        },
    ] {
        conductor
            .orders()
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    Ok((conductor, news))
}

/// The next frame painted, or what arrived instead.
fn painted(news: &mpsc::Receiver<News>) -> Result<Frame, String> {
    match news.recv_timeout(AT_MOST) {
        Ok(News::Painted(frame)) => Ok(frame),
        other => Err(format!("not a frame: {other:?}")),
    }
}

#[test]
fn a_fetch_is_made_the_page_drawn_again_and_a_failure_said() {
    let (origin, answered) = serve().expect("a server");
    let (conductor, news) = conducting(
        &origin,
        "fetch('/text').then(function (r) { return r.text(); })
           .then(function (t) { out.hidden = t == 'fetched'; });
         fetch('/hop', { redirect: 'error' })
           .catch(function (e) { bad.hidden = e.name != 'TypeError'; });",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    let waiting = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    let fetched = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_ne!(
        waiting, fetched,
        "the answer's reaction changed the page, and it was drawn again"
    );
    let failed = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_ne!(fetched, failed, "the failure's reaction was drawn too");
    let said = news.recv_timeout(AT_MOST);
    let Ok(News::Said(sentence)) = &said else {
        panic!("the failure was not said: {said:?}");
    };
    assert!(
        sentence.contains("failed") && sentence.contains("asked not to be redirected"),
        "{sentence}"
    );
    assert_eq!(
        answered.load(Ordering::SeqCst),
        2,
        "both requests were made"
    );

    conductor
        .orders()
        .send(Order::CloseEverything)
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    assert_eq!(news.recv_timeout(AT_MOST), Ok(News::Closed));
    assert!(conductor.finish());
}

#[test]
fn a_page_that_never_stops_fetching_does_not_keep_the_window_open() {
    let (origin, answered) = serve().expect("a server");
    let (conductor, news) = conducting(
        &origin,
        "function again() { fetch('/text').then(again, again); } again();",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    painted(&news).unwrap_or_else(|why| panic!("{why}"));
    // Some answers have been made, and the page asks again after each.
    while answered.load(Ordering::SeqCst) < 3 {
        let heard = news.recv_timeout(AT_MOST);
        assert!(matches!(heard, Ok(News::Painted(_))), "{heard:?}");
    }

    conductor
        .orders()
        .send(Order::CloseEverything)
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    let mut closed = false;
    for _ in 0..1000 {
        match news.recv_timeout(AT_MOST) {
            Ok(News::Painted(_)) => {}
            Ok(News::Closed) => {
                closed = true;
                break;
            }
            other => panic!("not closing: {other:?}"),
        }
    }
    assert!(closed, "the window's close was never heard between fetches");
    assert!(conductor.finish());
}
