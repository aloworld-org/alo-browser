/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's linked style sheet, made by the conductor (queue item 348, ADR
//! 0035 § 4).
//!
//! The conductor makes a page's sheets as it makes its fetches — between
//! orders, one at a time — and paints the page again after each answer, so a
//! page that links its style is shown with it.
//!
//! What it does **not** do yet is wait: the first frame is painted as soon
//! as the page loads, before its sheet arrives. Holding that frame back until
//! the load's sheets are answered, within a bound of its own, is ADR 0035
//! § 5 and queue item 351, which changes the first assertion below.

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

/// The green the sheet paints `#out`.
const GREEN: (u8, u8, u8, u8) = (47, 111, 79, 255);
const WHITE: (u8, u8, u8, u8) = (255, 255, 255, 255);

/// A server on this machine: its origin, and how many requests it answered.
/// `/site.css` is a style sheet; anything else is not found.
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
            let sheet = b"#out { background: rgb(47, 111, 79) }";
            let reply = if asked.starts_with(b"GET /site.css ") {
                let mut reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n",
                    sheet.len()
                )
                .into_bytes();
                reply.extend_from_slice(sheet);
                reply
            } else {
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
            };
            let _ = socket.write_all(&reply);
            counting.fetch_add(1, Ordering::SeqCst);
        }
    });
    Some((origin, answered))
}

/// A conductor over the real renderer binary with a page at `origin` open,
/// linking its sheet, at a size. The renderers have no fonts, so the page is
/// one box, `out`, which only the sheet colours.
fn conducting(origin: &str) -> Result<(Conductor, Receiver<News>), String> {
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
        "<link rel=stylesheet href=/site.css><link rel=stylesheet href=site.css>\
         <div id=out></div><script>var ran = true;</script>",
        Size::ZERO,
    )
    .at(url.clone())
    .with_sheet("body { margin: 0 } div { height: 20px }");
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
fn a_linked_sheet_is_made_once_and_the_page_painted_with_it() {
    let (origin, answered) = serve().expect("a server");
    let (conductor, news) = conducting(&origin).unwrap_or_else(|why| panic!("{why}"));

    let loaded = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    // Item 351 holds this frame back until the sheet has answered.
    assert_eq!(
        loaded.at(100, 10),
        Some(WHITE),
        "painted before its sheet arrived"
    );
    let styled = painted(&news).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(styled.at(100, 10), Some(GREEN), "painted again with it");
    assert_eq!(
        styled.at(100, 30),
        Some(WHITE),
        "and only the box it styles"
    );
    assert_eq!(answered.load(Ordering::SeqCst), 1, "two links, one request");

    conductor
        .orders()
        .send(Order::CloseEverything)
        .unwrap_or_else(|_| panic!("the conductor is gone"));
    assert_eq!(news.recv_timeout(AT_MOST), Ok(News::Closed));
    assert!(conductor.finish());
}
