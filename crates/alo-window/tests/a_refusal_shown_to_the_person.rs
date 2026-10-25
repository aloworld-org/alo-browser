/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A refusal is shown to the person, not only answered to the agent: queue
//! item 625.
//!
//! *Each refusal kind leaves its own sentence where a person can read it,
//! asserted per kind, and an agent that is refused twice in a row leaves two
//! sentences rather than one.*
//!
//! An agent in a process of its own — `alo-ask` — asks for verbs through the
//! browser's socket, in a conductor over the real renderer program, and is
//! refused: by the page, with `verb.rs`'s refusals, and by the browser
//! process before any page is asked — an address that is not one, a site
//! the person did not name, a tab that is not open. What the window is sent
//! is taken into a [`Showing`], as the event loop takes it. Each refusal is
//! checked to be its own sentence, the `said` its agent was answered byte
//! for byte, under the kind the agent was told; a read refused and a line
//! that is not a request are checked to put nothing there. The lines kept
//! are placed in numbers and drawn in the refusal's colours. The reference
//! renders are `the_window_composed.rs`'s.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Page;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::renderer_beside;
use alo_window::colours::{agent_line, refused_line};
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

/// Where the page is, on the one site agents are let reach.
const HERE: &str = "https://example.com/";

/// The window, in device pixels at one: a page of 480 × 400 below the strip.
const WINDOW: (u32, u32) = (480, 436);

/// A control for each of `verb.rs`'s refusals a verb can meet on a page
/// without a form.
const PAGE: &str = "<!doctype html><body style=margin:0>\
     <h1 style=margin:0>Hello</h1>\
     <label>Name <input></label>\
     <button>Send</button>\
     <button disabled>Off</button>\
     <label>Locked <input readonly></label>\
     <button>Twice</button><button>Twice</button>\
     <label>Short <input maxlength=3></label>\
     <label>Day <input type=date></label>\
     <label>Time <input type=time></label>";

/// A conductor over the real renderer program, keeping agents to
/// [`HERE`]'s site, with [`PAGE`] open in its one tab, and what the window
/// had been sent once that tab was first painted.
fn browsing() -> Result<(Conductor, Receiver<News>, Showing), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let url = alo_url::parse(HERE).map_err(|why| format!("{why:?}"))?;
    let sites =
        AgentSites::named(["https://example.com"]).map_err(|why| format!("no sites: {why:?}"))?;
    let (tell, news) = channel();
    let conductor = Conductor::start_keeping_agents_to(
        sites,
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(Trust::of(&[])?)),
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
    let root = std::env::temp_dir().join(format!("alo-refused-{}-{name}", std::process::id()));
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

