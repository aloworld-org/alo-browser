/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 714, 759 and 711 through **the window's own road**: the
//! functions `alo --conversation`'s event loop calls for each event
//! ([`ConversationWindow`]), the two confined renderers that draw the panel
//! and the first screen, and the thread that asks the search provider,
//! every thread's news given back as the event loop gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering hand-written replies in Brave's shape. The card's
//! Chromium is one that is not there; a live card is had by giving the
//! window what a card's Chrome thread tells it — opening, then a frame —
//! as the event loop would.
//!
//! Each asserted:
//!
//! 1. Under the box on the first screen, a press on the second offer chooses
//!    it — asking, not set up, which leaves what was typed — and on the
//!    first searches, as Return would (714).
//! 2. Above the box in a conversation, a press on the first offer searches.
//! 3. Over a live card, the same point is the page's with nothing offered
//!    and the offer's while words are offered over the box (759).
//! 4. *Search chats* takes the keyboard from the box, keys reach it and not
//!    the box, and the panel shows only the chats named so (711).
//! 5. The button that hides the panel lays the window out beside the rail,
//!    whose *New chat* and *Show the panel* work, at one and at two.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::Showing;
use alo_renderer::turn::Turn;
use alo_renderer::typed::Offer;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::{Side, start_viewport_as};
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

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// The paper the offer Return chooses stands on, and the conversation's.
const PAPER: [u8; 4] = [0xf4, 0xf1, 0xec, 0xff];

/// The card's frame's header colour, 30 in and down of the page box.
const HEADER: [u8; 4] = [0x1d, 0x4e, 0xd8, 0xff];

/// One search's answer: one site.
const ONE_SITE: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"Lifetimes explained","url":"https://e.example/lifetimes",
     "description":"Every reference has a lifetime."}
  ]}}"#;

