/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-ask --plain` (queue item 915): a person at a terminal reads the
//! page's tree as one node a line, indented, with its roles and names and
//! no position — through the real program, over a real socket, from a
//! process of its own.
//!
//! The browser process is a conductor holding a tab over the real renderer
//! program, the page the frozen `web-a-form` (httpbin's copy of the HTML
//! specification's pizza form) at the address it was served from, as
//! `an_agent_outside_the_browser.rs` holds it.

use alo_layout::Size;
use alo_net::{Pool, Trust};
use alo_renderer::Screen;
use alo_renderer::fetch_make::Network;
use alo_renderer::host::Renderers;
use alo_renderer::tab::Tabs;
use alo_renderer::{Frame, Page};
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

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// The frozen form, as the corpus holds it.
fn case() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/web-a-form")
}

/// A conductor over the real renderer program with the form open in its
/// one tab, painted once so it is known to be loaded.
fn browsing() -> Result<(Conductor, Receiver<News>), String> {
    let renderer = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if !renderer.is_file() {
        return Err(format!("no renderer at {}", renderer.display()));
    }
    let origin =
        std::fs::read_to_string(case().join("origin.txt")).map_err(|why| why.to_string())?;
    if origin.lines().next() != Some(FORM) {
        return Err(format!("the case was served from {origin:?}"));
    }
    let html = std::fs::read_to_string(case().join("page.html")).map_err(|why| why.to_string())?;
    let url = alo_url::parse(FORM).map_err(|why| format!("{why:?}"))?;
    let network = Network::over(Pool::with_trust(Trust::of(&[])?));
    let (tell, news) = channel();
    let conductor = Conductor::start(
        Tabs::over(Renderers::running(renderer, &[]).with_machine(alo_window::fonts::frozen())),
        Fonts::AsStarted,
        network,
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
fn socket() -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-ask-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// `lines` sent through `alo-ask` started with `arguments` before the
/// socket, and everything it printed.
fn ask(arguments: &[&str], socket: &Path, lines: &[String]) -> Result<String, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .args(arguments)
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
    Ok(String::from_utf8_lossy(&finished.stdout).into_owned())
}

/// The same lines asked plainly and as they come, of one browser.
fn both_ways(lines: &[String]) -> Result<(String, String), String> {
    let (conductor, _news) = browsing()?;
    let path = socket()?;
    let listening =
        Listening::at(&path, conductor.orders()).map_err(|why| format!("not listening: {why}"))?;
    let plain = ask(&["--plain"], &path, lines);
    let raw = ask(&[], &path, lines);
    listening.stop();
    let _ = conductor.orders().send(Order::CloseEverything);
    Ok((plain?, raw?))
}

#[test]
fn a_person_reads_the_tree_by_role_and_name_with_no_position() {
    let lines = vec![
        r#"{"v":1,"ask":"tabs"}"#.to_owned(),
        format!(r#"{{"v":1,"ask":"read","tab":"{FORM}"}}"#),
        format!(
            r#"{{"v":1,"ask":"act","tab":"{FORM}","target":{{"name":"Customer name:"}},"verb":"put_text","text":"Ada"}}"#
        ),
        format!(
            r#"{{"v":1,"ask":"act","tab":"{FORM}","target":{{"name":"Nothing here"}},"verb":"activate"}}"#
        ),
    ];
    let (plain, raw) = both_ways(&lines).unwrap_or_else(|why| panic!("{why}"));
    let printed: Vec<&str> = plain.lines().collect();

    // The tabs: the address, alone on its line.
    assert_eq!(printed.first(), Some(&FORM), "{plain}");
    // The tree: the tab, then one node a line, by number, role and name.
    assert_eq!(
        printed.get(1),
        Some(&format!("tab {FORM}").as_str()),
        "{plain}"
    );
    let field = printed
        .iter()
        .find(|line| line.contains("textbox \"Customer name:\""))
        .unwrap_or_else(|| panic!("no field by its name:\n{plain}"));
    assert!(field.trim_start().starts_with('['), "{field}");
    let button = printed
        .iter()
        .find(|line| line.contains("button \"Submit order\""))
        .unwrap_or_else(|| panic!("no button by its name:\n{plain}"));
    // Indented under what they are in: deeper than the tree's first node.
    let indent = |line: &str| line.len() - line.trim_start().len();
    let root = printed.get(2).copied().unwrap_or_default();
    assert_eq!(indent(root), 0, "{plain}");
    assert!(indent(field) > 0 && indent(button) > 0, "{plain}");
    // The verb as the browser said it, and the refusal by its kind.
    assert!(
        printed
            .iter()
            .any(|line| line.starts_with("put \"Ada\" into box#")),
        "{plain}"
    );
    assert_eq!(
        printed.last(),
        Some(&"refused (NotFound): nothing on the page is \"Nothing here\""),
        "{plain}"
    );
    // No position: the raw answer carries rectangles, and not one of their
    // fields is printed plainly.
    assert!(raw.contains("\"rect\":{\"height\""), "{raw}");
    for field in ["rect", "height", "width", "\"x\"", "\"y\""] {
        assert!(!plain.contains(field), "{field} printed:\n{plain}");
    }
    // As they come, every answer is still one JSON line.
    assert_eq!(raw.lines().count(), lines.len(), "{raw}");
    assert!(raw.lines().all(|line| line.starts_with('{')), "{raw}");
}
