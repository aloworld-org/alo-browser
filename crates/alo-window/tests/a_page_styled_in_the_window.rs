/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's linked style sheets, made off the conductor and waited for only
//! so long (queue items 348 and 351, ADR 0035 §§ 4–5, ADR 0041).
//!
//! The conductor hands a page's sheets to its network thread, one exchange
//! at a time, and never waits on a server itself. A load whose answer asked
//! for sheets is held: its first frame is painted once they have answered,
//! or once [`LONGEST_HOLD`] has passed — said to the person — whichever is
//! first. A sheet a script adds later holds nothing back.
//!
//! Each test here is one of ADR 0041 § 6's closing conditions, against a
//! server on this machine and the real confined renderer. The renderers have
//! no fonts, so a page is one box, `out`, which only its sheet colours.

use alo_layout::Size;
use alo_net::activity::Happened;
use alo_net::{Pool, Purpose, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::hold::{LONGEST_HOLD, SHOWN_BEFORE_STYLE};
use alo_window::message::{News, Order};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const AT_MOST: Duration = Duration::from_secs(60);

/// How long `/late.css` waits before it answers: well inside the bound.
const LATE: Duration = Duration::from_millis(500);

/// Far less than the bound, and far more than a frame takes on this machine:
/// how long an answer that must not wait on a server may take.
const PROMPTLY: Duration = Duration::from_secs(2);

/// The green a sheet paints `#out`.
const GREEN: (u8, u8, u8, u8) = (47, 111, 79, 255);
const WHITE: (u8, u8, u8, u8) = (255, 255, 255, 255);

const SHEET: &[u8] = b"#out { background: rgb(47, 111, 79) }";

/// A server on this machine, answering each connection on a thread of its
/// own:
/// - `/site.css`, a style sheet at once;
/// - `/late.css`, the same after [`LATE`];
/// - `/slow.css`, a sheet that **never finishes**: its head promises more
///   than it sends, and a byte follows every fifth of a second — inside any
///   patience — until the test says [`Server::stop`];
/// - `/text`, a word;
///
/// and anything else not found.
struct Server {
    origin: String,
    /// How many requests it has read.
    asked: Arc<AtomicUsize>,
    stopping: Arc<AtomicBool>,
}

impl Server {
    fn start() -> Option<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").ok()?;
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().ok()?.port());
        let asked = Arc::new(AtomicUsize::new(0));
        let stopping = Arc::new(AtomicBool::new(false));
        let (counting, stop) = (Arc::clone(&asked), Arc::clone(&stopping));
        std::thread::spawn(move || {
            for socket in listener.incoming() {
                let Ok(socket) = socket else {
                    return;
                };
                let (counting, stop) = (Arc::clone(&counting), Arc::clone(&stop));
                std::thread::spawn(move || answer(socket, &counting, &stop));
            }
        });
        Some(Self {
            origin,
            asked,
            stopping,
        })
    }

    /// End every `/slow.css` still trickling, short of what it promised.
    fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
}

fn answer(mut socket: TcpStream, counting: &AtomicUsize, stop: &AtomicBool) {
    if socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .is_err()
    {
        return;
    }
    let mut asked = Vec::new();
    let mut block = [0u8; 1024];
    while let Ok(got) = socket.read(&mut block) {
        asked.extend_from_slice(block.get(..got).unwrap_or_default());
        if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
            break;
        }
    }
    counting.fetch_add(1, Ordering::SeqCst);
    let sheet_head = |length: usize| {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {length}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n"
        )
        .into_bytes()
    };
    let reply = if asked.starts_with(b"GET /site.css ") || asked.starts_with(b"GET /late.css ") {
        if asked.starts_with(b"GET /late.css ") {
            std::thread::sleep(LATE);
        }
        let mut reply = sheet_head(SHEET.len());
        reply.extend_from_slice(SHEET);
        reply
    } else if asked.starts_with(b"GET /slow.css ") {
        if socket.write_all(&sheet_head(1_000_000)).is_err() {
            return;
        }
        while !stop.load(Ordering::SeqCst) {
            if socket.write_all(b" ").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        return;
    } else if asked.starts_with(b"GET /text ") {
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 7\r\n\
          Cache-Control: no-store\r\nConnection: close\r\n\r\nfetched"
            .to_vec()
    } else {
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
    };
    let _ = socket.write_all(&reply);
}

/// A conductor over the real renderer binary with `markup` from `origin`
/// open at 200 × 60, and the moment the page was sent to it.
fn conducting(origin: &str, markup: &str) -> Result<(Conductor, Receiver<News>, Instant), String> {
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
    let page = Page::new(markup, Size::ZERO)
        .at(url.clone())
        .with_sheet("body { margin: 0 } div { height: 20px }");
    let orders = conductor.orders();
    orders
        .send(Order::Resize(Size::new(200.0, 60.0)))
        .map_err(|why| why.to_string())?;
    let sent = Instant::now();
    orders
        .send(Order::Open {
            url,
            page: Some(Box::new(page)),
        })
        .map_err(|why| why.to_string())?;
    Ok((conductor, news, sent))
}

/// The next frame painted, or what arrived instead.
fn painted(news: &mpsc::Receiver<News>, within: Duration) -> Result<Frame, String> {
    match news.recv_timeout(within) {
        Ok(News::Painted(frame)) => Ok(frame),
        other => Err(format!("not a frame: {other:?}")),
    }
}

