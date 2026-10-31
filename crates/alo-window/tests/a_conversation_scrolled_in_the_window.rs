/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 703 (ADR 0085): the conversation keeps every turn and
//! scrolls — through **the window's own road**: the functions
//! `alo --conversation`'s event loop calls for each event
//! ([`ConversationWindow`]), the two confined renderers that draw the panel
//! and the first screen, and the thread that asks the search provider,
//! every thread's news given back as the event loop gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering hand-written replies in Brave's shape. The card's
//! Chromium is one that is not there, so an address opened fails, and says
//! so in its turn.
//!
//! Each asserted:
//!
//! 1. Two searches are two turns, the newest at the room's top, drawn there.
//! 2. A wheel over the conversation scrolls it back, as far as the first
//!    turn and no further; over the panel, nothing.
//! 3. A press on the first search's first title, scrolled back to, is
//!    claimed there and opens it as a new turn — its bubble the title,
//!    scrolled to its top — which, with no Chromium to open it, says why.
//! 4. A typed query holding markup is a turn whose words are those
//!    characters.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::conversation_scroll::Reach;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Showing};
use alo_renderer::turn::Turn;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::card_aim::Turned;
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
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// The first search's answer: three sites.
const FIRST: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"References and <strong>Borrowing</strong> &amp; you",
     "url":"https://doc.example.org/book/references.html",
     "description":"The borrow checker checks every reference at compile time.",
     "profile":{"name":"Example Book"}},
    {"title":"Fighting the borrow checker","url":"https://blog.example/fighting",
     "description":"Five patterns."},
    {"title":"Lifetimes","url":"https://lifetimes.example/"}
  ]}}"#;

/// The second search's answer: three other sites.
const SECOND: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"Lifetimes explained","url":"https://e.example/lifetimes",
     "description":"Every reference has a lifetime."},
    {"title":"Elision","url":"https://elision.example/"},
    {"title":"Variance","url":"https://variance.example/"}
  ]}}"#;

