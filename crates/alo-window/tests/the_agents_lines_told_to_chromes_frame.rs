/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the frame around Chrome's window is told of an agent's verbs is
//! what the agent was answered, word for word (queue item 679, and items
//! 623 and 625 through Chrome).
//!
//! Chrome is started on the frozen `web-a-form` at the address it was
//! served from, and the frame's side of it is this test's: everything the
//! thread holding Chrome tells the frame is kept. It is started headless
//! ([`ChromeServing::start_telling`]), because what the frame is told of
//! agents does not depend on where Chrome's window is — and a shown one
//! brought in front would send another test's Chrome window into Stage
//! Manager's strip (item 643), which `the_frame_around_chromes_window.rs`
//! failed on in the gate while this ran beside it shown. An agent in a process of its own,
//! `alo-ask`, reads the form, fills a field, is refused by the page twice and
//! by this process once, and sends two lines that are not verbs.
//!
//! The frame is told one line for each verb and nothing else, in order: the
//! sentence of the verb that ran and each refusal, each the `said` of the
//! answer the agent got. What the frame then draws of them is
//! `the_window_composed.rs`'s reference renders, from the same lines.

use alo_window::agent_lines::{AgentLine, AgentLines};
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::frame_told::ToFrame;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// A tab nobody opened.
const NO_TAB: &str = "https://example.com/";

/// The frozen form, as the corpus holds it.
fn case() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/web-a-form")
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket() -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-frame-lines-{}", std::process::id()));
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
            "alo-ask failed: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// A request to act on the tab at `tab`, on `target`, with `verb`.
fn act(id: u32, tab: &str, target: &str, verb: &str) -> String {
    format!(r#"{{"v":1,"id":{id},"ask":"act","tab":"{tab}","target":{target},{verb}}}"#)
}

/// An answer's `said`, as JSON writes it with its quotes escaped, read back
/// to the words; [`None`] for an answer that says nothing.
fn said(answer: &str) -> Option<String> {
    let (_, after) = answer.split_once(r#""said":""#)?;
    let mut words = String::new();
    let mut letters = after.chars();
    while let Some(letter) = letters.next() {
        match letter {
            '"' => return Some(words),
            '\\' => words.push(letters.next()?),
            _ => words.push(letter),
        }
    }
    None
}

/// The agent's line `news` tells the frame, if it tells one: every other
/// thing a frame is told named, so that a new one is decided here.
fn agents_line(news: ToFrame) -> Option<AgentLine> {
    match news {
        ToFrame::Agent(line) => Some(line),
        ToFrame::Stand(_)
        | ToFrame::Named(_)
        | ToFrame::Strip(_)
        | ToFrame::StripSaid(_)
        | ToFrame::Cast(_)
        | ToFrame::Uncast(_)
        | ToFrame::Moved(_)
        | ToFrame::Loading(_)
        | ToFrame::Travel(_) => None,
    }
}

#[test]
fn the_frame_is_told_each_verbs_answer_and_nothing_else() -> Result<(), String> {
    let page = std::fs::read(case().join("page.html")).map_err(|why| why.to_string())?;
    let (tell, told) = channel();
    let serving = ChromeServing::start_telling(
        Opening::Frozen {
            url: FORM.to_owned(),
            page,
        },
        (600, 700),
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no tab in Chrome: {why}"))?;
    let path = socket()?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let lines = [
        format!(r#"{{"v":1,"id":1,"ask":"read","tab":"{FORM}"}}"#),
        act(
            2,
            FORM,
            r#"{"name":"Customer name:"}"#,
            r#""verb":"put_text","text":"Ada Lovelace""#,
        ),
        act(
            3,
            FORM,
            r#"{"name":"Nothing here"}"#,
            r#""verb":"activate""#,
        ),
        act(
            4,
            FORM,
            r#"{"name":"Small"}"#,
            r#""verb":"put_text","text":"x""#,
        ),
        // Refused by this process, before any page was asked.
        act(5, NO_TAB, r#"{"name":"Small"}"#, r#""verb":"activate""#),
        // Neither a verb: a read refused, and a line that was no request.
        format!(r#"{{"v":1,"id":6,"ask":"read","tab":"{NO_TAB}"}}"#),
        "not json".to_owned(),
    ];
    let answers = ask(&path, &lines);
    listening.stop();
    // Its thread ends with the last sender of orders, and the frame's
    // channel with it.
    serving
        .records()
        .ok_or("the tab in Chrome did not finish")?;
    let answers = answers?;
    assert_eq!(answers.len(), lines.len(), "{answers:#?}");

    let heard: Vec<AgentLine> = told.iter().filter_map(agents_line).collect();
    // One a verb, in the words each was answered with.
    let expected: Vec<AgentLine> = answers
        .get(1..5)
        .unwrap_or_default()
        .iter()
        .map(|answer| AgentLine {
            sentence: said(answer).unwrap_or_else(|| panic!("no said in {answer}")),
            refused: answer.contains(r#""answer":"refused""#),
        })
        .collect();
    assert_eq!(heard, expected, "{answers:#?}");
    let sentences: Vec<(&str, bool)> = heard
        .iter()
        .map(|line| (line.sentence.as_str(), line.refused))
        .collect();
    let [
        (put, false),
        ("nothing on the page is \"Nothing here\"", true),
        (not_a_field, true),
        (no_tab, true),
    ] = sentences.as_slice()
    else {
        panic!("a verb that ran and three refusals: {sentences:#?}");
    };
    assert!(put.starts_with("put \"Ada Lovelace\" into box#"), "{put}");
    assert!(
        not_a_field.ends_with("which is not a thing text goes into"),
        "{not_a_field}"
    );
    assert_eq!(*no_tab, format!("no tab is open at {NO_TAB}"));

    // Kept as the frame keeps them: the most there are, the verb that ran
    // given up first.
    let mut kept = AgentLines::default();
    for line in heard {
        kept.heard(line);
    }
    assert_eq!(
        kept.lines()
            .iter()
            .map(|line| line.refused)
            .collect::<Vec<_>>(),
        [true, true, true]
    );
    Ok(())
}
