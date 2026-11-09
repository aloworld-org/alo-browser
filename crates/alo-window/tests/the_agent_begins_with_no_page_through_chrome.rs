/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 821 (ADR 0103): alo asked with no page open, through the
//! window's own road and a real Chromium — the machine's Chrome, and alo's
//! CEF host — against a model on this machine.
//!
//! The person types what they want with no page open and presses the
//! box's second line, *Ask alo to do it*. The run stands in a turn of its
//! own — drawn, its working line's tint on the first screen — while the
//! model is held; then it opens the address the person named (on this
//! machine, so opened only because they wrote it) through the window, the
//! turn becomes that page's card, and the model is sent the page read
//! through the relay. Proposing `localhost`, which the person did not
//! write, it is refused and the server never asked. Proposing
//! `rebind.invalid`, a name the person did not write, it is opened from the
//! address the browser process resolved it to — here this machine's own
//! server, answered by the agent's resolver as a public address would be —
//! and the card's Chromium never resolves the name again (queue item 881):
//! no resolver answers a `.invalid` name, so the page opens only if the
//! card took the name as alo resolved it, all along the window's road.
//! Following that page's *Sign in* link to a page with a password field,
//! the page is handed to the person — *This site needs you to sign in* —
//! and the model is not asked again.

#![cfg(target_os = "macos")]

mod carried;

use alo_layout::Size;
use alo_net::resolve::{Reach, Unresolved};
use alo_renderer::agent_work::Now;
use alo_renderer::host::Renderers;
use alo_renderer::turn::Turn;
use alo_window::agent_sites::AgentSites;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation_agent::{Agent, TellAgent};
use alo_window::conversation_agents::relay;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::Press;
use alo_window::openai_answer::Provider;
use alo_window::scale::Scale;
use alo_window::search_key;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting, a run of steps.
const AT_MOST: Duration = Duration::from_secs(120);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The panel's width at one: where the first screen begins.
const PANEL: f64 = 240.0;

/// The box's second line under it with words typed and no page open:
/// *Ask alo to do it*, 52 under the first, as `what_is_typed_on_the_first_screen.rs`
/// and `the_agent_beginning_with_no_page.rs` hold it: (300, 410) in the
/// first screen.
const ASK_ALO_TO_DO_IT: (f64, f64) = (PANEL + 300.0, 410.0);

/// Inside the run's working line in its own turn, clear of its words: the
/// turn's box at 98, the line from 113, 36 tall
/// (`the_agent_beginning_with_no_page.rs`).
const WORKING_TINT_AT: (u32, u32) = (600, 120);

/// The tint of the line saying the agent is at work.
const TINT: [u8; 4] = [0xf6, 0xe7, 0xe0, 0xff];

/// The page the person names.
const START: &str = "<!doctype html><title>Start</title><h1>The start page</h1>\
<p><a href=\"/signin.html\">Sign in</a></p>";

/// The page its link goes to: a sign-in form.
const SIGN_IN: &str = "<!doctype html><title>Sign in</title><h1>Sign in</h1>\
<form><p><label>Email <input name=\"email\"></label></p>\
<p><label>Password <input type=\"password\" name=\"pw\"></label></p>\
<p><button>Sign in</button></p></form>";

/// Each path the page's server was asked for.
type Asked = Arc<Mutex<Vec<String>>>;

