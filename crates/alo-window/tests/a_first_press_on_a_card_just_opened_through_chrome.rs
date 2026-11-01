/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 805: the first press on a card just opened is the page's —
//! through **the window's own road** ([`ConversationWindow`], the confined
//! renderers, and the Chromium), the machine's Chrome and alo's CEF host,
//! each the same test.
//!
//! Measured by hand (1 November 2026, alo's CEF host, the application
//! built with the window's events written out): as a card's Chromium
//! starts, the conversation's window loses the keyboard and has it back two
//! or three times, 10 to 30 ms each — a `Focused(false)` the window answers
//! by taking the front back ([`ConversationWindow::take_front`]) — and is
//! the key window again long before the card is drawn; a first press on the
//! page 0.95 s after it was open reached the page, and the keys after it
//! went to the page, three times of three. What lost the press found in
//! 740 was a **modifier held** by the window server's word: the tool that
//! posted the run's keys had posted ⌘ with a key and never its release, so
//! the next press came with ⌘, which is nobody's yet (798) — and said
//! nothing. Measured: that press with `ModifiersChanged(SUPER)` before it
//! and none of the page's after.
//!
//! So asserted here, with the window's keyboard lost and had back as the
//! Chromium starts, as measured:
//!
//! 1. **The first press is the page's.** As soon as the card's first frame
//!    shows the page — no frame of the first screen waited for — a press on
//!    the page's field reaches it (its `mousedown` turns the page orange),
//!    the card has the keyboard, and a key typed next reaches the field
//!    (its `input` turns the page blue), none of it the box's.
//! 2. **What the lists alo carries refused is said** (queue item 736): the
//!    page's `/fbevents.js`, on the window's terminal, once.
//! 3. **A press with a modifier held is said.** With ⌘ held, a press on
//!    the page reaches nothing — the page stays blue — and the window says
//!    why, in the first screen's line and on the terminal.
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
use alo_window::keys::{Held, Key, Modifier, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and drawing a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the card is watched after a press that must reach nothing.
const STILL: Duration = Duration::from_secs(2);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// Where the page's field is pressed, in the window's device pixels: the
/// card's page box at 240 + 81 across and 145 down, and the field's middle
/// in the page's CSS pixels at one.
const FIELD: (f64, f64) = (240.0 + 81.0 + 250.0, 145.0 + 210.0);

/// The page's colour before anything reaches it, `#2a9d8f`.
const TEAL: (u8, u8, u8) = (0x2a, 0x9d, 0x8f);

/// The page's colour once a press reaches its field, `#e76f51`.
const ORANGE: (u8, u8, u8) = (0xe7, 0x6f, 0x51);

/// The page's colour once a key reaches its field, `#1d4ed8`.
const BLUE: (u8, u8, u8) = (0x1d, 0x4e, 0xd8);

/// The page: teal, with a field under its top left corner; a press on the
/// field turns it orange, and a character typed into it blue. It asks for
/// `/fbevents.js` too, which `EasyPrivacy` names (queue item 736).
const PAGE: &str = "<!doctype html><title>First press</title>\
    <style>html,body{margin:0;height:100%;background:#2a9d8f}\
    input{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      font-size:40px;border:0;background:#fff}</style>\
    <input id=field><script src=\"/fbevents.js\"></script>\
    <script>const field = document.getElementById('field');\
      field.addEventListener('mousedown', () => document.body.style.background = '#e76f51');\
      field.addEventListener('input', () => document.body.style.background = '#1d4ed8');\
    </script>";

/// The line of `EasyPrivacy`, counted from one, that names `/fbevents.js`.
fn line_of_the_tracker() -> usize {
    alo_window::blocking::EASYPRIVACY
        .text
        .lines()
        .position(|line| line == "/fbevents.js")
        .map_or(0, |place| place + 1)
}

/// A server on this machine answering `/first.html`: its port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut read = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    read.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || read.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let read = String::from_utf8_lossy(&read);
                let path = read.split(' ').nth(1).unwrap_or_default();
                let (status, body) = if path == "/first.html" {
                    ("200 OK", PAGE)
                } else {
                    ("404 Not Found", "")
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
    /// Every press on the first screen, as it was decided.
    pressed: Vec<Pressed>,
    /// Whether Chrome's thread has said the card is opening.
    opening: bool,
    /// How many times the window asked to be brought in front.
    fronts: usize,
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
                Heard::Chrome(Went::Opening(_)) => self.opening = true,
                Heard::Chrome(Went::Unopened { url, why }) => {
                    self.unopened.push(format!("{url}: {why}"));
                }
                Heard::Chrome(Went::Closed(_)) => {
                    return Err(format!("{}: the card's Chromium closed", self.who));
                }
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                Heard::Drawn(Drawn::Pressed(Part::Start, pressed)) => {
                    self.pressed.push(pressed.clone());
                }
                _ => {}
            }
            self.window.heard(heard);
            self.follow();
        }
        Ok(true)
    }

    /// What the event loop does after each event: brings the window in
    /// front when asked, says its lines, draws again.
    fn follow(&mut self) {
        if self.window.take_front() {
            self.fronts += 1;
        }
        self.said.extend(self.window.take_said());
        let _ = self.window.take_redraw();
    }

    /// [`Driven::until_or`], within [`AT_MOST`], failing if it did not
    /// happen.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        if self.until_or(Instant::now() + AT_MOST, done)? {
            Ok(())
        } else {
            Err(format!(
                "{}: {what} did not happen; the window said {:?}, unopened {:?}, the card's \
                 colour is {:?}",
                self.who,
                self.said,
                self.unopened,
                self.colour()
            ))
        }
    }

    /// The colour of the card's newest frame, low down its middle, if it
    /// has one.
    fn colour(&self) -> Option<(u8, u8, u8)> {
        self.window
            .conversation()
            .card()
            .and_then(|card| card.frame.as_deref())
            .and_then(colour)
    }

    /// A person's click at `at`, in the window's device pixels, as the
    /// event loop hands it: the cursor moved there, the main button
    /// pressed and released.
    fn press(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
        self.follow();
    }

    /// A person typing `text`, a key at a time, as the event loop hands it.
    fn type_in(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.follow();
    }
}