/// A provider on this machine, answering two searches and refusing the
/// rest: its port.
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
                    Some(
                        "/res/v1/web/search?q=lifetimes"
                        | "/res/v1/web/search?q=rust+borrow+checker",
                    ) => ("200 OK", ONE_SITE),
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

    /// `text` typed, as the window reads keys.
    fn type_in(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
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

    /// The sizes of the panel's and the first screen's frames, as last
    /// painted.
    fn sizes(&self) -> Option<((u32, u32), (u32, u32))> {
        let scene = self.window.scene()?;
        let size = |frame: &Frame| (frame.width, frame.height);
        Some((scene.panel.map(size)?, scene.start.map(size)?))
    }

    /// Whether the first screen's pixel at `x`, `y` was last painted
    /// `colour`.
    fn shows(&self, x: u32, y: u32, colour: [u8; 4]) -> bool {
        self.painted()
            .and_then(|frame| pixel(&frame, x, y))
            .is_some_and(|painted| painted == colour)
    }

    /// What the box holds, and whether it has the keyboard.
    fn the_box(&self) -> (String, bool) {
        let window = self.window.conversation();
        let start = window.start(
            start_viewport_as(WINDOW, Scale::ONE, window.side()),
            Scale::ONE,
        );
        (start.typed, start.focused)
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The conversation's window of `window` device pixels at `scale`, both
/// parts painted, searching through the provider on `port`, its card's
/// Chromium one that is not there.
fn the_window(port: u16, window: (u32, u32), scale: Scale) -> Result<Driven, String> {
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
    let mut conversation =
        ConversationWindow::new(Some(scale), drawing, chrome).with_search(searcher);
    conversation.measured_as(Measured {
        size: window,
        scale_factor: f64::from(scale.factor()),
        outer: Some((100, 100)),
    });
    conversation.focused(true);
    conversation.ask_drawn();
    let mut driven = Driven {
        window: conversation,
        heard,
        said: Vec::new(),
    };
    driven.until("both parts painted", |driven| driven.sizes().is_some())?;
    Ok(driven)
}

/// Whether the newest turn is a search for `words` that has been answered.
fn searched(driven: &Driven, words: &str) -> bool {
    matches!(
        driven.thread().turns().last(),
        Some(Turn::Searched(results))
            if results.said == words && !matches!(results.showing, Showing::Searching)
    )
}

/// Steps 1 and 2: offers pressed under the box, and above it.
fn offers_pressed(driven: &mut Driven) -> Result<(), String> {
    // 1. The box at (640, 285) of the window; "lifetimes" offers searching
    //    and asking, 706 × 52 from (167, 333) of the first screen.
    driven.click((640.0, 285.0));
    driven.until("the box given the keyboard", |driven| driven.the_box().1)?;
    driven.type_in("lifetimes");
    driven.until("the offers painted under the box", |driven| {
        driven.shows(400, 359, PAPER)
    })?;
    // The second, asking: chosen, not set up, what was typed left.
    driven.click((640.0, 411.0));
    driven.until("asking chosen", |driven| {
        matches!(driven.window.conversation().chosen(), Some(Offer::Ask(_)))
    })?;
    assert_eq!(driven.the_box(), ("lifetimes".to_owned(), true));
    assert!(driven.thread().turns().is_empty());
    // The first, searching: as Return, the box emptied and keeping the
    // keyboard.
    driven.click((640.0, 359.0));
    driven.until("the search pressed answered", |driven| {
        searched(driven, "lifetimes")
    })?;
    assert_eq!(driven.the_box(), (String::new(), true));

    // 2. A conversation: two offers stand above the box at 740, their card
    //    118 tall from 614, the first from 621 to 673.
    driven.type_in("rust borrow checker");
    driven.until("the offers painted above the box", |driven| {
        driven.shows(400, 647, PAPER)
    })?;
    driven.click((640.0, 647.0));
    driven.until("the search pressed above the box answered", |driven| {
        searched(driven, "rust borrow checker")
    })?;
    assert_eq!(driven.thread().turns().len(), 2);
    Ok(())
}

/// Step 3: over a live card, a point is the page's with nothing offered and
/// the offer's while words are offered.
fn offers_over_a_card(driven: &mut Driven) -> Result<(), String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html").map_err(|why| why.to_string())?;
    let png = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../alo-renderer/tests/references/card-frame.png"),
    )
    .map_err(|why| format!("no card frame: {why}"))?;
    driven.window.heard(Heard::Chrome(Went::Opening(url)));
    driven.window.heard(Heard::Chrome(Went::Framed(png)));
    // The page box at (81, 145) of the first screen, its header 30 in.
    driven.until("the card's picture painted", |driven| {
        driven.shows(111, 175, HEADER)
    })?;
    // Nothing offered: (640, 647) is the page's, 81 + 240 from its left and
    // 145 from its top, and the page takes the keyboard.
    assert!(!driven.window.conversation().keys_to_the_card());
    driven.click((640.0, 647.0));
    assert!(driven.window.conversation().keys_to_the_card());
    // The box, at the window's foot, takes it back; words offered over it.
    driven.click((640.0, 768.0));
    driven.until("the box given the keyboard back", |driven| {
        driven.the_box().1 && !driven.window.conversation().keys_to_the_card()
    })?;
    driven.type_in("what is x");
    driven.until("the offers painted over the card's page", |driven| {
        driven.shows(400, 647, PAPER)
    })?;
    // The same point is the first offer's now, never the page's.
    driven.click((640.0, 647.0));
    assert!(
        !driven.window.conversation().keys_to_the_card(),
        "a press on an offer over the page went to the page"
    );
    driven.until("the search over the card answered", |driven| {
        searched(driven, "what is x")
    })?;
    assert!(
        driven.window.conversation().card().is_none(),
        "put away by the search"
    );
    Ok(())
}

