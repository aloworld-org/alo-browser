/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 745 and 746 (ADR 0087): a search's Videos and News tabs,
//! through **the window's own road** — the functions `alo --conversation`'s
//! event loop calls for each event ([`ConversationWindow`]), the two
//! confined renderers that draw the panel and the first screen, and the
//! thread that asks the search provider and fetches the thumbnails, every
//! thread's news given back as the event loop gives it.
//!
//! Never the real API, and never the network: the provider is a server on
//! `127.0.0.1` answering hand-written replies in Brave's shape, and serving
//! as its image proxy thumbnails made here, by hand, as flat pictures. The
//! card's Chromium is one that is not there, so a page opened says why in
//! its turn.
//!
//! Each asserted:
//!
//! 1. Words searched ask the web search and the image search, and neither
//!    videos nor news: they are paid for only when a person opens their
//!    tab.
//! 2. The Videos tab pressed asks the video search once, the words alone
//!    with the key, then each video's thumbnail from the proxy with none;
//!    the videos are painted, each picture where the renderer's layout
//!    assertion puts it.
//! 3. All and Videos again ask nothing more; the News tab asks the news
//!    search once, and its stories are painted; All and News again ask
//!    nothing more.
//! 4. A story's headline pressed opens its page as a new turn, its bubble
//!    the headline — markup in it still those characters — which, with no
//!    Chromium, says why.

#![cfg(target_os = "macos")]

use alo_paint::canvas::Canvas;
use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Results, Stories, Tab, Videos};
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

/// The web search's answer: four sites.
fn sites() -> String {
    let each: Vec<String> = (0..4)
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

/// The colours of the video thumbnails served, in order.
const COLOURS: [(u8, u8, u8); 2] = [(0x8a, 0x5a, 0x3e), (0x3e, 0x5c, 0x8a)];

/// The video search's answer: two videos whose thumbnails are at the
/// proxy on `port`, the first's title holding markup, and a third whose
/// thumbnail is named elsewhere.
fn videos(port: u16) -> String {
    let mut each: Vec<String> = (0..2)
        .map(|n| {
            let title = if n == 0 {
                "Fitting a <b>charger</b> &amp; its cable".to_owned()
            } else {
                format!("Video {n}")
            };
            format!(
                r#"{{"type":"video_result","title":"{title}","url":"https://videos{n}.example/watch",
                    "description":"What it shows.","age":"2 days ago",
                    "video":{{"duration":"04:0{n}","publisher":"Videos {n}","creator":"Someone"}},
                    "thumbnail":{{"src":"http://127.0.0.1:{port}/vthumb/{n}.png","original":"https://videos{n}.example/o.jpg"}}}}"#
            )
        })
        .collect();
    each.push(
        r#"{"title":"Elsewhere","url":"https://elsewhere.example/watch",
            "thumbnail":{"src":"http://localhost:9/thumb.png"}}"#
            .to_owned(),
    );
    format!(r#"{{"type":"videos","results":[{}]}}"#, each.join(","))
}

/// The news search's answer: two stories, the first's headline holding
/// markup.
fn news() -> String {
    r#"{"type":"news","results":[
        {"type":"news_result","title":"Chargers <b>&amp;</b> cables, explained",
         "url":"https://news0.example/a","description":"One line of it.","age":"2 days ago",
         "profile":{"name":"Consumer energy report"},"meta_url":{"hostname":"news0.example"}},
        {"title":"A second story","url":"https://news1.example/b","age":"1 week ago",
         "description":"One line of it.","meta_url":{"hostname":"news1.example"}}]}"#
        .to_owned()
}

