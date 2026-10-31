/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 737: the card's *Back* and *Forward*, read from the tab's
//! session history and followed — through **the window's own road**
//! ([`ConversationWindow`], the two confined renderers, and the Chromium),
//! the machine's Chrome and alo's CEF host, each the same test.
//!
//! The card drew both and greyed them always: nothing read where the page
//! could go. Here, each pressed where a person sees it:
//!
//! 1. the card's first page: told it can go **neither way** — the shown
//!    window's own `data:,` before it is not the card's — and a press on
//!    the greyed *Back* is decided as nothing;
//! 2. a link in the card's page followed: **Back** enabled, Forward not;
//! 3. *Back* pressed — decided as *Back*, so drawn enabled — and the card is
//!    at its first page again, with **Forward** enabled and Back not;
//! 4. *Forward* pressed, and the card is at the link's page again, Back
//!    enabled and Forward not.
//!
//! The pages carry no `Cache-Control: no-store`, so Chrome may keep a page
//! gone back from whole (its back-forward cache), as a real site's is; the
//! card is asserted loaded at each, either way.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::chrome_history::Travel;
use alo_window::conversation::Pressed;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn, Part};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and loading a page,
/// past the thirty seconds Chrome itself is given to answer.
const AT_MOST: Duration = Duration::from_secs(90);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The card's *Back* and *Forward*, at one: the panel's 240, then each 36
/// square button's middle as the renderer's card test measures them — 87
/// and 125 across, 103.5 down.
const BACK: (f64, f64) = (240.0 + 87.0 + 18.0, 103.5 + 18.0);
const FORWARD: (f64, f64) = (240.0 + 125.0 + 18.0, 103.5 + 18.0);

/// The first page: a link, large, under its top left corner.
const ONE_PAGE: &str = "<!doctype html><title>One</title>\
    <style>body{margin:0;background:#fff}\
    #next{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#1d4ed8;color:#fff}</style>\
    <a id=next href=/two.html>Two</a>";

/// The page the link leads to.
const TWO_PAGE: &str = "<!doctype html><title>Two</title>\
    <style>body{margin:0;background:#e76f51}</style><h1>Two</h1>";

/// Where the link is pressed, in the page's CSS pixels: its middle.
const LINK_AT: (f64, f64) = (250.0, 210.0);

/// A server on this machine answering the two pages: its port.
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
                    Some("/one.html") => ("200 OK", ONE_PAGE),
                    Some("/two.html") => ("200 OK", TWO_PAGE),
                    _ => ("404 Not Found", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
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
    /// Every press on the first screen, as it was decided.
    pressed: Vec<Pressed>,
    /// Every time Chrome's thread said where the card can go.
    travels: Vec<Travel>,
    /// How many times the first screen was painted.
    painted: usize,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    who: &'static str,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !done(self) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    format!(
                        "{}: {what} did not happen; the window said {:?}, Chrome's thread \
                         told {:?}, the card is {:?}",
                        self.who,
                        self.said,
                        self.travels,
                        self.card()
                    )
                })?;
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(200)))
            else {
                continue;
            };
            match &heard {
                Heard::Chrome(Went::Unopened { why, .. }) => {
                    return Err(format!("{}: the page did not open: {why}", self.who));
                }
                Heard::Chrome(Went::Closed(_)) => {
                    return Err(format!("{}: the card's Chromium closed", self.who));
                }
                Heard::Chrome(Went::Travel(travel)) => self.travels.push(*travel),
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                Heard::Drawn(Drawn::Painted(Part::Start, _)) => self.painted += 1,
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
        Ok(())
    }

    /// The card's address, whether it can go back and forward, and whether
    /// it is loading.
    fn card(&self) -> Option<(String, bool, bool, bool)> {
        self.window
            .conversation()
            .card()
            .map(|card| (card.url.to_string(), card.back, card.forward, card.loading))
    }

    /// Whether the card is at `address`, loaded and drawn, going back and
    /// forward as said.
    fn at(&self, address: &str, back: bool, forward: bool) -> bool {
        self.card() == Some((address.to_owned(), back, forward, false))
            && self
                .window
                .conversation()
                .card()
                .is_some_and(|card| card.frame.is_some())
    }

    /// Wait until the first screen has been painted twice more — once
    /// asked before now, and once asked after — so a press is decided
    /// against the card as it is now.
    fn painted_again(&mut self) -> Result<(), String> {
        let then = self.painted;
        self.until("the first screen painted again", |driven| {
            driven.painted >= then + 2
        })
    }

    /// A person's click at `at`, in the window's device pixels: the cursor
    /// moved there, the main button pressed and released.
    fn press(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// [`Driven::press`] on a part a renderer draws, and what it was
    /// decided to be.
    fn click(&mut self, at: (f64, f64)) -> Result<Pressed, String> {
        let before = self.pressed.len();
        self.press(at);
        self.until("the press decided", |driven| driven.pressed.len() > before)?;
        self.pressed
            .get(before)
            .cloned()
            .ok_or_else(|| format!("{}: no press decided", self.who))
    }
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
        travels: Vec::new(),
        painted: 0,
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
    let one = format!("http://127.0.0.1:{port}/one.html");
    let two = format!("http://127.0.0.1:{port}/two.html");
    let neither = Travel::default();
    let back_only = Travel {
        back: true,
        forward: false,
    };
    let forward_only = Travel {
        back: false,
        forward: true,
    };
    let mut driven = the_window(chromium, who)?;
    let pressed = driven.click(THE_BOX)?;
    assert_eq!(pressed, Pressed::TheBox, "{who}");
    for letter in one.chars() {
        let written = letter.to_string();
        driven
            .window
            .key(&Press::character(&written), Some(&written));
    }
    driven.window.key(&Press::of(Key::Return), None);
    assert_eq!(
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .typed,
        "",
        "{who}: Return empties the box"
    );

    // 1. The card's first page goes neither way, and its greyed Back is
    // no press at all.
    driven.until("the first page, loaded, told where it can go", |driven| {
        driven.at(&one, false, false) && !driven.travels.is_empty()
    })?;
    assert_eq!(driven.travels, vec![neither], "{who}: what was told");
    driven.painted_again()?;
    assert_eq!(
        driven.click(BACK)?,
        Pressed::Nothing,
        "{who}: the greyed Back"
    );

    // 2. A link followed: Back enabled.
    driven.press((240.0 + 81.0 + LINK_AT.0, 145.0 + LINK_AT.1));
    driven.until("the link's page, Back enabled", |driven| {
        driven.at(&two, true, false)
    })?;
    assert_eq!(driven.travels, vec![neither, back_only], "{who}");

    // 3. Back, pressed where a person sees it: back at the first page,
    // Forward enabled.
    driven.painted_again()?;
    assert_eq!(
        driven.click(BACK)?,
        Pressed::Back,
        "{who}: Back is drawn enabled"
    );
    driven.until("back at the first page, Forward enabled", |driven| {
        driven.at(&one, false, true)
    })?;
    assert_eq!(
        driven.travels,
        vec![neither, back_only, forward_only],
        "{who}"
    );

    // 4. Forward: at the link's page again.
    driven.painted_again()?;
    assert_eq!(
        driven.click(FORWARD)?,
        Pressed::Forward,
        "{who}: Forward is drawn enabled"
    );
    driven.until("forward at the link's page, Back enabled", |driven| {
        driven.at(&two, true, false)
    })?;
    assert_eq!(
        driven.travels,
        vec![neither, back_only, forward_only, back_only],
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
fn the_cards_back_and_forward_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn the_cards_back_and_forward_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("737")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
