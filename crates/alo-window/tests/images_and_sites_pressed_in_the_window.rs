/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 743 and 744 (ADR 0086): a search's images and its Sites
//! tab, through **the window's own road** — the functions
//! `alo --conversation`'s event loop calls for each event
//! ([`ConversationWindow`]), the two confined renderers that draw the panel
//! and the first screen, and the thread that asks the search provider and
//! fetches the thumbnails, every thread's news given back as the event loop
//! gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering hand-written replies in Brave's shape, and serving
//! as its image proxy thumbnails made here, by hand, as flat pictures. The
//! card's Chromium is one that is not there, so a page opened says why in
//! its turn.
//!
//! Each asserted:
//!
//! 1. Words searched draw the sites, then the row of images under them,
//!    each thumbnail drawn where the renderer's layout assertion puts its
//!    tile. The provider was asked the words alone twice — the web search
//!    and the image search, the key in each header — and each thumbnail
//!    from the proxy with no key; one named elsewhere was never asked for.
//! 2. *More sites ›* pressed shows the Sites tab: every site, the sixth's
//!    mark drawn where the fifth's description ends.
//! 3. The Images tab pressed shows the grid, its first picture drawn.
//! 4. An image pressed opens its page as a new turn, its bubble the image's
//!    title — markup in it still those characters — which, with no
//!    Chromium, says why.

#![cfg(target_os = "macos")]

use alo_paint::canvas::Canvas;
use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::Results;
use alo_renderer::results::{Found, Images, Tab};
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

/// Where the first screen begins in the window, at one: past the panel.
const PANEL: f64 = 240.0;

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// The web search's answer: six sites, each a line of description.
fn sites() -> String {
    let each: Vec<String> = (0..6)
        .map(|n| {
            format!(
                r#"{{"title":"Site number {n}","url":"https://site{n}.example/page",
                    "description":"One line of what it says.","profile":{{"name":"Site {n}"}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"type":"search","query":{{"original":"home ev charger"}},
            "web":{{"type":"search","results":[{}]}}}}"#,
        each.join(",")
    )
}

/// The colours of the thumbnails served, in order.
const COLOURS: [(u8, u8, u8); 5] = [
    (0x2f, 0x6b, 0x5e),
    (0x3e, 0x5c, 0x8a),
    (0x8a, 0x5a, 0x3e),
    (0x5a, 0x6b, 0x8a),
    (0x3d, 0x41, 0x48),
];

/// The image search's answer: five images whose thumbnails are at the
/// proxy on `port`, and a sixth whose thumbnail is named elsewhere.
fn images(port: u16) -> String {
    let mut each: Vec<String> = (0..5)
        .map(|n| {
            let title = if n == 1 {
                "A <b>wall</b> charger &amp; its cable".to_owned()
            } else {
                format!("Image {n}")
            };
            format!(
                r#"{{"type":"image_result","title":"{title}","url":"https://pictures{n}.example/page",
                    "source":"pictures{n}.example",
                    "thumbnail":{{"src":"http://127.0.0.1:{port}/thumb/{n}.png","width":500,"height":375}},
                    "properties":{{"url":"https://pictures{n}.example/original.jpg"}}}}"#
            )
        })
        .collect();
    each.push(
        r#"{"title":"Elsewhere","url":"https://elsewhere.example/page",
            "thumbnail":{"src":"http://localhost:9/thumb.png"}}"#
            .to_owned(),
    );
    format!(r#"{{"type":"images","results":[{}]}}"#, each.join(","))
}

