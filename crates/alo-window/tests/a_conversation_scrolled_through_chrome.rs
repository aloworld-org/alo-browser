/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 703 (ADR 0085): a conversation holding a live card scrolls,
//! and a press on the card's page lands where the page is drawn — through
//! **the window's own road** ([`ConversationWindow`]), the two confined
//! renderers, the thread asking a search provider of this test's own, and
//! the Chromium the card's page is open in: the machine's Chrome and alo's
//! CEF host, each the same test.
//!
//! Each asserted:
//!
//! 1. A search, then its second title pressed: a new turn, a live card
//!    under the results, its bubble the title.
//! 2. A wheel over the card's bar scrolls the conversation back; a press on
//!    the card's page, scrolled, reaches the page at the point drawn there
//!    (the page says where it was clicked). With the scroll's offset left
//!    out of the aim, the click lands 120 lower and this fails.
//! 3. A wheel over the card's page still scrolls the page (718), and not
//!    the conversation.
//! 4. Scrolled back to the results, the first title pressed: the card put
//!    away as its address, and the new page live in a turn of its own,
//!    scrolled to its top.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: everything is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::Showing;
use alo_renderer::turn::Turn;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::card_aim::Turned;
use alo_window::conversation_chrome::{ChromeOpener, Went};
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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and loading a page,
/// past the thirty seconds Chrome itself is given to answer.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long a page is waited for to say what a person's input did.
const SAID_WITHIN: Duration = Duration::from_secs(30);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The two titles, at one: the panel's 240, then where the renderer's
/// layout assertion of the All tab puts them, 20 into each short title, each
/// site with two lines of description.
const FIRST_TITLE: (f64, f64) = (240.0 + 100.0, 202.5);
const SECOND_TITLE: (f64, f64) = (240.0 + 100.0, 321.5);

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// A page, all one colour and tall, telling the server where it was
/// clicked and how far it is scrolled, by its name.
fn page(name: &str, colour: &str) -> String {
    format!(
        "<!doctype html><title>{name}</title>\
         <style>body{{margin:0;height:4000px;background:{colour}}}</style>\
         <script>const said = (what) => fetch('/said/{name}-' + what, {{cache: 'no-store'}});\
           addEventListener('click', (e) => said('click?' + e.clientX + ',' + e.clientY));\
           addEventListener('scroll', () => said('scrolled?' + Math.round(scrollY)));\
           said('ready');</script>"
    )
}

/// Every `/said/…` the pages told the server, in order, without `/said/`.
type Said = Arc<Mutex<Vec<String>>>;

/// A provider and the two pages it finds, on this machine: its port, and
/// what the pages said.
fn serve() -> Result<(u16, Said), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let long = "A description long enough to be cut at the end of its second line, \
                so that the site is as tall as the renderer's layout assertion says \
                a site is, and the second title is where that test puts it.";
    let answer = format!(
        r#"{{"type":"search","web":{{"results":[
          {{"title":"Page one","url":"http://127.0.0.1:{port}/one.html","description":"{long}"}},
          {{"title":"Page two","url":"http://127.0.0.1:{port}/two.html","description":"{long}"}}]}}}}"#
    );
    let (one, two) = (page("one", "#2a9d8f"), page("two", "#e76f51"));
    let said = Arc::new(Mutex::new(Vec::new()));
    let pages = Arc::clone(&said);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let (answer, one, two) = (answer.clone(), one.clone(), two.clone());
            let pages = Arc::clone(&pages);
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
                let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
                let (status, kind, body) = match path.as_str() {
                    "/res/v1/web/search?q=two+pages" => ("200 OK", "application/json", answer),
                    "/one.html" => ("200 OK", "text/html; charset=utf-8", one),
                    "/two.html" => ("200 OK", "text/html; charset=utf-8", two),
                    _ => match path.strip_prefix("/said/") {
                        Some(what) => {
                            if let Ok(mut said) = pages.lock() {
                                said.push(what.to_owned());
                            }
                            ("200 OK", "text/plain", String::new())
                        }
                        None => ("404 Not Found", "text/plain", String::new()),
                    },
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, said))
}

