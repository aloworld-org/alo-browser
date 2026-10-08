/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 348: a page's linked style sheet, asked for, decided, made and
//! delivered (ADR 0035 §§ 1–4).
//!
//! A page served from one local server links sheets from it and from a
//! second, which is another origin because it is another port. The page is
//! loaded into real [`Tabs`] over the confined `alo-render` binary, and every
//! sheet it asks for is decided, made through a real [`Pool`] and delivered
//! by [`alo_renderer::fetch_answering`], one at a time, each a task of its
//! own. Then three things are read back: what the page painted, what each
//! server was sent, and the session's record.
//!
//! The renderers are started with no fonts, so what a sheet did is read in
//! pixels: each sheet colours one 10-pixel band, and a band a sheet did not
//! reach is white.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alo_layout::Size;
use alo_net::activity::Happened;
use alo_net::cause::Cause;
use alo_net::cors::{Credentials, Mode};
use alo_net::csp::Policies;
use alo_net::{Pool, Purpose, Trust};
use alo_renderer::fetch_answering::Answering;
use alo_renderer::fetch_decide::Asker;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::sheet::{SheetAnswer, SheetAsk};
use alo_renderer::sheet_decide::{self, Decided};
use alo_renderer::sheet_make;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{Frame, FromRenderer, Page};

/// One request a server was sent: its request line and its headers.
type Heard = Arc<Mutex<Vec<String>>>;

/// Which of the two servers is answering.
#[derive(Clone, Copy)]
enum Server {
    /// The page's own origin.
    Home,
    /// Another origin.
    Other,
}

/// A port on this machine to listen on, and its origin.
fn listen() -> Option<(TcpListener, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    Some((listener, format!("http://127.0.0.1:{port}")))
}

/// Answer on `listener` as `server`, the page being at `home`: what it heard.
fn serve(listener: TcpListener, server: Server, home: String) -> Heard {
    let heard: Heard = Arc::new(Mutex::new(Vec::new()));
    let keeping = Arc::clone(&heard);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else {
                return;
            };
            answer_one(socket, server, &home, &keeping);
        }
    });
    heard
}

/// Read one request's head from `socket`, note it, and answer it.
fn answer_one(mut socket: TcpStream, server: Server, home: &str, heard: &Heard) {
    if socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .is_err()
    {
        return;
    }
    let mut asked = Vec::new();
    let mut block = [0u8; 4096];
    while let Ok(got) = socket.read(&mut block) {
        asked.extend_from_slice(block.get(..got).unwrap_or_default());
        if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&asked).into_owned();
    let request_head = text.split("\r\n\r\n").next().unwrap_or_default().to_owned();
    let line = request_head.lines().next().unwrap_or_default().to_owned();
    if let Ok(mut heard) = heard.lock() {
        heard.push(request_head);
    }
    let _ = socket.write_all(route(server, home, &line).as_bytes());
    let _ = socket.flush();
}

