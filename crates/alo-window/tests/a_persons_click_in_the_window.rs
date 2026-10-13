/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 535: a person's main button, from the window to the page
//! (ADR 0024 § 5) — and, for queue item 543, a form it sends.
//!
//! Each press is a point in the window's device pixels, as the window
//! server gives one, put through [`aim`] — the function the event loop asks
//! — against the window's own placement, and the order it makes is sent to
//! the real conductor over the real confined renderers.
//!
//! What a page hears is read from what its listeners fetch, from a server
//! on this machine: the conductor makes one request at a time, in the order
//! the page asked, so the server's log is the order the page heard them in.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::pointer::{Pointer, Went};
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_url::Url;
use alo_window::aim::{Aimed, aim, on_page};
use alo_window::beside::renderer_beside;
use alo_window::conductor::pointing::NOT_GONE;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::inbox::Orders;
use alo_window::message::{News, Order};
use alo_window::place::{Contents, Placement, screen};
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
    send(orders, Order::Resize(screen(WINDOW, SCALE)))?;
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

/// A press and release of the main button at `at`, in the window's device
/// pixels, as the event loop makes them of what the window server says:
/// aimed at the window as it shows `frame`, and sent only over the page.
fn click(orders: &Orders, frame: &Frame, at: (f64, f64)) -> Result<(), String> {
    let placed = Placement::of(
        WINDOW,
        SCALE,
        Contents {
            frame: Some((frame.width, frame.height)),
            ..Contents::default()
        },
    );
    let Aimed::Page(down) = aim(WINDOW, &placed, SCALE, at) else {
        return Err(format!("{at:?} is not over the page"));
    };
    send(
        orders,
        Order::Pointer(Pointer {
            went: Went::Down,
            at: down,
        }),
    )?;
    send(
        orders,
        Order::Pointer(Pointer {
            went: Went::Up,
            at: on_page(&placed, SCALE, at),
        }),
    )
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
fn a_click_in_the_window_is_heard_by_the_page_as_chrome_fires_it() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let origin = serve("127.0.0.1:0", &log).unwrap_or_else(|why| panic!("{why}"));
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        &format!("{origin}/"),
        "<body style=margin:0><div id=b style=position:absolute;left:50px;top:40px;\
         width:100px;height:60px></div><script>\
         const kinds = ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']; \
         for (let i = 0; i < kinds.length; i++) { \
           document.addEventListener(kinds[i], e => fetch('/' + e.type + '-' + \
             e.target.getAttribute('id') + '-' + e.clientX + '-' + e.clientY)); \
         }</script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    // An 800 × 600 page painted at the window's scale of two (item 562).
    assert_eq!((frame.width, frame.height), (1600, 1200));

    // (70.5, 50.25) on the page: at scale 2, 141 across and 100.5 down,
    // below the strip's 72.
    click(&orders, &frame, (141.0, 172.5)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        logged(&log, 5),
        [
            "/pointerdown-b-70.5-50.25",
            "/mousedown-b-70-50",
            "/pointerup-b-70.5-50.25",
            "/mouseup-b-70-50",
            "/click-b-70-50",
        ]
    );
    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert!(conductor.finish());
}

#[test]
fn a_click_on_the_frozen_link_is_decided_as_the_persons_and_said() {
    let page = include_str!("../../alo-corpus/cases/web-example-com/page.html");
    let origin = include_str!("../../alo-corpus/cases/web-example-com/origin.txt");
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        origin.lines().next().unwrap_or_default(),
        page,
    )
    .unwrap_or_else(|why| panic!("{why}"));

    // "Learn more" is at (160, 188.07813), 91.25 × 19 (its `agent.txt`):
    // (200, 197) on the page is (400, 466) in the window.
    click(&orders, &frame, (400.0, 466.0)).unwrap_or_else(|why| panic!("{why}"));
    let said = loop {
        match next(&news).unwrap_or_else(|why| panic!("{why}")) {
            News::Said(said) => break said,
            News::Painted(_) | News::Strip(_) | News::StripSaid(_) | News::Chosen(_) => {}
            News::Closed => panic!("the window closed"),
        }
    };
    let iana =
        alo_url::parse("https://iana.org/domains/example").unwrap_or_else(|_| Url::about_blank());
    assert_eq!(said, format!("this link goes to {iana}, {NOT_GONE}"));
    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert!(conductor.finish());
}

#[test]
fn a_click_on_a_forms_button_is_said_as_a_form_sent_not_a_link() {
    let (conductor, news) = conducting().unwrap_or_else(|why| panic!("{why}"));
    let orders = conductor.orders();
    let frame = showing(
        &orders,
        &news,
        "https://example.com/",
        "<body style=margin:0><form method=post action=/send>\
         <input type=hidden name=a value=1>\
         <button style=position:absolute;left:10px;top:150px;width:100px;height:30px;margin:0>\
         Send</button></form>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    // (20, 160) on the page, inside the button's 100 × 30 at (10, 150): at
    // scale 2, 40 across and 320 down, below the strip's 72.
    click(&orders, &frame, (40.0, 392.0)).unwrap_or_else(|why| panic!("{why}"));
    let said = loop {
        match next(&news).unwrap_or_else(|why| panic!("{why}")) {
            News::Said(said) => break said,
            News::Painted(_) | News::Strip(_) | News::StripSaid(_) | News::Chosen(_) => {}
            News::Closed => panic!("the window closed"),
        }
    };
    let action = alo_url::parse("https://example.com/send").unwrap_or_else(|_| Url::about_blank());
    assert_eq!(
        said,
        format!("this form is sent by POST to {action}, {NOT_GONE}")
    );
    send(&orders, Order::CloseEverything).unwrap_or_else(|why| panic!("{why}"));
    assert!(conductor.finish());
}