/// The colour at the middle of `png`, a frame of the card's page, three
/// quarters down — below the page's field.
fn colour(png: &[u8]) -> Option<(u8, u8, u8)> {
    let canvas = alo_paint::picture::read(png).ok()?;
    let (red, green, blue, _) = canvas
        .at(canvas.width() / 2, canvas.height() * 3 / 4)?
        .to_rgba8();
    Some((red, green, blue))
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`, holding the keyboard.
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
        opening: false,
        fronts: 0,
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
    let port = serve()?;
    let page = format!("http://127.0.0.1:{port}/first.html");
    let mut driven = the_window(chromium, who)?;
    driven.press(THE_BOX);
    driven.until("the box pressed", |driven| !driven.pressed.is_empty())?;
    assert_eq!(driven.pressed, [Pressed::TheBox], "{who}");
    driven.type_in(&page);
    driven.window.key(&Press::of(Key::Return), None);
    driven.follow();

    // The card's Chromium starts: the window loses the keyboard and has it
    // back, twice, as measured by hand, and asks for the front each time
    // it loses it.
    driven.until("the card opening", |driven| driven.opening)?;
    for _ in 0..2 {
        driven.window.focused(false);
        driven.follow();
        driven.window.focused(true);
        driven.follow();
    }
    assert_eq!(driven.fronts, 2, "{who}: the front taken back each time");

    // 1. The first frame showing the page, pressed at once.
    driven.until("the page's first frame", |driven| {
        driven.colour() == Some(TEAL)
    })?;
    driven.press(FIELD);
    assert!(
        driven.window.conversation().keys_to_the_card(),
        "{who}: the first press did not give the card the keyboard"
    );
    driven.until("the first press reaching the page", |driven| {
        driven.colour() == Some(ORANGE)
    })?;
    driven.type_in("a");
    driven.until("the key after it reaching the page", |driven| {
        driven.colour() == Some(BLUE)
    })?;
    assert_eq!(driven.pressed, [Pressed::TheBox], "{who}: the box pressed");
    assert_eq!(driven.window.told(), None, "{who}");

    // 3. With ⌘ held, a press reaches nothing and says so.
    driven.window.held(Held::NOTHING.with(Modifier::Command));
    driven.press(FIELD);
    let moved = driven.until_or(Instant::now() + STILL, |driven| {
        driven.colour() != Some(BLUE)
    })?;
    assert!(
        !moved,
        "{who}: a press with ⌘ held reached the page: {:?}",
        driven.colour()
    );
    let said = "a press with ⌘ held was not acted on: a press with a modifier is nobody's yet \
                (queue item 798)";
    assert_eq!(driven.window.told(), Some(said), "{who}");
    // Said on the terminal before it: the tracker the page asked for,
    // refused by the lists alo carries (queue item 736, ADR 0094).
    let tracker = format!(
        "the card's page asked for http://127.0.0.1:{port}/fbevents.js, refused as a tracker \
         or an advert by EasyPrivacy, line {}: /fbevents.js (ADR 0094)",
        line_of_the_tracker()
    );
    assert_eq!(driven.said, [tracker.as_str(), said], "{who}");
    assert!(
        driven.unopened.is_empty(),
        "{who}: unopened {:?}",
        driven.unopened
    );

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_first_press_on_a_card_just_opened_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_first_press_on_a_card_just_opened_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("805")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
