/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent in a process of its own, reading and driving a page nobody wrote
//! for us through the browser's socket: queue item 496, ADR 0055.
//!
//! Item 131 built the capability inside one process. Here the browser
//! process — a conductor holding a tab, over the real renderer program — is
//! reached only through its socket, by `alo-ask`, a separate program that
//! reads nothing of what it sends or is answered. The page is the frozen
//! `web-a-form`, httpbin's copy of the HTML specification's pizza form, at
//! the address it was served from.
//!
//! The agent reads the form as a tree, fills it by the words on it, chooses
//! a size, and sends the order by its button. Every refusal it meets arrives
//! as a kind, and the record the browser process wrote afterwards says what
//! it read and what it changed.

use alo_agent::{Outcome, Refusal, Verb};
use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Screen;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
use alo_window::agent_record::{Caller, Did};
use alo_window::agents::Listening;
use alo_window::asking::{MOST_LINE, Refused};
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts, Records};
use alo_window::message::{News, Order};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// The frozen form, as the corpus holds it.
fn case() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/web-a-form")
}

/// A conductor over the real renderer program, with the corpus's fonts, the
/// form open in its one tab at the case's size — and the first frame it
/// painted, so the form is known to be loaded before anybody asks.
fn browsing() -> Result<(Conductor, Receiver<News>), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let origin =
        std::fs::read_to_string(case().join("origin.txt")).map_err(|why| why.to_string())?;
    assert_eq!(origin.lines().next(), Some(FORM));
    let html = std::fs::read_to_string(case().join("page.html")).map_err(|why| why.to_string())?;
    let url = alo_url::parse(FORM).map_err(|why| format!("{why:?}"))?;
    let network = Network::over(Pool::with_trust(Trust::of(&[])?));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        network,
        // This test listens to the tab; the strip is `the_strip_in_the_window.rs`.
        move |said| said.is_strip() || tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    for order in [
        Order::Resize(Screen::new(Size::new(600.0, 700.0))),
        Order::Open {
            url: url.clone(),
            page: Some(Box::new(Page::new(html, Size::ZERO).at(url))),
        },
    ] {
        conductor
            .orders()
            .send(order)
            .map_err(|_| "the conductor is gone".to_owned())?;
    }
    let _: Frame = match news.recv_timeout(AT_MOST) {
        Ok(News::Painted(frame)) => frame,
        other => return Err(format!("the form was not painted: {other:?}")),
    };
    Ok((conductor, news))
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-ask-{}-{name}", std::process::id()));
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
    // Its input ends here, which is how it knows to stop.
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

/// One agent's session with the browser, from a process of its own: `lines`
/// sent, and what it was answered. Then the socket stopped — and whether it
/// was removed — and the window closed, and what the conductor recorded.
fn session(name: &str, lines: &[String]) -> Result<(Vec<String>, bool, Records), String> {
    let (conductor, _news) = browsing()?;
    let path = socket(name)?;
    let listening =
        Listening::at(&path, conductor.orders()).map_err(|why| format!("not listening: {why}"))?;
    let answers = ask(&path, lines);
    listening.stop();
    let removed = !path.exists();
    let _ = conductor.orders().send(Order::CloseEverything);
    let records = conductor.records().ok_or("the conductor did not finish")?;
    Ok((answers?, removed, records))
}

/// What each verb in the record changed, in a few words, in order.
fn changed(records: &Records) -> Vec<String> {
    records
        .agents
        .entries()
        .filter_map(|entry| match &entry.did {
            Did::Acted {
                came_to: Ok(outcome),
                ..
            } => Some(match outcome {
                Outcome::TextPut { text, .. } => format!("put {text}"),
                Outcome::Activated { name, .. } => format!("activated {name:?}"),
                Outcome::Submitted { method, to, .. } => format!("{} {to}", method.name()),
                other => format!("{other}"),
            }),
            _ => None,
        })
        .collect()
}

