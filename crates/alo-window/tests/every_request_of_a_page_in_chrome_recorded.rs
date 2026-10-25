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
//!
//! A third page holds a frame from its own site and one from the other,
//! each of which fetches, and the other-site frame starts a worker that
//! fetches too (queue item 636): each frame is a document of its own,
//! caused by the page, and each `fetch` is its frame's document's — the
//! worker's that of the frame that started it. When the worker has
//! fetched, the other-site frame goes to a second page of its own, whose
//! load is the first frame document's.
//!
//! A tab an agent asks to open at `/start` (ADR 0063, queue item 638) is
//! recorded as the agent's action done in that tab: both hops of its first
//! page carry that cause, what the page asks for is the page's and leads
//! back to the action, no line names the person, and an ask refused before
//! it mints no action. A tab the person then opens is still the person's.
//!
//! A fourth page holds three frames that are each sent to `mailto:`, a
//! scheme Chrome never pauses (queue item 651): one from the page's own site
//! and one from the other each sending itself, and one from the other sent
//! there by the page. Chrome tells of the first and third through the tab's
//! session and of the second through the frame's own target; each is
//! refused by name, and its line's cause is the document the frame holds.

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_net::activity::{Entry, Happened};
use alo_net::request::Purpose;
use alo_net::{Cause, Place, chain::End};
use alo_renderer::navigate::Rule;
use alo_window::agent_sites::AgentSites;
use alo_window::asking::Refused;
use alo_window::chrome::Chrome;
use alo_window::chrome_gate::Refusal;
use alo_window::chrome_tab::{Tab, Unopened};
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

/// The server's answer to `path` when it is one of the page whose frames
/// are sent to `mailto:`, on port `port`.
fn answer_mailto(path: &str, port: u16) -> Option<(&'static str, &'static str, String, String)> {
    let html = "text/html; charset=utf-8";
    Some(match path {
        "/mailto-framed.html" => (
            "200 OK",
            html,
            String::new(),
            format!(
                "<!doctype html><title>Framed</title>\
                 <iframe src=\"/mailto-same.html\"></iframe>\
                 <iframe src=\"http://localhost:{port}/mailto-other.html\"></iframe>\
                 <iframe src=\"http://localhost:{port}/quiet.html\" \
                 onload=\"this.onload = null; this.src = 'mailto:quiet@example.com'\"></iframe>\
                 <script>fetch('/from-page')</script>"
            ),
        ),
        "/mailto-same.html" | "/mailto-other.html" => {
            let which = path
                .trim_start_matches("/mailto-")
                .trim_end_matches(".html");
            (
                "200 OK",
                html,
                String::new(),
                format!(
                    "<!doctype html><script>addEventListener('load', () => \
                     fetch('/from-{which}').then(() => \
                     location.href = 'mailto:{which}@example.com'))</script>"
                ),
            )
        }
        "/quiet.html" => (
            "200 OK",
            html,
            String::new(),
            "<!doctype html><script>fetch('/from-quiet')</script>".to_owned(),
        ),
        _ => return None,
    })
}

