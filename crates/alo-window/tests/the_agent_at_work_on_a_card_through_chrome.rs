/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 704, ADR 0096: alo's own agent at work on the page the
//! conversation shows as a card — through **the window's own road**
//! ([`ConversationWindow`], its confined renderers, the relay an agent on
//! the socket reaches the card by, and the Chromium), the machine's Chrome
//! and alo's CEF host, each the same test.
//!
//! The model is **never the real one**: a server on this machine answers
//! each request in `OpenAI`'s shape with the next step of a script, aimed
//! at a node it finds by name in the tree the request carries — as a model
//! would — and keeps every request's body. The page is a local one with a
//! form (*Name*, a *Password* already holding `hunter2`, *Send*), *Say
//! hello* outside it, and a link *Delete everything*. Asserted, in order,
//! on one card:
//!
//! 1. **A request typed while the card is open is the agent's**: Return
//!    chooses *Ask alo*. It types *Ada* into *Name* and presses *Say hello*
//!    (the page fetched `/hello`); *Send*, a button sending a form, is
//!    **held**: the page sent nothing, until a person's press on *Allow* —
//!    found on the screen, pressed through the drawing thread's claim —
//!    after which the form is sent with `name=Ada`, and the run is done.
//!    No request to the model holds `hunter2`; every one is `store: false`.
//! 2. **Not now**: *Delete everything* is held, *Not now* pressed, the
//!    link never followed, and the model not asked again.
//! 3. **A password is never typed**: `put_text` into *Password* is refused
//!    by the card, by kind, and the run says it is stuck; the page heard
//!    no input in the field.
//! 4. **Stop**, pressed while the model is thinking: nothing more is done.
//! 5. **A press on the page takes over**: the run stops, the page is the
//!    person's, nothing more is done.
//! 6. **The step limit**: twenty presses, then it stops and says so.
//! 7. **A provider error** is said, and nothing is done.
//! 8. **The split view** (queue item 819): while it works the panel is
//!    the rail, the first screen the split view, and the card's page
//!    streamed at the split's page box, 798 × 740 — Chrome told the new
//!    size and every frame at it — and once the run ends, the panel back
//!    and the page streamed at the conversation's 878 × 520 again. *Allow*,
//!    *Not now* and *Stop* are found and pressed in the agent's column, and
//!    the page taken over by a press on the split's page box.
//! 9. **Every run kept as turns of the chat** (queue item 820): the chat's
//!    file, read back, holds each run with the page it worked on — what
//!    was asked, its steps in words, how it ended — and never the text it
//!    typed into the password field, which the person did not say, nor
//!    the password the page held.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: every address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_layout::Size;
use alo_renderer::Frame;
use alo_renderer::agent_ran::{Ended, Ran, TYPED};
use alo_renderer::agent_work::Now;
use alo_renderer::card_place;
use alo_renderer::host::Renderers;
use alo_renderer::split_place;
use alo_renderer::turn::Turn;
use alo_window::agent_sites::AgentSites;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::chat_record;
use alo_window::chat_seal::Key as ChatKey;
use alo_window::chat_store::ChatStore;
use alo_window::conversation::{STOPPED, TAKEN_OVER};
use alo_window::conversation_agent::{Agent, MOST_STEPS, TellAgent};
use alo_window::conversation_agents::relay;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::Side;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::openai_answer::Provider;
use alo_window::scale::Scale;
use alo_window::search_key;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting, a run of steps.
const AT_MOST: Duration = Duration::from_secs(120);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The panel's width at one: where the first screen begins.
const PANEL: f64 = 240.0;

/// The rail's width at one: where the first screen begins while the agent
/// works (queue item 819).
const RAIL: f64 = 64.0;

/// The first screen beside the rail in the design's window: the split
/// view's, 1216 × 820.
const SPLIT_ROOM: (u32, u32) = (1216, 820);

/// Where the inside of the agent's column begins in the split view: its
/// left edge, 832, its border and 16 ([`split_place::column`], held to it
/// below).
const COLUMN_INSIDE: u16 = 849;

/// Where the box's top is at the foot of the agent's column: 24 and 56
/// above the first screen's foot ([`card_place::box_top`], held to it
/// below).
const SPLIT_BOX_TOP: u16 = 740;

/// The split view's page box in it, in CSS pixels
/// ([`split_place::page_size`], held to it below).
const SPLIT_PAGE: (u32, u32) = (798, 740);

/// The box on the first screen with nothing in it, at one.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The box once the conversation holds a turn: 24 above the window's foot,
/// 56 tall, in the middle of the first screen's 1040.
const THE_BOX_CONVERSING: (f64, f64) = (PANEL + 520.0, 820.0 - 24.0 - 28.0);