/// A request to act on `target`, a JSON object, with `verb` and what it
/// carries.
fn act(id: u32, target: &str, verb: &str) -> String {
    format!(r#"{{"v":1,"id":{id},"ask":"act","tab":"{FORM}","target":{target},{verb}}}"#)
}

/// The order, as an agent places it: the tabs, the form read, three fields
/// filled and a size chosen by the words on them, the form read again, and
/// sent by its button.
fn the_order() -> Vec<String> {
    let put = |id, name: &str, text: &str| {
        act(
            id,
            &format!(r#"{{"name":"{name}"}}"#),
            &format!(r#""verb":"put_text","text":"{text}""#),
        )
    };
    vec![
        r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
        format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{FORM}"}}"#),
        put(3, "Customer name:", "Ada Lovelace"),
        put(4, "Telephone:", "+44 20 7946 0000"),
        put(5, "E-mail address:", "ada@example.com"),
        act(
            6,
            r#"{"role":"radio","name":"Large"}"#,
            r#""verb":"activate""#,
        ),
        format!(r#"{{"v":1,"id":7,"ask":"read","tab":"{FORM}"}}"#),
        act(
            8,
            r#"{"role":"button","name":"Submit order"}"#,
            r#""verb":"activate""#,
        ),
    ]
}

#[test]
fn an_agent_in_its_own_process_fills_and_sends_a_form_nobody_wrote_for_us() {
    let (answers, removed, _) = session("form", &the_order()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 8, "one answer a request: {answers:#?}");
    let answer = |at: usize| answers.get(at).map_or("", String::as_str);

    assert_eq!(
        answer(0),
        format!(r#"{{"answer":"tabs","id":1,"tabs":["{FORM}"],"v":1}}"#)
    );

    // The form as a tree, by the words on it, its boxes where the frozen
    // case's `agent.txt` has them. Since queue item 506 a field is set in a
    // control's own 13.33 px, 21 tall as Chrome's is, and since 511 it is
    // Chrome's 172 wide. The "Large" radio is Chrome's 242.59375 down.
    let tree = answer(1);
    assert!(tree.starts_with(r#"{"answer":"tree","id":2,"#), "{tree}");
    assert!(
        tree.contains(
            r#""name":"Customer name:","node":10,"rect":{"height":21.0,"width":172.0,"x":146.65625,"y":16.0},"role":"textbox""#
        ),
        "{tree}"
    );
    assert!(
        tree.contains(
            r#""name":"Large","node":45,"rect":{"height":13.0,"width":13.0,"x":29.0,"y":242.59375},"role":"radio","states":{"checked":"false"}"#
        ),
        "{tree}"
    );
    assert!(tree.contains(r#""name":"Submit order","#), "{tree}");

    for (at, id, text) in [
        (2, 3, "Ada Lovelace"),
        (3, 4, "+44 20 7946 0000"),
        (4, 5, "ada@example.com"),
    ] {
        let done = answer(at);
        assert!(
            done.starts_with(&format!(
                r#"{{"answer":"done","id":{id},"issues":[],"outcome":{{"kind":"TextPut","node":"#
            )),
            "{done}"
        );
        assert!(done.contains(&format!(r#""text":"{text}"}}"#)), "{done}");
    }
    assert!(
        answer(5).contains(r#""outcome":{"kind":"Activated","name":"Large","node":"#),
        "{}",
        answer(5)
    );

    // Read again: what was put in is in the fields, and the size chosen.
    let filled = answer(6);
    for read_back in [
        r#"{"children":[],"name":"Ada Lovelace","#,
        r#"{"children":[],"name":"+44 20 7946 0000","#,
        // By where it is rather than its id: the text put in above is in the
        // tree now, and the ids of the boxes after it have moved.
        r#""rect":{"height":13.0,"width":13.0,"x":29.0,"y":242.59375},"role":"radio","states":{"checked":"true"}"#,
    ] {
        assert!(filled.contains(read_back), "{read_back} in {filled}");
    }

    // The order is sent by its button, to where the form says, by POST.
    assert!(
        answer(7).contains(r#""outcome":{"kind":"Submitted","method":"POST","node":"#)
            && answer(7).contains(r#""to":"https://httpbin.org/post"}"#),
        "{}",
        answer(7)
    );

    assert!(removed, "the socket goes when it stops");
}

#[test]
fn the_record_says_what_the_agent_read_and_what_it_changed() {
    let (_, _, records) = session("record", &the_order()).unwrap_or_else(|why| panic!("{why}"));
    // Written by the browser process, in order, all of it by the one caller.
    let entries: Vec<_> = records.agents.entries().collect();
    assert_eq!(entries.len(), 8);
    assert!(entries.iter().all(|entry| entry.caller == Caller(0)));
    assert!(matches!(
        entries.first().map(|entry| &entry.did),
        Some(Did::Listed { tabs: 1 })
    ));
    let read: Vec<usize> = entries
        .iter()
        .filter_map(|entry| match entry.did {
            Did::Read { nodes, document } if document.is_some() => Some(nodes),
            _ => None,
        })
        .collect();
    assert_eq!(read.len(), 2, "two reads, of the form's document");
    assert!(
        read.first().is_some_and(|nodes| *nodes > 20),
        "the whole form was read: {read:?}"
    );
    assert_eq!(
        changed(&records),
        [
            "put Ada Lovelace",
            "put +44 20 7946 0000",
            "put ada@example.com",
            "activated Some(\"Large\")",
            "POST https://httpbin.org/post",
        ]
    );
    assert!(entries.iter().all(|entry| entry.tab.as_deref() != Some("")));
    let words: Vec<String> = entries.iter().map(ToString::to_string).collect();
    assert!(
        words.iter().any(|line| line.starts_with(&format!(
            "agent#0 at {FORM} on https://httpbin.org: action#"
        )) && line.contains("put \"Ada Lovelace\"")),
        "{words:#?}"
    );
}

#[test]
fn every_refusal_crosses_as_a_kind_and_is_recorded() {
    let (answers, _, records) = session(
        "refused",
        &[
            act(1, r#"{"name":"Nothing here"}"#, r#""verb":"activate""#),
            act(2, r#"{"name":"Small"}"#, r#""verb":"put_text","text":"x""#),
            act(
                3,
                r#"{"name":"Preferred delivery time:"}"#,
                r#""verb":"put_text","text":"12:00""#,
            ),
            r#"{"v":1,"id":4,"ask":"read","tab":"https://example.com/"}"#.to_owned(),
            act(5, r#"{"x":10,"y":20}"#, r#""verb":"activate""#),
            "not json".to_owned(),
            String::new(),
            r#"{"v":2,"id":"six","ask":"tabs"}"#.to_owned(),
            // Last: a line too long ends the connection after its answer.
            format!(
                r#"{{"v":1,"ask":"read","tab":"{}"}}"#,
                "a".repeat(MOST_LINE)
            ),
        ],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let kinds: Vec<&str> = answers
        .iter()
        .map(|answer| {
            answer
                .split(r#""kind":""#)
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or("")
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "NotFound",
            "NotAField",
            "Unbuilt",
            "NoSuchTab",
            "BadField",
            "NotJson",
            "UnknownVersion",
            "LineTooLong",
        ],
        "{answers:#?}"
    );
    assert!(
        answers.first().is_some_and(|answer| answer.contains(
            r#""refusal":{"kind":"NotFound","tab":"https://httpbin.org/forms/post","target":{"name":"Nothing here"}}"#
        )),
        "{answers:#?}"
    );
    assert!(
        answers
            .get(6)
            .is_some_and(|answer| answer.contains(r#""id":"six""#)),
        "the echo comes back with a refusal too: {answers:#?}"
    );
    assert!(
        answers.iter().all(|answer| answer.contains(r#""said":""#)),
        "each kind has its words beside it, for a person"
    );

    let did: Vec<Did> = records
        .agents
        .entries()
        .map(|entry| entry.did.clone())
        .collect();
    assert_eq!(did.len(), 8, "every request, blank line aside: {did:#?}");
    assert!(matches!(
        did.first(),
        Some(Did::Acted {
            verb: Verb::Activate,
            came_to: Err(Refusal::NotFound { .. }),
            ..
        })
    ));
    assert!(matches!(
        did.get(3),
        Some(Did::Refused(Refused::NoSuchTab { .. }))
    ));
    assert!(matches!(
        did.last(),
        Some(Did::Refused(Refused::LineTooLong { most })) if *most == MOST_LINE
    ));
}
