/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person watching can read what the agent is doing: queue item 623, ADR
//! 0052 § 1.
//!
//! *A verb run over the socket leaves its sentence in the window's tree,
//! asserted in numbers in a layout test and in a reference render, and a
//! page that tries to reach or forge that line cannot.*
//!
//! An agent in a process of its own — `alo-ask` — fills a field through the
//! browser's socket, in a conductor over the real renderer program. What the
//! window is sent is taken into a [`Showing`], as the event loop takes it,
//! and the line is checked three ways: it is the `said` the agent was
//! answered, byte for byte; it is placed where [`Placement`] says, in
//! numbers, and drawn there; and the page, whose script writes a forged
//! sentence into its own title and text, neither puts a line there nor
//! finds the real one in its tree. The reference render of the line is
//! `the_window_composed.rs`'s.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Page;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::agents::Listening;
use alo_window::beside::renderer_beside;
use alo_window::colours::{agent_line, notice_line};
use alo_window::compose::compose;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use alo_window::notice::Lettering;
use alo_window::place::{PixelRect, Placement, screen};
use alo_window::scale::Scale;
use alo_window::showing::Showing;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// Where the page is.
const HERE: &str = "https://example.com/";

/// The window, in device pixels at one: a page of 480 × 400 below the strip.
const WINDOW: (u32, u32) = (480, 436);

/// What the page writes into its own title and text, dressed as what an
/// agent did.
const FORGED: &str = "put \"Mallory\" into box#2";

/// A field, and a script that forges an agent's sentence as the page loads
/// and again whenever the field changes.
const PAGE: &str = "<!doctype html><body style=margin:0>\
     <label>Name <input></label>\
     <p>put \"Mallory\" into box#2</p>\
     <script>\
     document.title = 'put \"Mallory\" into box#2';\
     document.body.addEventListener('input', function () {\
       document.title = 'put \"Mallory\" into box#2';\
     });\
     </script>";