/// A server on this machine for the pages: its port, and what it was
/// asked.
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
                let Some((head, _)) = request(&mut socket) else {
                    return;
                };
                let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
                if let Ok(mut asked) = kept.lock() {
                    asked.push(path.clone());
                }
                let (status, page) = match path.as_str() {
                    "/start.html" => ("200 OK", START),
                    "/signin.html" => ("200 OK", SIGN_IN),
                    _ => ("404 Not Found", ""),
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

/// The model on this machine: every request's body, and whether its first
/// answer may go.
#[derive(Clone)]
struct Model {
    bodies: Arc<Mutex<Vec<String>>>,
    release: Arc<AtomicBool>,
}

impl Model {
    fn asked(&self) -> usize {
        self.bodies.lock().map_or(0, |bodies| bodies.len())
    }

    fn body(&self, at: usize) -> String {
        self.bodies
            .lock()
            .ok()
            .and_then(|bodies| bodies.get(at).cloned())
            .unwrap_or_default()
    }
}

/// Start the model on this machine, answering the run's three steps for
/// the page server at `port`: its endpoint, and what it was asked.
fn model(port: u16) -> Result<(String, Model), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let at = listener.local_addr().map_err(|why| why.to_string())?.port();
    let model = Model {
        bodies: Arc::new(Mutex::new(Vec::new())),
        release: Arc::new(AtomicBool::new(false)),
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
            let step = answering.asked();
            if let Ok(mut bodies) = answering.bodies.lock() {
                bodies.push(body.clone());
            }
            let reply = match step {
                // Held while the test looks at the run in its own turn.
                0 => {
                    let until = Instant::now() + AT_MOST;
                    while !answering.release.load(Ordering::SeqCst) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    calling(
                        "open",
                        &format!(
                            r#"{{"result":null,"address":"127.0.0.1:{port}/start.html","step":"Opening the start page","why":"","plan":["Open the start page","Sign in"],"on":1}}"#
                        ),
                    )
                }
                1 => calling(
                    "open",
                    &format!(
                        r#"{{"result":null,"address":"localhost:{port}/other.html","step":"Opening another page","why":"","plan":null,"on":1}}"#
                    ),
                ),
                2 => calling(
                    "open",
                    &format!(
                        r#"{{"result":null,"address":"http://{PINNED}:{port}/start.html","step":"Opening it by its name","why":"","plan":null,"on":1}}"#
                    ),
                ),
                3 => {
                    let node = node_of(&body, "link", "Sign in").unwrap_or(0);
                    calling(
                        "activate",
                        &format!(
                            r#"{{"node":{node},"step":"Pressing Sign in","why":"","plan":null,"on":2}}"#
                        ),
                    )
                }
                _ => calling("done", r#"{"result":"Signed in."}"#),
            };
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{reply}",
                reply.len()
            );
            let _ = socket.write_all(reply.as_bytes());
        }
    });
    Ok((format!("http://127.0.0.1:{at}/v1/responses"), model))
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
}

