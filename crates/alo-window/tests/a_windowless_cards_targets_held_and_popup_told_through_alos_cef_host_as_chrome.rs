/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A windowless card's page held as ADR 0060 holds every page, and its
//! popup never made but told (queue item 978, ADR 0113, ADR 0060's sixth
//! amendment).
//!
//! alo's CEF host is started with no window at all
//! (`Chrome::start_windowless`), which CEF draws in **Alloy style** — where
//! ADR 0060's hold of Chromium's own session once stalled a `window.open`
//! for good (CEF issue #4308, measured in 959). Its page, from a server of
//! this test's own on this machine reached by two names so that it is two
//! sites (`127.0.0.1` and `localhost`), holds a frame from the other site
//! that fetches, a dedicated worker, a shared worker and a service worker
//! that each fetch, a button whose script opens a popup and a link to
//! another window.
//!
//! **Held, and paused before the server answered**, asserted from the
//! server's side: every request the server received the tab let go first,
//! each in the target that asked for it — the frame in its own, the
//! dedicated worker through the page, the service worker in its own, and
//! the shared worker in its own, held from Chromium's own session.
//!
//! **No popup is made**: the button's `window.open`, activated by name,
//! returns `null`, as a blocked popup's does, and a link to another
//! window activated by name goes nowhere; the server never hears of either
//! address; no tab is opened for the page to close; and the host tells alo
//! each address (`open`), which alo opens as the conversation's next turn
//! as it opens a link pressed with ⌘. The host then ends on
//! `Browser.close`, status 0.
//!
//! With no CEF distribution named by `CEF_PATH`, it says why on its error
//! output and returns.

#![cfg(target_os = "macos")]

mod carried;

use alo_agent::verb::{Target, Verb};
use alo_renderer::Snapshot;
use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_hold::Kind;
use alo_window::chrome_tab::{Tab, Went};
use alo_window::chrome_told::Told;
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The page's view, in CSS pixels: the host's for its life.
const SIZE: (u32, u32) = (600, 400);

/// How long the page's frame and workers, and the host's word of the
/// popup, are waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's host, without its port, and
/// its path.
type Log = Arc<Mutex<Vec<(String, String)>>>;

