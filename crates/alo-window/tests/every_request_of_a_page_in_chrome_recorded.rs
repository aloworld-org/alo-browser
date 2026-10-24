/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every request of a page in Chrome is in the record: queue item 634, ADR
//! 0060 § 2, ADR 0012.
//!
//! The network is a server of this test's own on this machine, reached by
//! two names so that a redirect crosses from one site to the other — `/start`
//! at `127.0.0.1` answers `302` to the page at `localhost`. The page asks for
//! a style sheet, opens a WebSocket the server refuses, and holds a link to a
//! second page, which asks for a style sheet of its own.
//!
//! The record is read back line by line from [`Chrome::record`]: each hop of
//! the navigation a line, with the person's cause; the style sheet the
//! page's; the WebSocket a line saying it was opened and not decided; and
//! after an agent's `activate` on the link, the second page's navigation
//! the agent action's, and what that page asked for leading back to it.

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_net::activity::{Entry, Happened};
use alo_net::request::Purpose;
use alo_net::{Cause, chain::End};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what the page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's host, without its port, and
/// its path.
type Log = Arc<Mutex<Vec<(String, String)>>>;

/// The server's answer to `path`, on port `port`: its status, type, the
/// header it adds and its body.
fn answer(path: &str, port: u16) -> (&'static str, &'static str, String, String) {
    let html = "text/html; charset=utf-8";
    match path {
        "/start" => (
            "302 Found",
            "text/plain",
            format!("Location: http://localhost:{port}/page.html\r\n"),
            String::new(),
        ),
        "/page.html" => (
            "200 OK",
            html,
            String::new(),
            format!(
                "<!doctype html><title>First</title>\
                 <link rel=stylesheet href=\"/first.css\">\
                 <h1>First</h1><a href=\"/next.html\">Next</a>\
                 <script>new WebSocket('ws://127.0.0.1:{port}/socket')</script>"
            ),
        ),
        "/next.html" => (
            "200 OK",
            html,
            String::new(),
            "<!doctype html><title>Second</title>\
             <link rel=stylesheet href=\"/next.css\"><h1>Second</h1>"
                .to_owned(),
        ),
        "/first.css" | "/next.css" => ("200 OK", "text/css", String::new(), "h1{}".to_owned()),
        // The socket is refused: it was opened, which is all the record says.
        _ => ("404 Not Found", "text/plain", String::new(), String::new()),
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
                let (status, kind, header, body) = answer(&path, port);
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     {header}Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// Whether the server has received every one of `awaited`.
fn received_all(log: &Log, awaited: &[(&str, &str)]) -> bool {
    let seen = log.lock().map(|log| log.clone()).unwrap_or_default();
    awaited
        .iter()
        .all(|(host, path)| seen.iter().any(|(h, p)| h == host && p == path))
}

/// The tab read again and again — each read answering every request paused
/// and every socket told of meanwhile — until the server has received every
/// one of `awaited` and the tab has been told of `sockets` sockets.
fn read_until(
    tab: &mut Tab<'_>,
    log: &Log,
    awaited: &[(&str, &str)],
    sockets: usize,
) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    loop {
        tab.read().map_err(|why| why.to_string())?;
        if received_all(log, awaited) && tab.sockets().len() >= sockets {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "never received {awaited:?}: {log:?}, {:?}",
                tab.sockets()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A line as an assertion reads it: method, address, purpose and what
/// happened.
fn said(entry: &Entry) -> (String, String, Purpose, Happened) {
    (
        entry.method().to_owned(),
        entry.url().to_string(),
        entry.purpose().clone(),
        entry.happened().clone(),
    )
}

/// The one line for `url`, or what the lines were.
fn line<'r>(lines: &[&'r Entry], url: &str) -> Result<&'r Entry, String> {
    match lines
        .iter()
        .filter(|entry| entry.url().to_string() == url)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [only] => Ok(only),
        found => Err(format!("{} lines for {url}: {lines:#?}", found.len())),
    }
}