impl Driven {
    /// Give the window what its threads told it until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let by = Instant::now() + AT_MOST;
        while !done(self) {
            let Some(left) = by.checked_duration_since(Instant::now()) else {
                return Err(format!(
                    "{}: {what} did not happen; the window said {:?}",
                    self.who, self.said
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

    /// What the agent on the card is doing now, if a card holds it.
    fn now(&self) -> Option<Now> {
        self.window
            .conversation()
            .card()
            .and_then(|card| card.agent.as_ref())
            .map(|work| work.now.clone())
    }

    /// The first screen's last frame's pixel at `at`.
    fn start_pixel(&self, (x, y): (u32, u32)) -> Option<[u8; 4]> {
        let frame = self.window.scene()?.start?;
        let at = usize::try_from((y * frame.width + x) * 4).ok()?;
        frame.pixels.get(at..at + 4)?.try_into().ok()
    }
}

/// A name no resolver answers (RFC 6761), which the agent's own resolver
/// answers with this machine's address, standing for a public one.
const PINNED: &str = "rebind.invalid";

/// The agent's resolver: [`PINNED`] answered with this machine's address,
/// as the machine's resolver answers a public name with a public one; no
/// other name is found.
fn resolving(name: &str, _: u16, _: Reach) -> Result<Vec<IpAddr>, Unresolved> {
    if name == PINNED {
        return Ok(vec![IpAddr::from([127, 0, 0, 1])]);
    }
    Err(Unresolved::NoSuchName {
        host: name.to_owned(),
        why: "not this test's".to_owned(),
    })
}

fn the_window(chromium: Chromium, endpoint: &str, who: &'static str) -> Result<Driven, String> {
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
    let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_agent(
        Ok(Agent::new(provider, orders).resolving_with(Arc::new(resolving))),
        tell_agent,
    );
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
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

fn count(asked: &Asked, path: &str) -> usize {
    asked.lock().map_or(0, |asked| {
        asked.iter().filter(|each| each.as_str() == path).count()
    })
}

/// The fourth step (queue item 881): a name the person did not write,
/// opened from the address it was resolved to — the start page again, by
/// that name, asked of this machine's server.
fn opened_by_the_name_resolved(
    driven: &mut Driven,
    model: &Model,
    asked: &Asked,
    port: u16,
) -> Result<(), String> {
    let who = driven.who;
    driven.until("the page opened by the name resolved", |driven| {
        driven.window.conversation().card().is_some_and(|card| {
            card.url.to_string() == format!("http://{PINNED}:{port}/start.html")
        })
    })?;
    driven.until("the model sent the page by its name", |_| {
        model.asked() >= 4
    })?;
    assert!(
        model.body(3).contains("The start page"),
        "{who}: {}",
        model.body(3)
    );
    assert_eq!(
        count(asked, "/start.html"),
        2,
        "{who}: the start page asked by its name"
    );
    Ok(())
}

fn through(chromium: Chromium, who: &'static str) -> Result<(), String> {
    let (port, asked) = serve()?;
    let (endpoint, model) = model(port)?;
    let mut driven = the_window(chromium, &endpoint, who)?;
    // The box has the keyboard as the window opens (queue item 875).
    let typed = format!("Sign me in on 127.0.0.1:{port}/start.html");
    for letter in typed.chars() {
        let written = letter.to_string();
        driven
            .window
            .key(&Press::character(&written), Some(&written));
    }
    driven.until("the box's offers painted", |driven| {
        driven
            .window
            .conversation()
            .start(Size::new(1040.0, 820.0), Scale::ONE)
            .offers
            .len()
            == 2
            && driven.window.presses_decided()
    })?;
    driven.hear_for(Duration::from_millis(500))?;
    // Words, not an address: searching first, asking alo the second line.
    driven.press(ASK_ALO_TO_DO_IT);
    driven.until("the run at work with no page", |driven| {
        driven.window.conversation().working().is_some()
    })?;
    // The run in a turn of its own, drawn: its working line's tint.
    driven.until("the run's turn painted", |driven| {
        driven.start_pixel(WORKING_TINT_AT) == Some(TINT)
            && matches!(
                driven
                    .window
                    .conversation()
                    .start(Size::new(1040.0, 820.0), Scale::ONE)
                    .turns
                    .last(),
                Some(Turn::Asked(asked)) if asked.agent.is_some()
            )
    })?;
    assert!(
        model.body(0).contains(r#"\"page\":null"#),
        "{who}: {}",
        model.body(0)
    );
    // Let the model answer: it opens the page the person named.
    model.release.store(true, Ordering::SeqCst);
    driven.until(
        "the page the person named opened as the run's card",
        |driven| {
            driven.window.conversation().card().is_some_and(|card| {
                card.url.to_string().ends_with("/start.html")
                    && card.agent.is_some()
                    && card.said == typed
            })
        },
    )?;
    driven.until("the model sent the page", |_| model.asked() >= 2)?;
    assert!(
        model.body(1).contains("The start page"),
        "{who}: {}",
        model.body(1)
    );
    // localhost, which the person did not write: refused, never asked of.
    driven.until("the model told of the refusal", |_| model.asked() >= 3)?;
    assert!(
        model.body(2).contains("on this machine or its network"),
        "{who}: {}",
        model.body(2)
    );
    opened_by_the_name_resolved(&mut driven, &model, &asked, port)?;
    // Its link to a sign-in page: the page handed to the person.
    driven.until("the page handed over to sign in", |driven| {
        matches!(driven.now(), Some(Now::SignIn))
    })?;
    driven.hear_for(Duration::from_secs(2))?;
    assert_eq!(
        model.asked(),
        4,
        "{who}: the model asked while the page was the person's"
    );
    assert_eq!(
        count(&asked, "/other.html"),
        0,
        "{who}: localhost was asked"
    );
    assert_eq!(count(&asked, "/signin.html"), 1, "{who}");
    assert!(
        driven
            .window
            .conversation()
            .card()
            .is_some_and(|card| card.url.to_string().ends_with("/signin.html")),
        "{who}"
    );
    driven.window.finish();
    if let Some(why) = driven.window.unfinished() {
        return Err(format!("{who}: {why}"));
    }
    Ok(())
}

#[test]
fn the_agent_begins_with_no_page_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn the_agent_begins_with_no_page_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("821")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
