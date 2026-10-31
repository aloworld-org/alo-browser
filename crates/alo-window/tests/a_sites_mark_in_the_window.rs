/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 747 (ADR 0088): each site's own mark, through **the window's
//! own road** — the functions `alo --conversation`'s event loop calls for
//! each event ([`ConversationWindow`]), the two confined renderers that
//! draw the panel and the first screen, and the thread that asks the search
//! provider and fetches each site's icon from its image proxy, every
//! thread's news given back as the event loop gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering a hand-written reply in Brave's shape, and serving
//! as its image proxy icons made here, by hand.
//!
//! Asserted: words searched draw the sites with their letters, then each
//! site's mark — the first site's icon painted in its letter's place; the
//! second (named at the site itself), the third (bytes that do not decode),
//! the fourth (too large) and the fifth (none) keep their letters. The
//! provider was asked the words alone with the key, then each icon at the
//! proxy once — the first's, which the sixth names too, once — with no key,
//! cookie or `Referer`, before the images; the site itself never.

#![cfg(target_os = "macos")]

use alo_paint::canvas::Canvas;
use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Images, LARGEST_MARK, Results, Showing};
use alo_renderer::turn::Turn;
use alo_value::Rgba;
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

/// The web search's answer: six sites, each naming its mark at the proxy
/// on `port` — the first an icon, the second at the site itself, the third
/// bytes that do not decode, the fourth too large to be a mark, the fifth
/// none, and the sixth the first's icon again.
fn sites(port: u16) -> String {
    let marks = [
        format!("http://127.0.0.1:{port}/fav/icon.png"),
        "https://site1.example/favicon.ico".to_owned(),
        format!("http://127.0.0.1:{port}/fav/undecodable.png"),
        format!("http://127.0.0.1:{port}/fav/large.png"),
        String::new(),
        format!("http://127.0.0.1:{port}/fav/icon.png"),
    ];
    let each: Vec<String> = marks
        .iter()
        .enumerate()
        .map(|(n, mark)| {
            let meta = if mark.is_empty() {
                format!(r#"{{"hostname":"site{n}.example"}}"#)
            } else {
                format!(r#"{{"hostname":"site{n}.example","favicon":"{mark}"}}"#)
            };
            format!(
                r#"{{"title":"Site number {n}","url":"https://site{n}.example/page",
                    "description":"One line of what it says.","profile":{{"name":"Site {n}"}},
                    "meta_url":{meta}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"type":"search","query":{{"original":"home ev charger"}},
            "web":{{"type":"search","results":[{}]}}}}"#,
        each.join(",")
    )
}

/// The icon's colour.
const RED: (u8, u8, u8) = (0xc4, 0x30, 0x2b);

/// An icon 32 square in `RED`, as a PNG.
fn icon() -> Vec<u8> {
    let canvas = Canvas::new(32, 32, Rgba::from_rgba8(RED.0, RED.1, RED.2, 255));
    alo_paint::encode::to_png(&canvas).unwrap_or_default()
}

/// A provider and its image proxy on this machine: its port. Each
/// request's head is told to `asked`.
fn provide(asked: Sender<String>) -> Result<u16, String> {
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
                let (status, kind, body) = match target.as_str() {
                    "/res/v1/web/search?q=home+ev+charger" => {
                        ("200 OK", "application/json", sites(port).into_bytes())
                    }
                    "/res/v1/images/search?q=home+ev+charger" => (
                        "200 OK",
                        "application/json",
                        br#"{"type":"images","results":[]}"#.to_vec(),
                    ),
                    "/fav/icon.png" => ("200 OK", "image/png", icon()),
                    "/fav/undecodable.png" => {
                        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
                        bytes.extend_from_slice(&[0x5a; 64]);
                        ("200 OK", "image/png", bytes)
                    }
                    "/fav/large.png" => {
                        let mut bytes = icon();
                        bytes.resize(LARGEST_MARK + 1, 0);
                        ("200 OK", "image/png", bytes)
                    }
                    _ => ("500 Internal Server Error", "text/plain", Vec::new()),
                };
                let mut reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\n\
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

    /// The first screen as its renderer last painted it.
    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
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

    /// Whether the first screen was last painted with `colour` at `at`, in
    /// its own CSS pixels.
    fn painted_at(&self, (x, y): (u32, u32), colour: [u8; 4]) -> bool {
        self.painted()
            .and_then(|frame| pixel(&frame, x, y))
            .is_some_and(|painted| painted == colour)
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// A colour as painted.
fn painted((red, green, blue): (u8, u8, u8)) -> [u8; 4] {
    [red, green, blue, 0xff]
}

/// The conversation's window at one, both parts painted, searching through
/// the provider on `port` and fetching thumbnails from it, its card's
/// Chromium one that is not there.
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

#[test]
fn each_sites_mark_is_painted_in_its_letters_place_in_the_window() -> Result<(), String> {
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
    })?;
    driven.type_and_return("home ev charger");
    // The first site's icon in its 18 square at (80, 168) of the first
    // screen, the renderer's layout assertion (`a_sites_own_mark.rs`).
    driven.until("the first site's mark painted", |driven| {
        driven
            .results()
            .is_some_and(|results| matches!(results.images, Images::Found(_)))
            && driven.painted_at((89, 177), painted(RED))
    })?;
    let results = driven.results().ok_or("no results")?;
    let Showing::Found(found) = &results.showing else {
        return Err("no sites".to_owned());
    };
    let held: Vec<Option<Vec<u8>>> = found.iter().map(|site| site.mark.clone()).collect();
    assert_eq!(held.len(), 6);
    assert_eq!(held.first().cloned().flatten(), Some(icon()));
    assert_eq!(
        held.iter().map(Option::is_some).collect::<Vec<_>>(),
        vec![true, false, true, false, false, true],
        "the undecodable bytes are kept, for the renderer to refuse; the site's own and the \
         large are not"
    );
    // The second site keeps its letter, white on its tint, where the first
    // has its icon.
    assert!(
        !driven.painted_at((89, 273), painted(RED)),
        "the second site has no icon"
    );
    assert!(
        !driven.painted_at((89, 369), painted(RED)),
        "the third's bytes do not decode: its letter"
    );
    let heads: Vec<String> = asking.try_iter().collect();
    let lines: Vec<&str> = heads
        .iter()
        .filter_map(|head| head.lines().next())
        .collect();
    assert_eq!(
        lines,
        vec![
            "GET /res/v1/web/search?q=home+ev+charger HTTP/1.1",
            "GET /fav/icon.png HTTP/1.1",
            "GET /fav/undecodable.png HTTP/1.1",
            "GET /fav/large.png HTTP/1.1",
            "GET /res/v1/images/search?q=home+ev+charger HTTP/1.1",
        ],
        "the words alone, then each icon at the proxy once, before the images"
    );
    for head in &heads {
        let keyed = head.contains(&format!("X-Subscription-Token: {NOT_A_KEY}"));
        assert_eq!(keyed, head.contains("/res/v1/"), "{head}");
        assert!(!head.to_lowercase().contains("referer"), "{head}");
        assert!(!head.to_lowercase().contains("cookie"), "{head}");
    }
    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