#[test]
fn a_redirect_a_sub_resource_and_a_websocket_are_each_a_line_with_its_cause() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let start = alo_url::parse(&format!("http://127.0.0.1:{port}/start")).unwrap();
    let mut tab = Tab::open(&mut chrome, &start, SIZE).unwrap();
    read_until(
        &mut tab,
        &log,
        &[("localhost", "/first.css"), ("127.0.0.1", "/socket")],
        1,
    )
    .unwrap();
    drop(tab);

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    let page = format!("http://localhost:{port}/page.html");

    // Each hop of the navigation is a line of its own, in order, and both
    // are the person's: a redirect is whatever asked for the first hop.
    let (Some(first), Some(second)) = (lines.first(), lines.get(1)) else {
        panic!("no navigation in {lines:#?}");
    };
    assert_eq!(
        said(first),
        (
            "GET".to_owned(),
            start.to_string(),
            Purpose::Document,
            Happened::LetGo
        )
    );
    assert_eq!(
        said(second),
        ("GET".to_owned(), page, Purpose::Document, Happened::LetGo)
    );
    let Cause::Person { tab: person } = first.cause() else {
        panic!("the first hop is not the person's: {first}");
    };
    assert_eq!(second.cause(), first.cause());
    assert_eq!(
        record.documents().remembered(),
        1,
        "two hops, one document: the redirect minted none"
    );

    // The style sheet is the page's, whose load the person caused.
    let sheet = line(&lines, &format!("http://localhost:{port}/first.css")).unwrap();
    assert_eq!(
        said(sheet),
        (
            "GET".to_owned(),
            format!("http://localhost:{port}/first.css"),
            Purpose::Style,
            Happened::LetGo
        )
    );
    let Cause::Document { document } = sheet.cause() else {
        panic!("the sheet is not the page's: {sheet}");
    };
    assert_eq!(
        record.documents().cause_of(*document),
        Some(&Cause::Person { tab: *person })
    );
    let chain = sheet.chain(record.documents());
    assert_eq!(chain.end(), End::Person(*person));
    assert_eq!(
        chain.links().len(),
        2,
        "the sheet, then the page: {chain:?}"
    );

    // The WebSocket was opened, and not decided, and is the page's too.
    let socket = line(&lines, &format!("ws://127.0.0.1:{port}/socket")).unwrap();
    assert_eq!(
        said(socket),
        (
            "GET".to_owned(),
            format!("ws://127.0.0.1:{port}/socket"),
            Purpose::Unnamed,
            Happened::Undecided
        )
    );
    assert_eq!(socket.cause(), sheet.cause());
    assert_eq!(
        socket.to_string(),
        format!(
            "GET ws://127.0.0.1:{port}/socket (a kind this engine has no word for), caused by \
             {document} — opened, and not decided by this browser"
        )
    );

    // Nothing else but the favicon Chrome asks for on its own account,
    // which is the page's — never a line nobody can account for.
    for entry in &lines {
        let chain = entry.chain(record.documents());
        assert_eq!(chain.end(), End::Person(*person), "{entry}");
        assert!(chain.action().is_none(), "no agent here: {entry}");
    }
    let accounted = [
        start.to_string(),
        format!("http://localhost:{port}/page.html"),
        format!("http://localhost:{port}/first.css"),
        format!("ws://127.0.0.1:{port}/socket"),
        format!("http://localhost:{port}/favicon.ico"),
    ];
    for entry in &lines {
        assert!(
            accounted.contains(&entry.url().to_string()),
            "a line for {entry}"
        );
    }
}

#[test]
fn an_agents_activate_that_navigates_names_the_action_as_its_cause() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let page = alo_url::parse(&format!("http://localhost:{port}/page.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &page, SIZE).unwrap();
    read_until(&mut tab, &log, &[("localhost", "/first.css")], 1).unwrap();

    let outcome = tab
        .act(&Target::Named("Next".to_owned()), &Verb::Activate)
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Followed { to, .. } if to.ends_with("/next.html")),
        "{outcome:?}"
    );
    let action = tab.acted().unwrap();
    read_until(&mut tab, &log, &[("localhost", "/next.css")], 1).unwrap();
    drop(tab);

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    // The second page's navigation is the agent's action, in the page it
    // acted on — whenever Chrome paused it, which may be after the verb
    // had answered.
    let next = line(&lines, &format!("http://localhost:{port}/next.html")).unwrap();
    assert_eq!(next.purpose(), &Purpose::Document);
    let Cause::Agent {
        action: named,
        document: acted_in,
    } = next.cause()
    else {
        panic!("the navigation is not the agent's: {next}");
    };
    assert_eq!(*named, action);
    let first_sheet = line(&lines, &format!("http://localhost:{port}/first.css")).unwrap();
    assert_eq!(
        first_sheet.cause(),
        &Cause::Document {
            document: *acted_in
        },
        "the action was in the first page"
    );

    // What the second page asked for is the second page's, and leads back
    // to the action, and through the first page to the person.
    let sheet = line(&lines, &format!("http://localhost:{port}/next.css")).unwrap();
    let Cause::Document { document } = sheet.cause() else {
        panic!("the second sheet is not its page's: {sheet}");
    };
    assert_eq!(record.documents().cause_of(*document), Some(next.cause()));
    let chain = sheet.chain(record.documents());
    assert!(chain.followed_from(action), "{chain:?}");
    assert!(chain.person().is_some(), "{chain:?}");
    assert!(chain.is_whole(), "{chain:?}");

    // Nothing asked for before the action names it.
    for entry in lines
        .iter()
        .filter(|entry| entry.sequence() < next.sequence())
    {
        let chain = entry.chain(record.documents());
        assert!(!chain.followed_from(action), "{entry}");
    }
}
