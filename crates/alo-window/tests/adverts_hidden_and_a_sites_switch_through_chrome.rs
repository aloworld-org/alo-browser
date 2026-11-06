/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 806 and 807, ADR 0094's amendment of 6 November 2026: the
//! empty box an advert leaves is hidden by the lists' own hiding rules, put
//! into the page's document as a style sheet through Chromium's own
//! commands — no script of ours — and a site switched off by a person is
//! neither blocked nor hidden, until it is switched on again. Through the
//! machine's Chrome and the Chromium alo carries (its CEF host), headless.
//!
//! The page is a server of this test's own on this machine. It holds a box
//! of the class `ad-slot-container` — `EasyList`'s generic `##.ad-slot-container`,
//! one of the slots measured hidden on the Guardian's front page — a story
//! beside it, and `/prebid.js`, which `EasyList` refuses. What a person would
//! read is what the agent tree holds: Chromium leaves a box hidden by
//! `display: none` out of it.
//!
//! The page is asked for at a name under `localhost`, which Chromium itself
//! answers with this machine: `EasyList` turns its generic hiding off for
//! `127.0.0.1` and `localhost:` themselves (`@@://127.0.0.1$generichide`),
//! so a page there is hidden nothing generic, as it should be. Each of the
//! two tests switches its own site — `news.localhost` through Chrome,
//! `paper.localhost` through the CEF host — because both run in one
//! process, and the switches are the process's (`blocking_off::process`).
//!
//! Never the real network.

#[cfg(target_os = "macos")]
mod carried;

use alo_net::activity::Happened;
use alo_renderer::Snapshot;
use alo_window::beside::Chromium;
use alo_window::blocking_off;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what a page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the advert's box says, were it shown.
const BOX: &str = "Advertisement space";

/// What the story says.
const STORY: &str = "The story itself";

/// What the server received: each request's path.
type Log = Arc<Mutex<Vec<String>>>;

/// The server's answer to `path`.
fn page(path: &str) -> Option<(&'static str, &'static str)> {
    Some(match path {
        "/front.html" => (
            "text/html; charset=utf-8",
            "<!doctype html><h1>Front page</h1>\
             <aside class=\"ad-slot-container\"><p>Advertisement space</p></aside>\
             <p class=\"story\">The story itself</p>\
             <script src=\"/prebid.js\"></script><img src=\"/story.png\">",
        ),
        "/prebid.js" => ("text/javascript", "document.title = 'reached'"),
        "/story.png" => ("image/png", ""),
        _ => return None,
    })
}

/// A server on this machine: its port, and what it received.
fn serve() -> Option<(u16, Log)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let log: Log = Arc::default();
    let logging = Arc::clone(&log);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let logging = Arc::clone(&logging);
            std::thread::spawn(move || {
                if socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
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
                let asked = String::from_utf8_lossy(&asked);
                let path = asked.split(' ').nth(1).unwrap_or_default().to_owned();
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push(path.clone());
                }
                let (status, kind, body) = match page(&path) {
                    Some((kind, body)) => ("200 OK", kind, body),
                    None => ("404 Not Found", "text/plain", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// How many times the server received `path`.
fn received(log: &Log, path: &str) -> usize {
    log.lock()
        .map(|log| log.iter().filter(|each| *each == path).count())
        .unwrap_or_default()
}

/// Whether `snapshot` holds `words` anywhere a person would read them.
fn reads(snapshot: &Snapshot, words: &str) -> bool {
    snapshot
        .nodes()
        .iter()
        .any(|node| node.to_string().contains(words))
}

/// What a page came to: its tree, and how many requests the lists refused.
struct Came {
    snapshot: Snapshot,
    blocked: usize,
}

/// The page at `address` opened in `chrome` and read until the server has
/// received `/story.png` `pictures` times and the advert's box is `shown`
/// or not: the sheet is filled once Chromium has answered that it made it,
/// a moment after the commit, so a read may come before it.
fn open(
    chrome: &mut Chrome,
    address: &str,
    log: &Log,
    (pictures, shown): (usize, bool),
) -> Result<Came, String> {
    let address = alo_url::parse(address).map_err(|why| format!("{why:?}"))?;
    let mut tab = Tab::open(chrome, &address, SIZE).map_err(|why| why.to_string())?;
    let until = Instant::now() + AT_MOST;
    loop {
        let snapshot = tab.read().map_err(|why| why.to_string())?;
        if received(log, "/story.png") >= pictures
            && reads(&snapshot, STORY)
            && reads(&snapshot, BOX) == shown
        {
            return Ok(Came {
                snapshot,
                blocked: tab.blocked().len(),
            });
        }
        if Instant::now() > until {
            return Err(format!("the page never came: {snapshot:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The page on `host`, through `chromium`: hidden and refused; switched off
/// for its site, shown and let go; switched on, hidden and refused again.
fn through(chromium: &Chromium, host: &str) -> Result<(), String> {
    let (port, log) = serve().ok_or("no server")?;
    let address = format!("http://{host}:{port}/front.html");
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    let off = blocking_off::process();

    // 1. On: the box hidden, the story read, the advert never asked.
    let came = open(&mut chrome, &address, &log, (1, false))?;
    assert!(!reads(&came.snapshot, BOX), "{:?}", came.snapshot);
    assert!(reads(&came.snapshot, "Front page"));
    assert_eq!(came.blocked, 1);
    assert_eq!(received(&log, "/prebid.js"), 0);

    // 2. Switched off for the site: the box shown, the advert let go and
    //    written so.
    off.switch(host, true);
    let came = open(&mut chrome, &address, &log, (2, true))?;
    assert!(reads(&came.snapshot, BOX), "{:?}", came.snapshot);
    assert_eq!(came.blocked, 0);
    let until = Instant::now() + AT_MOST;
    while received(&log, "/prebid.js") == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(received(&log, "/prebid.js"), 1);
    let prebid = format!("http://{host}:{port}/prebid.js");
    let written: Vec<Happened> = chrome
        .record()
        .activity()
        .entries()
        .filter(|entry| entry.url().to_string() == prebid)
        .map(|entry| entry.happened().clone())
        .collect();
    let [refused, let_go] = written.as_slice() else {
        return Err(format!("two lines of /prebid.js, not {written:?}"));
    };
    assert!(matches!(refused, Happened::Refused { .. }), "{refused:?}");
    assert_eq!(*let_go, Happened::LetGo);

    // 3. On again.
    off.switch(host, false);
    let came = open(&mut chrome, &address, &log, (3, false))?;
    assert!(!reads(&came.snapshot, BOX), "{:?}", came.snapshot);
    assert_eq!(came.blocked, 1);
    assert_eq!(received(&log, "/prebid.js"), 1, "refused again");
    Ok(())
}

#[test]
fn an_adverts_box_is_hidden_and_a_site_switched_off_shows_it_through_chrome() {
    through(&Chromium::Machines, "news.localhost").unwrap();
}

/// The same page through the Chromium alo carries, its CEF host.
#[cfg(target_os = "macos")]
#[test]
fn an_adverts_box_is_hidden_and_a_site_switched_off_shows_it_through_alos_cef_host() {
    let Some(host) = carried::host("806").unwrap() else {
        return;
    };
    through(&host.chromium(), "paper.localhost").unwrap();
}
