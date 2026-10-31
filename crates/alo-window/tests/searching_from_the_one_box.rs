/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 708 (ADR 0083): words typed into the one box and Return,
//! searched, and the results drawn as the All tab — through **the window's
//! own road**: the functions `alo --conversation`'s event loop calls for
//! each event ([`ConversationWindow`]), the two confined renderers that
//! draw the panel and the first screen, and the thread that asks the search
//! provider, every thread's news given back as the event loop gives it.
//!
//! **Never the real API, and never the network.** The provider is a server
//! on `127.0.0.1` that answers hand-written replies in Brave's shape, given
//! to the searcher by `Provider::at`, with a key that is not a key. It
//! records what it was asked, and the test asserts that the words were all
//! the address carried and the key went in its header.
//!
//! Each asserted:
//!
//! 1. Return on words searches them: the box empties, the first screen
//!    says it is searching, then draws the sites found — the mark of the
//!    first in its tint where the layout assertion puts it.
//! 2. The provider answering 401, 429 and 500, the person reads why in the
//!    first screen's line and where the sites would be, in words, with no
//!    key in them.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::{SetUp, Showing};
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::{Provider, unanswered};
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_search::Searcher;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::search_key::Key as SearchKey;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// A hand-written answer in Brave's shape: three sites, one of them with
/// Brave's emphasis and an entity in its title.
const ANSWER: &str = r#"{"type":"search","query":{"original":"rust borrow checker"},
  "web":{"type":"search","results":[
    {"title":"References and <strong>Borrowing</strong> &amp; you",
     "url":"https://doc.example.org/book/references.html",
     "description":"The <strong>borrow checker</strong> checks every reference at compile time.",
     "profile":{"name":"Example Book"}},
    {"title":"Fighting the borrow checker","url":"https://blog.example/fighting",
     "description":"Five patterns."},
    {"title":"Lifetimes","url":"https://lifetimes.example/"}
  ]}}"#;

/// What the test's provider was asked: the request line and the headers.
type Asked = String;

/// A provider on this machine, answering by what was searched for: its
/// port. Each request is told to `asked`.
fn provide(asked: Sender<Asked>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let asked = asked.clone();
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
                let (status, body) = match target.as_str() {
                    "/res/v1/web/search?q=rust+borrow+checker" => ("200 OK", ANSWER),
                    "/res/v1/web/search?q=refused" => (
                        "401 Unauthorized",
                        r#"{"type":"ErrorResponse","error":{"code":"SUBSCRIPTION_TOKEN_INVALID"}}"#,
                    ),
                    "/res/v1/web/search?q=too+often" => (
                        "429 Too Many Requests",
                        r#"{"type":"ErrorResponse","error":{"code":"RATE_LIMITED"}}"#,
                    ),
                    _ => ("500 Internal Server Error", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
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

    /// The first screen's line.
    fn line(&self) -> Option<String> {
        self.window
            .scene()
            .and_then(|scene| scene.start_sentence.map(str::to_owned))
    }

    /// The first screen as its renderer last painted it.
    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }
}

/// The first screen, as the window asks it drawn now.
fn start(window: &ConversationWindow) -> alo_renderer::start::Start {
    window
        .conversation()
        .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The conversation's window at one, both parts painted, searching through
/// the provider on `port`.
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
    // Never asked here: no address is chosen.
    let chrome = ChromeOpener::start(
        Chromium::Machines,
        Box::new(move |w| tell.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    let key = SearchKey::of(NOT_A_KEY).ok_or("the test's key is not one a header carries")?;
    let provider = Provider::at(&format!("http://127.0.0.1:{port}/res/v1/web/search"), key)?;
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

#[test]
fn words_typed_and_return_are_searched_and_drawn_as_the_all_tab() -> Result<(), String> {
    let (asking, asked) = channel();
    let port = provide(asking)?;
    let mut driven = the_window(port)?;
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        start(&driven.window).focused
    })?;
    assert_eq!(start(&driven.window).search, SetUp::Ready);

    // 1. Return on words: searched, and the sites drawn.
    driven.type_and_return("rust borrow checker");
    let now = start(&driven.window);
    assert_eq!(now.typed, "", "the box kept what was typed");
    assert_eq!(
        driven.line().as_deref(),
        Some("searching Brave Search for \u{201c}rust borrow checker\u{201d}")
    );
    driven.until("the sites found", |driven| {
        matches!(
            driven.window.conversation().results().map(|r| &r.showing),
            Some(Showing::Found(_))
        )
    })?;
    let titles: Vec<String> = match driven.window.conversation().results().map(|r| &r.showing) {
        Some(Showing::Found(found)) => found.iter().map(|f| f.title.clone()).collect(),
        _ => Vec::new(),
    };
    assert_eq!(
        titles,
        vec![
            "References and Borrowing & you",
            "Fighting the borrow checker",
            "Lifetimes"
        ]
    );
    assert_eq!(
        driven.line().as_deref(),
        Some("Brave Search found 3 sites for \u{201c}rust borrow checker\u{201d}")
    );
    // Drawn by the first screen's confined renderer: the first site's mark
    // where the renderer's layout assertion puts it, in a tint of the four.
    driven.until("the results painted", |driven| {
        driven
            .painted()
            .and_then(|frame| pixel(&frame, 81, 177))
            .is_some_and(|colour| {
                [
                    [0x2f, 0x6b, 0x5e, 0xff],
                    [0x3e, 0x5c, 0x8a, 0xff],
                    [0x5a, 0x6b, 0x8a, 0xff],
                    [0x6b, 0x65, 0x5c, 0xff],
                ]
                .contains(&colour)
            })
    })?;
    // What the provider was asked: the words alone, the key in its header.
    let head = asked
        .recv_timeout(AT_MOST)
        .map_err(|_| "the provider was asked nothing")?;
    assert!(
        head.starts_with("GET /res/v1/web/search?q=rust+borrow+checker HTTP/1.1\r\n"),
        "{head}"
    );
    let lower = head.to_ascii_lowercase();
    assert!(
        lower.contains(&format!("\r\nx-subscription-token: {NOT_A_KEY}\r\n")),
        "{head}"
    );
    assert!(lower.contains("\r\naccept: application/json\r\n"), "{head}");
    for never in ["cookie:", "referer:", "origin:"] {
        assert!(!lower.contains(never), "{never} was sent: {head}");
    }

    // 2. The provider refusing, rate limiting and failing: said in words.
    for (words, status) in [("refused", 401), ("too often", 429), ("broken", 500)] {
        // The box keeps the keyboard after a search: typed into again.
        assert!(start(&driven.window).focused);
        driven.type_and_return(words);
        let expected = unanswered(status);
        driven.until(&format!("{status} said"), |driven| {
            driven.line().as_deref() == Some(expected.as_str())
        })?;
        assert_eq!(
            driven
                .window
                .conversation()
                .results()
                .map(|r| r.showing.clone()),
            Some(Showing::Unanswered(expected.clone()))
        );
        assert!(!expected.contains(NOT_A_KEY), "{expected}");
        let _ = asked.recv_timeout(AT_MOST);
    }
    assert!(
        driven.said.iter().all(|said| !said.contains(NOT_A_KEY)),
        "{:?}",
        driven.said
    );

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
