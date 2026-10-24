/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page from the network, opened in Chrome, every request held to the
//! browser process's rules: queue item 633, ADR 0060 §§ 2–3.
//!
//! The network is a server of this test's own on this machine, reached by
//! two names so that it is two sites — `127.0.0.1` and `localhost` — and it
//! writes down every request it receives. Its page holds a frame from the
//! other site that fetches, a dedicated worker that fetches, and a service
//! worker that fetches as it installs: each of the three is a target Chrome
//! would let ask for things the tab's own session never sees.
//!
//! **Paused before the server answered**, asserted from the server's side:
//! a request Chrome pauses goes nowhere until it is let go, and the tab
//! writes a request down as it lets it go, so a request the server received
//! that the tab did not let go is a request that escaped. None did.

use alo_net::Cause;
use alo_net::activity::Happened;
use alo_renderer::Snapshot;
use alo_renderer::navigate::Rule;
use alo_renderer::site::Site;
use alo_window::agent_sites::AgentSites;
use alo_window::asking::Refused;
use alo_window::chrome::Chrome;
use alo_window::chrome_gate::Refusal;
use alo_window::chrome_hold::Kind;
use alo_window::chrome_tab::{Tab, Unopened, Went};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long the page's frame and workers are waited for.
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
        "/page.html" => (
            html,
            format!(
                "<!doctype html><title>From the network</title><h1>From the network</h1>\
                 <iframe src=\"http://localhost:{port}/frame.html\"></iframe>\
                 <script>new Worker('/worker.js');\
                 navigator.serviceWorker.register('/sw.js');</script>"
            ),
        ),
        "/frame.html" => (html, "<script>fetch('/from-frame')</script>".to_owned()),
        "/worker.js" => (script, "fetch('/from-worker')".to_owned()),
        "/sw.js" => (
            script,
            "self.addEventListener('install', \
             (e) => e.waitUntil(fetch('/from-service-worker')))"
                .to_owned(),
        ),
        "/shared.html" => (
            html,
            "<!doctype html><title>Shared</title><h1>Shared</h1>\
             <script>new SharedWorker('/shared.js').port.start();\
             new SharedWorker('/shared.js');</script>"
                .to_owned(),
        ),
        "/shared.js" => (
            script,
            format!(
                "fetch('/from-shared'); importScripts('/imported.js');\
                 new WebSocket('ws://127.0.0.1:{port}/socket');\
                 onconnect = () => fetch('/on-connect');"
            ),
        ),
        "/imported.js" => (script, "fetch('/from-imported')".to_owned()),
        "/mailto.html" => (
            html,
            "<!doctype html><title>Still here</title><h1>Still here</h1>\
             <script>addEventListener('load', () => setTimeout(() => \
             location.href = 'mailto:someone@example.com'))</script>"
                .to_owned(),
        ),
        "/from-frame"
        | "/from-worker"
        | "/from-service-worker"
        | "/from-shared"
        | "/from-imported"
        | "/on-connect" => ("text/plain", "fetched".to_owned()),
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
                // A connection Chrome opened ahead of need and closed unused
                // asked for nothing.
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

