/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 778, ADR 0076 B4 as amended: a card's page cannot move its
//! window into view.
//!
//! A page may move and resize its own `--app` window (`moveTo`,
//! `resizeTo`). Measured through alo's CEF host before this item, a parked
//! card's page calling `moveTo(200, 200)` put the transparent window wholly
//! on a display, where the host took it for alo's *Expand* and drew it
//! opaque: the page seen. The host now refuses a page's own move or resize
//! while its card is unseen (`OnContentsBoundsChange`, said handled), and
//! carries it out once the card has been expanded, as Chrome does for an
//! `--app` window.
//!
//! Through alo's CEF host, a card whose page asks every 200 ms to be moved
//! to 200, 200 and resized to 400 × 300, and says how often by its own
//! address's fragment, is streamed until it has asked at least ten times;
//! then, each asserted:
//!
//! - its window is **where alo parked it**, at Chromium's own size,
//!   floating over every ordinary window, by the window server's word —
//!   not at 200, 200 nor 400 × 300;
//! - it is **drawn on no display** (alpha 0, or meeting none), against
//!   every display's bounds, and its application is still an accessory
//!   one (`UIElement`, queue item 781);
//! - it **still streams** frames of the page box;
//! - and once *Expand* has shown it, the page's next ask **is** carried
//!   out: its window opaque at 200, 200.
//!
//! Through the machine's Chrome a page's move is Chrome's to carry out, and
//! nothing of alo's stands in front of it (queue item 787).
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/moving.html`.

#![cfg(target_os = "macos")]

mod carried;
mod listed;

use alo_window::agent_sites::AgentSites;
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_screencast::png_size;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::frame_told::ToFrame;
use alo_window::message::{CardOrder, Order};
use alo_window::scale::Scale;
use alo_window::window_server;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// The card's page box, as `alo-renderer` lays it out.
const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// Where the page asks its window to be, in the screen's points: wholly on
/// the main display, as the measured `moveTo(200, 200)` put it.
const ASKED_AT: (i64, i64) = (200, 200);

/// How many times the page must have asked before its window is looked at.
const ASKED_TIMES: u32 = 10;

/// The page: a teal ground, a box turning so that it always has a frame
/// to stream, and a script asking every 200 ms for its window
/// at [`ASKED_AT`] and 400 × 300, counting each ask in its address's
/// fragment (`#asked-N`), which the card tells as the page moves within
/// its document.
const MOVING_PAGE: &str = "<!doctype html><title>A page that moves its window</title>\
    <style>body{margin:0;background:#2a9d8f}\
      #turning{width:60px;height:60px;margin:20px;background:#e76f51;\
        animation:turn 1s linear infinite}\
      @keyframes turn{to{transform:rotate(360deg)}}</style>\
    <h1>Moving</h1><div id=turning></div>\
    <script>let asked = 0;\
      setInterval(() => { moveTo(200, 200); resizeTo(400, 300); asked += 1;\
        history.replaceState(null, '', '#asked-' + asked); }, 200);</script>";