/// A whole response: `status`, `headers`, and `body`.
fn reply(status: &str, headers: &[String], body: &str) -> String {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for header in headers {
        out.push_str(header);
        out.push_str("\r\n");
    }
    let _ = write!(
        out,
        "Cache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    out
}

/// A style sheet, as a server sends one.
fn css(body: &str) -> String {
    reply("200 OK", &["Content-Type: text/css".to_owned()], body)
}

/// What each server says to each request line. Every sheet colours its own
/// band; the two that must never cross would colour theirs black.
fn route(server: Server, home: &str, line: &str) -> String {
    let agreed = format!("Access-Control-Allow-Origin: {home}");
    match (server, line) {
        (Server::Home, "GET /site.css HTTP/1.1") => css("#own { background: rgb(255, 0, 0) }"),
        (Server::Home, "GET /hop.css HTTP/1.1") => {
            reply("302 Found", &["Location: /moved.css".to_owned()], "")
        }
        (Server::Home, "GET /moved.css HTTP/1.1") => css("#hop { background: rgb(0, 0, 255) }"),
        (Server::Other, "GET /cdn.css HTTP/1.1") => css("#cdn { background: rgb(0, 128, 0) }"),
        // Something else the page named: bytes that would style the page
        // if they ever reached it, sent as what they are.
        (Server::Other, "GET /page.html HTTP/1.1") => reply(
            "200 OK",
            &["Content-Type: text/html".to_owned()],
            "#page { background: rgb(0, 0, 0) }",
        ),
        (Server::Other, "GET /cors.css HTTP/1.1") => reply(
            "200 OK",
            &["Content-Type: text/css".to_owned(), agreed],
            "#cors { background: rgb(255, 255, 0) }",
        ),
        // Asked for with `crossorigin`, and not agreeing to be read.
        (Server::Other, "GET /closed.css HTTP/1.1") => css("#closed { background: rgb(0, 0, 0) }"),
        _ => reply("404 Not Found", &[], "no"),
    }
}

/// The bands, top to bottom, each 10 pixels tall.
const BANDS: [&str; 6] = ["own", "cdn", "page", "cors", "closed", "hop"];

/// The page: every link, and a band for each.
fn page_at(home: &str, other: &str) -> Option<Page> {
    let url = alo_url::parse(&format!("{home}/page")).ok()?;
    let bands = BANDS
        .iter()
        .map(|id| format!("<div id={id}></div>"))
        .collect::<Vec<_>>()
        .concat();
    Some(
        Page::new(
            format!(
                "<!doctype html>\
                 <link rel=stylesheet href=site.css>\
                 <link rel=stylesheet href=/site.css>\
                 <link rel=stylesheet href={other}/cdn.css>\
                 <link rel=stylesheet href={other}/page.html>\
                 <link rel=stylesheet href={other}/cors.css crossorigin>\
                 <link rel=stylesheet href={other}/closed.css crossorigin>\
                 <link rel=stylesheet href=/hop.css>\
                 <body>{bands}<script>var ran = true;</script></body>"
            ),
            Size::new(40.0, 60.0),
        )
        .at(url)
        .with_sheet("body { margin: 0 } div { height: 10px }"),
    )
}

/// Two servers, a tab over the real renderer binary showing the page under
/// `policy` when there is one, and a session's network.
struct Set {
    home: String,
    other: String,
    home_heard: Heard,
    other_heard: Heard,
    tabs: Tabs,
    tab: alo_net::cause::TabId,
    document: alo_net::cause::DocumentId,
    network: Network,
}

fn set_up(policy: Option<&str>) -> Option<Set> {
    let (home_listener, home) = listen()?;
    let (other_listener, other) = listen()?;
    let home_heard = serve(home_listener, Server::Home, home.clone());
    let other_heard = serve(other_listener, Server::Other, home.clone());
    let mut tabs = Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]));
    let tab = tabs.open(alo_url::parse(&format!("{home}/")).ok()?);
    let mut page = page_at(&home, &other)?;
    if let Some(policy) = policy {
        page = page.with_policy(policy);
    }
    let Ok(FromRenderer::Loaded { .. }) = tabs.load(tab, page, Cause::Person { tab }) else {
        return None;
    };
    let document = tabs.tab(tab).and_then(Tab::document)?;
    let pool = Pool::with_trust(Trust::of(&[]).ok()?).patient_for(Duration::from_secs(5));
    Some(Set {
        home,
        other,
        home_heard,
        other_heard,
        tabs,
        tab,
        document,
        network: Network::over(pool),
    })
}

/// Answer every sheet the page asked for, and paint it: the frame, and what
/// the person was told.
fn answer_all(set: &mut Set) -> Option<(Frame, Vec<String>)> {
    let mut answering = Answering::new();
    answering.take_from(&mut set.tabs, set.tab);
    let mut told = Vec::new();
    while let Some(answered) = answering.answer_next(&mut set.tabs, &mut set.network) {
        if answered.tab != set.tab || !matches!(answered.delivered, Ok(Some(_))) {
            return None;
        }
        told.extend(answered.said);
    }
    let Ok(FromRenderer::Painted(frame)) = set.tabs.paint(set.tab) else {
        return None;
    };
    Some((frame, told))
}

/// The colour of band `id`, as painted.
fn band(frame: &Frame, id: &str) -> Option<[u8; 3]> {
    let at = BANDS.iter().position(|band| *band == id)?;
    let y = u32::try_from(at * 10 + 5).ok()?;
    let (red, green, blue, _) = frame.at(20, y)?;
    Some([red, green, blue])
}