/// Whether `snapshot` holds `words` anywhere a person would read them.
fn reads(snapshot: &Snapshot, words: &str) -> bool {
    snapshot
        .nodes()
        .iter()
        .any(|node| node.to_string().contains(words))
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
            return Err(format!("never received {missing:?}; received {seen:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_page_from_the_network_is_read_and_every_request_was_paused_before_it_went() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/page.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
    let awaited = [
        ("127.0.0.1", "/page.html"),
        ("localhost", "/frame.html"),
        ("localhost", "/from-frame"),
        ("127.0.0.1", "/worker.js"),
        ("127.0.0.1", "/from-worker"),
        ("127.0.0.1", "/sw.js"),
        ("127.0.0.1", "/from-service-worker"),
    ];
    let snapshot = read_until(&mut tab, &log, &awaited).unwrap();
    assert!(reads(&snapshot, "From the network"), "{snapshot:?}");

    // Every request the server received, the tab let go first: none
    // escaped, from any target.
    let went = let_go_first(&tab, &log);
    // And each was paused in the target that asked for it: the frame from
    // the other site and the service worker in their own, the dedicated
    // worker's in the page's (measured: it has no `Fetch` of its own).
    let paused_in = |host: &str, path: &str, kind: Kind| {
        went.iter().any(|(went, by)| {
            *by == kind && went.as_ref() == Some(&(host.to_owned(), path.to_owned()))
        })
    };
    assert!(paused_in("127.0.0.1", "/page.html", Kind::Page), "{went:?}");
    assert!(
        paused_in("localhost", "/frame.html", Kind::Page),
        "{went:?}"
    );
    assert!(
        paused_in("localhost", "/from-frame", Kind::Frame),
        "{went:?}"
    );
    assert!(paused_in("127.0.0.1", "/worker.js", Kind::Page), "{went:?}");
    assert!(
        paused_in("127.0.0.1", "/from-worker", Kind::Page),
        "{went:?}"
    );
    assert!(
        paused_in("127.0.0.1", "/sw.js", Kind::ServiceWorker),
        "{went:?}"
    );
    assert!(
        paused_in("127.0.0.1", "/from-service-worker", Kind::ServiceWorker),
        "{went:?}"
    );
    assert_eq!(tab.refusals(), &[], "nothing here breaks a rule");
}

#[test]
fn a_pages_shared_worker_is_held_and_every_request_of_it_paused_before_it_went() {
    // Chrome tells the tab's session nothing of a shared worker, measured
    // against Chrome 155; Chrome's own session is told of it, waiting, and
    // the tab holds it there.
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/shared.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
    let awaited = [
        ("127.0.0.1", "/shared.js"),
        ("127.0.0.1", "/from-shared"),
        ("127.0.0.1", "/imported.js"),
        ("127.0.0.1", "/from-imported"),
        ("127.0.0.1", "/on-connect"),
        ("127.0.0.1", "/socket"),
    ];
    let snapshot = read_until(&mut tab, &log, &awaited).unwrap();
    assert!(reads(&snapshot, "Shared"), "{snapshot:?}");
    let went = let_go_first(&tab, &log);
    let paused_in = |path: &str, kind: Kind| {
        went.iter().any(|(went, by)| {
            *by == kind && went.as_ref() == Some(&("127.0.0.1".to_owned(), path.to_owned()))
        })
    };
    // Its script is fetched for it by the page that started it; everything
    // it asks for itself is paused in its own target.
    assert!(paused_in("/shared.js", Kind::Page), "{went:?}");
    for path in [
        "/from-shared",
        "/imported.js",
        "/from-imported",
        "/on-connect",
    ] {
        assert!(paused_in(path, Kind::SharedWorker), "{path}: {went:?}");
    }
    // One worker, though two were asked for: it was connected to twice.
    let started = went
        .iter()
        .filter(|(went, _)| {
            went.as_ref()
                .is_some_and(|(_, path)| path == "/from-shared")
        })
        .count();
    assert_eq!(started, 1, "{went:?}");
    // Its WebSocket cannot be held, and is told of in its own target.
    assert_eq!(
        tab.sockets(),
        &[format!("ws://127.0.0.1:{port}/socket")],
        "{went:?}"
    );
    assert_eq!(tab.refusals(), &[], "nothing here breaks a rule");
    drop(tab);

    // And each is in the record as the page's: what a shared worker asks
    // for is the document's that started it, as a dedicated worker's is.
    let record = chrome.record();
    let page = record
        .activity()
        .entries()
        .find(|entry| entry.url().to_string() == address.to_string())
        .map(|entry| entry.cause().clone());
    assert!(
        matches!(page, Some(Cause::Person { .. })),
        "the page is not the person's: {page:?}"
    );
    for (path, happened) in [
        ("/from-shared", Happened::LetGo),
        ("/from-imported", Happened::LetGo),
        ("/socket", Happened::Undecided),
    ] {
        let scheme = if path == "/socket" { "ws" } else { "http" };
        let url = format!("{scheme}://127.0.0.1:{port}{path}");
        let lines: Vec<_> = record
            .activity()
            .entries()
            .filter(|entry| entry.url().to_string() == url)
            .collect();
        let [line] = lines.as_slice() else {
            panic!("{} lines for {url}", lines.len());
        };
        assert_eq!(line.happened(), &happened, "{line}");
        let Cause::Document { document } = line.cause() else {
            panic!("not the page's: {line}");
        };
        assert_eq!(record.documents().cause_of(*document), page.as_ref());
    }
}

/// Every request the server received, checked to have been let go by `tab`
/// at least as many times — but a WebSocket's handshake, which Chrome does
/// not let be held and the tab was told of (ADR 0060 § 3): what the tab let
/// go, and in which target.
fn let_go_first(tab: &Tab<'_>, log: &Log) -> Vec<(Option<(String, String)>, Kind)> {
    let went: Vec<(Option<(String, String)>, Kind)> = tab
        .went()
        .iter()
        .map(|Went { url, by }| (host_and_path(url), *by))
        .collect();
    let sockets: Vec<_> = tab
        .sockets()
        .iter()
        .filter_map(|url| host_and_path(url))
        .collect();
    let seen = received(log);
    for request in seen.iter().filter(|request| !sockets.contains(request)) {
        let received = seen.iter().filter(|other| *other == request).count();
        let let_go = went
            .iter()
            .filter(|(went, _)| went.as_ref() == Some(request))
            .count();
        assert!(
            let_go >= received,
            "{request:?} reached the server {received} times and was let go {let_go}: \
             {went:?}"
        );
    }
    went
}

#[test]
fn a_page_sending_its_tab_to_a_refused_scheme_is_refused_by_name() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/mailto.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &address, SIZE).unwrap();
    let until = Instant::now() + AT_MOST;
    let mut snapshot = tab.read().unwrap();
    while tab.refusals().is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        snapshot = tab.read().unwrap();
    }
    assert_eq!(
        tab.refusals(),
        &[Refusal {
            asked: "mailto:someone@example.com".to_owned(),
            rule: Rule::Scheme {
                scheme: "mailto".to_owned()
            },
        }]
    );
    assert_eq!(tab.refused(), &["mailto:someone@example.com".to_owned()]);
    // And the tab is where it was.
    assert!(reads(&snapshot, "Still here"), "{snapshot:?}");
    assert_eq!(
        received(&log)
            .iter()
            .filter(|(_, path)| path != "/favicon.ico")
            .collect::<Vec<_>>(),
        [&("127.0.0.1".to_owned(), "/mailto.html".to_owned())]
    );
}