/// A server on this machine answering `/moving.html` with [`MOVING_PAGE`]:
/// its port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let Some(path) = asked.split(' ').nth(1) else {
                    return;
                };
                let (status, body) = if path == "/moving.html" {
                    ("200 OK", MOVING_PAGE)
                } else {
                    ("404 Not Found", "")
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// The sizes of the frames told, each as its PNG states it.
type Sizes = Vec<Option<(u32, u32)>>;

/// How many times the page has asked, by an address it moved to.
fn asked(address: &str) -> Option<u32> {
    address.rsplit_once("#asked-")?.1.parse().ok()
}

/// The frames' sizes told until the page says it has asked at least
/// [`ASKED_TIMES`] times, waited for up to thirty seconds: the frames, and
/// how many times it had asked.
fn streamed_until_asked(told: &Receiver<ToFrame>) -> Result<(Sizes, u32), String> {
    let until = Instant::now() + Duration::from_secs(30);
    let mut frames = Vec::new();
    let mut most = 0;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Cast(frame)) => frames.push(png_size(&frame.png().unwrap_or_default())),
            Ok(ToFrame::Moved(url)) => {
                most = most.max(asked(&url.to_string()).unwrap_or(0));
                if most >= ASKED_TIMES {
                    return Ok((frames, most));
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Err(format!(
        "the page asked to move its window only {most} times in thirty seconds"
    ))
}

/// The window server's layer of a window floating over every ordinary one,
/// where the host keeps a card's window while it is unseen (queue item
/// 781); an ordinary window's is 0.
const FLOATING: i64 = 3;

/// The windows of `process` the window server lists on the screen at
/// `layer`, with how opaque each is drawn and its bounds.
fn windows(process: u32, layer: i64) -> Result<Vec<(u8, Bounds)>, String> {
    Ok(window_server::on_the_screen(process)
        .map_err(|why| why.to_string())?
        .into_iter()
        .filter(|listed| listed.layer == layer)
        .filter_map(|listed| listed.bounds.map(|bounds| (listed.alpha, bounds)))
        .collect())
}

#[test]
fn a_cards_page_never_moves_its_parked_window_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("778")? else {
        return Ok(());
    };
    let port = serve()?;
    let address = format!("http://127.0.0.1:{port}/moving.html");
    let url = alo_url::parse(&address).map_err(|why| format!("{why:?}"))?;
    let (tell, told) = channel();
    let serving = ChromeServing::start_as_a_card(
        host.chromium(),
        Opening::Network(url),
        PAGE,
        Scale::ONE,
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no card in Chromium: {why}"))?;
    let process = serving.process();
    let parked = serving
        .placed()
        .map(|placed| placed.window.bounds)
        .ok_or("the card's window was not read")?;
    let orders = serving.orders();
    orders
        .send(Order::Card(CardOrder::Stream {
            width: PAGE.0,
            height: PAGE.1,
            page: PAGE,
            scale: Scale::ONE,
        }))
        .map_err(|_| "the card's thread has gone")?;
    let (frames, times) = streamed_until_asked(&told)?;
    let now = windows(process, FLOATING)?;
    eprintln!(
        "alo's CEF host: parked at {parked:?}; the page asked {times} times to be at \
         {ASKED_AT:?}, 400 × 300; the window server lists {now:?}; {} frames meanwhile",
        frames.len()
    );

    // Where alo parked it, at its own size: the page's asks refused.
    assert!(
        !now.is_empty(),
        "the card's window is not on the screen floating, where it would not stream"
    );
    assert!(
        now.iter()
            .all(|(_, at)| (at.left, at.top, at.width, at.height)
                == (parked.left, parked.top, parked.width, parked.height)),
        "the page moved its parked window: {now:?}, parked at {parked:?}"
    );
    // Drawn on no display, and still out of the Dock.
    let displays: Vec<Bounds> = window_server::displays()
        .map_err(|why| why.to_string())?
        .iter()
        .map(|display| display.bounds)
        .collect();
    let shown = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
    assert!(
        shown
            .iter()
            .all(|listed| !window_server::seen(listed, &displays)),
        "the card's window is seen on a display: {shown:?}, the displays {displays:?}"
    );
    assert_eq!(listed::application_type(process)?, "UIElement");
    // Still streaming the page box.
    assert!(
        frames.len() >= 5 && frames.iter().all(|size| *size == Some(PAGE)),
        "the card did not stream its page box while its page asked: {:?}",
        frames.iter().take(3).collect::<Vec<_>>()
    );

    // Expanded, the window is a person's, and the page's next ask is
    // carried out, as Chrome carries out an `--app` window's.
    orders
        .send(Order::Card(CardOrder::Expand {
            at: (120, 80),
            size: (900, 600),
        }))
        .map_err(|_| "the card's thread has gone")?;
    let until = Instant::now() + Duration::from_secs(20);
    let mut moved = None;
    while moved.is_none() && Instant::now() < until {
        moved = windows(process, 0)?
            .into_iter()
            .find(|(alpha, at)| *alpha == u8::MAX && (at.left, at.top) == ASKED_AT);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        moved.is_some(),
        "the expanded window was not moved by its page: {:?}",
        window_server::on_the_screen(process)
    );
    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or("the card's thread did not finish")?;
    Ok(())
}