/// What one server heard: each request's line.
fn lines(heard: &Heard) -> Vec<String> {
    heard
        .lock()
        .map(|heads| {
            heads
                .iter()
                .map(|head| head.lines().next().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The head of the request whose line starts with `line`.
fn head_of(heard: &Heard, line: &str) -> String {
    heard
        .lock()
        .ok()
        .and_then(|heads| heads.iter().find(|head| head.starts_with(line)).cloned())
        .unwrap_or_default()
}

const WHITE: [u8; 3] = [255, 255, 255];

#[test]
fn a_pages_sheets_are_made_once_each_and_only_style_sheets_reach_it() {
    let mut set = set_up(None).expect("set up");
    let (frame, told) = answer_all(&mut set).expect("answered");

    assert_eq!(
        band(&frame, "own"),
        Some([255, 0, 0]),
        "the page's own sheet"
    );
    assert_eq!(
        band(&frame, "cdn"),
        Some([0, 128, 0]),
        "another origin's, no-cors"
    );
    assert_eq!(
        band(&frame, "cors"),
        Some([255, 255, 0]),
        "one that agreed to be read"
    );
    assert_eq!(
        band(&frame, "hop"),
        Some([0, 0, 255]),
        "one a redirect led to"
    );
    assert_eq!(
        band(&frame, "page"),
        Some(WHITE),
        "an HTML page's bytes never crossed"
    );
    assert_eq!(
        band(&frame, "closed"),
        Some(WHITE),
        "nor a sheet that did not agree"
    );

    // Two links to one URL are one request.
    assert_eq!(
        lines(&set.home_heard),
        [
            "GET /site.css HTTP/1.1",
            "GET /hop.css HTTP/1.1",
            "GET /moved.css HTTP/1.1"
        ]
    );
    assert_eq!(
        lines(&set.other_heard),
        [
            "GET /cdn.css HTTP/1.1",
            "GET /page.html HTTP/1.1",
            "GET /cors.css HTTP/1.1",
            "GET /closed.css HTTP/1.1",
        ]
    );
    // A no-cors sheet says where it is from as the referrer policy allows,
    // and nothing as an origin; a CORS one says its origin.
    let cdn = head_of(&set.other_heard, "GET /cdn.css");
    assert!(cdn.contains(&format!("Referer: {}/", set.home)), "{cdn}");
    assert!(!cdn.to_ascii_lowercase().contains("\norigin:"), "{cdn}");
    let cors = head_of(&set.other_heard, "GET /cors.css");
    assert!(cors.contains(&format!("Origin: {}", set.home)), "{cors}");

    // The person is told why two did not arrive; the page only that they
    // did not.
    let told = told.join("\n");
    assert!(
        told.contains("page.html did not arrive") && told.contains("text/html"),
        "{told}"
    );
    assert!(
        told.contains("closed.css did not arrive") && told.contains("Access-Control-Allow-Origin"),
        "{told}"
    );

    // Every request is a style request in the record, caused by the document.
    let styles: Vec<_> = set
        .network
        .pool
        .activity()
        .entries()
        .filter(|entry| *entry.purpose() == Purpose::Style)
        .collect();
    assert_eq!(styles.len(), 7, "every hop of every sheet");
    for entry in styles {
        assert_eq!(
            entry.cause(),
            &Cause::Document {
                document: set.document
            },
            "{}",
            entry.url()
        );
    }
}

/// The closing condition's third clause: a header `style-src` refusing a sheet
/// makes no request and records the refusal.
#[test]
fn a_header_style_src_refusing_a_sheet_makes_no_request_and_records_why() {
    let mut set = set_up(Some("style-src 'self'")).expect("set up");
    let (frame, told) = answer_all(&mut set).expect("answered");
    assert!(
        lines(&set.other_heard).is_empty(),
        "{:?}",
        lines(&set.other_heard)
    );
    assert_eq!(band(&frame, "own"), Some([255, 0, 0]));
    assert_eq!(band(&frame, "cdn"), Some(WHITE));
    let refused: Vec<String> = set
        .network
        .pool
        .activity()
        .entries()
        .filter(|entry| *entry.purpose() == Purpose::Style)
        .filter_map(|entry| match entry.happened() {
            Happened::Refused { rule } if rule.contains("style-src") => {
                Some(entry.url().serialised.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        refused,
        ["cdn.css", "page.html", "cors.css", "closed.css"]
            .map(|path| format!("{}/{path}", set.other)),
    );
    assert!(told.join("\n").contains("style-src"), "{told:?}");
}

/// The closing condition's second clause, read off the message itself: a
/// cross-origin response that is not `text/css` sends no body across.
#[test]
fn a_cross_origin_answer_that_is_not_a_style_sheet_is_a_message_with_no_body() {
    let (home_listener, home) = listen().expect("a port");
    let (other_listener, other) = listen().expect("a port");
    let _ = serve(home_listener, Server::Home, home.clone());
    let other_heard = serve(other_listener, Server::Other, home.clone());
    let page = alo_url::parse(&format!("{home}/page")).expect("a URL");
    let policies = Policies::none();
    let ask = SheetAsk {
        number: 12,
        url: format!("{other}/page.html"),
        mode: Mode::NoCors,
        credentials: Credentials::Include,
        referrer: None,
        nonce: None,
    };
    let cause = Cause::Document {
        document: alo_net::cause::Identities::default().a_document(),
    };
    let Decided::Make(sheet) = sheet_decide::decide(
        &ask,
        &Asker {
            url: &page,
            policies: &policies,
        },
        &cause,
    ) else {
        panic!("a no-cors sheet from another origin was refused");
    };
    let pool = Pool::with_trust(Trust::of(&[]).expect("trust")).patient_for(Duration::from_secs(5));
    let made = sheet_make::make(&sheet, &mut Network::over(pool));
    assert_eq!(
        lines(&other_heard),
        ["GET /page.html HTTP/1.1"],
        "it was made"
    );
    assert_eq!(
        made.answer,
        SheetAnswer::failed(12),
        "and nothing of it crosses"
    );
    assert!(
        alo_renderer::wire::write_to_renderer(&alo_renderer::ToRenderer::Sheet(Box::new(
            made.answer
        )))
        .windows(7)
        .all(|seen| seen != b"#page {"),
        "not a byte of the body is in the message"
    );
    assert!(
        made.said.join("\n").contains("text/html"),
        "{:?}",
        made.said
    );
}