/// Step 4: Search chats.
fn search_chats(driven: &mut Driven) -> Result<(), String> {
    // Search chats 220 × 38 at (10, 102) of the panel.
    driven.click((120.0, 121.0));
    driven.until("Search chats given the keyboard", |driven| {
        driven.window.conversation().find().focused()
    })?;
    assert!(!driven.the_box().1, "the box let it go");
    driven.type_in("NEW");
    assert_eq!(driven.window.conversation().find().typed(), "NEW");
    assert_eq!(driven.the_box().0, "", "nothing typed reached the box");
    assert_eq!(driven.window.conversation().showing().chats.len(), 1);
    driven.type_in("x");
    assert!(driven.window.conversation().showing().chats.is_empty());
    // With the keyboard, Search chats is drawn white.
    driven.until("the panel painted searching", |driven| {
        driven
            .window
            .scene()
            .and_then(|scene| scene.panel.and_then(|frame| pixel(frame, 120, 104)))
            .is_some_and(|colour| colour == [0xff, 0xff, 0xff, 0xff])
    })?;
    driven.window.key(&Press::of(Key::Backspace), None);
    assert_eq!(driven.window.conversation().showing().chats.len(), 1);
    // New chat takes the keyboard from it, and makes a second chat found.
    driven.click((60.0, 75.0));
    driven.until("a second chat", |driven| {
        driven.window.conversation().showing().chats.len() == 2
    })?;
    assert!(!driven.window.conversation().find().focused());
    Ok(())
}

/// Step 5, at one: hidden, the window is the rail and the first screen
/// beside it; the rail's New chat and Show the panel.
fn the_rail(driven: &mut Driven) -> Result<(), String> {
    // The button 36 square at (194, 11).
    driven.click((212.0, 29.0));
    driven.until("the window laid out beside the rail", |driven| {
        driven.sizes() == Some(((64, 820), (1216, 820)))
    })?;
    assert_eq!(driven.window.conversation().side(), Side::Rail);
    // The new chat's greeting: the box 720 wide, centred in the 1216 left,
    // from 64 + 248, pressed there — where the panel's 240 was, nothing.
    driven.click((64.0 + 248.0 + 240.0, 285.0));
    driven.until("the box pressed where it is now", |driven| {
        driven.the_box().1
    })?;
    // The rail's New chat, 44 square at (10, 56).
    let chats = driven.window.conversation().showing();
    assert!(chats.chats.is_empty(), "the rail shows no chat");
    driven.click((32.0, 78.0));
    driven.until("a chat made from the rail", |driven| {
        driven.thread().turns().is_empty() && !driven.the_box().1
    })?;
    // Show the panel, 44 square at (10, 764).
    driven.click((32.0, 786.0));
    driven.until("the panel shown again", |driven| {
        driven.sizes() == Some(((240, 820), (1040, 820)))
    })?;
    assert_eq!(driven.window.conversation().side(), Side::Panel);
    assert_eq!(driven.window.conversation().showing().chats.len(), 3);
    Ok(())
}

#[test]
fn offers_and_the_panels_buttons_pressed_through_the_windows_own_road() -> Result<(), String> {
    let port = provide()?;
    let mut driven = the_window(port, WINDOW, Scale::ONE)?;
    offers_pressed(&mut driven)?;
    offers_over_a_card(&mut driven)?;
    search_chats(&mut driven)?;
    the_rail(&mut driven)?;
    driven.window.finish();
    if let Some(why) = driven.window.unfinished() {
        return Err(why.to_owned());
    }

    // 5, at two: the same window on a Retina screen.
    let mut driven = the_window(port, (2560, 1640), Scale::TWO)?;
    driven.click((424.0, 58.0));
    driven.until("the window laid out beside the rail at two", |driven| {
        driven.sizes() == Some(((128, 1640), (2432, 1640)))
    })?;
    driven.click((64.0, 1572.0));
    driven.until("the panel shown again at two", |driven| {
        driven.sizes() == Some(((480, 1640), (2080, 1640)))
    })?;
    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