/// A flat picture 500 × 375 in `colour`, as a PNG.
fn thumbnail((red, green, blue): (u8, u8, u8)) -> Vec<u8> {
    let canvas = Canvas::new(500, 375, Rgba::from_rgba8(red, green, blue, 255));
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
                let picture = target
                    .strip_prefix("/thumb/")
                    .and_then(|rest| rest.strip_suffix(".png"))
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| COLOURS.get(n).copied());
                let (status, kind, body) = match (target.as_str(), picture) {
                    ("/res/v1/web/search?q=home+ev+charger", _) => {
                        ("200 OK", "application/json", sites().into_bytes())
                    }
                    ("/res/v1/images/search?q=home+ev+charger", _) => {
                        ("200 OK", "application/json", images(port).into_bytes())
                    }
                    (_, Some(colour)) => ("200 OK", "image/png", thumbnail(colour)),
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

/// A thumbnail's colour as painted.
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
fn images_more_sites_and_the_images_tab_pressed_in_the_window() -> Result<(), String> {
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

    // 1. The sites, then the row of images: the first tile's 4:3 picture
    //    at (80, 459) 168 × 126 in the first screen, the fifth's at 792.
    driven.type_and_return("home ev charger");
    driven.until("the images found and their row painted", |driven| {
        driven
            .results()
            .is_some_and(|results| matches!(results.images, Images::Found(_)))
            && driven.painted_at((90, 490), painted(COLOURS[0]))
            && driven.painted_at((800, 490), painted(COLOURS[4]))
    })?;
    let results = driven.results().ok_or("no results")?;
    let Images::Found(found) = &results.images else {
        return Err("no images".to_owned());
    };
    assert_eq!(found.len(), 6, "five from the proxy, one named elsewhere");
    assert!(found.iter().take(5).all(|image| image.picture.is_some()));
    assert_eq!(
        found.get(5).map(|image| image.picture.is_some()),
        Some(false)
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
            "GET /res/v1/images/search?q=home+ev+charger HTTP/1.1",
            "GET /thumb/0.png HTTP/1.1",
            "GET /thumb/1.png HTTP/1.1",
            "GET /thumb/2.png HTTP/1.1",
            "GET /thumb/3.png HTTP/1.1",
            "GET /thumb/4.png HTTP/1.1",
        ],
        "the words alone, twice, then each thumbnail from the proxy and none from elsewhere"
    );
    for head in &heads {
        let keyed = head.contains(&format!("X-Subscription-Token: {NOT_A_KEY}"));
        assert_eq!(keyed, head.contains("/res/v1/"), "{head}");
        assert!(!head.to_lowercase().contains("referer"), "{head}");
        assert!(!head.to_lowercase().contains("cookie"), "{head}");
    }

    // 2. More sites, 20 under the row: the Sites tab, every site — the
    //    sixth's mark at (80, 648) in its tint.
    driven.click((PANEL + 120.0, 594.0));
    let sixth = alo_url::parse("https://site5.example/page").map_err(|why| format!("{why:?}"))?;
    let tint = [
        [0x2f, 0x6b, 0x5e, 0xff],
        [0x3e, 0x5c, 0x8a, 0xff],
        [0x5a, 0x6b, 0x8a, 0xff],
        [0x6b, 0x65, 0x5c, 0xff],
    ]
    .get(Found::new("", sixth, "").tint())
    .copied()
    .ok_or("a tint")?;
    driven.until("the Sites tab painted", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::Sites)
            && driven.painted_at((81, 657), tint)
    })?;
    assert_eq!(driven.window.conversation().showing().found.len(), 6);

    // 3. The Images tab, 274.8 to 349.7 along the row on Sites: the grid,
    //    its first 4:3 picture at (80, 161) 208 × 156.
    driven.click((PANEL + 312.0, 123.0));
    driven.until("the Images tab painted", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::Images)
            && driven.painted_at((90, 175), painted(COLOURS[0]))
    })?;
    assert_eq!(driven.window.conversation().showing().pictured.len(), 6);

    // 4. The second image pressed: its page opened as a new turn, its
    //    bubble its title, read as text — and, with no Chromium, why not.
    driven.click((PANEL + 400.0, 240.0));
    driven.until("the image's page not opened, said in its turn", |driven| {
        matches!(driven.thread().turns().last(), Some(Turn::Failed { .. }))
    })?;
    let thread = driven.thread();
    let Some(Turn::Failed { said, why }) = thread.turns().last() else {
        return Err(format!("not a failure: {:?}", thread.turns().last()));
    };
    assert_eq!(said, "A <b>wall</b> charger & its cable");
    assert!(
        why.starts_with("https://pictures1.example/page could not be opened"),
        "{why}"
    );
    assert_eq!(thread.turns().len(), 2);

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