/// A verb on the thing called `name` in the tab at `tab`: `verb` is the
/// request's own fields for it.
fn act(id: u32, tab: &str, name: &str, verb: &str) -> String {
    format!(r#"{{"v":1,"id":{id},"ask":"act","tab":"{tab}","target":{{"name":"{name}"}},{verb}}}"#)
}

/// Putting `text` into the thing called `name` on the page.
fn put(id: u32, name: &str, text: &str) -> String {
    act(
        id,
        HERE,
        name,
        &format!(r#""verb":"put_text","text":"{text}""#),
    )
}

/// Activating the thing called `name` on the page.
fn activate(id: u32, name: &str) -> String {
    act(id, HERE, name, r#""verb":"activate""#)
}

/// `sentence` as JSON writes it in a string: its quotes and backslashes
/// escaped, which is all an ASCII sentence needs.
fn in_json(sentence: &str) -> String {
    sentence.replace('\\', "\\\\").replace('"', "\\\"")
}

/// What the window says of agents, as (sentence, refused) pairs, oldest
/// first.
fn told(showing: &Showing) -> Vec<(&str, bool)> {
    showing
        .agent_lines()
        .iter()
        .map(|line| (line.sentence.as_str(), line.refused))
        .collect()
}

/// What the window was told of agents, in order.
#[derive(Debug, Default)]
struct Heard {
    /// Each verb that ran's sentence.
    did: Vec<String>,
    /// Each refusal's sentence.
    refused: Vec<String>,
}

/// What the window is sent from now until it closes, taken into `showing`.
fn until_closed(news: &Receiver<News>, showing: &mut Showing) -> Result<Heard, String> {
    let mut heard = Heard::default();
    loop {
        let next = news
            .recv_timeout(AT_MOST)
            .map_err(|why| format!("the window was not closed: {why}"))?;
        match &next {
            News::AgentSaid(said) => heard.did.push(said.clone()),
            News::AgentRefused(said) => heard.refused.push(said.clone()),
            News::Closed => return Ok(heard),
            _ => {}
        }
        showing.hear(next);
    }
}

/// One agent's session: `lines` asked through the socket, what it was
/// answered, what the window was told of it, and what the window was
/// showing as it closed.
fn session(name: &str, lines: &[String]) -> Result<(Vec<String>, Heard, Showing), String> {
    let (conductor, news, mut showing) = browsing()?;
    if !showing.agent_lines().is_empty() {
        return Err(format!(
            "a line before any agent asked: {:?}",
            told(&showing)
        ));
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
    let heard = until_closed(&news, &mut showing)?;
    if !conductor.finish() {
        return Err("the conductor did not finish".to_owned());
    }
    Ok((answers?, heard, showing))
}

#[test]
fn each_kind_of_refusal_leaves_its_own_sentence_for_the_person() {
    // Each verb, and the kind it is refused with.
    let asked = [
        (put(1, "Nme", "Ada"), "NotFound"),
        (activate(2, "Twice"), "Ambiguous"),
        (activate(3, "Hello"), "NotOperable"),
        (activate(4, "Off"), "Disabled"),
        (put(5, "Send", "Ada"), "NotAField"),
        (put(6, "Locked", "Ada"), "ReadOnly"),
        (put(7, "Day", "tomorrow"), "NotADate"),
        (put(8, "Time", "10:00"), "Unbuilt"),
        (
            act(9, HERE, "Hello", r#""verb":"scroll","scroll":40"#),
            "DoesNotScroll",
        ),
        (put(10, "Short", "Lovelace"), "TooLong"),
        (
            act(11, "not an address", "Name", r#""verb":"activate""#),
            "NotAnAddress",
        ),
        (
            act(12, "https://example.org/", "Name", r#""verb":"activate""#),
            "NotGranted",
        ),
        (
            act(
                13,
                "https://example.com/else",
                "Name",
                r#""verb":"activate""#,
            ),
            "NoSuchTab",
        ),
    ];
    let lines: Vec<String> = asked.iter().map(|(line, _)| line.clone()).collect();
    let (answers, heard, showing) = session("kinds", &lines).unwrap_or_else(|why| panic!("{why}"));
    assert!(heard.did.is_empty(), "{heard:#?}");
    assert_eq!(answers.len(), asked.len(), "{answers:#?}");
    assert_eq!(heard.refused.len(), asked.len(), "{heard:#?}");
    for (((_, kind), answer), said) in asked.iter().zip(&answers).zip(&heard.refused) {
        assert!(
            answer.starts_with(r#"{"answer":"refused","#),
            "{kind}: {answer}"
        );
        assert!(answer.contains(&format!(r#""kind":"{kind}""#)), "{answer}");
        // The sentence the agent was given, not a second wording.
        assert!(
            answer.contains(&format!(r#""said":"{}""#, in_json(said))),
            "{kind}: {said:?} is not what {answer} said"
        );
    }
    // Every one its own: no two kinds left the same words.
    let mut distinct = heard.refused.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), asked.len(), "{heard:#?}");
    assert_eq!(
        heard.refused.first().map(String::as_str),
        Some("nothing on the page is \"Nme\"")
    );
    // The window keeps the last three, each a refusal.
    let last: Vec<(&str, bool)> = heard
        .refused
        .iter()
        .skip(asked.len() - 3)
        .map(|said| (said.as_str(), true))
        .collect();
    assert_eq!(told(&showing), last);
}

#[test]
fn a_read_refused_and_a_line_that_is_no_request_leave_nothing() {
    let lines = [
        r#"{"v":1,"id":1,"ask":"read","tab":"https://example.org/"}"#.to_owned(),
        "not a request".to_owned(),
        format!(r#"{{"v":1,"id":3,"ask":"act","tab":"{HERE}","verb":"activate"}}"#),
    ];
    let (answers, heard, showing) =
        session("nothing", &lines).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 3, "{answers:#?}");
    for (answer, kind) in answers
        .iter()
        .zip(["NotGranted", "NotJson", "MissingField"])
    {
        assert!(answer.contains(&format!(r#""kind":"{kind}""#)), "{answer}");
    }
    assert!(heard.refused.is_empty(), "{heard:#?}");
    assert!(showing.agent_lines().is_empty(), "{:?}", told(&showing));
}

#[test]
fn refused_twice_in_a_row_is_two_sentences_below_what_ran_placed_and_drawn() {
    let lines = [
        put(1, "Name", "Ada Lovelace"),
        put(2, "Nme", "Grace"),
        activate(3, "Off"),
    ];
    let (answers, heard, showing) = session("twice", &lines).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 3, "{answers:#?}");
    let [did] = heard.did.as_slice() else {
        panic!("not one verb ran: {heard:#?}");
    };
    let [first, second] = heard.refused.as_slice() else {
        panic!("not two refusals: {heard:#?}");
    };
    assert_eq!(first, "nothing on the page is \"Nme\"");
    assert!(second.ends_with(" says it is disabled"), "{second}");
    assert_eq!(
        told(&showing),
        [
            (did.as_str(), false),
            (first.as_str(), true),
            (second.as_str(), true)
        ]
    );

    let scene = showing.scene(WINDOW, Scale::ONE);
    let placed = Placement::of(WINDOW, Scale::ONE, scene.contents());
    let line = |y| PixelRect {
        x: 0,
        y,
        width: 480,
        height: 32,
    };
    assert_eq!(
        placed.agent_lines,
        [Some(line(340)), Some(line(372)), Some(line(404))]
    );
    let lettering = Lettering::compiled_in().unwrap_or_else(|| panic!("DejaVu Sans"));
    let canvas = compose(&scene, &lettering);
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    for (top, colours) in [
        (340, agent_line()),
        (372, refused_line()),
        (404, refused_line()),
    ] {
        assert_eq!(at(0, top), Some(colours.rule.to_rgba8()), "rule at {top}");
        assert_eq!(
            at(479, top + 31),
            Some(colours.ground.to_rgba8()),
            "ground at {top}"
        );
        let inked = (0..480)
            .flat_map(|x| (top + 1..top + 32).map(move |y| (x, y)))
            .filter(|(x, y)| at(*x, *y) == Some(colours.ink.to_rgba8()))
            .count();
        assert!(inked > 0, "no words on the line at {top}");
    }
    // The page's frame is untouched above them.
    let frame = scene.frame.unwrap_or_else(|| panic!("no frame"));
    assert_eq!(at(479, 339), frame.at(479, 339 - 36));
}

#[test]
fn a_refusal_outlasts_the_verb_that_runs_after_it_and_not_the_one_after_that() {
    let kept = [put(1, "Nme", "Ada"), put(2, "Name", "Ada Lovelace")];
    let (_, heard, showing) = session("kept", &kept).unwrap_or_else(|why| panic!("{why}"));
    let (Some(refused), Some(did)) = (heard.refused.first(), heard.did.first()) else {
        panic!("{heard:#?}");
    };
    assert_eq!(
        told(&showing),
        [(refused.as_str(), true), (did.as_str(), false)]
    );

    let gone = [
        put(1, "Nme", "Ada"),
        put(2, "Name", "Ada Lovelace"),
        put(3, "Name", "Grace Hopper"),
    ];
    let (_, heard, showing) = session("gone", &gone).unwrap_or_else(|why| panic!("{why}"));
    let last = heard.did.last().map_or("", String::as_str);
    assert!(
        last.starts_with("put \"Grace Hopper\" into box#"),
        "{heard:#?}"
    );
    assert_eq!(told(&showing), [(last, false)]);
}