/// Where the column begins on the first screen, 1040 wide at one: (1040 -
/// 880) / 2 ([`card_place::column_left`], held to it below).
const LEFT: u16 = 80;

/// How far down the conversation is seen: 820 - 24 - 56 - 24
/// ([`card_place::room_bottom`], held to it below).
const ROOM_BOTTOM: u16 = 716;

/// The ink *Allow* is drawn in.
const INK: [u8; 4] = [0x1b, 0x24, 0x33, 0xff];

/// The tint of the line saying the agent is at work.
const TINT: [u8; 4] = [0xf6, 0xe7, 0xe0, 0xff];

/// The test's page.
const FORM: &str = "<!doctype html><title>Form</title><h1>A form</h1>\
<form action=\"/sent\" method=\"post\">\
<p><label>Name <input name=\"name\"></label></p>\
<p><label>Password <input id=\"pw\" type=\"password\" name=\"pw\" value=\"hunter2\"></label></p>\
<p><button>Send</button></p></form>\
<p><button type=\"button\" onclick=\"fetch('/hello')\">Say hello</button></p>\
<p><a href=\"/deleted\">Delete everything</a></p>\
<script>document.getElementById('pw').addEventListener('input', function () { fetch('/typed'); });</script>";

/// What the page's server was asked: each request's path, and a POST's
/// body after it.
type Asked = Arc<Mutex<Vec<String>>>;

/// A server on this machine for the page: its port, and what it was asked.
fn serve() -> Result<(u16, Asked), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let asked: Asked = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&asked);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let kept = Arc::clone(&kept);
            std::thread::spawn(move || {
                let Some((head, body)) = request(&mut socket) else {
                    return;
                };
                let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
                if let Ok(mut asked) = kept.lock() {
                    asked.push(if body.is_empty() {
                        path.clone()
                    } else {
                        format!("{path} {body}")
                    });
                }
                let (status, page) = match path.as_str() {
                    "/form.html" => ("200 OK", FORM.to_owned()),
                    "/sent" => ("200 OK", "<!doctype html><h1>Sent</h1>".to_owned()),
                    "/deleted" => ("200 OK", "<!doctype html><h1>Deleted</h1>".to_owned()),
                    "/hello" | "/typed" => ("200 OK", "ok".to_owned()),
                    _ => ("404 Not Found", String::new()),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{page}",
                    page.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, asked))
}

/// One request read off `socket`: its head and its body, by its length.
fn request(socket: &mut TcpStream) -> Option<(String, String)> {
    let _ = socket.set_read_timeout(Some(Duration::from_secs(10)));
    let mut read = Vec::new();
    let mut block = [0u8; 8192];
    while let Ok(got) = socket.read(&mut block) {
        if got == 0 {
            break;
        }
        read.extend_from_slice(block.get(..got).unwrap_or_default());
        let text = String::from_utf8_lossy(&read).to_string();
        if let Some(end) = text.find("\r\n\r\n") {
            let head = text.get(..end).unwrap_or_default().to_owned();
            let length = head
                .lines()
                .find_map(|line| {
                    let line = line.to_ascii_lowercase();
                    line.strip_prefix("content-length:")
                        .map(|length| length.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if read.len() >= end + 4 + length {
                let body = text.get(end + 4..).unwrap_or_default().to_owned();
                return Some((head, body));
            }
        }
    }
    None
}

/// One step of the model's script.
#[derive(Debug, Clone)]
enum Scripted {
    /// Put `text` into the node of `role` called `name`.
    Put(&'static str, &'static str, &'static str),
    /// Press the node of `role` called `name`.
    Press(&'static str, &'static str),
    /// Finished, saying so.
    Done(&'static str),
    /// Wait until the test lets the answer go, then the step after it.
    Held,
    /// Answer with this status and nothing.
    Status(u16),
}

/// The model on this machine: its script, and every request's body.
#[derive(Clone)]
struct Model {
    script: Arc<Mutex<VecDeque<Scripted>>>,
    bodies: Arc<Mutex<Vec<String>>>,
    /// Lets a held answer go.
    release: Arc<AtomicBool>,
    /// How many requests were answered.
    answered: Arc<AtomicUsize>,
}

impl Model {
    fn will(&self, steps: &[Scripted]) {
        if let Ok(mut script) = self.script.lock() {
            script.clear();
            script.extend(steps.iter().cloned());
        }
    }

    fn asked(&self) -> usize {
        self.bodies.lock().map_or(0, |bodies| bodies.len())
    }

    fn bodies(&self) -> Vec<String> {
        self.bodies
            .lock()
            .map(|bodies| bodies.clone())
            .unwrap_or_default()
    }

    fn let_go(&self) {
        self.release.store(true, Ordering::SeqCst);
    }

    fn answered(&self) -> usize {
        self.answered.load(Ordering::SeqCst)
    }

    fn next(&self) -> Option<Scripted> {
        self.script.lock().ok()?.pop_front()
    }
}

/// Start the model on this machine: its endpoint, and how it is scripted.
fn model() -> Result<(String, Model), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let model = Model {
        script: Arc::new(Mutex::new(VecDeque::new())),
        bodies: Arc::new(Mutex::new(Vec::new())),
        release: Arc::new(AtomicBool::new(false)),
        answered: Arc::new(AtomicUsize::new(0)),
    };
    let answering = model.clone();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let Some((_, body)) = request(&mut socket) else {
                continue;
            };
            if let Ok(mut bodies) = answering.bodies.lock() {
                bodies.push(body.clone());
            }
            let (status, reply) = answer(&answering, &body);
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{reply}",
                reply.len()
            );
            let _ = socket.write_all(reply.as_bytes());
            answering.answered.fetch_add(1, Ordering::SeqCst);
        }
    });
    Ok((format!("http://127.0.0.1:{port}/v1/responses"), model))
}

