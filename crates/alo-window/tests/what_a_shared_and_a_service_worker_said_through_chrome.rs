/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a shared worker and a service worker said while a verb ran, through
//! Chrome: queue item 654, `docs/contracts/agent-socket.md`.
//!
//! Each is a target of its own in Chrome — a shared worker told of to
//! Chrome's own session only, a service worker to the tab's — so what it
//! says is told of through its own session, never the tab's. The page, at
//! a server of this test's own, starts a shared worker and registers a
//! service worker that takes the page over; each says something every
//! fifty milliseconds from the moment it starts, saying when, and throws
//! every fourth time. A button's click holds the page's own thread for a
//! second, so the verb runs for that long while they go on.
//!
//! So each verb hears what each said **while it ran and nothing else**:
//! every moment said is between the verb's start and its answer, which a
//! worker told to tell of its script without first forgetting fails at
//! once — it tells again of everything it said since it started. And
//! unlike a dedicated worker's, what each throws is heard while the page's
//! thread is held (measured against Chrome 155).
//!
//! The first click also registers a second service worker, which starts
//! while the verb runs and says so. A service worker answers nothing before
//! it runs, so it is told to tell as it is held and let go without waiting
//! for the answer; held for it, it would never run, and the verb would
//! wait on it until Chrome's patience ran out.

use alo_agent::verb::{Target, Verb};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what the page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's host, without its port, and
/// its path.
type Log = Arc<Mutex<Vec<(String, String)>>>;

/// What a worker says every fifty milliseconds: `{say}` and when, and every
/// fourth time `{throw}` and when instead.
fn ticks(say: &str, throw: &str) -> String {
    format!(
        "let n = 0; setInterval(() => {{ n++; if (n % 4 === 0) throw new {throw} threw ' + \
         Date.now()); {say}, Date.now()); }}, 50);"
    )
}

/// The server's answer to `path`, on port `port`: its type and its body.
fn answer(path: &str, _port: u16) -> (&'static str, String) {
    let html = "text/html; charset=utf-8";
    let script = "text/javascript";
    match path {
        "/heard.html" => (
            html,
            "<!doctype html><html lang=en><title>Heard</title><body>\
             <button>Wait</button><script>
const hold = () => { const from = Date.now(); while (Date.now() - from < 1000) {} };
let clicks = 0;
new SharedWorker('/shared.js');
navigator.serviceWorker.register('/sw.js');
document.body.firstChild.addEventListener('click', () => {
  if (++clicks === 1) navigator.serviceWorker.register('/late/sw.js');
  hold();
});
</script>"
                .to_owned(),
        ),
        "/shared.js" => (script, ticks("console.log('shared tick'", "Error('shared")),
        "/sw.js" => (
            script,
            format!(
                "{} self.addEventListener('install', () => self.skipWaiting()); \
                 self.addEventListener('activate', (e) => e.waitUntil(clients.claim()));",
                ticks("console.warn('service tick'", "TypeError('service"),
            ),
        ),
        "/late/sw.js" => (
            script,
            "console.log('late started', Date.now());".to_owned(),
        ),
        _ => (html, String::new()),
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
                let host = asked
                    .lines()
                    .find_map(|line| line.strip_prefix("Host: "))
                    .and_then(|host| host.split(':').next())
                    .unwrap_or_default()
                    .to_owned();
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
                if path.is_empty() {
                    return;
                }
                if let Ok(mut log) = logging.lock() {
                    log.push((host, path.clone()));
                }
                let (kind, body) = answer(&path, port);
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
/// meanwhile — until the server has received every one of `awaited`.
fn read_until(tab: &mut Tab<'_>, log: &Log, awaited: &[(&str, &str)]) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    loop {
        tab.read().map_err(|why| why.to_string())?;
        let seen = log.lock().map(|log| log.clone()).unwrap_or_default();
        if awaited
            .iter()
            .all(|(host, path)| seen.iter().any(|(h, p)| h == host && p == path))
        {
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

/// The button named `name` clicked in `tab`: what was heard, the moment
/// before the verb began and the moment after it was answered.
fn clicked(tab: &mut Tab<'_>, name: &str) -> Result<(Vec<String>, u128, u128), String> {
    let before = now();
    let (_, heard) = tab
        .act_and_hear(&Target::Named(name.to_owned()), &Verb::Activate)
        .map_err(|why| format!("{name}: {why}"))?;
    Ok((heard, before, now()))
}

/// Whether some line of `heard` begins with `said` and is placed at `at`.
fn heard_at(heard: &[String], said: &str, at: &str) -> bool {
    heard
        .iter()
        .any(|line| line.starts_with(said) && line.contains(&format!("(at {at}, line ")))
}

/// Every line of `heard` says a moment, and each is between `before` and
/// `after`.
fn all_while_it_ran(heard: &[String], before: u128, after: u128) -> Result<(), String> {
    for line in heard {
        match moment(line) {
            Some(at) if (before..=after).contains(&at) => {}
            Some(at) => {
                return Err(format!(
                    "said at {at}, outside the verb's {before}..={after}: {line}"
                ));
            }
            None => return Err(format!("no moment said: {line}")),
        }
    }
    Ok(())
}

#[test]
fn a_shared_and_a_service_worker_are_heard_only_while_a_verb_runs() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/heard.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(
        &mut tab,
        &log,
        &[("127.0.0.1", "/shared.js"), ("127.0.0.1", "/sw.js")],
    )
    .unwrap();
    // Long enough that each runs and has said a good deal before the verb,
    // which a target that did not forget would tell of again.
    std::thread::sleep(Duration::from_secs(1));

    let shared = format!("http://127.0.0.1:{port}/shared.js");
    let service = format!("http://127.0.0.1:{port}/sw.js");
    let late = format!("http://127.0.0.1:{port}/late/sw.js");
    for click in 0..2 {
        let (heard, before, after) = clicked(&mut tab, "Wait").unwrap();
        all_while_it_ran(&heard, before, after).unwrap();
        let mut said = vec![
            ("the click: console.log: shared tick ", &shared),
            ("the click: uncaught: Error: shared threw ", &shared),
            ("the click: console.warn: service tick ", &service),
            ("the click: uncaught: TypeError: service threw ", &service),
        ];
        if click == 0 {
            said.push(("the click: console.log: late started ", &late));
        }
        for (said, at) in said {
            assert!(heard_at(&heard, said, at), "{said:?} at {at}: {heard:#?}");
        }
        // Between two verbs nothing is heard, and nothing said then is told
        // of to the next.
        std::thread::sleep(Duration::from_millis(300));
    }
}
