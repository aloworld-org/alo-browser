/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 761 (ADR 0086 § 1): the Sites tab's *Show more sites*,
//! through **the window's own road** — the functions `alo --conversation`'s
//! event loop calls for each event ([`ConversationWindow`]), the two
//! confined renderers that draw the panel and the first screen, and the
//! thread that asks the search provider, every thread's news given back as
//! the event loop gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering hand-written replies in Brave's shape.
//!
//! Each asserted:
//!
//! 1. A search whose answer says `more_results_available` asks nothing
//!    more: the further page is asked only when a person presses.
//! 2. On the Sites tab, *Show more sites* pressed asks the web search
//!    again with `offset=1` — the words and the offset alone, with the key
//!    — and a second press while it is asked asks nothing; what comes is
//!    appended, and the button is painted under it.
//! 3. A further page not answered says why, and a press asks the same
//!    offset again; a page saying there are no more ends it: no button.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Next, Results, Tab};
use alo_renderer::turn::Turn;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_search::Searcher;
use alo_window::conversation_thread::Thread;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::search_key::Key as SearchKey;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// Where the first screen begins in the window, at one: past the panel.
const PANEL: f64 = 240.0;

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// A web search's answer: two sites numbered from `first`, and whether
/// Brave says there are more.
fn page(first: usize, more: bool) -> String {
    let each: Vec<String> = (first..first + 2)
        .map(|n| {
            format!(
                r#"{{"title":"Site number {n}","url":"https://site{n}.example/page",
                    "description":"One line of what it says.","profile":{{"name":"Site {n}"}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"type":"search","query":{{"original":"home ev charger","more_results_available":{more}}},
            "web":{{"type":"search","results":[{}]}}}}"#,
        each.join(",")
    )
}

/// A provider on this machine: its port. Each request's head is told to
/// `asked`. The further page at offset 1 is answered only after a second,
/// so a press while it is asked can be made; the one at offset 2 is
/// answered 503 the first time.
fn provide(asked: Sender<String>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let seconds = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let asked = asked.clone();
            let seconds = Arc::clone(&seconds);
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut head = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    head.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || head.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let target = head.split(' ').nth(1).unwrap_or_default().to_owned();
                let _ = asked.send(head);
                let json = |body: String| ("200 OK", body.into_bytes());
                let (status, body) = match target.as_str() {
                    "/res/v1/web/search?q=home+ev+charger" => json(page(0, true)),
                    "/res/v1/web/search?q=home+ev+charger&offset=1" => {
                        std::thread::sleep(Duration::from_secs(1));
                        json(page(2, true))
                    }
                    "/res/v1/web/search?q=home+ev+charger&offset=2"
                        if seconds.fetch_add(1, Ordering::SeqCst) == 0 =>
                    {
                        ("503 Service Unavailable", Vec::new())
                    }
                    "/res/v1/web/search?q=home+ev+charger&offset=2" => json(page(4, false)),
                    "/res/v1/images/search?q=home+ev+charger" => {
                        json(r#"{"type":"images","results":[]}"#.to_owned())
                    }
                    _ => ("500 Internal Server Error", Vec::new()),
                };
                let mut reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                reply.extend_from_slice(&body);
                let _ = socket.write_all(&reply);
            });
        }
    });
    Ok(port)
}

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every line the window said on the terminal.
    said: Vec<String>,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !done(self) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("{what} did not happen; the window said {:?}", self.said))?;
            let Ok(heard) = self.heard.recv_timeout(left) else {
                continue;
            };
            if let Heard::Drawn(Drawn::Said(part, said)) = &heard {
                return Err(format!("{part} said {said}"));
            }
            self.window.heard(heard);
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
            let _ = self.window.take_front();
        }
        Ok(())
    }

    /// A person's click at `at`, in the window's device pixels.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// `text` typed, as the window reads keys, then Return.
    fn type_and_return(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
    }

    /// The chat on screen's turns.
    fn thread(&self) -> Thread {
        self.window
            .conversation()
            .thread()
            .cloned()
            .unwrap_or_default()
    }

    /// The newest search's results.
    fn results(&self) -> Option<Results> {
        self.thread()
            .turns()
            .iter()
            .rev()
            .find_map(|turn| match turn {
                Turn::Searched(results) => Some(results.clone()),
                _ => None,
            })
    }

    /// How many sites the newest search shows.
    fn sites(&self) -> usize {
        self.results()
            .map_or(0, |results| results.drawn_sites().len())
    }

    /// Whether the first screen was last painted with `colour` at `at`, in
    /// its own CSS pixels.
    fn painted_at(&self, (x, y): (u32, u32), colour: [u8; 4]) -> bool {
        self.window
            .scene()
            .and_then(|scene| scene.start.cloned())
            .and_then(|frame| pixel(&frame, x, y))
            .is_some_and(|painted| painted == colour)
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The request line of each head the provider was asked since last: each
/// asked with the key, and nothing of the person's.
fn lines(asking: &Receiver<String>) -> Vec<String> {
    asking
        .try_iter()
        .map(|head| {
            assert!(!head.to_lowercase().contains("referer"), "{head}");
            assert!(!head.to_lowercase().contains("cookie"), "{head}");
            assert!(
                head.contains(&format!("X-Subscription-Token: {NOT_A_KEY}")),
                "{head}"
            );
            head.lines().next().unwrap_or_default().to_owned()
        })
        .collect()
}

/// The conversation's window at one, both parts painted, searching through
/// the provider on `port`, its card's Chromium one that is not there.
fn the_window(port: u16) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let told = tell.clone();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    let chrome = ChromeOpener::start(
        Chromium::Carried(PathBuf::from("/nonexistent/alo Chromium")),
        Box::new(move |w| tell.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    let key = SearchKey::of(NOT_A_KEY).ok_or("the test's key is not one a header carries")?;
    let provider = Provider::at(&format!("http://127.0.0.1:{port}/res/v1/web/search"), key)?
        .with_images(
            &format!("http://127.0.0.1:{port}/res/v1/images/search"),
            &format!("http://127.0.0.1:{port}/"),
        )?;
    let searcher = Searcher::start(
        Ok(provider),
        Box::new(move |s| told.send(Heard::Searched(s)).is_ok()),
    )
    .map_err(|why| format!("no search thread: {why}"))?;
    let mut window =
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_search(searcher);
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    let mut driven = Driven {
        window,
        heard,
        said: Vec::new(),
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

/// The Sites tab's middle along the tab row (209.8 to 270.8), in the
/// window.
const SITES: (f64, f64) = (PANEL + 240.0, 123.0);

/// The button's white, and its place under two, four and (with the
/// sentence of why above it) four sites: the renderer's layout assertion
/// puts it at (80, 360) 139 × 40 under two sites, each site 74 tall and 22
/// apart.
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// A press on the button whose top is at `top`, in the window.
fn button(top: f64) -> (f64, f64) {
    (PANEL + 120.0, top + 20.0)
}

#[test]
fn show_more_sites_asks_each_further_page_once_per_press() -> Result<(), String> {
    let (asked, asking) = channel();
    let port = provide(asked)?;
    let mut driven = the_window(port)?;
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
            && driven.window.presses_decided()
    })?;

    // 1. The search; Brave says there are more, and nothing more is asked.
    driven.type_and_return("home ev charger");
    driven.until("the sites found and the images answered", |driven| {
        driven.results().is_some_and(|results| {
            matches!(results.images, alo_renderer::results::Images::Found(_))
        })
    })?;
    assert_eq!(
        lines(&asking),
        vec![
            "GET /res/v1/web/search?q=home+ev+charger HTTP/1.1",
            "GET /res/v1/images/search?q=home+ev+charger HTTP/1.1",
        ],
        "a further page is asked only for a press"
    );
    assert_eq!(
        driven.results().map(|results| results.pages.next),
        Some(Next::Offered)
    );

    // 2. Sites, then *Show more sites* under its two sites, twice: one
    //    request, at offset 1; the two that come appended, and the button
    //    painted under the four.
    driven.click(SITES);
    driven.until("Sites shown with its button", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::Sites)
            && driven.painted_at((90, 364), WHITE)
    })?;
    driven.click(button(360.0));
    driven.until("the second page asked", |driven| {
        driven
            .results()
            .is_some_and(|results| results.pages.next == Next::Asking)
    })?;
    // Pressed again while it is asked — answered a second after it was —
    // whether the frame pressed still shows the button or already shows it
    // disabled, nothing more is asked.
    driven.click(button(360.0));
    driven.until("the second page appended and painted", |driven| {
        driven.sites() == 4 && driven.painted_at((90, 556), WHITE)
    })?;
    assert_eq!(
        lines(&asking),
        vec!["GET /res/v1/web/search?q=home+ev+charger&offset=1 HTTP/1.1"],
        "the words and the offset, once for two presses"
    );

    // 3. Offset 2, not answered: why, and the button again under it; a
    //    press asks offset 2 again, which says there are no more.
    driven.click(button(552.0));
    driven.until("the third page not answered", |driven| {
        driven
            .results()
            .is_some_and(|results| matches!(results.pages.next, Next::Unanswered(_)))
    })?;
    driven.until("the button painted under why", |driven| {
        driven.painted_at((90, 601), WHITE)
    })?;
    driven.click(button(597.0));
    driven.until("the third page appended", |driven| driven.sites() == 6)?;
    assert_eq!(
        lines(&asking),
        vec![
            "GET /res/v1/web/search?q=home+ev+charger&offset=2 HTTP/1.1",
            "GET /res/v1/web/search?q=home+ev+charger&offset=2 HTTP/1.1",
        ]
    );
    let results = driven.results().ok_or("no results")?;
    assert_eq!(results.pages.added, 2);
    assert_eq!(results.pages.next, Next::None);
    assert_eq!(
        results
            .drawn_sites()
            .iter()
            .map(|site| site.title.as_str())
            .collect::<Vec<_>>(),
        (0..6)
            .map(|n| format!("Site number {n}"))
            .collect::<Vec<_>>()
    );
    Ok(())
}
