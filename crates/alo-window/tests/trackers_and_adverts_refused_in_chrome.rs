/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 736, ADR 0094: a tracker and an advert a page asks for are
//! refused by the lists alo carries — `EasyList` and `EasyPrivacy`, read by
//! Brave's matcher — before they leave, and recorded with the list, line
//! and rule that refused each; a page that asks for nothing they name loads
//! as it would with no lists at all. Through the machine's Chrome and the
//! Chromium alo carries (its CEF host), each the same test, headless.
//!
//! The network is a server of this test's own on this machine, which
//! writes down every request it receives. The page with a tracker asks for
//! `/prebid.js` — named by `EasyList`'s `/prebid.$script,domain=~prebid.org`
//! — and `/fbevents.js` — `EasyPrivacy`'s `/fbevents.js`, Facebook's pixel —
//! both generic rules of the lists as published, which is why a page on
//! `127.0.0.1` is named by them at all; then a picture and a `fetch` the
//! lists do not name. Asserted from the server's side: the two never
//! reached it, and everything else did.
//!
//! Never the real network, and never a real tracker.

#[cfg(target_os = "macos")]
mod carried;

use alo_net::activity::Happened;
use alo_renderer::Snapshot;
use alo_window::beside::Chromium;
use alo_window::blocking::{Blocked, EASYLIST, EASYPRIVACY};
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

/// What the server received: each request's path.
type Log = Arc<Mutex<Vec<String>>>;