/// Wait until the pages have said `wanted`, or say why not.
fn heard(said: &Said, wanted: &str, who: &str) -> Result<(), String> {
    let until = Instant::now() + SAID_WITHIN;
    loop {
        let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
        if so_far.iter().any(|each| each == wanted) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "{who}: the page never said {wanted}; it said {so_far:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
    said: Vec<String>,
    /// Whether the card's page was said closed.
    shut: bool,
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
                        "{}: {what} did not happen; the window said {:?}; the turns are {:?}",
                        self.who,
                        self.said,
                        self.thread()
                            .turns()
                            .iter()
                            .map(|turn| match turn {
                                Turn::Page(card) =>
                                    format!("page {} framed {}", card.url, card.frame.is_some()),
                                other => format!(
                                    "{} said {:?}",
                                    match other {
                                        Turn::Searched(_) => "search",
                                        Turn::Visited { .. } => "visited",
                                        _ => "failed",
                                    },
                                    other.said()
                                ),
                            })
                            .collect::<Vec<_>>()
                    )
                })?;
            let Ok(heard) = self.heard.recv_timeout(left) else {
                continue;
            };
            match &heard {
                Heard::Chrome(Went::Unopened { why, .. }) => {
                    return Err(format!("{}: the page did not open: {why}", self.who));
                }
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
                }
                Heard::Chrome(Went::Shut(_)) => self.shut = true,
                _ => {}
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

    /// A wheel turned `lines` toward the person — back up the conversation
    /// — with the cursor at `at`.
    fn wheel(&mut self, at: (f64, f64), lines: f32) {
        self.window.moved(at);
        self.window.wheeled(Turned::Lines {
            across: 0.0,
            down: lines,
        });
    }

    fn thread(&self) -> Thread {
        self.window
            .conversation()
            .thread()
            .cloned()
            .unwrap_or_default()
    }

    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }
}

/// The colour of the pixel at `x`, `y` of `frame`, without its alpha.
fn rgb(frame: &Frame, x: u32, y: u32) -> Option<[u8; 3]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 3)?.try_into().ok()
}

const PAPER: [u8; 3] = [0xf4, 0xf1, 0xec];

/// Whether `colour` is near enough `wanted`: a frame of Chrome's is a PNG
/// of its own drawing, decoded and scaled by nothing at one, so it is the
/// page's colour, give or take its own rounding.
fn near(colour: Option<[u8; 3]>, wanted: [u8; 3]) -> bool {
    colour.is_some_and(|colour| {
        colour
            .iter()
            .zip(wanted)
            .all(|(got, want)| got.abs_diff(want) <= 2)
    })
}

/// The conversation's window at one, both parts painted, searching through
/// the provider on `port`, its card's Chromium `chromium`.
fn the_window(chromium: Chromium, port: u16, who: &'static str) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let (drawn, told) = (tell.clone(), tell.clone());
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
        shut: false,
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

/// Step 1: a search, its second title pressed — a live card in a new turn
/// under the results, at `two`, its bubble the title, scrolled to its top.
fn search_and_open_the_second(driven: &mut Driven, said: &Said, two: &str) -> Result<(), String> {
    let who = driven.who;
    // 1. A search, its second title pressed: a live card in a new turn.
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
    })?;
    for letter in "two pages".chars() {
        let written = letter.to_string();
        driven
            .window
            .key(&Press::character(&written), Some(&written));
    }
    driven.window.key(&Press::of(Key::Return), None);
    driven.until("the sites painted", |driven| {
        matches!(
            driven.window.conversation().results().map(|r| &r.showing),
            Some(Showing::Found(found)) if !found.is_empty()
        ) && driven
            .painted()
            .is_some_and(|frame| !near(rgb(&frame, 81, 296), PAPER))
    })?;
    driven.click(SECOND_TITLE);
    driven.until("the second page's frame", |driven| {
        driven.window.told().is_some()
            || driven
                .window
                .conversation()
                .card()
                .is_some_and(|card| card.frame.is_some())
    })?;
    driven.until("the second page's turn measured", |driven| {
        driven.thread().reach().is_some()
    })?;
    assert_eq!(driven.window.told(), None, "{who}: the title was refused");
    heard(said, "two-ready", who)?;
    let card = driven
        .window
        .conversation()
        .card()
        .cloned()
        .ok_or("no card")?;
    assert_eq!(card.url.to_string(), two, "{who}");
    assert_eq!(card.said, "Page two", "{who}: the bubble is the title");
    assert_eq!(driven.thread().turns().len(), 2, "{who}");
    assert_eq!(driven.thread().down().to_bits(), 0.0_f32.to_bits());
    Ok(())
}

