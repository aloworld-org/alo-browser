/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 956 and 958, ADR 0112 §§ 1 and 4: an agent outside alo opens
//! a new tab — a new chat — and a page in the card it shows, over the
//! conversation's socket, through **the window's own road**
//! ([`ConversationWindow`], the confined renderers, and the Chromium), the
//! machine's Chrome and alo's CEF host, each the same test, asked by
//! `alo-ask`, a process of its own; and the window is called what the
//! card's page is called.
//!
//! Asserted, from `alo-ask`'s answers and the window's own state:
//!
//! 1. **`open` with no tab** opens a new chat whose card shows page one,
//!    answered `opened` once it has; the one tab is it; the window is
//!    called *One*, its title.
//! 2. **`open` in that tab** sends the card's page to page two, answered by
//!    where it arrived; the window is called *Two*.
//! 3. **`new_tab`** is a new chat with nothing in it: no card, no tab, and
//!    the window is called *alo* again.
//! 4. An address on a site not named is refused by its site, and the window
//!    is never handed it.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_renderer::host::Renderers;
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation_agents::relay;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and drawing a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

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
    let root = std::env::temp_dir().join(format!("alo-956c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// `alo-ask`, started with `lines` to send, answering as the window is
/// driven.
fn asking(socket: &Path, lines: &[String]) -> Result<Child, String> {
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
    Ok(asking)
}

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    who: &'static str,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does —
    /// its title asked after each, as the event loop asks — until `done`
    /// holds of it, within [`AT_MOST`].
    fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) -> Result<(), String> {
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
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{}: {part} said {said}", self.who));
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

    /// What `lines` were answered, the window driven while `alo-ask` waits
    /// for it — a new tab is the window's to open.
    fn ask(&mut self, socket: &Path, lines: &[String]) -> Result<Vec<String>, String> {
        let mut child = asking(socket, lines)?;
        let mut stdout = child.stdout.take().ok_or("alo-ask has no output")?;
        let reading = std::thread::spawn(move || {
            let mut printed = String::new();
            let _ = stdout.read_to_string(&mut printed);
            printed
        });
        let finished = self.until("alo-ask answered", |_| {
            matches!(child.try_wait(), Ok(Some(_)) | Err(_))
        });
        if finished.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        finished?;
        let printed = reading
            .join()
            .map_err(|_| "alo-ask's output was not read".to_owned())?;
        Ok(printed.lines().map(str::to_owned).collect())
    }

    /// Until the window is called `title`.
    fn called(&mut self, title: &str) -> Result<(), String> {
        let mut last = None;
        let waited = self.until(&format!("the window called {title:?}"), |driven| {
            if let Some(now) = driven.window.take_title() {
                last = Some(now);
            }
            last.as_deref() == Some(title)
        });
        waited.map_err(|why| format!("{why}; last called {last:?}"))
    }
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`, its cards and new tabs reached by the agents of `reaching`.
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
    // A new tab asked for is handed to the window, as the event loop's
    // proxy hands it in `window::conversation`.
    let asked = tell.clone();
    reaching
        .reached
        .answered_by(Box::new(move |new| asked.send(Heard::NewTab(new)).is_ok()));
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
    let mut driven = Driven { window, heard, who };
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
    let elsewhere = format!("http://localhost:{port}/one.html");
    let sites = AgentSites::named([one.as_str()]).map_err(|why| why.to_string())?;
    let (orders, reaching) = relay(sites).map_err(|why| why.to_string())?;
    let path = socket(if who == "Chrome" { "chrome" } else { "host" })?;
    let listening = Listening::at(&path, orders).map_err(|why| format!("not listening: {why}"))?;
    let mut driven = the_window(chromium, reaching, who)?;
    driven.called("alo")?;
    let first = driven.window.conversation().on_screen();

    // 1. A new chat whose card shows page one, and the window called by it.
    let answers = driven.ask(
        &path,
        &[
            format!(r#"{{"v":1,"id":1,"ask":"open","address":"{elsewhere}"}}"#),
            format!(r#"{{"v":1,"id":2,"ask":"open","address":"{one}"}}"#),
            r#"{"v":1,"id":3,"ask":"tabs"}"#.to_owned(),
        ],
    )?;
    assert!(
        answers.first().is_some_and(|answer| answer
            .starts_with(r#"{"answer":"refused","id":1,"refusal":{"kind":"NotGranted","#)),
        "{who}: {answers:?}"
    );
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(
            format!(r#"{{"answer":"opened","id":2,"said":"opened {one}","tab":"{one}","v":1}}"#)
                .as_str()
        ),
        "{who}"
    );
    assert_eq!(
        answers.get(2).map(String::as_str),
        Some(format!(r#"{{"answer":"tabs","id":3,"tabs":["{one}"],"v":1}}"#).as_str()),
        "{who}"
    );
    let opened_in = driven.window.conversation().on_screen();
    assert_ne!(opened_in, first, "{who}: a new chat");
    assert!(
        driven
            .window
            .conversation()
            .card()
            .is_some_and(|card| card.url.to_string() == one),
        "{who}: the card shows page one"
    );
    driven.called("One")?;

    // 2. The card's page sent to page two, and the window called by it.
    let answers = driven.ask(
        &path,
        &[format!(
            r#"{{"v":1,"id":4,"ask":"open","tab":"{one}","address":"{two}"}}"#
        )],
    )?;
    assert_eq!(
        answers.first().map(String::as_str),
        Some(
            format!(r#"{{"answer":"opened","id":4,"said":"opened {two}","tab":"{two}","v":1}}"#)
                .as_str()
        ),
        "{who}"
    );
    driven.called("Two")?;
    driven.until("the card shows page two", |driven| {
        driven
            .window
            .conversation()
            .card()
            .is_some_and(|card| card.url.to_string() == two)
    })?;

    // 3. A new chat with nothing in it: no card, no tab, called alo.
    let answers = driven.ask(
        &path,
        &[
            r#"{"v":1,"id":5,"ask":"new_tab"}"#.to_owned(),
            r#"{"v":1,"id":6,"ask":"tabs"}"#.to_owned(),
        ],
    )?;
    assert!(
        answers
            .first()
            .is_some_and(|answer| answer.starts_with(r#"{"answer":"new_tab","id":5,"#)),
        "{who}: {answers:?}"
    );
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(r#"{"answer":"tabs","id":6,"tabs":[],"v":1}"#),
        "{who}"
    );
    assert!(driven.window.conversation().card().is_none(), "{who}");
    let new = driven.window.conversation().on_screen();
    assert!(new != opened_in && new != first, "{who}: another new chat");
    driven.called("alo")?;

    driven.window.finish();
    listening.stop();
    assert!(!path.exists(), "{who}: the socket is removed");
    match driven.window.unfinished() {
        Some(why) => Err(format!("{who}: {why}")),
        None => Ok(()),
    }
}

#[test]
fn a_new_chat_opened_by_an_agent_outside_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_new_chat_opened_by_an_agent_outside_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("956")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