/// The server's answer to `path`: its type and body, or nothing for a path
/// it does not have.
fn page(path: &str) -> Option<(&'static str, &'static str)> {
    let html = "text/html; charset=utf-8";
    let script = "text/javascript";
    Some(match path {
        "/tracked.html" => (
            html,
            "<!doctype html><h1>A page with a tracker</h1>\
             <script src=\"/prebid.js\"></script>\
             <script src=\"/fbevents.js\"></script>\
             <img src=\"/story.png\">\
             <script>fetch('/after-them')</script>",
        ),
        "/clean.html" => (
            html,
            "<!doctype html><link rel=stylesheet href=\"/clean.css\">\
             <h1>Nothing to refuse</h1><img src=\"/story.png\">\
             <script src=\"/app.js\"></script>",
        ),
        "/app.js" => (script, "fetch('/from-app')"),
        "/prebid.js" | "/fbevents.js" => (script, "document.title = 'reached'"),
        "/clean.css" => ("text/css", "h1 { color: teal }"),
        "/after-them" | "/from-app" => ("text/plain", "ok"),
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
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
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

/// What the server has received so far.
fn received(log: &Log) -> Vec<String> {
    log.lock().map(|log| log.clone()).unwrap_or_default()
}

/// Whether `snapshot` holds `words` anywhere a person would read them.
fn reads(snapshot: &Snapshot, words: &str) -> bool {
    snapshot
        .nodes()
        .iter()
        .any(|node| node.to_string().contains(words))
}

/// The tab read again and again — each read answering every request paused
/// meanwhile — until the server has received every one of `awaited`.
fn read_until(tab: &mut Tab<'_>, log: &Log, awaited: &[&str]) -> Result<Snapshot, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let snapshot = tab.read().map_err(|why| why.to_string())?;
        let seen = received(log);
        let missing: Vec<_> = awaited
            .iter()
            .filter(|path| !seen.iter().any(|seen| seen == *path))
            .collect();
        if missing.is_empty() {
            return Ok(snapshot);
        }
        if Instant::now() > until {
            return Err(format!("never received {missing:?}; received {seen:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The line of `list` the rule `rule` is on, counted from one.
fn line_of(list: &str, rule: &str) -> Option<u32> {
    let place = list.lines().position(|line| line == rule)?;
    u32::try_from(place + 1).ok()
}

/// The page with a tracker and an advert, and the page with neither, each
/// opened through `chromium`.
fn refused_through(chromium: &Chromium) -> Result<(), String> {
    let (port, log) = serve().ok_or("no server")?;
    let mut chrome = Chrome::start_from(chromium, false).map_err(|why| why.to_string())?;
    the_tracked_page(&mut chrome, port, &log)?;
    the_clean_page(&mut chrome, port, &log)
}

/// The page with a tracker and an advert, served on `port`, opened in
/// `chrome`: both refused, recorded, and named in the tab.
fn the_tracked_page(chrome: &mut Chrome, port: u16, log: &Log) -> Result<(), String> {
    let at = |path: &str| format!("http://127.0.0.1:{port}{path}");
    let address = alo_url::parse(&at("/tracked.html")).map_err(|why| format!("{why:?}"))?;
    let mut tab = Tab::open(chrome, &address, SIZE).map_err(|why| why.to_string())?;
    let snapshot = read_until(
        &mut tab,
        log,
        &["/tracked.html", "/story.png", "/after-them"],
    )?;
    assert!(reads(&snapshot, "A page with a tracker"), "{snapshot:?}");

    // Neither reached the server, and the page ran on past both.
    let seen = received(log);
    assert!(
        !seen
            .iter()
            .any(|path| path == "/prebid.js" || path == "/fbevents.js"),
        "a request the lists name reached the server: {seen:?}"
    );
    let advert = "/prebid.$script,domain=~prebid.org";
    let tracker = "/fbevents.js";
    let expected = [
        (
            at("/prebid.js"),
            Blocked {
                list: Some("EasyList"),
                line: line_of(EASYLIST.text, advert),
                rule: Some(advert.to_owned()),
            },
        ),
        (
            at("/fbevents.js"),
            Blocked {
                list: Some("EasyPrivacy"),
                line: line_of(EASYPRIVACY.text, tracker),
                rule: Some(tracker.to_owned()),
            },
        ),
    ];
    assert!(
        expected.iter().all(|(_, blocked)| blocked.line.is_some()),
        "the rules are in the lists carried"
    );
    assert_eq!(tab.blocked(), expected.as_slice());
    assert_eq!(tab.refused(), [at("/prebid.js"), at("/fbevents.js")]);
    assert_eq!(
        tab.refusals(),
        &[],
        "no rule of the browser's own refused them"
    );
    drop(tab);

    // Each in the record, refused with the list, line and rule.
    for (url, blocked) in &expected {
        let lines: Vec<_> = chrome
            .record()
            .activity()
            .entries()
            .filter(|entry| entry.url().to_string() == *url)
            .map(|entry| entry.happened().clone())
            .collect();
        assert_eq!(
            lines,
            [Happened::Refused {
                rule: blocked.to_string()
            }],
            "{url}"
        );
    }
    assert_eq!(
        expected[0].1.to_string(),
        format!(
            "a tracker or an advert by EasyList, line {}: {advert}",
            expected[0].1.line.unwrap_or_default()
        )
    );
    Ok(())
}

/// A page that needs nothing refused, served on `port`, opened in
/// `chrome`: everything it asks for reaches the server, nothing is
/// refused, and every line of the record is let go.
fn the_clean_page(chrome: &mut Chrome, port: u16, log: &Log) -> Result<(), String> {
    let at = |path: &str| format!("http://127.0.0.1:{port}{path}");
    let before = received(log).len();
    let address = alo_url::parse(&at("/clean.html")).map_err(|why| format!("{why:?}"))?;
    let mut tab = Tab::open(chrome, &address, SIZE).map_err(|why| why.to_string())?;
    let awaited = [
        "/clean.html",
        "/clean.css",
        "/story.png",
        "/app.js",
        "/from-app",
    ];
    let snapshot = read_until(&mut tab, log, &awaited)?;
    assert!(reads(&snapshot, "Nothing to refuse"), "{snapshot:?}");
    assert_eq!(tab.blocked(), &[]);
    assert_eq!(tab.refused(), &[] as &[String]);
    let mut asked: Vec<String> = received(log).split_off(before);
    asked.retain(|path| path != "/favicon.ico");
    asked.sort();
    let mut wanted: Vec<String> = awaited.iter().map(|path| (*path).to_owned()).collect();
    wanted.sort();
    assert_eq!(
        asked, wanted,
        "the clean page asked for what it holds, once each"
    );
    drop(tab);
    let clean: Vec<_> = chrome
        .record()
        .activity()
        .entries()
        .filter(|entry| {
            awaited
                .iter()
                .any(|path| entry.url().to_string() == at(path))
        })
        .map(|entry| entry.happened().clone())
        .collect();
    // `/story.png` is in it twice, once for each page.
    assert_eq!(clean.len(), awaited.len() + 1, "{clean:?}");
    assert!(
        clean.iter().all(|happened| *happened == Happened::LetGo),
        "{clean:?}"
    );
    Ok(())
}

#[test]
fn a_tracker_and_an_advert_are_refused_and_recorded_through_chrome() {
    refused_through(&Chromium::Machines).unwrap();
}

/// The same pages through the Chromium alo carries, its CEF host.
#[cfg(target_os = "macos")]
#[test]
fn a_tracker_and_an_advert_are_refused_and_recorded_through_alos_cef_host() {
    let Some(host) = carried::host("736").unwrap() else {
        return;
    };
    refused_through(&host.chromium()).unwrap();
}
