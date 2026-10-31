/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 751, ADR 0089: a card is drawn from its page's first paint,
//! not its `load` — through **the window's own road**
//! ([`ConversationWindow`], the confined renderers, and the Chromium), the
//! machine's Chrome and alo's CEF host, each the same test.
//!
//! The card's tab was opened as `alo --chrome` opens one, streamed only once
//! its page had loaded: a slow page was a white card for as long as it
//! loaded, and one whose `load` never came within Chrome's thirty seconds
//! was said unopened though it had drawn. Here `/slow.html` holds its `load`
//! back with a picture the server answers only when the test lets it, and
//! its background is a colour of its own:
//!
//! 1. the card opens, and shows **a frame of that colour while the card
//!    still says it is loading** — before the page has loaded;
//! 2. the page not loaded past Chrome's own patience
//!    ([`LONGEST_SILENCE`]), the card is still open, still drawn and still
//!    loading — never said unopened (ADR 0089: a card whose page never
//!    loads stays a card, saying so, until a person closes it);
//! 3. the picture answered, the page loads, and the card no longer says it
//!    is loading.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::chrome::LONGEST_SILENCE;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and drawing a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The page's own colour, `#2a9d8f`, which no white card is.
const TEAL: (u8, u8, u8) = (0x2a, 0x9d, 0x8f);

/// A page whose `load` waits on a picture the server holds.
const SLOW: &str = "<!doctype html><title>Slow</title>\
    <style>html,body{margin:0;height:100%;background:#2a9d8f}</style>\
    <h1>Slow</h1><img src=/held alt=\"\">";

/// A server on this machine answering `/slow.html`, holding `/held` open
/// until `release` is set: its port.
fn serve(release: Arc<AtomicBool>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let release = Arc::clone(&release);
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let (status, body) = match asked.split(' ').nth(1) {
                    Some("/held") => {
                        // Answered, with nothing, once the test lets it.
                        let until = Instant::now() + AT_MOST * 2;
                        while !release.load(Ordering::SeqCst) && Instant::now() < until {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        ("404 Not Found", "")
                    }
                    Some("/slow.html") => ("200 OK", SLOW),
                    _ => ("404 Not Found", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
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
    /// Every address said unopened, and why.
    unopened: Vec<String>,
    /// Every time Chrome's thread said the page was loading or loaded.
    loading: Vec<bool>,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    who: &'static str,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it or `by`.
    fn until_or(&mut self, by: Instant, done: impl Fn(&Self) -> bool) -> Result<bool, String> {
        while !done(self) {
            let Some(left) = by.checked_duration_since(Instant::now()) else {
                return Ok(false);
            };
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(200)))
            else {
                continue;
            };
            match &heard {
                Heard::Chrome(Went::Unopened { url, why }) => {
                    self.unopened.push(format!("{url}: {why}"));
                }
                Heard::Chrome(Went::Loading(loading)) => self.loading.push(*loading),
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                _ => {}
            }
            self.window.heard(heard);
            let _ = self.window.take_front();
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
        }
        Ok(true)
    }

    /// [`Driven::until_or`], within [`AT_MOST`], failing if it did not
    /// happen.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        if self.until_or(Instant::now() + AT_MOST, done)? {
            Ok(())
        } else {
            Err(format!(
                "{}: {what} did not happen; the window said {:?}, unopened {:?}",
                self.who, self.said, self.unopened
            ))
        }
    }

    /// A person's click at `at`, in the window's device pixels.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// `text` typed and Return, as the window reads keys.
    fn type_and_return(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
    }

    /// Whether the card at `address` holds a frame of the page's own
    /// colour at its middle.
    fn card_drawn_at(&self, address: &str) -> bool {
        self.window.conversation().card().is_some_and(|card| {
            card.url.to_string() == address && card.frame.as_deref().is_some_and(teal)
        })
    }

    /// Whether the card says its page is loading.
    fn card_loading(&self) -> Option<bool> {
        self.window.conversation().card().map(|card| card.loading)
    }
}

/// Whether `png`, a frame of the card's page, is the page's own colour at
/// its middle.
fn teal(png: &[u8]) -> bool {
    let Ok(canvas) = alo_paint::picture::read(png) else {
        return false;
    };
    canvas
        .at(canvas.width() / 2, canvas.height() * 3 / 4)
        .is_some_and(|pixel| {
            let (red, green, blue, _) = pixel.to_rgba8();
            (red, green, blue) == TEAL
        })
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`.
fn the_window(chromium: Chromium, who: &'static str) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    let chrome = ChromeOpener::start(
        chromium,
        Box::new(move |w| tell.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome);
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
        unopened: Vec::new(),
        loading: Vec::new(),
        said: Vec::new(),
        who,
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

fn through(chromium: Chromium, who: &'static str) -> Result<(), String> {
    let release = Arc::new(AtomicBool::new(false));
    let port = serve(Arc::clone(&release))?;
    let mut driven = the_window(chromium, who)?;
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        driven
            .window
            .conversation()
            .start(
                alo_window::conversation_place::start_viewport(WINDOW, Scale::ONE),
                Scale::ONE,
            )
            .focused
    })?;

    // 1. Drawn before it loads, the card saying it is loading.
    let slow = format!("http://127.0.0.1:{port}/slow.html");
    driven.type_and_return(&slow);
    driven.until("a frame of the page in the card, said loading", |driven| {
        !driven.unopened.is_empty() || (driven.card_drawn_at(&slow) && !driven.loading.is_empty())
    })?;
    assert_eq!(
        driven.unopened,
        Vec::<String>::new(),
        "{who}: said unopened"
    );
    assert_eq!(
        driven.card_loading(),
        Some(true),
        "{who}: the card does not say its page is loading"
    );
    assert_eq!(
        driven.loading,
        vec![true],
        "{who}: what Chrome's thread told"
    );

    // 2. Not loaded past Chrome's own patience: still a card, still
    // loading, never said unopened.
    let past = Instant::now() + LONGEST_SILENCE + Duration::from_secs(5);
    let ended = driven.until_or(past, |driven| {
        !driven.unopened.is_empty() || driven.window.conversation().card().is_none()
    })?;
    assert!(
        !ended,
        "{who}: the card went while its page loaded: {:?}, said {:?}",
        driven.unopened, driven.said
    );
    assert!(
        driven.card_drawn_at(&slow),
        "{who}: the card lost its frame"
    );
    assert_eq!(driven.card_loading(), Some(true), "{who}");

    // 3. The picture answered, the page loads, and the card says so.
    release.store(true, Ordering::SeqCst);
    driven.until("the card no longer loading", |driven| {
        driven.card_loading() == Some(false)
    })?;
    assert_eq!(driven.loading, vec![true, false], "{who}");
    assert!(
        driven.card_drawn_at(&slow),
        "{who}: the card lost its frame"
    );
    assert_eq!(driven.unopened, Vec::<String>::new(), "{who}");

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_card_drawn_before_its_page_loads_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_card_drawn_before_its_page_loads_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("751")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