/// The server's answer to `path`, on port `port`: its type and body, or
/// nothing for a path it does not have.
fn page(path: &str, port: u16) -> Option<(&'static str, String)> {
    let html = "text/html; charset=utf-8";
    let script = "text/javascript";
    Some(match path {
        "/card.html" => (
            html,
            format!(
                "<!doctype html><title>Windowless</title><h1>Windowless</h1>\
                 <iframe src=\"http://localhost:{port}/frame.html\"></iframe>\
                 <button onclick=\"document.querySelector('h1').textContent = \
                 window.open('http://127.0.0.1:{port}/popup.html') === null \
                 ? 'Opened none' : 'Opened one'\">Open</button>\
                 <a href=\"http://localhost:{port}/away.html\" target=_blank>Away</a>\
                 <script>new Worker('/worker.js');\
                 new SharedWorker('/shared.js').port.start();\
                 navigator.serviceWorker.register('/sw.js');</script>"
            ),
        ),
        "/frame.html" => (html, "<script>fetch('/from-frame')</script>".to_owned()),
        "/worker.js" => (script, "fetch('/from-worker')".to_owned()),
        "/shared.js" => (
            script,
            "fetch('/from-shared'); onconnect = () => fetch('/on-connect');".to_owned(),
        ),
        "/sw.js" => (
            script,
            "self.addEventListener('install', \
             (e) => e.waitUntil(fetch('/from-service-worker')))"
                .to_owned(),
        ),
        "/popup.html" | "/away.html" => (html, "<!doctype html><title>Popup</title>".to_owned()),
        "/from-frame"
        | "/from-worker"
        | "/from-shared"
        | "/on-connect"
        | "/from-service-worker" => ("text/plain", "fetched".to_owned()),
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
                let host = asked
                    .lines()
                    .find_map(|line| line.strip_prefix("Host: "))
                    .and_then(|host| host.split(':').next())
                    .unwrap_or_default()
                    .to_owned();
                // A connection Chromium opened ahead of need and closed
                // unused asked for nothing.
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push((host, path.clone()));
                }
                let (status, kind, body) = match page(&path, port) {
                    Some((kind, body)) => ("200 OK", kind, body),
                    None => ("404 Not Found", "text/plain", String::new()),
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
fn received(log: &Log) -> Vec<(String, String)> {
    log.lock().map(|log| log.clone()).unwrap_or_default()
}

/// A request's host and path, as the server would write them down.
fn host_and_path(url: &str) -> Option<(String, String)> {
    let url = alo_url::parse(url).ok()?;
    Some((
        alo_url::reading::hostname(&url),
        alo_url::reading::pathname(&url),
    ))
}

/// The tab read again and again — each read answering every request paused
/// meanwhile — until the server has received every one of `awaited`.
fn read_until(tab: &mut Tab<'_>, log: &Log, awaited: &[(&str, &str)]) -> Result<Snapshot, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let snapshot = tab.read().map_err(|why| why.to_string())?;
        let seen = received(log);
        let missing: Vec<_> = awaited
            .iter()
            .filter(|(host, path)| !seen.iter().any(|(h, p)| h == host && p == path))
            .collect();
        if missing.is_empty() {
            return Ok(snapshot);
        }
        if Instant::now() > until {
            return Err(format!(
                "never received {missing:?} in {AT_MOST:?}; received {seen:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// What a tab let go — each request's host and path, if it reads as an
/// address — and the kind of target it was paused in.
type LetGo = Vec<(Option<(String, String)>, Kind)>;

/// What `tab` let go, and in which target — each request the server
/// received checked to have been let go first, at least as many times.
fn let_go_first(tab: &Tab<'_>, log: &Log) -> Result<LetGo, String> {
    let went: LetGo = tab
        .went()
        .iter()
        .map(|Went { url, by }| (host_and_path(url), *by))
        .collect();
    let seen = received(log);
    for request in &seen {
        let received = seen.iter().filter(|other| *other == request).count();
        let let_go = went
            .iter()
            .filter(|(went, _)| went.as_ref() == Some(request))
            .count();
        if let_go < received {
            return Err(format!(
                "{request:?} reached the server {received} times and was let go {let_go}: {went:?}"
            ));
        }
    }
    Ok(went)
}

#[test]
fn a_windowless_cards_frame_and_workers_are_held_and_its_popup_is_told_not_made() {
    let Some(host) = carried::host("978").unwrap() else {
        return;
    };
    held_and_told(&host.chromium()).unwrap();
}

/// The page opened windowless through the host `chromium`, its targets
/// seen held and its popups told and never made, and the host ended.
fn held_and_told(chromium: &Chromium) -> Result<(), String> {
    let Chromium::Carried(program) = chromium else {
        return Err("only the Chromium alo carries opens a page windowless".to_owned());
    };
    let (port, log) = serve().ok_or("no server")?;
    let mut chrome =
        Chrome::start_windowless(program, SIZE, Scale::ONE).map_err(|why| why.to_string())?;
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/card.html"))
        .map_err(|why| format!("{why:?}"))?;
    {
        let opened = Instant::now();
        let mut tab = Tab::open(&mut chrome, &address, SIZE).map_err(|why| why.to_string())?;
        eprintln!("opened windowless in {:?}", opened.elapsed());
        every_target_held(&mut tab, &log)?;
        no_popup_made(&mut tab, &log, port)?;
    }
    let closing = Instant::now();
    let ended = chrome.close();
    eprintln!("the host ended in {:?}: {ended:?}", closing.elapsed());
    assert!(
        ended.is_some_and(|status| status.success()),
        "the host did not end in order on Browser.close: {ended:?}"
    );
    Ok(())
}

/// Every target of the page in `tab` seen held: every request the server
/// received the tab let go first, each in the target that asked for it.
fn every_target_held(tab: &mut Tab<'_>, log: &Log) -> Result<(), String> {
    let awaited = [
        ("127.0.0.1", "/card.html"),
        ("localhost", "/frame.html"),
        ("localhost", "/from-frame"),
        ("127.0.0.1", "/worker.js"),
        ("127.0.0.1", "/from-worker"),
        ("127.0.0.1", "/shared.js"),
        ("127.0.0.1", "/from-shared"),
        ("127.0.0.1", "/on-connect"),
        ("127.0.0.1", "/sw.js"),
        ("127.0.0.1", "/from-service-worker"),
    ];
    read_until(tab, log, &awaited)?;
    let went = let_go_first(tab, log)?;
    let paused_in = |host: &str, path: &str, kind: Kind| {
        went.iter().any(|(went, by)| {
            *by == kind && went.as_ref() == Some(&(host.to_owned(), path.to_owned()))
        })
    };
    assert!(paused_in("127.0.0.1", "/card.html", Kind::Page), "{went:?}");
    assert!(
        paused_in("localhost", "/from-frame", Kind::Frame),
        "{went:?}"
    );
    assert!(
        paused_in("127.0.0.1", "/from-worker", Kind::Page),
        "a dedicated worker's are paused through its page: {went:?}"
    );
    assert!(paused_in("127.0.0.1", "/shared.js", Kind::Page), "{went:?}");
    for path in ["/sw.js", "/from-service-worker"] {
        assert!(
            paused_in("127.0.0.1", path, Kind::ServiceWorker),
            "{path}: {went:?}"
        );
    }
    for path in ["/from-shared", "/on-connect"] {
        assert!(
            paused_in("127.0.0.1", path, Kind::SharedWorker),
            "held from Chromium's own session in Alloy style: {path}: {went:?}"
        );
    }
    Ok(())
}

/// The page in `tab`, served on `port`, asked for two popups — a button's
/// `window.open` and a link to another window, each activated by name —
/// and each told and never made.
fn no_popup_made(tab: &mut Tab<'_>, log: &Log, port: u16) -> Result<(), String> {
    let pressed = Instant::now();
    let activated = tab.act(&Target::Named("Open".to_owned()), &Verb::Activate);
    eprintln!(
        "the button activated in {:?}: {activated:?}",
        pressed.elapsed()
    );
    activated.map_err(|why| format!("the press stalled or failed: {why}"))?;
    told_open(tab, &format!("http://127.0.0.1:{port}/popup.html"))?;
    eprintln!("the popup told after {:?}", pressed.elapsed());
    // A link to another window is a popup too, and told the same way.
    tab.act(&Target::Named("Away".to_owned()), &Verb::Activate)
        .map_err(|why| format!("the link stalled or failed: {why}"))?;
    told_open(tab, &format!("http://localhost:{port}/away.html"))?;
    // Read once more, a second on, so that a popup made would have been
    // told of by now: `window.open` returned with no window, as a blocked
    // popup's does.
    std::thread::sleep(Duration::from_secs(1));
    let snapshot = tab.read().map_err(|why| why.to_string())?;
    assert!(
        snapshot
            .nodes()
            .iter()
            .any(|node| node.to_string().contains("Opened none")),
        "{}",
        snapshot.to_outline()
    );
    assert_eq!(tab.closed(), 0, "no tab was made for the page to close");
    let popup = |path: &str| path.ends_with("/popup.html") || path.ends_with("/away.html");
    let seen = received(log);
    assert!(
        !seen.iter().any(|(_, path)| popup(path)),
        "neither popup was asked for: {seen:?}"
    );
    assert!(
        !tab.went().iter().any(|went| popup(&went.url)),
        "{:?}",
        tab.went()
    );
    Ok(())
}

/// `tab` read until its Chromium has told alo to open `address`, within
/// [`AT_MOST`].
fn told_open(tab: &mut Tab<'_>, address: &str) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    let mut heard = Vec::new();
    loop {
        tab.read().map_err(|why| why.to_string())?;
        heard.extend(tab.take_told());
        if heard.contains(&Told::Open(address.to_owned())) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "{address} was not told in {AT_MOST:?}: told {heard:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
