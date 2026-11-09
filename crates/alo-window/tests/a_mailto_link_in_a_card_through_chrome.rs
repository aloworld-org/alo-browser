/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 777, ADR 0060 § 2 as amended: a `mailto:` link pressed in a
//! card leaves the card at its page, drawn — through **the window's own
//! road** ([`ConversationWindow`], the confined renderers, and the
//! Chromium), the machine's Chrome and alo's CEF host, each the same test.
//!
//! Measured over the pipe (770): the machine's Chrome starts the
//! navigation and stops it with nothing committed, while alo's CEF host
//! committed Chrome's error page (`chrome-error://chromewebdata/`) over the
//! page, so the card showed an error where the person's page had been. The
//! host now cancels a navigation to a scheme Chromium does not load itself
//! before Chromium decides it (`alo-cef-host`'s `handed.rs`).
//!
//! The page here is teal, and its link — to `mailto:` — turns the page
//! orange when it is clicked, before its own navigation begins, so a frame
//! of orange is the press reaching the page. Then, for [`STILL`]: the card
//! is at the page it was at, never said unopened, its newest frame still
//! the page's orange, and it says *Loading* no more.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation::Pressed;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn, Part};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and drawing a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the card is watched once the press has reached the page: the
/// host's error page committed within 70 ms of the press (770).
const STILL: Duration = Duration::from_secs(3);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// Where the page's link is pressed, in the window's device pixels: the
/// card's page box at 240 + 81 across and 145 down, and the link's middle
/// in the page's CSS pixels at one.
const LINK: (f64, f64) = (240.0 + 81.0 + 250.0, 145.0 + 210.0);

/// The page's colour before its link is clicked, `#2a9d8f`.
const TEAL: (u8, u8, u8) = (0x2a, 0x9d, 0x8f);

/// The page's colour once its link is clicked, `#e76f51`.
const ORANGE: (u8, u8, u8) = (0xe7, 0x6f, 0x51);

/// The page: teal, a link to `mailto:` under its top left corner which
/// turns it orange as it is clicked — and does not stop its navigation.
const PAGE: &str = "<!doctype html><title>Mail</title>\
    <style>html,body{margin:0;height:100%;background:#2a9d8f}\
    a{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#264653;color:#fff}</style>\
    <a href=\"mailto:someone@example.com\" \
       onclick=\"document.body.style.background = '#e76f51'\">Write</a>";

/// A server on this machine answering `/mail.html`: its port. Every page
/// asked for is written into `asked`.
///
/// A connection is read for as long as anything is waited for: Chrome opens
/// connections before it has anything to ask, and under load asks on one
/// more than five seconds later — a server that gave up on it then and
/// answered *Not Found* to nothing had that answer read as the page's
/// (queue item 893). A connection that asks for nothing is not answered.
fn serve(asked: Arc<Mutex<Vec<String>>>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let asked = Arc::clone(&asked);
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(AT_MOST));
                let mut read = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    read.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || read.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let read = String::from_utf8_lossy(&read);
                if read.trim().is_empty() {
                    return;
                }
                let path = read.split(' ').nth(1).unwrap_or_default().to_owned();
                let (status, body) = if path == "/mail.html" {
                    ("200 OK", PAGE)
                } else {
                    ("404 Not Found", "")
                };
                if let (true, Ok(mut asked)) = (status == "200 OK", asked.lock()) {
                    asked.push(path);
                }
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

/// A colour, red, green and blue.
type Colour = (u8, u8, u8);