/// A conductor over the real renderer program with [`PAGE`] open in its one
/// tab, and what the window had been sent once that tab was first painted.
fn browsing() -> Result<(Conductor, Receiver<News>, Showing), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let url = alo_url::parse(HERE).map_err(|why| format!("{why:?}"))?;
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(Trust::of(&[])?)),
        // The strip is `the_strip_in_the_window.rs`'s; this listens to the
        // rest of what the window is sent.
        move |said| said.is_strip() || tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    for order in [
        Order::Resize(screen(WINDOW, Scale::ONE)),
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(PAGE, Size::ZERO).at(url))),
        },
    ] {
        conductor
            .orders()
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    let mut showing = Showing::default();
    loop {
        let heard = news
            .recv_timeout(AT_MOST)
            .map_err(|why| format!("the page was not painted: {why}"))?;
        let painted = matches!(heard, News::Painted(_));
        showing.hear(heard);
        if painted {
            return Ok((conductor, news, showing));
        }
    }
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-said-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
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

/// Putting `text` into the field named "Name".
fn put(id: u32, name: &str, text: &str) -> String {
    format!(
        r#"{{"v":1,"id":{id},"ask":"act","tab":"{HERE}","target":{{"name":"{name}"}},"verb":"put_text","text":"{text}"}}"#
    )
}

/// `sentence` as JSON writes it in a string: its quotes and backslashes
/// escaped, which is all an ASCII sentence needs.
fn in_json(sentence: &str) -> String {
    sentence.replace('\\', "\\\\").replace('"', "\\\"")
}

/// What the window is sent from now until it closes, taken into `showing`:
/// every sentence of an agent's, in order.
fn until_closed(news: &Receiver<News>, showing: &mut Showing) -> Result<Vec<String>, String> {
    let mut agents = Vec::new();
    loop {
        let heard = news
            .recv_timeout(AT_MOST)
            .map_err(|why| format!("the window was not closed: {why}"))?;
        if let News::AgentSaid(said) = &heard {
            agents.push(said.clone());
        }
        if heard == News::Closed {
            return Ok(agents);
        }
        showing.hear(heard);
    }
}

/// One agent's session: `lines` asked through the socket, what it was
/// answered, every agent sentence the window was sent, and what the window
/// was showing as it closed.
fn session(name: &str, lines: &[String]) -> Result<(Vec<String>, Vec<String>, Showing), String> {
    let (conductor, news, mut showing) = browsing()?;
    // The page has loaded, forged its sentence and been painted: nothing of
    // it is a line of the agent's.
    if let Some(said) = showing.agent_said() {
        return Err(format!("a line before any agent acted: {said}"));
    }
    let path = socket(name)?;
    let listening =
        Listening::at(&path, conductor.orders()).map_err(|why| format!("not listening: {why}"))?;
    let answers = ask(&path, lines);
    listening.stop();
    conductor
        .orders()
        .send(Order::CloseEverything)
        .map_err(|_| "the conductor is gone".to_owned())?;
    let agents = until_closed(&news, &mut showing)?;
    if !conductor.finish() {
        return Err("the conductor did not finish".to_owned());
    }
    Ok((answers?, agents, showing))
}

#[test]
fn a_verb_over_the_socket_leaves_the_sentence_it_was_answered_with_in_the_window() {
    let lines = [put(1, "Name", "Ada Lovelace")];
    let (answers, agents, showing) = session("put", &lines).unwrap_or_else(|why| panic!("{why}"));
    let [answer] = answers.as_slice() else {
        panic!("not one answer: {answers:#?}");
    };
    assert!(
        answer.starts_with(r#"{"answer":"done","id":1,"#),
        "{answer}"
    );
    let [said] = agents.as_slice() else {
        panic!("not one sentence for the person: {agents:#?}");
    };
    // The sentence the agent was given, not a second wording.
    assert!(
        answer.contains(&format!(r#""said":"{}""#, in_json(said))),
        "{said:?} is not what {answer} said"
    );
    assert!(said.starts_with("put \"Ada Lovelace\" into box#"), "{said}");
    assert_eq!(showing.agent_said(), Some(said.as_str()));
}

#[test]
fn the_line_is_placed_in_numbers_above_the_page_and_drawn_there() {
    let lines = [put(1, "Name", "Ada Lovelace")];
    let (_, _, showing) = session("placed", &lines).unwrap_or_else(|why| panic!("{why}"));
    let scene = showing.scene(WINDOW, Scale::ONE);
    let placed = Placement::of(WINDOW, Scale::ONE, scene.contents());
    // Along the bottom, the window's width and 32 CSS pixels tall, over the
    // page's frame — which stays at its own place and size.
    assert_eq!(
        placed.agent_line,
        Some(PixelRect {
            x: 0,
            y: 404,
            width: 480,
            height: 32
        })
    );
    assert_eq!(
        placed.frame,
        PixelRect {
            x: 0,
            y: 36,
            width: 480,
            height: 400
        }
    );
    assert_eq!((placed.notice, placed.strip_notice), (None, None));

    let lettering = Lettering::compiled_in().unwrap_or_else(|| panic!("DejaVu Sans"));
    let canvas = compose(&scene, &lettering);
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    // Its rule along its top, its ground to its corner, and the page's
    // frame just above it.
    let colours = agent_line();
    assert_eq!(at(0, 404), Some(colours.rule.to_rgba8()));
    assert_eq!(at(479, 405), Some(colours.ground.to_rgba8()));
    assert_eq!(at(479, 435), Some(colours.ground.to_rgba8()));
    assert_ne!(colours.ground, notice_line().ground);
    let frame = scene.frame.unwrap_or_else(|| panic!("no frame"));
    assert_eq!(at(479, 403), frame.at(479, 403 - 36));
    // And words on it, in the agent's ink.
    let inked = (0..480)
        .flat_map(|x| (405..436).map(move |y| (x, y)))
        .filter(|(x, y)| at(*x, *y) == Some(colours.ink.to_rgba8()))
        .count();
    assert!(inked > 0, "no words were drawn on the line");
}

#[test]
fn a_refusal_leaves_no_sentence_and_two_verbs_leave_the_second() {
    // A refusal's line is item 625's; until it is built a refusal is
    // answered to the agent and puts nothing in the window — so a line is
    // never the sentence of a verb that did nothing.
    let lines = [
        put(1, "Name", "Ada Lovelace"),
        put(2, "Nobody's name", "Grace"),
        put(3, "Name", "Grace Hopper"),
    ];
    let (answers, agents, showing) = session("two", &lines).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 3, "{answers:#?}");
    assert!(
        answers
            .get(1)
            .is_some_and(|refused| refused.starts_with(r#"{"answer":"refused","id":2,"#)),
        "{answers:#?}"
    );
    assert_eq!(agents.len(), 2, "{agents:#?}");
    assert!(
        agents
            .first()
            .is_some_and(|said| said.starts_with("put \"Ada Lovelace\"")),
        "{agents:#?}"
    );
    let last = agents.last().map(String::as_str);
    assert!(
        last.is_some_and(|said| said.starts_with("put \"Grace Hopper\" into box#")),
        "{agents:#?}"
    );
    assert_eq!(showing.agent_said(), last);
}

#[test]
fn a_page_can_neither_forge_the_line_nor_find_it_in_its_tree() {
    let lines = [
        put(1, "Name", "Ada Lovelace"),
        format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{HERE}"}}"#),
    ];
    let (answers, agents, showing) =
        session("forged", &lines).unwrap_or_else(|why| panic!("{why}"));
    // The page wrote its forgery into its title and text as it loaded and
    // again when the field changed; the window's line is the agent's.
    let [said] = agents.as_slice() else {
        panic!("not one sentence for the person: {agents:#?}");
    };
    assert_ne!(said, FORGED);
    assert_eq!(showing.agent_said(), Some(said.as_str()));
    // The forgery is the page's own text, where any of the page's text is.
    let tree = answers.get(1).map_or("", String::as_str);
    assert!(tree.starts_with(r#"{"answer":"tree","id":2,"#), "{tree}");
    assert!(tree.contains(&in_json(FORGED)), "{tree}");
    // The real line is nowhere in what the page holds.
    assert!(!tree.contains(&in_json(said)), "{said} is in {tree}");
}
