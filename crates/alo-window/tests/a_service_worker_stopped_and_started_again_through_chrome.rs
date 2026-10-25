/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A service worker of a page in Chrome that stops and starts again: queue
//! item 655, ADR 0060 § 2.
//!
//! Measured against Chrome 155: a service worker the tab holds, once
//! stopped, keeps its target and its session and answers nothing sent to
//! it, and when the page next needs it, it starts again in that same target
//! — with no new attach — **waiting** until it is let go there. Before the
//! tab knew that, a verb run while it was stopped waited on it until
//! Chrome's patience ran out, and the page's request through it, once it
//! started again, waited for ever.
//!
//! The page, at a server of this test's own, registers a service worker
//! that takes it over, says something every fifty milliseconds saying when,
//! and answers the page's `/x` by fetching `/from-sw`. A button's click
//! holds the page's own thread for half a second; another's fetches `/x`.
//! The tab stops the worker between two verbs, the way Chrome stopping it
//! is told of. So:
//!
//! - a verb while it is stopped is answered at once and hears nothing of
//!   it;
//! - the page's fetch starts it again, and its `/from-sw` is paused in its
//!   own target, let go and reaches the server;
//! - and the verb after that hears it again, and only what it said while
//!   that verb ran.

use alo_agent::verb::{Target, Verb};
use alo_window::chrome::Chrome;
use alo_window::chrome_hold::Kind;
use alo_window::chrome_tab::{Tab, Went};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what the page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// How long Chrome is waited for, as a tab asks it something: shorter than
/// its usual patience, so that a verb that waits on a stopped worker fails
/// by it rather than by the test's own bound.
const PATIENCE: Duration = Duration::from_secs(15);

/// How long a verb run while the worker is stopped may take: the click's
/// half second, and room for a loaded machine — far short of `PATIENCE`.
const PROMPT: Duration = Duration::from_secs(5);

/// What the server received: each request's path.
type Log = Arc<Mutex<Vec<String>>>;

/// The server's answer to `path`: its type and its body.
fn answer(path: &str) -> (&'static str, &'static str) {
    let html = "text/html; charset=utf-8";
    match path {
        "/again.html" => (
            html,
            "<!doctype html><html lang=en><title>Again</title><body>\
             <button>Wait</button><button>Fetch</button><script>
const [wait, again] = document.querySelectorAll('button');
navigator.serviceWorker.addEventListener('controllerchange', () => fetch('/x?ready'));
navigator.serviceWorker.register('/sw.js');
wait.addEventListener('click', () => { const from = Date.now(); while (Date.now() - from < 500) {} });
again.addEventListener('click', () => fetch('/x?again'));
</script>",
        ),
        "/sw.js" => (
            "text/javascript",
            "console.log('service started', Date.now());
setInterval(() => console.warn('service tick', Date.now()), 50);
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(clients.claim()));
self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);
  if (url.pathname === '/x') e.respondWith(fetch('/from-sw' + url.search));
});",
        ),
        _ => ("text/plain", "ok"),
    }
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
                let (kind, body) = answer(path.split('?').next().unwrap_or_default());
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// The tab read again and again — each read answering every request paused
/// meanwhile — until the server has received `awaited`.
fn read_until(tab: &mut Tab<'_>, log: &Log, awaited: &str) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    loop {
        tab.read().map_err(|why| why.to_string())?;
        let seen = log.lock().map(|log| log.clone()).unwrap_or_default();
        if seen.iter().any(|path| path == awaited) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!("never received {awaited:?}: {seen:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The time now, in the milliseconds a page's `Date.now()` counts.
fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis())
}

/// The moment a line says it was said at: the first number of thirteen
/// digits in it.
fn moment(line: &str) -> Option<u128> {
    line.split(|c: char| !c.is_ascii_digit())
        .find(|digits| digits.len() == 13)
        .and_then(|digits| digits.parse().ok())
}

/// What a verb heard, and how long it took.
struct Clicked {
    heard: Vec<String>,
    took: Duration,
}

/// The button named `name` clicked in `tab`, every line it heard checked
/// to have been said while it ran.
fn clicked(tab: &mut Tab<'_>, name: &str) -> Result<Clicked, String> {
    let (before, started) = (now(), Instant::now());
    let (_, heard) = tab
        .act_and_hear(&Target::Named(name.to_owned()), &Verb::Activate)
        .map_err(|why| format!("{name}: {why}"))?;
    let (after, took) = (now(), started.elapsed());
    for line in &heard {
        match moment(line) {
            Some(at) if (before..=after).contains(&at) => {}
            _ => return Err(format!("not said while {name} ran: {line}")),
        }
    }
    Ok(Clicked { heard, took })
}

/// Whether some line of `heard` is the worker's tick.
fn ticked(heard: &[String], port: u16) -> bool {
    let at = format!("(at http://127.0.0.1:{port}/sw.js, line ");
    heard.iter().any(|line| {
        line.starts_with("the click: console.warn: service tick ") && line.contains(&at)
    })
}

#[test]
fn a_service_worker_that_stops_is_let_go_again_and_no_verb_waits_on_it() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap().waiting_at_most(PATIENCE);
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/again.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(&mut tab, &log, "/from-sw?ready").unwrap();

    let running = clicked(&mut tab, "Wait").unwrap();
    assert!(ticked(&running.heard, port), "{:#?}", running.heard);

    assert_eq!(tab.stop_service_workers_for_tests().unwrap(), 1);
    // Long enough that it would have said a good deal, had it gone on.
    std::thread::sleep(Duration::from_millis(300));
    let stopped = clicked(&mut tab, "Wait").unwrap();
    assert!(
        stopped.took < PROMPT,
        "a verb waited {:?} on a stopped worker",
        stopped.took
    );
    assert!(
        stopped.heard.iter().all(|line| !line.contains("service")),
        "{:#?}",
        stopped.heard
    );
    assert!(
        !log.lock()
            .unwrap()
            .iter()
            .any(|path| path == "/from-sw?again"),
        "nothing has asked for it yet"
    );

    // The page's fetch starts it again, waiting; held again and let go, it
    // answers by fetching, which is paused in its own target.
    clicked(&mut tab, "Fetch").unwrap();
    read_until(&mut tab, &log, "/from-sw?again").unwrap();
    let again = format!("http://127.0.0.1:{port}/from-sw?again");
    assert!(
        tab.went().contains(&Went {
            url: again,
            by: Kind::ServiceWorker,
        }),
        "{:#?}",
        tab.went()
    );

    std::thread::sleep(Duration::from_millis(300));
    let heard_again = clicked(&mut tab, "Wait").unwrap();
    assert!(ticked(&heard_again.heard, port), "{:#?}", heard_again.heard);
}