fn through(chromium: Chromium, who: &'static str) -> Result<(), String> {
    let (port, said) = serve()?;
    let mut driven = the_window(chromium, port, who)?;
    let one = format!("http://127.0.0.1:{port}/one.html");
    let two = format!("http://127.0.0.1:{port}/two.html");

    search_and_open_the_second(&mut driven, &said, &two)?;

    // 2. A wheel over the card's bar, three lines back: 120. The page box,
    //    from 145 + 120 = 265, drawn there.
    driven.wheel((240.0 + 500.0, 120.0), 3.0);
    assert_eq!(
        driven.thread().down().to_bits(),
        (-120.0_f32).to_bits(),
        "{who}: the bar's wheel scrolled the conversation"
    );
    let colour = [0xe7, 0x6f, 0x51];
    driven.until("the conversation drawn scrolled", |driven| {
        driven.painted().is_some_and(|frame| {
            near(rgb(&frame, 500, 200), PAPER) && near(rgb(&frame, 500, 300), colour)
        })
    })?;
    // The page's (250, 100), where it is drawn now.
    driven.click((240.0 + 81.0 + 250.0, 265.0 + 100.0));
    heard(&said, "two-click?250,100", who)?;

    // 3. A wheel over the card's page: the page's, three notches down.
    driven.wheel((240.0 + 81.0 + 250.0, 265.0 + 200.0), -3.0);
    heard(&said, "two-scrolled?120", who)?;
    assert_eq!(
        driven.thread().down().to_bits(),
        (-120.0_f32).to_bits(),
        "{who}: the page's wheel scrolled the conversation"
    );

    // 4. Back to the results, from beside the card, and the first title.
    driven.wheel((240.0 + 60.0, 400.0), 30.0);
    let reach = driven.thread().reach().ok_or("measured")?;
    assert_eq!(
        driven.thread().down().to_bits(),
        (-reach.earlier).to_bits(),
        "{who}: back to the first turn's top"
    );
    driven.until("the results drawn at the top", |driven| {
        driven.painted().is_some_and(|frame| {
            !near(rgb(&frame, 81, 177), PAPER) && !near(rgb(&frame, 500, 300), colour)
        })
    })?;
    driven.click(FIRST_TITLE);
    driven.until("the first page's frame", |driven| {
        driven.window.told().is_some()
            || driven
                .window
                .conversation()
                .card()
                .is_some_and(|card| card.url.to_string() == one && card.frame.is_some())
    })?;
    assert_eq!(driven.window.told(), None, "{who}: the title was refused");
    let thread = driven.thread();
    assert_eq!(thread.down().to_bits(), 0.0_f32.to_bits(), "{who}: its top");
    match thread.turns() {
        [
            Turn::Searched(_),
            Turn::Visited { said, url },
            Turn::Page(card),
        ] => {
            assert_eq!(said, "Page two", "{who}");
            assert_eq!(url.to_string(), two, "{who}");
            assert_eq!(card.said, "Page one", "{who}");
        }
        other => return Err(format!("{who}: the turns are {other:?}")),
    }
    heard(&said, "one-ready", who)?;

    // 5. A search typed while the card is live puts it away, and closes
    //    its page (found by hand: it was put away and left open). The box
    //    at the window's foot: 820 - 24 - 56 = 740 down, 56 tall.
    driven.click((640.0, 768.0));
    driven.until("the box given the keyboard again", |driven| {
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
    })?;
    for letter in "again".chars() {
        let written = letter.to_string();
        driven
            .window
            .key(&Press::character(&written), Some(&written));
    }
    driven.window.key(&Press::of(Key::Return), None);
    assert!(driven.window.conversation().card().is_none(), "{who}");
    driven.until("the card's page closed", |driven| driven.shut)?;

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_conversation_scrolled_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_conversation_scrolled_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("703")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
