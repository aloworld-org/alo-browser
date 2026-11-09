/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 770: a card says *Loading* from its navigation's start, not
//! its commit — through **the window's own road** ([`ConversationWindow`],
//! the two confined renderers, and the Chromium), the machine's Chrome and
//! alo's CEF host, each the same test.
//!
//! The card said *Loading* only once its page's own frame had begun a
//! document: a link pressed in the card said nothing for as long as its
//! server took to answer, where Chrome's own tab turns its spinner from the
//! press. Here the server holds each page asked for while the test says
//! so, and answers it when let:
//!
//! 1. a link pressed in the card's first page, its page held: the card
//!    says **Loading** while it is still at the first page — nothing
//!    committed — and still does a while later; let go, the link's page
//!    commits and loads, and the card says it no more;
//! 2. a link to a page the server answers `204 No Content` — a navigation
//!    that **never commits**: *Loading* while it is held, and once it is
//!    answered the card says it no more, at the page it was at, still
//!    drawn;
//! 3. the card's *Back* pressed, the page gone back to held — the first
//!    page keeps an `unload` listener, so Chrome keeps no copy of it whole
//!    and asks the server again: *Loading* while the card is still at the
//!    link's page, and once let go, back at the first page, loaded.
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
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and loading a page,
/// past the thirty seconds Chrome itself is given to answer.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long a held page is kept held once the card has said *Loading*, to
/// see that it goes on saying it while nothing commits.
const STILL: Duration = Duration::from_secs(2);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The card's *Back*, at one: the panel's 240, then the 36 square button's
/// middle as the renderer's card test measures it — 87 across, 103.5 down.
const BACK: (f64, f64) = (240.0 + 87.0 + 18.0, 103.5 + 18.0);

/// Where each page's link is pressed, in the window's device pixels: the
/// card's page box at 240 + 81 across and 145 down, and the link's middle
/// in the page's CSS pixels at one.
const LINK: (f64, f64) = (240.0 + 81.0 + 250.0, 145.0 + 210.0);

/// The first page: a link, large, under its top left corner, and an
/// `unload` listener, so that Chrome does not keep it whole when it is
/// left (its back-forward cache) and going back to it asks the server.
const ONE_PAGE: &str = "<!doctype html><title>One</title>\
    <style>body{margin:0;background:#fff}\
    a{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#1d4ed8;color:#fff}</style>\
    <a href=/two.html>Two</a>\
    <script>addEventListener('unload', () => {})</script>";

/// The link's page: a link, in the same place, to a page that is never a
/// document.
const TWO_PAGE: &str = "<!doctype html><title>Two</title>\
    <style>body{margin:0;background:#e76f51}\
    a{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#264653;color:#fff}</style>\
    <a href=/nothing>Nothing</a>";

