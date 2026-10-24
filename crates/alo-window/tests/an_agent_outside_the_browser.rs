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
use alo_window::agent_record::{AgentRecord, Caller, Did};
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::asking::{MOST_LINE, Refused};
use alo_window::beside::renderer_beside;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::conductor::{Conductor, Fonts, Records};
use alo_window::message::{News, Order};
use alo_window::pages::RenderedBy;
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

/// The same session, with the form open in the machine's Chrome rather
/// than this engine (queue item 630): served by [`ChromeServing`], reached
/// through the same socket, and its record taken back as it closes.
fn session_in_chrome(
    name: &str,
    lines: &[String],
) -> Result<(Vec<String>, bool, AgentRecord), String> {
    let page = std::fs::read(case().join("page.html")).map_err(|why| why.to_string())?;
    let serving = ChromeServing::start(
        Opening::Frozen {
            url: FORM.to_owned(),
            page,
        },
        (600, 700),
        AgentSites::Every,
    )
    .map_err(|why| format!("no tab in Chrome: {why}"))?;
    let path = socket(name)?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let answers = ask(&path, lines);
    listening.stop();
    let removed = !path.exists();
    let record = serving
        .records()
        .ok_or("the tab in Chrome did not finish")?;
    Ok((answers?, removed, record))
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

/// Requests every one of which is refused: three by the page, the rest by
/// this interface.
fn the_refused() -> Vec<String> {
    vec![
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
    ]
}

/// The kind of each answer that is a refusal, and nothing for one that is
/// not.
fn kinds(answers: &[String]) -> Vec<&str> {
    answers
        .iter()
        .map(|answer| {
            answer
                .split(r#""kind":""#)
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or("")
        })
        .collect()
}

#[test]
fn every_refusal_crosses_as_a_kind_and_is_recorded() {
    let (answers, _, records) =
        session("refused", &the_refused()).unwrap_or_else(|why| panic!("{why}"));
    let kinds = kinds(&answers);
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

/// The answer to the request `id` in `answers`.
fn answered(answers: &[String], id: u32) -> &str {
    let id = format!(r#""id":{id},"#);
    answers
        .iter()
        .find(|answer| answer.contains(&id))
        .map_or("", String::as_str)
}

#[test]
fn the_same_order_down_the_same_socket_fills_the_form_in_chrome() {
    let (answers, removed, _) =
        session_in_chrome("chrome-form", &the_order()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(answers.len(), 8, "one answer a request: {answers:#?}");

    // The same one tab, by its address.
    assert_eq!(
        answered(&answers, 1),
        format!(r#"{{"answer":"tabs","id":1,"tabs":["{FORM}"],"v":1}}"#)
    );
    // The form as Chrome's tree, read as ours: its fields by the words on
    // them. Their nodes and boxes are Chrome's (item 620), not the engine's.
    let tree = answered(&answers, 2);
    assert!(tree.starts_with(r#"{"answer":"tree","id":2,"#), "{tree}");
    for field in ["Customer name:", "Telephone:", "E-mail address:"] {
        let named = format!(r#"{{"children":[],"name":"{field}","node":"#);
        let at = tree
            .find(&named)
            .unwrap_or_else(|| panic!("{field} in {tree}"));
        let after = tree.get(at..).unwrap_or_default();
        assert!(
            after
                .split(r#""role":"#)
                .nth(1)
                .is_some_and(|role| role.starts_with(r#""textbox""#)),
            "{field} is a textbox: {after}"
        );
    }
    for (id, text) in [
        (3, "Ada Lovelace"),
        (4, "+44 20 7946 0000"),
        (5, "ada@example.com"),
    ] {
        let done = answered(&answers, id);
        assert!(
            done.starts_with(&format!(
                r#"{{"answer":"done","id":{id},"issues":[],"outcome":{{"kind":"TextPut","node":"#
            )) && done.contains(&format!(r#""text":"{text}"}}"#)),
            "{done}"
        );
    }
    assert!(
        answered(&answers, 6).contains(r#""outcome":{"kind":"Activated","name":"Large","node":"#),
        "{}",
        answered(&answers, 6)
    );

    // Read again: what was put in is in the fields, and the size chosen.
    let filled = answered(&answers, 7);
    for text in ["Ada Lovelace", "+44 20 7946 0000", "ada@example.com"] {
        let read_back = format!(r#"{{"children":[],"name":"{text}","node":"#);
        assert!(filled.contains(&read_back), "{read_back} in {filled}");
    }
    // Up to the next name, which is the radio's own.
    let large = filled
        .split(r#""name":"Large","node":"#)
        .nth(1)
        .and_then(|after| after.split(r#""name":"#).next())
        .unwrap_or_default();
    assert!(
        large.contains(r#""role":"radio","states":{"checked":"true"}"#),
        "Large is checked: {large}"
    );

    // Sent by its button, to where the form says, by POST: what the engine
    // answers, word for word but for the node.
    assert!(
        answered(&answers, 8).contains(r#""outcome":{"kind":"Submitted","method":"POST","node":"#)
            && answered(&answers, 8).contains(r#""to":"https://httpbin.org/post"}"#),
        "{}",
        answered(&answers, 8)
    );
    assert!(removed, "the socket goes when it stops");
}

#[test]
fn every_refusal_crosses_as_the_same_kind_through_chrome() {
    let (ours, _, _) =
        session("refused-ours", &the_refused()).unwrap_or_else(|why| panic!("{why}"));
    let (chromes, _, record) =
        session_in_chrome("refused-chrome", &the_refused()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(kinds(&chromes), kinds(&ours), "{chromes:#?}");
    // The interface's own refusals are not the page's, and are word for
    // word what the engine's tab is answered.
    for at in 3..ours.len() {
        assert_eq!(chromes.get(at), ours.get(at), "answer {at}");
    }
    let refused_by_the_page = record
        .entries()
        .filter(|entry| {
            matches!(
                entry.did,
                Did::Acted {
                    came_to: Err(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(refused_by_the_page, 3, "each verb an action, refused");
}

/// An entry with what is each renderer's own description of the page set
/// aside — the number of a box, how many boxes a tree has — and with what
/// each verb came to as [`changed`] writes it: what is left is what the
/// agent asked, of which document, as which action, and what came of it.
fn as_asked(entry: &alo_window::agent_record::Entry) -> String {
    let did = match &entry.did {
        Did::Listed { tabs } => format!("listed {tabs}"),
        Did::Read { document, .. } => format!("read {document:?}"),
        Did::AskedRenderer => "asked which renderer draws it".to_owned(),
        Did::Acted {
            action,
            document,
            target,
            verb,
            came_to,
        } => {
            let came_to = match came_to {
                Ok(Outcome::TextPut { text, .. }) => format!("put {text}"),
                Ok(Outcome::Activated { name, .. }) => format!("activated {name:?}"),
                Ok(Outcome::Submitted { method, to, .. }) => format!("{} {to}", method.name()),
                Ok(other) => format!("{other:?}"),
                Err(refusal) => format!("{refusal:?}"),
            };
            format!("{action} in {document:?}: {verb:?} {target}: {came_to}")
        }
        Did::Refused(refused) => format!("refused {refused:?}"),
    };
    format!(
        "{} at {:?} on {:?}: {did}",
        entry.caller, entry.tab, entry.site
    )
}

#[test]
fn the_record_of_the_order_differs_in_the_renderer_it_names() {
    let (_, _, ours) = session("record-ours", &the_order()).unwrap_or_else(|why| panic!("{why}"));
    let (_, _, chromes) =
        session_in_chrome("record-chrome", &the_order()).unwrap_or_else(|why| panic!("{why}"));
    let ours: Vec<_> = ours.agents.entries().collect();
    let chromes: Vec<_> = chromes.entries().collect();
    assert_eq!(
        chromes
            .iter()
            .map(|entry| as_asked(entry))
            .collect::<Vec<_>>(),
        ours.iter().map(|entry| as_asked(entry)).collect::<Vec<_>>(),
    );
    // Every entry that reached the tab names its renderer; the list of tabs
    // reached none.
    assert!(
        ours.iter()
            .skip(1)
            .all(|entry| entry.rendered_by == Some(RenderedBy::ThisEngine))
    );
    assert!(chromes.iter().skip(1).all(|entry| matches!(
        &entry.rendered_by,
        Some(RenderedBy::Chrome { product }) if product.starts_with("Chrome/")
    )));
    assert!([&ours, &chromes].iter().all(|entries| {
        entries
            .first()
            .is_some_and(|entry| entry.rendered_by.is_none())
    }));
    let line = chromes.get(2).map(ToString::to_string).unwrap_or_default();
    assert!(
        line.starts_with(&format!(
            "agent#0 at {FORM} on https://httpbin.org: action#0 "
        )) && line.contains("; drawn by Chrome/"),
        "{line}"
    );
}

#[test]
fn a_person_asking_which_renderer_draws_the_form_is_told_this_engine() {
    let lines = [format!(
        r#"{{"v":1,"id":1,"ask":"renderer","tab":"{FORM}"}}"#
    )];
    let (answers, _, records) = session("renderer", &lines).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        answers,
        [format!(
            r#"{{"answer":"renderer","id":1,"renderer":{{"kind":"ThisEngine"}},"said":"this browser's engine","tab":"{FORM}","v":1}}"#
        )],
        "queue item 622: the browser process says which renderer draws the tab"
    );
    let entries: Vec<_> = records.agents.entries().cloned().collect();
    assert_eq!(entries.len(), 1, "{entries:#?}");
    assert!(
        entries
            .first()
            .is_some_and(|entry| entry.did == Did::AskedRenderer
                && entry.rendered_by == Some(RenderedBy::ThisEngine)
                && entry.caller == Caller(0)),
        "{entries:#?}"
    );
}