/// A provider on this machine: its port.
fn provide() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
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
                let (status, body) = match head.split(' ').nth(1) {
                    Some("/res/v1/web/search?q=rust+borrow+checker") => ("200 OK", FIRST),
                    Some("/res/v1/web/search?q=lifetimes") => ("200 OK", SECOND),
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

    /// The first screen as its renderer last painted it.
    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The design's four tints a site's letter stands on, in order.
const TINTS: [[u8; 4]; 4] = [
    [0x2f, 0x6b, 0x5e, 0xff],
    [0x3e, 0x5c, 0x8a, 0xff],
    [0x5a, 0x6b, 0x8a, 0xff],
    [0x6b, 0x65, 0x5c, 0xff],
];

/// The tint the site at `address` is drawn on.
fn tint(address: &str) -> Option<[u8; 4]> {
    let url = alo_url::parse(address).ok()?;
    TINTS.get(Found::new("", url, "").tint()).copied()
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

/// Whether the newest turn is a search whose sites have come.
fn found(driven: &Driven) -> bool {
    matches!(
        driven.thread().turns().last(),
        Some(Turn::Searched(results)) if matches!(results.showing, Showing::Found(_))
    )
}

/// Step 2: over the panel a wheel does nothing; over the conversation,
/// three lines back is 120 back, thirty more as far as the first turn,
/// whose first mark, `first`, is then drawn at the top; a trackpad's pixels
/// move it by as many.
fn scroll_back(driven: &mut Driven, reach: Reach, first: [u8; 4]) -> Result<(), String> {
    // 2. Over the panel a wheel does nothing; over the conversation, three
    //    lines back is 120 back, and thirty more as far as the first turn.
    driven.window.moved((100.0, 400.0));
    driven.window.wheeled(Turned::Lines {
        across: 0.0,
        down: 3.0,
    });
    assert_eq!(driven.thread().down().to_bits(), 0.0_f32.to_bits());
    driven.window.moved((640.0, 400.0));
    driven.window.wheeled(Turned::Lines {
        across: 0.0,
        down: 3.0,
    });
    assert_eq!(driven.thread().down().to_bits(), (-120.0_f32).to_bits());
    driven.window.wheeled(Turned::Lines {
        across: 0.0,
        down: 30.0,
    });
    assert_eq!(
        driven.thread().down().to_bits(),
        (-reach.earlier).to_bits(),
        "back as far as the first turn's top"
    );
    driven.until("the first search drawn at the top", |driven| {
        driven
            .painted()
            .and_then(|frame| pixel(&frame, 81, 177))
            .is_some_and(|colour| colour == first)
    })?;
    // A trackpad's pixels, on down again by 10 at one.
    driven.window.wheeled(Turned::Pixels {
        across: 0.0,
        down: -10.0,
    });
    assert_eq!(
        driven.thread().down().to_bits(),
        (10.0 - reach.earlier).to_bits()
    );
    driven.window.wheeled(Turned::Pixels {
        across: 0.0,
        down: 10.0,
    });
    driven.until("drawn back at the first turn's top", |driven| {
        driven
            .painted()
            .and_then(|frame| pixel(&frame, 81, 177))
            .is_some_and(|colour| colour == first)
    })?;
    Ok(())
}

#[test]
fn two_searches_scrolled_back_and_an_earlier_title_pressed_open_a_new_turn() -> Result<(), String> {
    let port = provide()?;
    let mut driven = the_window(port)?;
    let first = tint("https://doc.example.org/").ok_or("a tint")?;
    let second = tint("https://e.example/").ok_or("a tint")?;
    assert_ne!(first, second, "the two searches' first marks tell apart");
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
    })?;

    // 1. Two searches, two turns, the newest at the room's top: its first
    //    mark where the renderer's layout assertion puts a first mark.
    driven.type_and_return("rust borrow checker");
    driven.until("the first search's sites", found)?;
    driven.type_and_return("lifetimes");
    driven.until(
        "the second search's sites, painted and measured",
        |driven| {
            found(driven)
                && driven.thread().turns().len() == 2
                && driven.thread().reach().is_some()
                && driven
                    .painted()
                    .and_then(|frame| pixel(&frame, 81, 177))
                    .is_some_and(|colour| colour == second)
        },
    )?;
    let thread = driven.thread();
    assert_eq!(
        thread.down().to_bits(),
        0.0_f32.to_bits(),
        "the newest at the top"
    );
    let reach = thread.reach().ok_or("measured")?;
    assert!(reach.earlier > 32.0, "{reach:?}");

    scroll_back(&mut driven, reach, first)?;

    // 3. The first search's first title, where it is drawn now: claimed,
    //    and opened as a new turn whose bubble is the title, scrolled to its
    //    top — which, with no Chromium, says why in its place.
    driven.click((240.0 + 200.0, 202.5));
    driven.until("the page not opened, said in its turn", |driven| {
        driven.window.told().is_some()
            || matches!(driven.thread().turns().last(), Some(Turn::Failed { .. }))
    })?;
    let thread = driven.thread();
    assert!(
        driven
            .window
            .told()
            .is_none_or(|told| told.contains("could not be opened")),
        "{:?}",
        driven.window.told()
    );
    let Some(Turn::Failed { said, why }) = thread.turns().last() else {
        return Err(format!("not a failure: {:?}", thread.turns().last()));
    };
    assert_eq!(said, "References and Borrowing & you");
    assert!(
        why.starts_with("https://doc.example.org/book/references.html could not be opened"),
        "{why}"
    );
    assert_eq!(thread.turns().len(), 3);
    assert_eq!(
        thread.down().to_bits(),
        0.0_f32.to_bits(),
        "the new turn's top"
    );

    // 4. Markup typed is a turn whose words are those characters.
    driven.click(THE_BOX);
    driven.type_and_return("<b>bold</b></p><script>x()</script>");
    driven.until("the markup searched", |driven| {
        matches!(
            driven.thread().turns().last(),
            Some(Turn::Searched(results)) if matches!(results.showing, Showing::Unanswered(_))
        )
    })?;
    assert_eq!(
        driven.thread().turns().last().map(Turn::said),
        Some("<b>bold</b></p><script>x()</script>")
    );
    assert_eq!(driven.thread().turns().len(), 4);

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
