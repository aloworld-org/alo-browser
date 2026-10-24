/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab of this engine's, after an agent follows a link to a part of its
//! page: queue item 640, the engine's half, beside
//! `a_tab_in_chrome_named_where_it_is.rs`'s Chrome half.
//!
//! The same page and the same lines down the socket, from `alo-ask`, a
//! process of its own, to a conductor over the real renderer program. The
//! engine answers the link as Chrome does — `Followed`, to the link's own
//! `href` — and **goes nowhere**: going where a page was let go is session
//! history's, queue item 85, and the conductor does not do it for any link
//! yet. So the tab is still named where it was opened, without the part,
//! where the same tab in Chrome is named with it. That is the one way the
//! two renderers differ here, and it is the difference every link already
//! makes between them (item 630).

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Page, Screen};
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::renderer_beside;
use alo_window::conductor::{Conductor, Fonts};
use alo_window::message::{News, Order};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const AT_MOST: Duration = Duration::from_secs(60);

/// Where the page is stated to be.
const PARTS: &str = "https://example.org/part.html";

/// A page whose link goes to a part of itself, as Chrome's test serves it
/// but for the button, which moves the page by script.
const PAGE: &str = "<!doctype html><title>Parts</title><h1>Parts</h1>\
                    <a href=\"#second\">To the second part</a>\
                    <div style=\"height:3000px\"></div><h2 id=\"second\">Second part</h2>";

/// The renderer program, beside `alo` as cargo built it.
fn renderer() -> Result<PathBuf, String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    Ok(renderer)
}

/// A conductor letting agents reach every site, with the page open in one
/// tab and painted before anybody asks, and what it tells of: kept, since a
/// conductor with nobody to tell stops.
fn browsing() -> Result<(Conductor, Receiver<News>), String> {
    let (tell, news) = channel();
    let conductor = Conductor::start_keeping_agents_to(
        AgentSites::Every,
        Tabs::over(Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        Network::over(Pool::with_trust(Trust::of(&[])?)),
        move |said| said.is_strip() || tell.send(said).is_ok(),
    )
    .map_err(|why| format!("no conductor: {why}"))?;
    let orders = conductor.orders();
    orders
        .send(Order::Resize(Screen::new(Size::new(600.0, 400.0))))
        .map_err(|_| "the conductor is gone".to_owned())?;
    let at = alo_url::parse(PARTS).map_err(|why| format!("{why:?}"))?;
    orders
        .send(Order::Open {
            url: at.clone(),
            page: Some(Box::new(Page::new(PAGE.to_owned(), Size::ZERO).at(at))),
        })
        .map_err(|_| "the conductor is gone".to_owned())?;
    loop {
        match news.recv_timeout(AT_MOST) {
            Ok(News::Painted(_)) => return Ok((conductor, news)),
            Ok(News::Chosen(None)) => {}
            other => return Err(format!("the page was not painted: {other:?}")),
        }
    }
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-part-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
fn ask(socket: &Path, lines: &[&str]) -> Result<Vec<String>, String> {
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

/// The `kind` an answer's outcome or refusal says, or nothing.
fn kind(answer: &str) -> &str {
    answer
        .split(r#""kind":""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("")
}

#[test]
fn a_tab_in_the_engine_is_still_named_where_it_was_opened_after_a_link_to_a_part() {
    let (conductor, _news) = browsing().unwrap_or_else(|why| panic!("{why}"));
    let path = socket("part").unwrap_or_else(|why| panic!("{why}"));
    let listening = Listening::at(&path, conductor.orders())
        .unwrap_or_else(|why| panic!("not listening: {why}"));
    let answers = ask(
        &path,
        &[
            r#"{"v":1,"id":1,"ask":"act","tab":"https://example.org/part.html","target":{"role":"link","name":"To the second part"},"verb":"activate"}"#,
            r#"{"v":1,"id":2,"ask":"tabs"}"#,
            r#"{"v":1,"id":3,"ask":"read","tab":"https://example.org/part.html#second"}"#,
            r#"{"v":1,"id":4,"ask":"read","tab":"https://example.org/part.html"}"#,
        ],
    );
    listening.stop();
    let _ = conductor.orders().send(Order::CloseEverything);
    let _ = conductor.records();
    let answers = answers.unwrap_or_else(|why| panic!("{why}"));
    // Answered as Chrome answers it: followed, to the link's own words.
    assert!(
        answers.first().is_some_and(
            |answer| kind(answer) == "Followed" && answer.contains(r##""to":"#second""##)
        ),
        "{answers:#?}"
    );
    // And gone nowhere: named, listed and read where it was opened.
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(r#"{"answer":"tabs","id":2,"tabs":["https://example.org/part.html"],"v":1}"#),
        "{answers:#?}"
    );
    assert_eq!(
        answers.get(2).map(|answer| kind(answer)),
        Some("NoSuchTab"),
        "{answers:#?}"
    );
    assert!(
        answers.get(3).is_some_and(|tree| tree
            .starts_with(r#"{"answer":"tree","id":4,"tab":"https://example.org/part.html","#)
            && tree.contains(r#""name":"Second part""#)),
        "{answers:#?}"
    );
}