/// What the model answers the request `body` with: the script's next step.
fn answer(model: &Model, body: &str) -> (u16, String) {
    loop {
        match model.next() {
            Some(Scripted::Held) => {
                let until = Instant::now() + AT_MOST;
                while !model.release.swap(false, Ordering::SeqCst) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            Some(Scripted::Status(status)) => return (status, String::new()),
            Some(Scripted::Done(result)) => {
                return (200, calling("done", &format!(r#"{{"result":"{result}"}}"#)));
            }
            Some(Scripted::Press(role, name)) => {
                let node = node_of(body, role, name).unwrap_or(0);
                return (
                    200,
                    calling(
                        "activate",
                        &format!(r#"{{"node":{node},"step":"Pressing {name}","why":""}}"#),
                    ),
                );
            }
            Some(Scripted::Put(role, name, text)) => {
                let node = node_of(body, role, name).unwrap_or(0);
                return (
                    200,
                    calling(
                        "put_text",
                        &format!(
                            r#"{{"node":{node},"text":"{text}","step":"Typing {text} into {name}","why":""}}"#
                        ),
                    ),
                );
            }
            None => return (500, String::new()),
        }
    }
}

/// The node of `role` called `name` in the tree `body` carries — a JSON
/// string in a JSON string, so each quote in it is written `\\\"` — by its
/// number.
fn node_of(body: &str, role: &str, name: &str) -> Option<u64> {
    let written = format!("] {role} \\\\\\\"{name}\\\\\\\"");
    let at = body.find(&written)?;
    let before = body.get(..at)?;
    let open = before.rfind('[')?;
    before.get(open + 1..)?.parse().ok()
}

/// A response in `OpenAI`'s shape calling `name` with `arguments`.
fn calling(name: &str, arguments: &str) -> String {
    let arguments = arguments.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"object\":\"response\",\"status\":\"completed\",\"output\":[{{\"type\":\"function_call\",\
         \"call_id\":\"c\",\"name\":\"{name}\",\"arguments\":\"{arguments}\"}}],\
         \"usage\":{{\"input_tokens\":100,\"output_tokens\":10}}}}"
    )
}

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it",
            path.display()
        ))
    }
}

/// The conversation's window, and what was heard of its threads.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    who: &'static str,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    /// How many times the first screen was painted, and when last.
    painted: (usize, Option<Instant>),
}

