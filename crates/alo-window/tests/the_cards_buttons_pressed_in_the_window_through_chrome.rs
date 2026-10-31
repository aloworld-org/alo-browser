/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 742: what running the card by hand found, through **the
//! window's own road** — the functions `alo --conversation`'s event loop
//! calls for each event ([`ConversationWindow`]), the two confined
//! renderers that draw the panel and the first screen and claim a press on
//! them, and the Chromium the card's page is open in — the machine's Chrome
//! and alo's CEF host, each the same test.
//!
//! Items 717 and 718 were tested by handing the opener its orders and the
//! renderer's claims directly, and passed while, pressed in the real
//! window, *Expand* and *Close* did nothing. Here nothing is handed past
//! the window: the cursor is moved to a point of the design's window at one
//! and the main button pressed and released, keys are read as the window
//! reads them, and every thread's news is given back to the window as its
//! event loop gives it ([`Heard`]). Each asserted:
//!
//! 1. **Return empties the box**: the address typed and Return, the box
//!    holds nothing and offers nothing, and the card answers the address.
//! 2. **The card's address follows its page**: a press on a link in the
//!    card's page, and the card shows the address the page went to.
//! 3. **Expand** puts the same tab's window over the first screen, where
//!    the window server lists it, and nothing is refused.
//! 4. **Close** closes the card and its page, and nothing is refused.
//!
//! And the window, which had the keyboard as Return was pressed, asks for
//! the front back when it is told it lost it while the card opened (what a
//! parked Chromium takes; the window server's word on it is the one event
//! this test says itself, since its process has no window in front).
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: the address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation_chrome::{ChromeOpener, Placing, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::window_server;
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

/// Where the window's outer corner is said to be, in device pixels at one:
/// low enough on the screen that the expanded window is put where asked.
const OUTER: (i32, i32) = (100, 100);

/// The box, at one: 400 into the first screen (as item 710's test presses
/// it).
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The card's *Expand to the whole window* and *Close*, at one: the
/// panel's 240, then each 36 square button's middle as the renderer's card
/// test measures them — 879 and 917 across, 103.5 down.
const EXPAND: (f64, f64) = (240.0 + 879.0 + 18.0, 103.5 + 18.0);
const CLOSE: (f64, f64) = (240.0 + 917.0 + 18.0, 103.5 + 18.0);

/// The first page: a link, large, under its top left corner.
const CARD_PAGE: &str = "<!doctype html><title>Card</title>\
    <style>body{margin:0;background:#fff}\
    #next{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#1d4ed8;color:#fff}</style>\
    <a id=next href=/wiki/Talk:Two>Two</a>";

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
                    Some("/card.html") => ("200 OK", CARD_PAGE),
                    Some("/wiki/Talk:Two") => ("200 OK", TWO_PAGE),
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
    /// The process of the Chromium the card's page opened in, once open.
    process: Option<u32>,
    /// Whether the card's page was said closed by a person.
    shut: bool,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    /// Whether the window asked for the front back.
    front_asked: bool,
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
                        "{}: {what} did not happen; the window said {:?}",
                        self.who, self.said
                    )
                })?;
            // Looked at again at least every fifth of a second, for what
            // only the window server says.
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(200)))
            else {
                continue;
            };
            let opening = matches!(heard, Heard::Chrome(Went::Opening(_)));
            match &heard {
                Heard::Chrome(Went::Opened { shown, .. }) => self.process = Some(shown.process),
                Heard::Chrome(Went::Shut(_)) => self.shut = true,
                Heard::Chrome(Went::Unopened { why, .. }) => {
                    return Err(format!("{}: the page did not open: {why}", self.who));
                }
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                _ => {}
            }
            self.window.heard(heard);
            if opening {
                // What the window server says as the card's Chromium shows
                // its parked window: the window lost the keyboard.
                self.window.focused(false);
            }
            self.front_asked |= self.window.take_front();
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
        }
        Ok(())
    }

    /// A person's click at `at`, in the window's device pixels: the cursor
    /// moved there, the main button pressed and released.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// A key pressed, as the window reads it.
    fn key(&mut self, press: &Press, text: Option<&str>) {
        self.window.key(press, text);
    }
}