/// Close the window: told at once, whatever the network is doing. Why not,
/// when it was not.
fn close(conductor: &Conductor, news: &mpsc::Receiver<News>) -> Result<(), String> {
    conductor
        .orders()
        .send(Order::CloseEverything)
        .map_err(|why| why.to_string())?;
    let asked = Instant::now();
    match news.recv_timeout(AT_MOST) {
        Ok(News::Closed) if asked.elapsed() < PROMPTLY => Ok(()),
        Ok(News::Closed) => Err(format!("closing waited on a server: {:?}", asked.elapsed())),
        other => Err(format!("not closed: {other:?}")),
    }
}

/// The session's record, as `path outcome` for each style request.
fn sheets_recorded(network: &Network) -> Vec<String> {
    network
        .pool
        .activity()
        .entries()
        .filter(|entry| entry.purpose() == &Purpose::Style)
        .map(|entry| {
            let outcome = match entry.happened() {
                Happened::Answered { status, .. } => status.to_string(),
                Happened::Served { status } => format!("served {status}"),
                Happened::Refused { rule } => format!("refused: {rule}"),
                Happened::Failed { .. } => "failed".to_owned(),
            };
            format!("{} {outcome}", entry.url().path)
        })
        .collect()
}

#[test]
fn a_page_whose_sheet_answers_late_is_painted_once_and_styled() {
    let server = Server::start().expect("a server");
    let (conductor, news, sent) = conducting(
        &server.origin,
        "<link rel=stylesheet href=/late.css><link rel=stylesheet href=late.css>\
         <div id=out></div><script>var ran = true;</script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    let first = painted(&news, AT_MOST).unwrap_or_else(|why| panic!("{why}"));
    assert!(sent.elapsed() >= LATE, "painted before its sheet answered");
    assert_eq!(first.at(100, 10), Some(GREEN), "the first frame is styled");
    assert_eq!(first.at(100, 30), Some(WHITE), "and only the box it styles");
    assert_eq!(
        server.asked.load(Ordering::SeqCst),
        1,
        "two links, one request"
    );

    close(&conductor, &news).unwrap_or_else(|why| panic!("{why}"));
    let network = conductor.hand_back().expect("the session's network");
    assert_eq!(sheets_recorded(&network), ["/late.css 200"]);
}

#[test]
fn a_sheet_that_never_finishes_is_waited_for_only_so_long() {
    let server = Server::start().expect("a server");
    let (conductor, news, sent) = conducting(
        &server.origin,
        "<link rel=stylesheet href=/slow.css><div id=out></div>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    let first = painted(&news, AT_MOST).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        sent.elapsed() >= LONGEST_HOLD,
        "painted after {:?}, before the bound",
        sent.elapsed()
    );
    assert_eq!(first.at(100, 10), Some(WHITE), "painted with what had come");
    assert_eq!(
        news.recv_timeout(AT_MOST),
        Ok(News::Said(SHOWN_BEFORE_STYLE.to_owned())),
        "and said to be"
    );

    // The sheet's exchange is still in flight, and the window is answered.
    conductor
        .orders()
        .send(Order::Resize(Size::new(300.0, 80.0)))
        .unwrap_or_else(|why| panic!("{why}"));
    let resized = painted(&news, PROMPTLY).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((resized.width, resized.height), (300, 80));
    close(&conductor, &news).unwrap_or_else(|why| panic!("{why}"));

    // Its exchange ends only when the server stops; the record was handed
    // back after it, and holds the request as its head answered it.
    server.stop();
    let network = conductor.hand_back().expect("the session's network");
    assert_eq!(sheets_recorded(&network), ["/slow.css 200"]);
}

#[test]
fn a_sheet_a_script_adds_after_the_load_holds_back_no_frame() {
    let server = Server::start().expect("a server");
    let (conductor, news, sent) = conducting(
        &server.origin,
        "<div id=out></div><script>\
           fetch('/text').then(function () {\
             var link = document.createElement('link');\
             link.setAttribute('rel', 'stylesheet');\
             link.setAttribute('href', '/slow.css');\
             document.body.appendChild(link);\
           });\
         </script>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    // It links nothing, so it is painted at once.
    let first = painted(&news, AT_MOST).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        sent.elapsed() < LONGEST_HOLD,
        "a page linking no sheet waited"
    );
    assert_eq!(first.at(100, 10), Some(WHITE));
    // The fetch's answer, whose reaction adds the sheet, is painted too.
    painted(&news, AT_MOST).unwrap_or_else(|why| panic!("{why}"));

    // And while that sheet trickles, nothing of the page is held back.
    conductor
        .orders()
        .send(Order::Resize(Size::new(300.0, 80.0)))
        .unwrap_or_else(|why| panic!("{why}"));
    let resized = painted(&news, PROMPTLY).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((resized.width, resized.height), (300, 80));
    assert_eq!(resized.at(100, 10), Some(WHITE), "its style has not come");
    close(&conductor, &news).unwrap_or_else(|why| panic!("{why}"));

    server.stop();
    assert!(conductor.finish());
}

#[test]
fn a_tab_closed_while_held_paints_nothing_more() {
    let server = Server::start().expect("a server");
    let (conductor, news, _) = conducting(
        &server.origin,
        "<link rel=stylesheet href=/slow.css><div id=out></div>",
    )
    .unwrap_or_else(|why| panic!("{why}"));

    // Held: nothing arrives while its sheet trickles.
    assert_eq!(
        news.recv_timeout(LONGEST_HOLD.saturating_sub(Duration::from_millis(250))),
        Err(RecvTimeoutError::Timeout)
    );
    close(&conductor, &news).unwrap_or_else(|why| panic!("{why}"));

    server.stop();
    assert!(conductor.finish());
    // Its sheet's answer came after the close, and was answered by nobody.
    assert_eq!(news.try_recv(), Err(mpsc::TryRecvError::Disconnected));
}