/// The card's address, whether it is loading, and the colour of its newest
/// frame, if it has one.
type Seen = (String, bool, Option<Colour>);

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every press on the first screen, as it was decided.
    pressed: Vec<Pressed>,
    /// Every time Chrome's thread said the page was loading or not.
    loading: Vec<bool>,
    /// Every page Chrome's thread said did not open.
    unopened: Vec<String>,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    who: &'static str,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it or `by`: whether it did.
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
                Heard::Chrome(Went::Closed(_)) => {
                    return Err(format!("{}: the card's Chromium closed", self.who));
                }
                Heard::Chrome(Went::Loading(loading)) => self.loading.push(*loading),
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                Heard::Drawn(Drawn::Pressed(Part::Start, pressed)) => {
                    self.pressed.push(pressed.clone());
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
                "{}: {what} did not happen; the window said {:?}, unopened {:?}, loading {:?}, \
                 the card is {:?}",
                self.who,
                self.said,
                self.unopened,
                self.loading,
                self.card()
            ))
        }
    }

    /// The card's address, whether it is loading, and the colour at its
    /// newest frame's middle, low down.
    fn card(&self) -> Option<Seen> {
        self.window.conversation().card().map(|card| {
            (
                card.url.to_string(),
                card.loading,
                card.frame.as_deref().and_then(colour),
            )
        })
    }

    /// Whether the card is at `address`, not loading, and drawn in
    /// `colour`.
    fn at(&self, address: &str, colour: (u8, u8, u8)) -> bool {
        self.card().is_some_and(|(url, loading, drawn)| {
            url == address && !loading && drawn == Some(colour)
        })
    }

    /// Wait until the drawing thread has drawn the newest drawing the
    /// window asked for ([`ConversationWindow::drawn_as_asked`], queue item
    /// 862), so a press, sent after it on the same thread, is decided
    /// against the card as it is now. Not until it has painted twice more:
    /// a page that changes nothing on the first screen asks for no drawing,
    /// and under load every drawing asked had often been painted before
    /// the card was seen at its page, so none came (queue item 893).
    fn drawn_as_asked(&mut self) -> Result<(), String> {
        self.until("the first screen drawn as last asked", |driven| {
            driven.window.drawn_as_asked()
        })
    }

    /// A person's click at `at`, in the window's device pixels: the cursor
    /// moved there, the main button pressed and released.
    fn press(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }
}

/// The colour at the middle of `png`, a frame of the card's page, three
/// quarters down — below the page's link.
fn colour(png: &[u8]) -> Option<(u8, u8, u8)> {
    let canvas = alo_paint::picture::read(png).ok()?;
    let (red, green, blue, _) = canvas
        .at(canvas.width() / 2, canvas.height() * 3 / 4)?
        .to_rgba8();
    Some((red, green, blue))
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
        pressed: Vec::new(),
        loading: Vec::new(),
        unopened: Vec::new(),
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
    let asked = Arc::new(Mutex::new(Vec::new()));
    let port = serve(Arc::clone(&asked))?;
    let page = format!("http://127.0.0.1:{port}/mail.html");
    let mut driven = the_window(chromium, who)?;
    driven.press(THE_BOX);
    driven.until("the box pressed", |driven| !driven.pressed.is_empty())?;
    assert_eq!(driven.pressed, [Pressed::TheBox], "{who}");
    for letter in page.chars() {
        let written = letter.to_string();
        driven
            .window
            .key(&Press::character(&written), Some(&written));
    }
    driven.window.key(&Press::of(Key::Return), None);
    driven.until("the page, drawn teal and loaded", |driven| {
        driven.at(&page, TEAL)
    })?;
    driven.loading.clear();

    // The link pressed: the page turns orange, so the press reached it,
    // and its navigation to `mailto:` is asked for.
    driven.drawn_as_asked()?;
    driven.press(LINK);
    driven.until("the press reaching the page", |driven| {
        driven
            .card()
            .is_some_and(|(_, _, drawn)| drawn == Some(ORANGE))
    })?;
    let left = driven.until_or(Instant::now() + STILL, |driven| {
        !driven.unopened.is_empty()
            || driven
                .card()
                .is_none_or(|(url, _, drawn)| url != page || drawn != Some(ORANGE))
    })?;
    assert!(
        !left,
        "{who}: the mailto: link moved the card: {:?}, unopened {:?}, loading {:?}",
        driven.card(),
        driven.unopened,
        driven.loading
    );
    assert!(
        driven.at(&page, ORANGE),
        "{who}: the card is {:?}, loading {:?}",
        driven.card(),
        driven.loading
    );
    assert!(
        driven.loading.iter().step_by(2).all(|loading| *loading)
            && driven
                .loading
                .iter()
                .skip(1)
                .step_by(2)
                .all(|loading| !loading),
        "{who}: loading was told as {:?}",
        driven.loading
    );
    assert_eq!(
        asked.lock().map(|asked| asked.clone()).unwrap_or_default(),
        ["/mail.html"],
        "{who}"
    );
    assert_eq!(driven.window.told(), None, "{who}: a press was refused");
    assert!(
        driven.said.is_empty(),
        "{who}: the window said {:?}",
        driven.said
    );

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_mailto_link_in_a_card_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_mailto_link_in_a_card_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("777")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