/// A flat picture 480 × 270 in `colour`, as a PNG.
fn thumbnail((red, green, blue): (u8, u8, u8)) -> Vec<u8> {
    let canvas = Canvas::new(480, 270, Rgba::from_rgba8(red, green, blue, 255));
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
                    .strip_prefix("/vthumb/")
                    .and_then(|rest| rest.strip_suffix(".png"))
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| COLOURS.get(n).copied());
                let json = |body: String| ("200 OK", "application/json", body.into_bytes());
                let (status, kind, body) = match (target.as_str(), picture) {
                    ("/res/v1/web/search?q=home+ev+charger", _) => json(sites()),
                    ("/res/v1/images/search?q=home+ev+charger", _) => {
                        json(r#"{"type":"images","results":[]}"#.to_owned())
                    }
                    ("/res/v1/videos/search?q=home+ev+charger", _) => json(videos(port)),
                    ("/res/v1/news/search?q=home+ev+charger", _) => json(news()),
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

/// The request line of each head the provider was asked since last.
fn lines(asking: &Receiver<String>) -> Vec<String> {
    asking
        .try_iter()
        .map(|head| {
            assert!(!head.to_lowercase().contains("referer"), "{head}");
            assert!(!head.to_lowercase().contains("cookie"), "{head}");
            let keyed = head.contains(&format!("X-Subscription-Token: {NOT_A_KEY}"));
            assert_eq!(keyed, head.contains("/res/v1/"), "{head}");
            head.lines().next().unwrap_or_default().to_owned()
        })
        .collect()
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
        )?
        .with_videos(&format!("http://127.0.0.1:{port}/res/v1/videos/search"))?
        .with_news(&format!("http://127.0.0.1:{port}/res/v1/news/search"))?;
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

/// A tab of the newest search's row, by its middle along the row on the
/// All tab, and the row's middle, in the window.
fn tab(at: f64) -> (f64, f64) {
    (PANEL + at, 123.0)
}

/// The middles of All (80 to 125.5), Videos (352.3 to 426) and News (428.8
/// to 492.7) along the row, which every tab draws alike.
const ALL: f64 = 100.0;
const VIDEOS: f64 = 390.0;
const NEWS: f64 = 460.0;

#[test]
fn videos_and_news_asked_once_each_when_their_tabs_are_pressed() -> Result<(), String> {
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

    // 1. The sites and the images; neither videos nor news.
    driven.type_and_return("home ev charger");
    driven.until("the sites found and the images answered", |driven| {
        driven.results().is_some_and(|results| {
            matches!(results.images, alo_renderer::results::Images::Found(_))
        })
    })?;
    assert_eq!(
        lines(&asking),
        vec![
            "GET /res/v1/web/search?q=home+ev+charger HTTP/1.1",
            "GET /res/v1/images/search?q=home+ev+charger HTTP/1.1",
        ],
        "videos and news are asked only when their tab is opened"
    );
    let results = driven.results().ok_or("no results")?;
    assert!(matches!(results.videos, Videos::NotAsked));
    assert!(matches!(results.news, Stories::NotAsked));

    videos_pressed(&mut driven, &asking)?;
    news_pressed(&mut driven, &asking)?;
    a_headline_pressed(&mut driven, &asking)?;

    driven.window.finish();
    match driven.window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}

fn videos_pressed(driven: &mut Driven, asking: &Receiver<String>) -> Result<(), String> {
    // 2. Videos: asked once, the words alone with the key, then each
    //    thumbnail from the proxy and none from elsewhere; the first
    //    picture painted in its tile at (80, 164) 200 × 112, the second at
    //    (80, 296).
    driven.click(tab(VIDEOS));
    driven.until("the videos found and painted", |driven| {
        driven
            .results()
            .is_some_and(|results| matches!(results.videos, Videos::Found(_)))
            && driven.painted_at((100, 180), painted(COLOURS[0]))
            && driven.painted_at((100, 312), painted(COLOURS[1]))
    })?;
    assert_eq!(
        lines(asking),
        vec![
            "GET /res/v1/videos/search?q=home+ev+charger HTTP/1.1",
            "GET /vthumb/0.png HTTP/1.1",
            "GET /vthumb/1.png HTTP/1.1",
        ],
        "the words alone, then each thumbnail from the proxy and none from elsewhere"
    );
    let results = driven.results().ok_or("no results")?;
    let found = results.videos.found();
    assert_eq!(found.len(), 3);
    assert_eq!(
        found
            .first()
            .map(|video| (video.title.as_str(), video.length.as_str())),
        Some(("Fitting a <b>charger</b> & its cable", "04:00"))
    );
    assert_eq!(
        found
            .iter()
            .map(|video| video.picture.is_some())
            .collect::<Vec<_>>(),
        vec![true, true, false]
    );
    assert_eq!(driven.window.conversation().showing().found.len(), 3);
    Ok(())
}

fn news_pressed(driven: &mut Driven, asking: &Receiver<String>) -> Result<(), String> {
    // 3. All and Videos again ask nothing; News asks once — the videos'
    //    answer is not asked again before it — and paints its stories, the
    //    line under the first at 256; All and News again ask nothing.
    driven.click(tab(ALL));
    driven.until("All shown again", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::All)
    })?;
    driven.click(tab(VIDEOS));
    driven.until("Videos shown again", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::Videos)
    })?;
    driven.click(tab(NEWS));
    driven.until("the news found and painted", |driven| {
        driven
            .results()
            .is_some_and(|results| matches!(results.news, Stories::Found(_)))
            && driven.painted_at((500, 256), [0xe6, 0xe0, 0xd6, 0xff])
    })?;
    driven.click(tab(ALL));
    driven.until("All shown once more", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::All)
    })?;
    driven.click(tab(NEWS));
    driven.until("News shown again", |driven| {
        driven
            .results()
            .is_some_and(|results| results.tab == Tab::News)
            && driven.painted_at((500, 256), [0xe6, 0xe0, 0xd6, 0xff])
    })?;
    // The thread asks in the order tabs were opened, so a second ask for
    // the videos, opened before News, would have come before News's answer.
    assert_eq!(
        lines(asking),
        vec!["GET /res/v1/news/search?q=home+ev+charger HTTP/1.1"],
        "News once; Videos and News never again"
    );
    let results = driven.results().ok_or("no results")?;
    assert_eq!(
        results
            .news
            .found()
            .iter()
            .map(|story| (story.site.as_str(), story.age.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Consumer energy report", "2 days ago"),
            ("news1.example", "1 week ago")
        ]
    );
    Ok(())
}

fn a_headline_pressed(driven: &mut Driven, asking: &Receiver<String>) -> Result<(), String> {
    // 4. The first headline, at (80, 192) 20 tall: its page opened as a
    //    new turn, its bubble the headline as text — and, with no
    //    Chromium, why not.
    driven.click((PANEL + 150.0, 202.0));
    driven.until("the story's page not opened, said in its turn", |driven| {
        matches!(driven.thread().turns().last(), Some(Turn::Failed { .. }))
    })?;
    let thread = driven.thread();
    let Some(Turn::Failed { said, why }) = thread.turns().last() else {
        return Err(format!("not a failure: {:?}", thread.turns().last()));
    };
    assert_eq!(said, "Chargers <b>&</b> cables, explained");
    assert!(
        why.starts_with("https://news0.example/a could not be opened"),
        "{why}"
    );
    assert_eq!(thread.turns().len(), 2);
    assert!(
        lines(asking).is_empty(),
        "a page opened asks the provider nothing"
    );
    Ok(())
}