#[test]
fn an_agent_naming_a_site_it_was_not_given_is_refused_before_chrome_hears_of_it() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let given = format!("http://127.0.0.1:{port}");
    let sites = AgentSites::named([given.as_str()]).unwrap();
    let elsewhere = format!("http://localhost:{port}/page.html");

    let before = chrome.written();
    match Tab::open_for_agent(&mut chrome, &elsewhere, &sites, SIZE) {
        Err(Unopened::Refused(Refused::NotGranted { tab, site })) => {
            assert_eq!(tab, elsewhere);
            assert_eq!(site, Site::of(&alo_url::parse(&elsewhere).unwrap()));
            assert_eq!(site.to_string(), "http://localhost");
        }
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(_) => panic!("opened a site the agent was not given"),
    }
    match Tab::open_for_agent(&mut chrome, "not an address", &sites, SIZE) {
        Err(Unopened::Refused(Refused::NotAnAddress { tab })) => {
            assert_eq!(tab, "not an address");
        }
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(_) => panic!("opened what is not an address"),
    }
    // A person's own address that is not on the network is not this.
    let data = alo_url::parse("data:text/html,<p>hi").unwrap();
    match Tab::open(&mut chrome, &data, SIZE) {
        Err(Unopened::NotTheNetwork { scheme }) => assert_eq!(scheme, "data"),
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(_) => panic!("opened a data: address as a page from the network"),
    }
    assert_eq!(chrome.written(), before, "not one command was written");
    assert_eq!(received(&log), [], "nothing was asked of the server");

    // The site it was given opens.
    let mut tab =
        Tab::open_for_agent(&mut chrome, &format!("{given}/page.html"), &sites, SIZE).unwrap();
    let snapshot = tab.read().unwrap();
    assert!(reads(&snapshot, "From the network"), "{snapshot:?}");
}

#[test]
fn a_frozen_pages_service_worker_is_held_and_reaches_no_server() {
    // A frozen page served at the address of a real server, as a corpus
    // case is: the bytes are the tab's, and the service worker it registers
    // would be fetched from the server. Held, its script is paused in its
    // own target and refused with every other request of a frozen page.
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let address = format!("http://127.0.0.1:{port}/frozen.html");
    let page = b"<!doctype html><title>Frozen</title><h1>Frozen</h1>\
                 <script>navigator.serviceWorker.register('/sw.js')</script>";
    let mut tab = Tab::open_frozen(&mut chrome, &address, page.to_vec(), SIZE).unwrap();
    let script = format!("http://127.0.0.1:{port}/sw.js");
    let until = Instant::now() + AT_MOST;
    let mut snapshot = tab.read().unwrap();
    while !tab.refused().contains(&script) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        snapshot = tab.read().unwrap();
    }
    assert!(reads(&snapshot, "Frozen"), "{snapshot:?}");
    assert!(tab.refused().contains(&script), "{:?}", tab.refused());
    assert_eq!(received(&log), [], "the server was asked nothing");
}
