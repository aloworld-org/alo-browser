/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a frame from another site and a dedicated worker said while a verb
//! ran, through Chrome: queue item 652, `docs/contracts/agent-socket.md`.
//!
//! Each is a target of its own in Chrome, so what it says is told of
//! through its own session, never the tab's. The network is a server of this
//! test's own, reached by two names: the page at `127.0.0.1` holds a frame
//! from `localhost`, which starts a worker of its own, and starts a worker
//! of its own too. The frame and both workers say something every fifty
//! milliseconds from the moment they start, each saying when; the frame and
//! its worker throw every fourth time. A button's click holds the page's
//! own thread for a second, so the verb runs for that long while they go on.
//!
//! So each verb hears what each said **while it ran and nothing else**:
//! every moment said is between the verb's start and its answer, which a
//! frame or worker told to tell of its script without first forgetting
//! fails at once — it tells again of everything it said since it started.
//! The frame also starts a worker every quarter second, which says when
//! it started and closes; one started while the verb runs is heard too,
//! told to tell as it is held.
//!
//! **A dedicated worker of the page throws unheard while the page is
//! busy**, and one the page starts while its thread is held does not start
//! until it is free: Chrome hands its uncaught error to the page's own thread first,
//! and tells of it only once that thread is free — after the verb, when
//! nothing is heard (measured against Chrome 155). Its console is heard;
//! the frame's worker, whose page is the frame's free thread, is heard
//! throwing.

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

/// What a frame or worker says every fifty milliseconds: `{say}` and when,
/// and every fourth time `{throw}` and when, if it throws at all.
fn ticks(say: &str, throw: Option<&str>) -> String {
    let throw = throw.map_or_else(String::new, |throw| {
        format!("if (n % 4 === 0) throw new {throw} threw ' + Date.now()); ")
    });
    format!("let n = 0; setInterval(() => {{ n++; {throw}{say}, Date.now()); }}, 50);")
}

/// The server's answer to `path`, on port `port`: its type and its body.
fn answer(path: &str, port: u16) -> (&'static str, String) {
    let html = "text/html; charset=utf-8";
    let script = "text/javascript";
    match path {
        "/heard.html" => (
            html,
            format!(
                "<!doctype html><html lang=en><title>Heard</title><body>\
                 <button>Wait</button>\
                 <iframe src=\"http://localhost:{port}/frame.html\"></iframe><script>
const hold = () => {{ const from = Date.now(); while (Date.now() - from < 1000) {{}} }};
new Worker('/page-worker.js');
document.body.firstChild.addEventListener('click', hold);
</script>"
            ),
        ),
        "/frame.html" => (
            html,
            format!(
                "<!doctype html><script>\nnew Worker('/frame-worker.js');\n{}\n{}\n</script>",
                // A worker started every quarter second, which says when it
                // started and closes: each one started while a verb runs is
                // started after that verb began to be heard.
                "let started = 0; const starting = setInterval(() => { if (++started > 60) \
                 clearInterval(starting); new Worker('/late.js'); }, 250);",
                ticks("console.log('frame tick'", Some("Error('frame"))
            ),
        ),
        "/frame-worker.js" => (
            script,
            ticks("console.warn('worker tick'", Some("TypeError('worker")),
        ),
        "/page-worker.js" => (script, ticks("console.info('page worker tick'", None)),
        "/late.js" => (
            script,
            "console.log('started', Date.now()); close();".to_owned(),
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
fn a_frame_and_a_worker_are_heard_only_while_a_verb_runs() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/heard.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(
        &mut tab,
        &log,
        &[
            ("localhost", "/frame-worker.js"),
            ("127.0.0.1", "/page-worker.js"),
        ],
    )
    .unwrap();
    // Long enough that each has said a good deal before the verb, which a
    // target that did not forget would tell of again.
    std::thread::sleep(Duration::from_millis(500));

    let frame = format!("http://localhost:{port}/frame.html");
    let frame_worker = format!("http://localhost:{port}/frame-worker.js");
    let page_worker = format!("http://127.0.0.1:{port}/page-worker.js");
    let late = format!("http://localhost:{port}/late.js");
    for _ in 0..2 {
        let (heard, before, after) = clicked(&mut tab, "Wait").unwrap();
        all_while_it_ran(&heard, before, after).unwrap();
        for (said, at) in [
            ("the click: console.log: frame tick ", &frame),
            ("the click: uncaught: Error: frame threw ", &frame),
            ("the click: console.warn: worker tick ", &frame_worker),
            (
                "the click: uncaught: TypeError: worker threw ",
                &frame_worker,
            ),
            ("the click: console.info: page worker tick ", &page_worker),
            ("the click: console.log: started ", &late),
        ] {
            assert!(heard_at(&heard, said, at), "{said:?} at {at}: {heard:#?}");
        }
        // Between two verbs nothing is heard, and nothing said then is told
        // of to the next.
        std::thread::sleep(Duration::from_millis(300));
    }
}
