/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 716, ADR 0075's amendment: an agent outside the browser
//! reaches the page the conversation shows as a card, over the socket ADR
//! 0055 opens, kept to the sites a person named — through **the window's
//! own road** ([`ConversationWindow`], the confined renderers, and the
//! Chromium), the machine's Chrome and alo's CEF host, each the same test,
//! and asked by `alo-ask`, a process of its own.
//!
//! The socket is the conversation's, for as long as it runs, and follows
//! the newest card ([`alo_window::conversation_agents`]). Asserted, each
//! from `alo-ask`'s answers:
//!
//! 1. **Before any card**: no tabs, and the page asked for by name is no
//!    tab open.
//! 2. **A card open on page one**: the one tab is it, its tree is read, and
//!    a page on a site not named (`localhost`, beside the `127.0.0.1`
//!    named) is refused by its site, `NotGranted`.
//! 3. **Page two replacing it**: the one tab is page two, whose tree is
//!    read, and page one is no tab open — the socket follows the card.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation::Pressed;
use alo_window::conversation_agents::relay;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn, Part};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and drawing a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// A server on this machine answering `/one.html` and `/two.html`: its
/// port.
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
                let (status, body) = match path {
                    "/one.html" => (
                        "200 OK",
                        "<!doctype html><title>One</title><h1>Page one</h1>",
                    ),
                    "/two.html" => (
                        "200 OK",
                        "<!doctype html><title>Two</title><h1>Page two</h1>",
                    ),
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

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-716-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it
/// printed, a line per answer.
fn ask(socket: &Path, lines: &[String]) -> Result<Vec<String>, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(lines.join("\n").as_bytes())
        .map_err(|why| format!("the lines were not sent: {why}"))?;
    drop(stdin);
    let finished = asking
        .wait_with_output()
        .map_err(|why| format!("alo-ask did not finish: {why}"))?;
    if !finished.status.success() {
        return Err(format!(
            "alo-ask: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// The lines asking for the tabs, the tree at each of `read`, numbered from
/// one.
fn asking(read: &[&str]) -> Vec<String> {
    core::iter::once(r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned())
        .chain(
            read.iter()
                .zip(2..)
                .map(|(tab, id)| format!(r#"{{"v":1,"id":{id},"ask":"read","tab":"{tab}"}}"#)),
        )
        .collect()
}

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every press on the first screen, as it was decided.
    pressed: Vec<Pressed>,
    who: &'static str,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it, within [`AT_MOST`].
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let by = Instant::now() + AT_MOST;
        while !done(self) {
            let Some(left) = by.checked_duration_since(Instant::now()) else {
                return Err(format!("{}: {what} did not happen", self.who));
            };
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(200)))
            else {
                continue;
            };
            match &heard {
                Heard::Chrome(Went::Unopened { url, why }) => {
                    return Err(format!("{}: {url} did not open: {why}", self.who));
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
            let _ = self.window.take_front();
            let _ = self.window.take_said();
            let _ = self.window.take_redraw();
        }
        Ok(())
    }

    /// A person's click at `at`, in the window's device pixels.
    fn press(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// `address` typed into the box, which has the keyboard, then Return;
    /// until the card shows it, loaded and drawn.
    fn go(&mut self, address: &str) -> Result<(), String> {
        for letter in address.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
        self.until(&format!("{address} shown in the card"), |driven| {
            driven.window.conversation().card().is_some_and(|card| {
                card.url.to_string() == address && !card.loading && card.frame.is_some()
            })
        })
    }
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`, its cards reached by the agents of `orders`' socket as
/// `reaching` says.
fn the_window(
    chromium: Chromium,
    reaching: alo_window::conversation_agents::Reaching,
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
    let chrome = ChromeOpener::start_reached(
        chromium,
        reaching,
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

/// Whether `answer` is a refusal of request `id` of the kind `kind`, naming
/// `tab`.
fn refused(answer: Option<&String>, id: u32, kind: &str, tab: &str) -> bool {
    answer.is_some_and(|answer| {
        answer.starts_with(&format!(
            r#"{{"answer":"refused","id":{id},"refusal":{{"kind":"{kind}","#
        )) && answer.contains(&format!(r#""tab":"{tab}""#))
    })
}

/// Whether `answer` is the tree of request `id`, holding `words`.
fn a_tree_holding(answer: Option<&String>, id: u32, words: &str) -> bool {
    answer.is_some_and(|answer| {
        answer.starts_with(&format!(r#"{{"answer":"tree","id":{id},"#)) && answer.contains(words)
    })
}

fn through(chromium: Chromium, who: &'static str) -> Result<(), String> {
    let port = serve()?;
    let one = format!("http://127.0.0.1:{port}/one.html");
    let two = format!("http://127.0.0.1:{port}/two.html");
    let elsewhere = format!("http://localhost:{port}/one.html");
    let sites = AgentSites::named([one.as_str()]).map_err(|why| why.to_string())?;
    let (orders, reaching) = relay(sites).map_err(|why| why.to_string())?;
    let path = socket(if who == "Chrome" { "chrome" } else { "host" })?;
    let listening = Listening::at(&path, orders).map_err(|why| format!("not listening: {why}"))?;
    let mut driven = the_window(chromium, reaching, who)?;

    // 1. Before any card.
    let answers = ask(&path, &asking(&[&one]))?;
    assert_eq!(
        answers.first().map(String::as_str),
        Some(r#"{"answer":"tabs","id":1,"tabs":[],"v":1}"#),
        "{who}"
    );
    assert!(
        refused(answers.get(1), 2, "NoSuchTab", &one),
        "{who}: {answers:?}"
    );

    // 2. Page one in the card.
    driven.press(THE_BOX);
    driven.until("the box pressed", |driven| !driven.pressed.is_empty())?;
    driven.go(&one)?;
    let answers = ask(&path, &asking(&[&one, &elsewhere]))?;
    assert_eq!(
        answers.first().map(String::as_str),
        Some(format!(r#"{{"answer":"tabs","id":1,"tabs":["{one}"],"v":1}}"#).as_str()),
        "{who}"
    );
    assert!(
        a_tree_holding(answers.get(1), 2, "Page one"),
        "{who}: {answers:?}"
    );
    assert!(
        refused(answers.get(2), 3, "NotGranted", &elsewhere),
        "{who}: {answers:?}"
    );

    // 3. Page two replacing it, typed into the box, which kept the
    // keyboard: the socket follows.
    driven.go(&two)?;
    let answers = ask(&path, &asking(&[&two, &one]))?;
    assert_eq!(
        answers.first().map(String::as_str),
        Some(format!(r#"{{"answer":"tabs","id":1,"tabs":["{two}"],"v":1}}"#).as_str()),
        "{who}"
    );
    assert!(
        a_tree_holding(answers.get(1), 2, "Page two"),
        "{who}: {answers:?}"
    );
    assert!(
        refused(answers.get(2), 3, "NoSuchTab", &one),
        "{who}: {answers:?}"
    );

    driven.window.finish();
    listening.stop();
    assert!(!path.exists(), "{who}: the socket is removed");
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn an_agent_reaching_a_page_the_conversation_opened_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn an_agent_reaching_a_page_the_conversation_opened_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("716")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
