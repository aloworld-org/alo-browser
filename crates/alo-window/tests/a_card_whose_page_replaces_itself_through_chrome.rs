/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 749: what running a search's result by hand found, through
//! **the window's own road** ([`ConversationWindow`], the confined
//! renderers, and the Chromium) — the machine's Chrome and alo's CEF host,
//! each the same test.
//!
//! By hand, a title on `www.reddit.com` opened a card that stayed white,
//! then closed with nothing said; the next address typed went nowhere.
//! Two causes, each asserted here against pages of the test's own:
//!
//! 1. **A page that replaces itself before it loads.** Reddit first
//!    answers a challenge whose script goes on to the page itself
//!    (`location.replace`); the challenge's own `load` never comes, and the
//!    tab waited for it — its loader, the one `Page.navigate` named — for
//!    all of Chrome's thirty seconds, and the card was said unopened. Here
//!    `/challenge.html` holds its `load` back with a picture the server
//!    never answers, and replaces itself with `/real.html`: the card must
//!    show a frame, at `/real.html`.
//! 2. **A card that fails leaves the conversation able to open the next.**
//!    A press on a site's title takes the keyboard from the box (here a
//!    press on the card's bar, which claims nothing, does the same); a card
//!    with no picture cannot be pressed for it; and the card failing gave
//!    it to nothing, so the next address typed by hand reached nothing.
//!    Here `/never.html` is never answered, so its navigation never
//!    commits (a page that commits and never loads is a card, queue item
//!    751, ADR 0089): the card is said unopened — kept in the first
//!    screen's line and said on the terminal, not wiped by the next paint —
//!    the box has the keyboard again, and the next address, typed with no
//!    press first, opens.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
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

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// A point of the card's bar that claims nothing — right of its address,
/// left of *Expand* — at one: the panel's 240, then the bar's middle.
const THE_BAR: (f64, f64) = (240.0 + 700.0, 103.5 + 18.0);

/// The challenge: a picture never answered holds its `load` back, and its
/// script goes on to the page itself, as `www.reddit.com`'s does.
const CHALLENGE: &str = "<!doctype html><title>Challenge</title>\
    <img src=/held alt=\"\"><script>location.replace('/real.html')</script>";

/// The page itself.
const REAL: &str = "<!doctype html><title>Real</title>\
    <style>body{margin:0;background:#2a9d8f}</style><h1>Real</h1>";

/// A server on this machine answering the pages, holding `/held` open
/// unanswered for as long as the test may run: its port.
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
                    Some("/held" | "/never.html") => {
                        // Never answered while the test runs.
                        std::thread::sleep(AT_MOST * 2);
                        return;
                    }
                    Some("/challenge.html") => ("200 OK", CHALLENGE),
                    Some("/real.html") => ("200 OK", REAL),
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
                        "{}: {what} did not happen; the window said {:?}",
                        self.who, self.said
                    )
                })?;
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
        Ok(())
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

    /// Whether the box has the keyboard, as the window asks it drawn now.
    fn box_focused(&self) -> bool {
        self.window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
    }

    /// Whether the card shows a frame of the page at `address`.
    fn card_framed_at(&self, address: &str) -> bool {
        self.window
            .conversation()
            .card()
            .is_some_and(|card| card.frame.is_some() && card.url.to_string() == address)
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
    let mut driven = the_window(chromium, who)?;
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", Driven::box_focused)?;

    // 1. A page that replaces itself before it loads opens in the card.
    let real = format!("http://127.0.0.1:{port}/real.html");
    driven.type_and_return(&format!("http://127.0.0.1:{port}/challenge.html"));
    driven.until("a frame of the page the challenge went on to", |driven| {
        !driven.unopened.is_empty() || driven.card_framed_at(&real)
    })?;
    assert_eq!(
        driven.unopened,
        Vec::<String>::new(),
        "{who}: a page that replaced itself was said unopened"
    );

    // 2. A page never answered, the box's keyboard taken by a press on the
    // card's bar while it waits.
    let never = format!("http://127.0.0.1:{port}/never.html");
    driven.type_and_return(&never);
    driven.until("the card opening", |driven| {
        driven
            .window
            .conversation()
            .card()
            .is_some_and(|card| card.url.to_string() == never)
    })?;
    driven.click(THE_BAR);
    driven.until("the box's keyboard taken by the press", |driven| {
        !driven.box_focused()
    })?;
    driven.until("the page said unopened", |driven| {
        !driven.unopened.is_empty()
    })?;
    let told = driven.window.told().unwrap_or_default().to_owned();
    assert!(
        told.starts_with(&format!("{never} could not be opened: ")),
        "{who}: the line says {told:?}"
    );
    assert!(
        driven.said.contains(&told),
        "{who}: not said on the terminal: {:?}",
        driven.said
    );
    driven.until("the first screen painted after the card", |driven| {
        driven.window.conversation().card().is_none()
            && driven.window.scene().and_then(|scene| scene.start_sentence) == Some(told.as_str())
    })?;
    assert!(
        driven.box_focused(),
        "{who}: the box did not have the keyboard back"
    );
    // The next address, typed with no press first, opens.
    driven.type_and_return(&real);
    driven.until("the next address opened in the card", |driven| {
        driven.card_framed_at(&real)
    })?;
    assert_eq!(driven.unopened.len(), 1, "{who}: {:?}", driven.unopened);

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_card_whose_page_replaces_itself_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_card_whose_page_replaces_itself_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("749")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