impl Driven {
    /// Give the window what its threads told it until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let by = Instant::now() + AT_MOST;
        while !done(self) {
            let Some(left) = by.checked_duration_since(Instant::now()) else {
                return Err(format!(
                    "{}: {what} did not happen; the window said {:?}; last pressed {:?}; the card at {:?}, loading {:?}",
                    self.who,
                    self.said,
                    self.window.conversation().last_pressed(),
                    self.window
                        .conversation()
                        .card()
                        .map(|card| card.url.to_string()),
                    self.window.conversation().card().map(|card| card.loading),
                ));
            };
            let Ok(heard) = self
                .heard
                .recv_timeout(left.min(Duration::from_millis(100)))
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
                Heard::Drawn(Drawn::Painted(alo_window::conversation_drawing::Part::Start, _)) => {
                    self.painted = (self.painted.0 + 1, Some(Instant::now()));
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

    /// Hear the window's threads for `wait`, whatever they say.
    fn hear_for(&mut self, wait: Duration) -> Result<(), String> {
        let until = Instant::now() + wait;
        match self.until("nothing", |_| Instant::now() >= until) {
            Ok(()) => Ok(()),
            Err(why) if why.contains("nothing did not happen") => Ok(()),
            Err(why) => Err(why),
        }
    }

    /// A person's click at `at`, in the window's device pixels.
    fn press(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// A press on the box at `at`, until it has the keyboard: the press is
    /// claimed by the first screen's renderer, which may be painting.
    fn press_the_box(&mut self, at: (f64, f64)) -> Result<(), String> {
        // The conversation painted beside the panel, 1040 wide: after a
        // run, the split view's last frame is still on the screen until
        // the conversation is painted again, and a press there is on the
        // page (queue item 819).
        self.until("the conversation painted beside the panel", |driven| {
            driven
                .start_frame()
                .is_some_and(|frame| frame.width == 1040)
        })?;
        self.press(at);
        self.until("the box given the keyboard", |driven| {
            driven
                .window
                .conversation()
                .start(Size::new(1040.0, 820.0), Scale::ONE)
                .focused
        })
    }

    /// `text` typed into the box, which has the keyboard, then Return.
    fn type_and_return(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
    }

    /// The box pressed, `address` typed and Return; until the card shows
    /// it, loaded and drawn.
    fn go(&mut self, address: &str) -> Result<(), String> {
        let at = if self.window.conversation().conversing() {
            THE_BOX_CONVERSING
        } else {
            THE_BOX
        };
        self.press_the_box(at)?;
        self.type_and_return(address);
        self.until(&format!("{address} shown in the card"), |driven| {
            driven.window.conversation().card().is_some_and(|card| {
                card.url.to_string() == address && !card.loading && card.frame.is_some()
            })
        })?;
        // Its first frames painted on the first screen.
        self.hear_for(Duration::from_millis(800))
    }

    /// What the agent on the card is doing now, if it is there.
    fn now(&self) -> Option<Now> {
        self.window
            .conversation()
            .card()
            .and_then(|card| card.agent.as_ref())
            .map(|work| work.now.clone())
    }

    /// Ask alo `asked`, typed into the box with Return, which asks alo
    /// first while the card is open.
    fn ask(&mut self, asked: &str) -> Result<(), String> {
        self.press_the_box(THE_BOX_CONVERSING)?;
        self.type_and_return(asked);
        self.until("the agent at work", |driven| {
            driven.window.conversation().working().is_some()
        })
    }

    /// The first screen's last frame.
    fn start_frame(&self) -> Option<&Frame> {
        self.window.scene()?.start
    }

    /// The first screen's last frame, if it was painted as the split view
    /// beside the rail (queue item 819): 1216 wide, where beside the panel
    /// it is 1040.
    fn split_frame(&self) -> Option<&Frame> {
        self.start_frame()
            .filter(|frame| (frame.width, frame.height) == SPLIT_ROOM)
    }

    /// Where *Allow* is on the screen now, found as a person finds it: the
    /// ink of its face, in the agent's column's left 300, above the box.
    fn allow(&self) -> Option<(f64, f64)> {
        let frame = self.split_frame()?;
        let from = u32::from(COLUMN_INSIDE);
        let to = from + 300;
        for y in 0..u32::from(SPLIT_BOX_TOP).min(frame.height) {
            let mut run = 0;
            for x in from..to.min(frame.width) {
                if pixel(frame, x, y) == Some(INK) {
                    run += 1;
                    if run >= 45 {
                        let centre = f64::from(x) - 22.0;
                        return Some((RAIL + centre, f64::from(y) + 13.0));
                    }
                } else {
                    run = 0;
                }
            }
        }
        None
    }

    /// [`Self::allow`], or why not: the first screen as it was last
    /// painted written beside the test's own temporary files, to be looked
    /// at.
    fn allow_on_the_screen(&mut self) -> Result<(f64, f64), String> {
        // The first screen painted as it is now: a debug build's renderer
        // takes a second or more to paint it.
        let shown = self.until("Allow on the screen", |driven| driven.allow().is_some());
        if let Some(found) = self.allow() {
            return Ok(found);
        }
        let _ = shown;
        let written =
            std::env::temp_dir().join(format!("alo-704-{}-no-allow.png", std::process::id()));
        if let Some(png) = self
            .start_frame()
            .and_then(|frame| alo_renderer::frame::frame_to_png(frame).ok())
        {
            let _ = std::fs::write(&written, png);
        }
        Err(format!(
            "{}: no Allow on the screen, the agent {:?}, scrolled {:?}; the screen is {}; said {:?}; told {:?}; painted {} times, last {:?} ago",
            self.who,
            self.now(),
            self.window
                .conversation()
                .thread()
                .map(alo_window::conversation_thread::Thread::down),
            written.display(),
            self.said,
            self.window
                .scene()
                .and_then(|scene| scene.start_sentence.map(str::to_owned)),
            self.painted.0,
            self.painted.1.map(|at| at.elapsed())
        ))
    }

    /// Where *Stop* is on the screen now: at the end of the tinted line
    /// that says the agent is at work, at the top of the agent's column,
    /// 4 in from the column's 351 inside it and 55 wide.
    fn stop(&self) -> Option<(f64, f64)> {
        let frame = self.split_frame()?;
        let x = 855;
        let rows: Vec<u32> = (0..frame.height)
            .filter(|&y| pixel(frame, x, y) == Some(TINT))
            .collect();
        let (first, last) = (rows.first()?, rows.last()?);
        Some((RAIL + 1168.0, f64::from(first + last) / 2.0))
    }

    /// The middle of the card's page as the split view shows it.
    fn page(&self) -> Option<(f64, f64)> {
        self.split_frame()?;
        let page = split_place::page_box(split_room());
        Some((
            RAIL + f64::from(page.left() + page.size.width / 2.0),
            f64::from(page.top() + page.size.height / 2.0),
        ))
    }

    /// The size of the card's newest frame, in device pixels.
    fn card_frame_size(&self) -> Option<(u32, u32)> {
        let png = self.window.conversation().card()?.frame.clone()?;
        let canvas = alo_paint::encode::picture_from_png(&png).ok()?;
        Some((canvas.width(), canvas.height()))
    }

    /// What stands down the window's left now.
    fn side(&self) -> Option<Side> {
        self.window.scene().map(|scene| scene.side)
    }
}

/// The first screen beside the rail in the design's window, as a size.
fn split_room() -> Size {
    Size::new(1216.0, 820.0)
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The conversation's window at one, both parts painted, its Chrome
/// `chromium`, the agent asking the model at `endpoint`.
fn the_window(
    chromium: Chromium,
    endpoint: &str,
    who: &'static str,
    chats: &Path,
) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    let (orders, reaching) = relay(AgentSites::Every).map_err(|why| why.to_string())?;
    let went = tell.clone();
    let chrome = ChromeOpener::start_reached(
        chromium,
        reaching,
        Box::new(move |w| went.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    let key = search_key::Key::of("test-key-not-real").ok_or("no key")?;
    let provider = Provider::at(endpoint, key)?;
    let told = tell;
    let tell_agent: TellAgent = Arc::new(move |said| told.send(Heard::Agent(said)).is_ok());
    let (store, opened) = ChatStore::open(chats, test_key())?;
    let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome)
        .with_agent(Ok(Agent::new(provider, orders)), tell_agent)
        .with_chats(store, opened);
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
        who,
        said: Vec::new(),
        painted: (0, None),
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

/// How many pixels of the card's newest frame are drawn in Chrome's
/// highlight's border ([`marked_pixels`]).
fn card_marked(driven: &Driven) -> Result<usize, String> {
    driven
        .window
        .conversation()
        .card()
        .and_then(|card| card.frame.as_deref())
        .ok_or_else(|| format!("{}: the card has no frame", driven.who))
        .and_then(marked_pixels)
}

/// How many pixels of the card's frame `png` are drawn in Chrome's
/// highlight's border, alo's accent (`#e76f51`), within a few levels: the
/// page itself has none.
fn marked_pixels(png: &[u8]) -> Result<usize, String> {
    let canvas = alo_paint::encode::picture_from_png(png).map_err(|why| format!("{why:?}"))?;
    let near = |a: u8, b: u8| a.abs_diff(b) <= 24;
    Ok((0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter_map(|(x, y)| canvas.at(x, y).map(alo_value::color::Rgba::to_rgba8))
        .filter(|&(red, green, blue, _)| near(red, 0xe7) && near(green, 0x6f) && near(blue, 0x51))
        .count())
}

fn count(asked: &Asked, path: &str) -> usize {
    asked.lock().map_or(0, |asked| {
        asked
            .iter()
            .filter(|each| each.split(' ').next() == Some(path))
            .count()
    })
}

fn stopped_saying(now: Option<Now>) -> String {
    match now {
        Some(Now::Stopped { why } | Now::Stuck { why, .. }) => why,
        other => format!("{other:?}"),
    }
}

/// 1: it fills a field, presses a button, and waits for *Allow* before
/// sending the form.
fn fills_presses_and_asks_before_sending(
    driven: &mut Driven,
    model: &Model,
    asked: &Asked,
) -> Result<(), String> {
    let who = driven.who;
    model.will(&[
        Scripted::Put("textbox", "Name", "Ada"),
        Scripted::Press("button", "Say hello"),
        Scripted::Press("button", "Send"),
        Scripted::Done("Sent your name, Ada."),
    ]);
    driven.ask("Send my name Ada through the form")?;
    // The split view (queue item 819): the panel the rail, and Chrome told
    // the split's page box, its frames at it.
    assert_eq!(driven.side(), Some(Side::Rail), "{who}");
    driven.until("the page streamed at the split view's page box", |driven| {
        driven.card_frame_size() == Some(SPLIT_PAGE)
    })?;
    driven.until("the agent asking before Send", |driven| {
        matches!(driven.now(), Some(Now::Asking { .. }))
    })?;
    assert_eq!(count(asked, "/hello"), 1, "{who}: Say hello was pressed");
    assert_eq!(count(asked, "/sent"), 0, "{who}: Send held for the person");
    let Some(Now::Asking { what }) = driven.now() else {
        return Err(format!("{who}: not asking"));
    };
    assert!(
        what.starts_with("Press \u{201c}Send\u{201d} on 127.0.0.1:"),
        "{who}: {what}"
    );
    // Held while nobody presses — and *Say hello*, pressed, is not marked
    // by Chrome over the card's page (queue item 822): its frames carry no
    // inspector highlight and no tooltip.
    driven.hear_for(Duration::from_secs(2))?;
    assert_eq!(
        card_marked(driven)?,
        0,
        "{who}: Chrome's highlight is over the card's page"
    );
    assert_eq!(
        count(asked, "/sent"),
        0,
        "{who}: never sent without a press"
    );
    // The person's press on *Allow*, found on the screen.
    let allow = driven.allow_on_the_screen()?;
    driven.press(allow);
    driven.until("the run done", |driven| {
        matches!(driven.now(), Some(Now::Done { .. }))
    })?;
    // The run over: the panel back, and Chrome told the conversation's page
    // box again (queue item 819).
    assert_eq!(driven.side(), Some(Side::Panel), "{who}");
    driven.until("the page streamed at the card's page box again", |driven| {
        driven.card_frame_size() == Some((878, 520))
    })?;
    let sent: Vec<String> = asked
        .lock()
        .map(|asked| {
            asked
                .iter()
                .filter(|each| each.starts_with("/sent"))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(sent.len(), 1, "{who}: {sent:?}");
    assert!(
        sent.iter().all(|sent| sent.contains("name=Ada")),
        "{who}: {sent:?}"
    );
    let bodies = model.bodies();
    assert_eq!(bodies.len(), 4, "{who}");
    for body in &bodies {
        assert!(
            !body.contains("hunter2"),
            "{who}: a password was sent: {body}"
        );
        assert!(body.contains("\"store\":false"), "{who}");
        assert!(!body.contains("test-key"), "{who}");
    }
    assert!(
        bodies.iter().any(|body| body.contains("(withheld)")),
        "{who}"
    );
    Ok(())
}

/// 2: *Not now* presses nothing, and ends the run.
fn not_now_presses_nothing(
    driven: &mut Driven,
    model: &Model,
    asked: &Asked,
) -> Result<(), String> {
    let who = driven.who;
    let before = model.asked();
    model.will(&[
        Scripted::Press("link", "Delete everything"),
        Scripted::Done("Deleted."),
    ]);
    driven.ask("Delete everything here")?;
    driven.until("the agent asking before deleting", |driven| {
        matches!(driven.now(), Some(Now::Asking { .. }))
    })?;
    let (x, y) = driven.allow_on_the_screen()?;
    // *Not now*, 8 after *Allow*: their middles 76.5 apart.
    driven.press((x + 76.5, y));
    driven.until("the run ended", |driven| {
        matches!(driven.now(), Some(Now::Stopped { .. }))
    })?;
    assert!(
        stopped_saying(driven.now()).starts_with("Not done: you said not now."),
        "{who}: {:?}",
        driven.now()
    );
    driven.hear_for(Duration::from_secs(1))?;
    assert_eq!(count(asked, "/deleted"), 0, "{who}: the link was followed");
    assert_eq!(
        model.asked(),
        before + 1,
        "{who}: the model was asked again"
    );
    Ok(())
}

/// 3: a password is refused by the card, by kind, and nothing typed.
fn never_types_a_password(driven: &mut Driven, model: &Model, asked: &Asked) -> Result<(), String> {
    let who = driven.who;
    model.will(&[Scripted::Put("textbox", "Password", "letmein")]);
    // Not the person's words: what it types is never kept (queue item 820).
    driven.ask("Sign me in")?;
    driven.until("the run stuck", |driven| {
        matches!(driven.now(), Some(Now::Stuck { .. }))
    })?;
    assert!(
        stopped_saying(driven.now()).contains("alo never types one"),
        "{who}: {:?}",
        driven.now()
    );
    driven.hear_for(Duration::from_secs(1))?;
    assert_eq!(
        count(asked, "/typed"),
        0,
        "{who}: the password field heard input"
    );
    Ok(())
}

/// 4 and 5: *Stop*, and a press on the page, while the model thinks.
fn stop_and_take_over(driven: &mut Driven, model: &Model, asked: &Asked) -> Result<(), String> {
    let who = driven.who;
    for (how, why) in [("Stop", STOPPED), ("the page", TAKEN_OVER)] {
        let hellos = count(asked, "/hello");
        let before = model.asked();
        model.will(&[Scripted::Held, Scripted::Press("button", "Say hello")]);
        driven.ask("Say hello")?;
        driven.until("the model asked", |_| model.asked() > before)?;
        // The working line, and *Stop* at its end, painted: the first
        // screen as the person sees it while the model thinks.
        driven.until("Stop on the screen", |driven| driven.stop().is_some())?;
        let at = if how == "Stop" {
            driven.stop()
        } else {
            driven.page()
        }
        .ok_or_else(|| format!("{who}: {how} is not on the screen"))?;
        driven.press(at);
        driven.until(&format!("the run stopped by {how}"), |driven| {
            matches!(driven.now(), Some(Now::Stopped { .. }))
        })?;
        assert_eq!(stopped_saying(driven.now()), why, "{who}: {how}");
        // The model's answer comes, and nothing is done with it.
        let answered = model.answered();
        model.let_go();
        driven.until("the held answer sent", |_| model.answered() > answered)?;
        driven.hear_for(Duration::from_secs(2))?;
        assert_eq!(count(asked, "/hello"), hellos, "{who}: pressed after {how}");
        assert_eq!(model.asked(), before + 1, "{who}: asked again after {how}");
    }
    Ok(())
}

/// 6 and 7: the step limit, and a model that cannot answer.
fn its_limits_are_said(driven: &mut Driven, model: &Model, asked: &Asked) -> Result<(), String> {
    let who = driven.who;
    let hellos = count(asked, "/hello");
    let presses: Vec<Scripted> = (0..MOST_STEPS + 2)
        .map(|_| Scripted::Press("button", "Say hello"))
        .collect();
    model.will(&presses);
    driven.ask("Say hello again and again")?;
    driven.until("the run stopped at its limit", |driven| {
        matches!(driven.now(), Some(Now::Stopped { .. }))
    })?;
    assert!(
        stopped_saying(driven.now()).starts_with("alo stopped after 20 steps without finishing."),
        "{who}: {:?}",
        driven.now()
    );
    driven.hear_for(Duration::from_secs(1))?;
    assert_eq!(count(asked, "/hello"), hellos + MOST_STEPS, "{who}");
    let steps = driven
        .window
        .conversation()
        .card()
        .and_then(|card| card.agent.as_ref())
        .map_or(0, |work| work.steps.len());
    assert_eq!(steps, MOST_STEPS, "{who}: one line per step");
    // Queue item 822: twenty presses on one button, and nothing of
    // Chrome's drawn over the card's page — no inspector highlight, no
    // tooltip — in the frames streamed after the run stopped.
    driven.hear_for(Duration::from_millis(1500))?;
    assert_eq!(
        card_marked(driven)?,
        0,
        "{who}: Chrome's highlight is over the card's page"
    );
    model.will(&[Scripted::Status(503)]);
    driven.ask("Say hello once")?;
    driven.until("the run stopped for the model", |driven| {
        matches!(driven.now(), Some(Now::Stopped { .. }))
    })?;
    assert_eq!(
        stopped_saying(driven.now()),
        "alo could not go on: OpenAI could not answer just now (503).",
        "{who}"
    );
    Ok(())
}

fn through(chromium: Chromium, who: &'static str) -> Result<(), String> {
    assert_eq!(split_place::page_size(split_room()), SPLIT_PAGE);
    assert_eq!(
        (split_place::column(split_room()).left() + 17.0).to_bits(),
        f32::from(COLUMN_INSIDE).to_bits()
    );
    assert_eq!(
        card_place::box_top(split_room()).to_bits(),
        f32::from(SPLIT_BOX_TOP).to_bits()
    );
    let room = Size::new(1040.0, 820.0);
    assert_eq!(
        card_place::column_left(room).to_bits(),
        f32::from(LEFT).to_bits()
    );
    assert_eq!(
        card_place::room_bottom(room, 0).to_bits(),
        f32::from(ROOM_BOTTOM).to_bits()
    );
    let (port, asked) = serve()?;
    let form = format!("http://127.0.0.1:{port}/form.html");
    let (endpoint, model) = model()?;
    let chats = std::env::temp_dir().join(format!(
        "alo-agent-runs-kept-{}-{}",
        std::process::id(),
        who.len()
    ));
    let _ = std::fs::remove_dir_all(&chats);
    let mut driven = the_window(chromium, &endpoint, who, &chats)?;
    driven.go(&form)?;
    fills_presses_and_asks_before_sending(&mut driven, &model, &asked)?;
    driven.go(&form)?;
    not_now_presses_nothing(&mut driven, &model, &asked)?;
    never_types_a_password(&mut driven, &model, &asked)?;
    stop_and_take_over(&mut driven, &model, &asked)?;
    its_limits_are_said(&mut driven, &model, &asked)?;
    driven.window.finish();
    if let Some(why) = driven.window.unfinished() {
        return Err(format!("{who}: {why}"));
    }
    kept_as_turns(&chats, who)?;
    let _ = std::fs::remove_dir_all(&chats);
    Ok(())
}

/// Queue item 820: every run kept in the chat's file with the page it
/// worked on — what was asked, its steps in words and how it ended — and
/// read back so; never a text typed that the person did not say, never the
/// password the page held.
fn kept_as_turns(chats: &Path, who: &str) -> Result<(), String> {
    let written = kept_words(chats, 0)?;
    for never in ["letmein", "hunter2"] {
        assert!(!written.contains(never), "{who}: {never} kept: {written}");
    }
    let chat = chat_record::read(written.as_bytes()).map_err(|why| why.0)?;
    let runs: Vec<Vec<Ran>> = chat
        .turns
        .iter()
        .filter_map(|turn| match turn {
            Turn::Visited { ran, .. } => Some(ran.clone()),
            _ => None,
        })
        .collect();
    let [first, second] = runs.as_slice() else {
        return Err(format!("{who}: not two pages: {:?}", chat.turns));
    };
    assert_eq!(
        first
            .iter()
            .map(|ran| ran.asked.as_str())
            .collect::<Vec<_>>(),
        vec!["Send my name Ada through the form"],
        "{who}"
    );
    let sent = first.first().ok_or("no run")?;
    assert_eq!(
        sent.steps
            .iter()
            .map(|step| step.line.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Typing Ada into Name",
            "Pressing Say hello",
            "Pressing Send"
        ],
        "{who}: Ada is the person's own word"
    );
    assert_eq!(
        sent.ended,
        Ended::Done {
            said: "Sent your name, Ada.".to_owned()
        },
        "{who}"
    );
    second_page_kept(second, who);
    Ok(())
}

/// The second page's runs, kept: each asked, how each ended, and no text
/// typed that the person did not say.
fn second_page_kept(second: &[Ran], who: &str) {
    assert_eq!(
        second
            .iter()
            .map(|ran| ran.asked.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Delete everything here",
            "Sign me in",
            "Say hello",
            "Say hello",
            "Say hello again and again",
            "Say hello once"
        ],
        "{who}"
    );
    let ended: Vec<String> = second
        .iter()
        .map(|ran| match &ran.ended {
            Ended::Done { said } => format!("done: {said}"),
            Ended::Stuck { why } => format!("stuck: {why}"),
            Ended::Stopped { why } => format!("stopped: {why}"),
        })
        .collect();
    assert!(
        ended
            .first()
            .is_some_and(|said| said.starts_with("stopped: Not done: you said not now.")),
        "{who}: {ended:?}"
    );
    assert!(
        ended.get(1).is_some_and(|said| said.starts_with("stuck: ") && said.contains("alo never types one")),
        "{who}: {ended:?}"
    );
    assert_eq!(
        second.get(1).map(|ran| ran.steps.clone()),
        Some(vec![alo_renderer::agent_work::Step {
            line: TYPED.to_owned(),
            why: String::new()
        }]),
        "{who}: the password's text never kept"
    );
    assert_eq!(
        ended.get(2..4),
        Some(
            &[
                format!("stopped: {STOPPED}"),
                format!("stopped: {TAKEN_OVER}")
            ][..]
        ),
        "{who}"
    );
    assert_eq!(
        second.get(4).map(|ran| ran.steps.len()),
        Some(MOST_STEPS),
        "{who}: one line per step"
    );
    assert_eq!(
        ended.get(5).map(String::as_str),
        Some("stopped: alo could not go on: OpenAI could not answer just now (503)."),
        "{who}"
    );
}

#[test]
fn the_agent_at_work_on_a_card_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn the_agent_at_work_on_a_card_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("704")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}

/// The key this test's chats are sealed under: its own, never the
/// keychain's (ADR 0098).
fn test_key() -> ChatKey {
    ChatKey::of([0x2a; 32])
}

/// The words of the chat numbered `number` kept in `chats`, opened under
/// the test's key.
fn kept_words(chats: &std::path::Path, number: u64) -> Result<String, String> {
    ChatStore::open(chats, test_key())?.0.words(number)
}