/// The first screen, as the window asks it drawn now.
fn start(window: &ConversationWindow) -> alo_renderer::start::Start {
    window
        .conversation()
        .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
}

/// Whether the window server lists a window of `process` on the screen at
/// `placing`'s corner, as wide as its page.
fn expanded_at(process: u32, placing: Placing) -> Result<bool, String> {
    let listed = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
    Ok(listed
        .iter()
        .filter(|window| window.layer == 0)
        .filter_map(|window| window.bounds)
        .any(|bounds| {
            (bounds.left, bounds.top, bounds.width)
                == (
                    i64::from(placing.at.0),
                    i64::from(placing.at.1),
                    i64::from(placing.size.0),
                )
        }))
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
        outer: Some(OUTER),
    });
    window.focused(true);
    window.ask_drawn();
    let mut driven = Driven {
        window,
        heard,
        process: None,
        shut: false,
        said: Vec::new(),
        front_asked: false,
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
    let mut driven = the_window(chromium, who)?;
    // The box pressed, the address typed, Return.
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        start(&driven.window).focused
    })?;
    let address = format!("http://127.0.0.1:{port}/card.html");
    for letter in address.chars() {
        let written = letter.to_string();
        driven.key(&Press::character(&written), Some(&written));
    }
    assert_eq!(start(&driven.window).offers.len(), 3, "{who}: offered");
    driven.key(&Press::of(Key::Return), None);
    // 1. The box emptied, and nothing offered under it.
    let now = start(&driven.window);
    assert_eq!(now.typed, "", "{who}: the box kept what was typed");
    assert!(now.offers.is_empty(), "{who}: offers stayed under the box");
    driven.until("the card's first frame", |driven| {
        driven
            .window
            .conversation()
            .card()
            .is_some_and(|card| card.frame.is_some())
    })?;
    assert_eq!(
        driven
            .window
            .conversation()
            .card()
            .map(|card| card.said.clone()),
        Some(address.clone()),
        "{who}: the card answers what was typed"
    );
    assert!(
        driven.front_asked,
        "{who}: the window did not ask for the front back"
    );

    // 2. A press on the link, and the card's address follows the page.
    driven.click((240.0 + 81.0 + LINK_AT.0, 145.0 + LINK_AT.1));
    driven.until("the card's address following its page", |driven| {
        driven.window.conversation().card().is_some_and(|card| {
            card.url.to_string() == format!("http://127.0.0.1:{port}/wiki/Talk:Two")
        })
    })?;

    // 3. Expand, pressed where a person sees it.
    let process = driven.process.ok_or("no Chromium was said to be open")?;
    let placing = Placing::over_the_first_screen(OUTER, WINDOW, 1.0);
    driven.click(EXPAND);
    driven.until("Expand decided", |driven| {
        driven.window.told().is_some() || { expanded_at(process, placing).unwrap_or(false) }
    })?;
    assert_eq!(driven.window.told(), None, "{who}: Expand was refused");
    assert!(
        expanded_at(process, placing)?,
        "{who}: the window is not over the first screen at {placing:?}"
    );

    // 4. Close, pressed where a person sees it.
    driven.click(CLOSE);
    driven.until("the card closed", |driven| {
        driven.window.told().is_some() || driven.shut
    })?;
    assert_eq!(driven.window.told(), None, "{who}: Close was refused");
    assert!(
        driven.window.conversation().card().is_none(),
        "{who}: the card stayed"
    );
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
fn the_cards_buttons_pressed_in_the_window_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn the_cards_buttons_pressed_in_the_window_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("742")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
