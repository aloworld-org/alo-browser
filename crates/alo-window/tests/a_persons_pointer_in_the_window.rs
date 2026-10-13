/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 542: a person's pointer moving, from the window to the page
//! (ADR 0024 § 5).
//!
//! Each move is a point in the window's device pixels, as the window server
//! gives one, put through [`aim`] and [`Cursor`] — what the event loop asks
//! — against the window's own placement, and the order they make is sent to
//! the real conductor over the real confined renderers.
//!
//! What a page hears is read from what its listeners fetch, from a server
//! on this machine: the conductor makes one request at a time, in the order
//! the page asked, so the server's log is the order the page heard them in.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::aim::{aim, on_page};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::cursor::Cursor;
use alo_window::inbox::Orders;
use alo_window::message::{News, Order};
use alo_window::place::{Contents, Placement, viewport};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long any answer the test expects may take before the test says so.
const AT_MOST: Duration = Duration::from_secs(60);

/// The window, in device pixels, at scale 2: an 800 × 600 page below the
/// strip, the size the frozen page's corpus files were laid out at.
const WINDOW: (u32, u32) = (1600, 1272);
const SCALE: u32 = 2;

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
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let network =
        Network::over(Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(5)));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(renderers),
        Fonts::AsStarted,
        network,
        move |said| tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    Ok((conductor, news))
}

fn send(orders: &Orders, order: Order) -> Result<(), String> {
    orders
        .send(order)
        .map_err(|_| "the conductor is gone".to_owned())
}

/// What the conductor says next, or why it said nothing.
fn next(news: &Receiver<News>) -> Result<News, String> {
    news.recv_timeout(AT_MOST)
        .map_err(|why| format!("the conductor said nothing in {AT_MOST:?}: {why}"))
}

/// The page's frame, once the selected tab is painted, passing over the
/// strip's news.
fn painted(news: &Receiver<News>) -> Result<Frame, String> {
    loop {
        if let News::Painted(frame) = next(news)? {
            return Ok(frame);
        }
    }
}

/// Open `html` at `address` in a window of [`WINDOW`], and wait for it to be
/// painted: the frame.
fn showing(
    orders: &Orders,
    news: &Receiver<News>,
    address: &str,
    html: &str,
) -> Result<Frame, String> {
    send(orders, Order::Resize(viewport(WINDOW, SCALE)))?;
    let url = alo_url::parse(address).map_err(|why| format!("not a URL: {why:?}"))?;
    send(
        orders,
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(html, Size::ZERO).at(url))),
        },
    )?;
    painted(news)
}

/// What the cursor moving to `at`, in the window's device pixels, sends,
/// aimed at the window as it shows `frame` — sent, if it is anything.
fn moved(
    orders: &Orders,
    cursor: &mut Cursor,
    frame: &Frame,
    at: (f64, f64),
) -> Result<(), String> {
    let placed = Placement::of(
        WINDOW,
        SCALE,
        Contents {
            frame: Some((frame.width, frame.height)),
            ..Contents::default()
        },
    );
    let over = aim(WINDOW, &placed, SCALE, at);
    match cursor.moved(over, on_page(&placed, SCALE, at)) {
        Some(pointer) => send(orders, Order::Pointer(pointer)),
        None => Ok(()),
    }
}

/// Serve every request on `bind` with an empty answer, logging its path.
fn serve(bind: &str, log: &Arc<Mutex<Vec<String>>>) -> Result<String, String> {
    let listener = TcpListener::bind(bind).map_err(|why| format!("no server at {bind}: {why}"))?;
    let at = listener
        .local_addr()
        .map_err(|why| format!("no address: {why}"))?;
    let log = Arc::clone(log);
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
            let line = String::from_utf8_lossy(&asked);
            let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
            if let Ok(mut log) = log.lock() {
                log.push(path);
            }
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nCache-Control: no-store\r\n\
                  Connection: close\r\n\r\n",
            );
        }
    });
    Ok(format!("http://{at}"))
}

/// The log once it holds `count` paths, or what it held when the test gave up.
fn logged(log: &Arc<Mutex<Vec<String>>>, count: usize) -> Vec<String> {
    let deadline = Instant::now().checked_add(AT_MOST);
    loop {
        let now = log.lock().map(|log| log.clone()).unwrap_or_default();
        if now.len() >= count || deadline.is_none_or(|deadline| Instant::now() > deadline) {
            return now;
        }
        std::thread::yield_now();
    }
}

#[test]
fn moving_over_the_page_and_onto_the_strip_is_heard_as_chrome_fires_it() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let origin = serve("127.0.0.1:0", &log).unwrap_or_else(|why| panic!("{why}"));
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        &format!("{origin}/"),
        "<html id=html><body id=body style=margin:0><div id=b style=position:absolute;left:50px;top:40px;\
         width:100px;height:60px></div><script>\
         function who(t) { return t === null ? 'null' : t.getAttribute('id'); } \
         const kinds = ['mouseover', 'mouseout', 'mousemove']; \
         for (let i = 0; i < kinds.length; i++) { \
           document.addEventListener(kinds[i], e => fetch('/' + e.type + '-' + \
             who(e.target) + '-' + who(e.relatedTarget) + '-' + e.clientX + '-' + \
             e.clientY)); \
         }</script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((frame.width, frame.height), (800, 600));
    let mut cursor = Cursor::default();

    // Over the strip first: the page is told nothing.
    moved(&orders, &mut cursor, &frame, (141.0, 30.0)).unwrap_or_else(|why| panic!("{why}"));
    // (70.5, 50.25) on the page — at scale 2, 141 across and 100.5 down,
    // below the strip's 72 — then (80, 50.25), both over b; then (80, 150)
    // over nothing but the root; then back over the strip, at (80, -21).
    for at in [
        (141.0, 172.5),
        (160.0, 172.5),
        (160.0, 372.0),
        (160.0, 30.0),
    ] {
        moved(&orders, &mut cursor, &frame, at).unwrap_or_else(|why| panic!("{why}"));
        painted(&news).unwrap_or_else(|why| panic!("{why}"));
    }
    assert_eq!(
        logged(&log, 7),
        [
            "/mouseover-b-null-70-50",
            "/mousemove-b-null-70-50",
            "/mousemove-b-null-80-50",
            "/mouseout-b-html-80-150",
            "/mouseover-html-b-80-150",
            "/mousemove-html-null-80-150",
            "/mouseout-html-null-80--21",
        ]
    );
    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert!(conductor.finish());
}