/// A server on this machine answering the pages, each held while `hold` is
/// set: its port. Every page asked for is written into `asked` as it is
/// asked, before it is held.
fn serve(hold: Arc<AtomicBool>, asked: Arc<Mutex<Vec<String>>>) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let hold = Arc::clone(&hold);
            let asked = Arc::clone(&asked);
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
                let path = read.split(' ').nth(1).unwrap_or_default().to_owned();
                let (status, body) = match path.as_str() {
                    "/one.html" => ("200 OK", ONE_PAGE),
                    "/two.html" => ("200 OK", TWO_PAGE),
                    "/nothing" => ("204 No Content", ""),
                    _ => ("404 Not Found", ""),
                };
                if status != "404 Not Found" {
                    if let Ok(mut asked) = asked.lock() {
                        asked.push(path);
                    }
                    let until = Instant::now() + AT_MOST * 2;
                    while hold.load(Ordering::SeqCst) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(20));
                    }
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

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every press on the first screen, as it was decided.
    pressed: Vec<Pressed>,
    /// Every time Chrome's thread said the page was loading or not.
    loading: Vec<bool>,
    /// How many times the first screen was painted.
    painted: usize,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    /// Every page the server was asked for.
    asked: Arc<Mutex<Vec<String>>>,
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
                Heard::Chrome(Went::Unopened { why, .. }) => {
                    return Err(format!("{}: the page did not open: {why}", self.who));
                }
                Heard::Chrome(Went::Closed(_)) => {
                    return Err(format!("{}: the card's Chromium closed", self.who));
                }
                Heard::Chrome(Went::Loading(loading)) => self.loading.push(*loading),
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
        Ok(true)
    }

    /// [`Driven::until_or`], within [`AT_MOST`], failing if it did not
    /// happen.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        if self.until_or(Instant::now() + AT_MOST, done)? {
            Ok(())
        } else {
            Err(format!(
                "{}: {what} did not happen; the window said {:?}, Chrome's thread told \
                 loading {:?}, the server was asked {:?}, the card is {:?}",
                self.who,
                self.said,
                self.loading,
                self.asked(),
                self.card()
            ))
        }
    }

    /// The card's address, whether it is loading, and whether it can go
    /// back.
    fn card(&self) -> Option<(String, bool, bool)> {
        self.window
            .conversation()
            .card()
            .map(|card| (card.url.to_string(), card.loading, card.back))
    }

    /// Whether the card is at `address`, drawn, and saying `loading` or
    /// not.
    fn at(&self, address: &str, loading: bool) -> bool {
        self.card()
            .is_some_and(|(url, said, _)| url == address && said == loading)
            && self
                .window
                .conversation()
                .card()
                .is_some_and(|card| card.frame.is_some())
    }

    /// Every page the server was asked for, in order.
    fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
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

    /// With the server holding `page`, asked for after `then` pages, wait
    /// for the card to say *Loading* while it is still at `from` — nothing
    /// committed — and go on saying it there for [`STILL`].
    fn loading_while_held(&mut self, page: &str, then: usize, from: &str) -> Result<(), String> {
        self.until("the held page asked for, the card loading", |driven| {
            driven.asked().get(then).is_some_and(|asked| asked == page)
                && driven.card().is_some_and(|(_, loading, _)| loading)
        })?;
        let (url, loading, _) = self.card().unwrap_or_default();
        if url != from || !loading {
            return Err(format!(
                "{}: while {page} was held the card was at {url}, loading {loading}",
                self.who
            ));
        }
        let moved = self.until_or(Instant::now() + STILL, |driven| {
            driven
                .card()
                .is_none_or(|(url, loading, _)| url != from || !loading)
        })?;
        if moved {
            return Err(format!(
                "{}: while {page} was still held the card became {:?}",
                self.who,
                self.card()
            ));
        }
        Ok(())
    }
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`.
fn the_window(
    chromium: Chromium,
    asked: Arc<Mutex<Vec<String>>>,
    who: &'static str,
) -> Result<Driven, String> {
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
        painted: 0,
        said: Vec::new(),
        asked,
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
    let hold = Arc::new(AtomicBool::new(false));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let port = serve(Arc::clone(&hold), Arc::clone(&asked))?;
    let one = format!("http://127.0.0.1:{port}/one.html");
    let two = format!("http://127.0.0.1:{port}/two.html");
    let mut driven = the_window(chromium, asked, who)?;
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
    driven.until("the first page, drawn and loaded", |driven| {
        driven.at(&one, false)
    })?;
    // A page this quick may have loaded before the card's first pass told
    // anything: what is told from here is each navigation's.
    assert_eq!(driven.loading.last(), Some(&false), "{who}: opened");
    driven.loading.clear();

    // 1. A link pressed, its page held: Loading before anything commits.
    hold.store(true, Ordering::SeqCst);
    driven.drawn_as_asked()?;
    driven.press(LINK);
    driven.loading_while_held("/two.html", 1, &one)?;
    hold.store(false, Ordering::SeqCst);
    driven.until("the link's page, drawn and loaded", |driven| {
        driven.at(&two, false)
    })?;
    assert_eq!(driven.loading, vec![true, false], "{who}");

    // 2. A link to no document: Loading while it is held, and no more once
    // it is answered, at the page the card was at.
    hold.store(true, Ordering::SeqCst);
    driven.drawn_as_asked()?;
    driven.press(LINK);
    driven.loading_while_held("/nothing", 2, &two)?;
    let painted = driven.painted;
    hold.store(false, Ordering::SeqCst);
    // Painted once the card no longer says it: nothing else changes on the
    // first screen, so the press below is decided against that.
    driven.until("no longer loading, nothing committed, painted", |driven| {
        driven.card().is_some_and(|(_, loading, _)| !loading) && driven.painted > painted
    })?;
    assert!(
        driven.at(&two, false),
        "{who}: the 204 moved the card: {:?}",
        driven.card()
    );
    assert_eq!(driven.loading, vec![true, false, true, false], "{who}");

    // 3. Back, the page gone back to held: Loading from the press.
    hold.store(true, Ordering::SeqCst);
    assert_eq!(
        driven.click(BACK)?,
        Pressed::Back,
        "{who}: Back is drawn enabled"
    );
    driven.loading_while_held("/one.html", 3, &two)?;
    hold.store(false, Ordering::SeqCst);
    driven.until("back at the first page, drawn and loaded", |driven| {
        driven.at(&one, false)
    })?;
    assert_eq!(
        driven.loading,
        vec![true, false, true, false, true, false],
        "{who}"
    );
    assert_eq!(
        driven.asked(),
        ["/one.html", "/two.html", "/nothing", "/one.html"],
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
fn a_cards_loading_from_a_navigations_start_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_cards_loading_from_a_navigations_start_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("770")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