/// The server's answer to `path`, on port `port`: its status, type, the
/// header it adds and its body.
fn answer(path: &str, port: u16) -> (&'static str, &'static str, String, String) {
    if let Some(answer) = answer_mailto(path, port) {
        return answer;
    }
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
        "/framed.html" => (
            "200 OK",
            html,
            String::new(),
            format!(
                "<!doctype html><title>Framed</title>\
                 <iframe src=\"/same.html\"></iframe>\
                 <iframe src=\"http://localhost:{port}/other.html\"></iframe>\
                 <script>fetch('/from-page')</script>"
            ),
        ),
        "/same.html" => (
            "200 OK",
            html,
            String::new(),
            "<!doctype html><script>fetch('/from-same')</script>".to_owned(),
        ),
        "/other.html" => (
            "200 OK",
            html,
            String::new(),
            "<!doctype html><script>fetch('/from-other');\
             new Worker('/worker.js').onmessage = () => location = '/other2.html'</script>"
                .to_owned(),
        ),
        "/other2.html" => (
            "200 OK",
            html,
            String::new(),
            "<!doctype html><script>fetch('/from-other2')</script>".to_owned(),
        ),
        "/worker.js" => (
            "200 OK",
            "text/javascript",
            String::new(),
            "fetch('/from-worker').then(() => postMessage(1))".to_owned(),
        ),
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
        place: Place::Document(acted_in),
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

#[test]
fn a_tab_an_agent_opened_is_its_action_in_that_tab_and_never_the_persons() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let here = format!("http://127.0.0.1:{port}");
    let there = format!("http://localhost:{port}");
    let sites = AgentSites::named([here.as_str(), there.as_str()]).unwrap();

    // An ask refused before it is accepted mints no action.
    match Tab::open_for_agent(&mut chrome, "http://example.com/", &sites, SIZE) {
        Err(Unopened::Refused(Refused::NotGranted { .. })) => {}
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(_) => panic!("opened a site the agent was not given"),
    }

    let start = format!("{here}/start");
    let mut tab = Tab::open_for_agent(&mut chrome, &start, &sites, SIZE).unwrap();
    let action = tab.opened_by().unwrap();
    assert_eq!(action.as_u64(), 0, "the refused ask minted an action");
    assert_eq!(tab.acted(), None, "opening is not a verb");
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
    // Both hops of the first page are the action's, done in the tab.
    let (Some(first), Some(second)) = (lines.first(), lines.get(1)) else {
        panic!("no navigation in {lines:#?}");
    };
    assert_eq!(first.url().to_string(), start);
    assert_eq!(second.url().to_string(), format!("{there}/page.html"));
    let Cause::Agent {
        action: named,
        place: Place::Tab(opened),
    } = first.cause()
    else {
        panic!("the first hop is not the agent's opening: {first}");
    };
    assert_eq!(*named, action);
    assert_eq!(
        second.cause(),
        first.cause(),
        "a redirect is the first hop's"
    );
    assert!(
        first
            .to_string()
            .contains(&format!("caused by {action}, in {opened}")),
        "{first}"
    );

    // What the page asked for is the page's, and leads back to the opening
    // and to no person.
    let sheet = line(&lines, &format!("{there}/first.css")).unwrap();
    let Cause::Document { document } = sheet.cause() else {
        panic!("the sheet is not the page's: {sheet}");
    };
    assert_eq!(record.documents().cause_of(*document), Some(first.cause()));
    let chain = sheet.chain(record.documents());
    assert_eq!(chain.end(), End::Opened(action, *opened), "{chain:?}");
    assert_eq!(chain.action(), Some(action));
    assert_eq!(chain.person(), None, "{chain:?}");
    assert!(chain.is_whole(), "{chain:?}");
    for entry in &lines {
        let chain = entry.chain(record.documents());
        assert_eq!(chain.end(), End::Opened(action, *opened), "{entry}");
        assert!(chain.followed_from(action), "{entry}");
    }

    // A tab the person opens is still the person's.
    let opened = *opened;
    let sequence = lines.iter().map(|entry| entry.sequence()).max();
    let next = alo_url::parse(&format!("{there}/next.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &next, SIZE).unwrap();
    assert_eq!(tab.opened_by(), None);
    read_until(&mut tab, &log, &[("localhost", "/next.css")], 0).unwrap();
    drop(tab);
    let record = chrome.record();
    let theirs: Vec<&Entry> = record
        .activity()
        .entries()
        .filter(|entry| Some(entry.sequence()) > sequence)
        .collect();
    let page = line(&theirs, &next.to_string()).unwrap();
    let Cause::Person { tab: person } = page.cause() else {
        panic!("the person's page is not theirs: {page}");
    };
    assert_ne!(*person, opened);
    for entry in &theirs {
        let chain = entry.chain(record.documents());
        assert_eq!(chain.end(), End::Person(*person), "{entry}");
        assert!(chain.action().is_none(), "no agent here: {entry}");
    }
}

#[test]
fn a_frames_fetch_is_its_own_documents_and_that_document_the_pages() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let framed = alo_url::parse(&format!("http://127.0.0.1:{port}/framed.html")).unwrap();
    let mut tab = Tab::open(&mut chrome, &framed, SIZE).unwrap();
    read_until(
        &mut tab,
        &log,
        &[
            ("127.0.0.1", "/from-page"),
            ("127.0.0.1", "/from-same"),
            ("localhost", "/from-other"),
            ("localhost", "/from-worker"),
            ("localhost", "/from-other2"),
        ],
        0,
    )
    .unwrap();
    drop(tab);

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    let at = |host: &str, path: &str| line(&lines, &format!("http://{host}:{port}{path}"));
    let document = |entry: &Entry| match entry.cause() {
        Cause::Document { document } => *document,
        other => panic!("{entry} is not a document's: {other}"),
    };

    // The page is the person's, and what it asks for its own.
    let page_line = at("127.0.0.1", "/framed.html").unwrap();
    let Cause::Person { tab: person } = page_line.cause() else {
        panic!("the page is not the person's: {page_line}");
    };
    let page = document(at("127.0.0.1", "/from-page").unwrap());
    assert_eq!(
        record.documents().cause_of(page),
        Some(&Cause::Person { tab: *person })
    );

    // Each frame's document is loaded by the page, through the page's own
    // session for the other-site frame as for the same-site one.
    for (host, path) in [("127.0.0.1", "/same.html"), ("localhost", "/other.html")] {
        let frame = at(host, path).unwrap();
        assert_eq!(frame.purpose(), &Purpose::Document, "{frame}");
        assert_eq!(
            frame.cause(),
            &Cause::Document { document: page },
            "{frame}"
        );
    }

    // Each frame's fetch is its own document's, which the page caused.
    let same = document(at("127.0.0.1", "/from-same").unwrap());
    let other = document(at("localhost", "/from-other").unwrap());
    assert_ne!(same, page);
    assert_ne!(other, page);
    assert_ne!(same, other);
    for frame in [same, other] {
        assert_eq!(
            record.documents().cause_of(frame),
            Some(&Cause::Document { document: page })
        );
    }
    let chain = at("localhost", "/from-other")
        .unwrap()
        .chain(record.documents());
    assert_eq!(
        chain.links(),
        &[
            Cause::Document { document: other },
            Cause::Document { document: page },
            Cause::Person { tab: *person },
        ],
        "the frame's fetch, the frame, the page"
    );

    // The worker's script and its fetch are the frame's that started it.
    let worker = at("localhost", "/worker.js").unwrap();
    assert_eq!(worker.cause(), &Cause::Document { document: other });
    assert_eq!(
        at("localhost", "/from-worker").unwrap().cause(),
        &Cause::Document { document: other }
    );

    // The frame going elsewhere is its document's doing, as a page's own
    // navigation is — which it is only if the frame's commit, told of in
    // its own target, was read — and the new document's fetch its own.
    let second = at("localhost", "/other2.html").unwrap();
    assert_eq!(second.cause(), &Cause::Document { document: other });
    let next = document(at("localhost", "/from-other2").unwrap());
    assert_ne!(next, other);
    assert_eq!(
        record.documents().cause_of(next),
        Some(&Cause::Document { document: other })
    );

    // Four documents — the page, its two frames and the second frame's
    // second — and every line leads back, whole, to the person.
    assert_eq!(record.documents().remembered(), 4);
    for entry in &lines {
        let chain = entry.chain(record.documents());
        assert!(chain.is_whole(), "{entry}: {chain:?}");
        assert_eq!(chain.end(), End::Person(*person), "{entry}");
    }
}

/// The page whose frames are sent to `mailto:`, opened in `chrome` and read
/// until each frame has fetched and three asks were refused: the
/// refusals, in the order of what was asked.
fn refusals_of_frames(chrome: &mut Chrome, port: u16, log: &Log) -> Result<Vec<Refusal>, String> {
    let framed = alo_url::parse(&format!("http://127.0.0.1:{port}/mailto-framed.html"))
        .map_err(|why| why.why)?;
    let mut tab = Tab::open(chrome, &framed, SIZE).map_err(|why| why.to_string())?;
    read_until(
        &mut tab,
        log,
        &[
            ("127.0.0.1", "/from-page"),
            ("127.0.0.1", "/from-same"),
            ("localhost", "/from-other"),
            ("localhost", "/from-quiet"),
        ],
        0,
    )?;
    let until = Instant::now() + AT_MOST;
    while tab.refusals().len() < 3 && Instant::now() < until {
        tab.read().map_err(|why| why.to_string())?;
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut refusals = tab.refusals().to_vec();
    refusals.sort_by(|a, b| a.asked.cmp(&b.asked));
    Ok(refusals)
}

#[test]
fn a_frame_sent_to_a_scheme_chrome_never_pauses_is_refused_and_its_documents() {
    let (port, log) = serve().unwrap();
    let mut chrome = Chrome::start().unwrap();
    let refusals = refusals_of_frames(&mut chrome, port, &log).unwrap();
    let asked = |who: &str| format!("mailto:{who}@example.com");
    let mailto = Rule::Scheme {
        scheme: "mailto".to_owned(),
    };
    assert_eq!(
        refusals,
        ["other", "quiet", "same"].map(|who| Refusal {
            asked: asked(who),
            rule: mailto.clone(),
        }),
        "each frame's ask refused by name"
    );

    let record = chrome.record();
    let lines: Vec<&Entry> = record.activity().entries().collect();
    let at = |host: &str, path: &str| line(&lines, &format!("http://{host}:{port}{path}"));
    let document = |entry: &Entry| match entry.cause() {
        Cause::Document { document } => *document,
        other => panic!("{entry} is not a document's: {other}"),
    };
    let page = document(at("127.0.0.1", "/from-page").unwrap());
    for (who, host) in [
        ("same", "127.0.0.1"),
        ("other", "localhost"),
        ("quiet", "localhost"),
    ] {
        // The frame's document, read from what it fetched: a frame's own,
        // which the page caused.
        let frame = document(at(host, &format!("/from-{who}")).unwrap());
        assert_ne!(frame, page, "{who}");
        assert_eq!(
            record.documents().cause_of(frame),
            Some(&Cause::Document { document: page }),
            "{who}"
        );
        // Chrome names the frame that is to go and never who asked, so the
        // page sending its frame away is the frame's document's line too.
        let refused = line(&lines, &asked(who)).unwrap();
        assert_eq!(
            said(refused),
            (
                "GET".to_owned(),
                asked(who),
                Purpose::Document,
                Happened::Refused {
                    rule: mailto.to_string()
                },
            ),
            "{who}"
        );
        assert_eq!(
            refused.cause(),
            &Cause::Document { document: frame },
            "{who}"
        );
    }

    // No frame went: four documents, the page and its three frames, and
    // every line leads back to the person.
    assert_eq!(record.documents().remembered(), 4);
    let Some(person) = lines
        .first()
        .map(|entry| entry.chain(record.documents()).end())
    else {
        panic!("no lines");
    };
    for entry in &lines {
        let chain = entry.chain(record.documents());
        assert!(chain.is_whole(), "{entry}: {chain:?}");
        assert_eq!(chain.end(), person, "{entry}");
    }
    assert!(matches!(person, End::Person(_)), "{person:?}");
}
