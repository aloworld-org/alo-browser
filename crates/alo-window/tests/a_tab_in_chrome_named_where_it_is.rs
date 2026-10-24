/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab in Chrome, reached down the agent socket, is named by where its page
//! is now: queue item 630, with item 497's rule and ADR 0055 § 5.
//!
//! The network is a server of this test's own on this machine, reached by
//! two names that are two sites — `127.0.0.1` and `localhost`. Its first page
//! holds a link to a page on the other. An agent follows the link through
//! the socket, and the tab is named by the second page's address from then,
//! never again by the first's, and judged by its site: let reach it when
//! every site is, refused for it when only the first site is.
//!
//! A third page sends itself to the second by script, a moment after it has
//! loaded, with nothing asked of the tab while it goes; the tab is named by
//! where it went all the same, because Chrome's word on it is taken in
//! before a tab is looked for.
//!
//! A fourth page moves within itself: its link goes to a part of it, and its
//! button moves it to another address of its own by `history.pushState`.
//! Neither commits a document, and the tab is named where it moved all the
//! same — with the part, for the link (queue item 640). The link is answered
//! `Followed`, as this engine answers it; what the engine names its tab
//! after the same link is `a_tab_in_the_engine_after_a_link_to_a_part.rs`.

use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::chrome_serving::{ChromeServing, Opening};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The size the page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long the tab is waited for to show the second page.
const AT_MOST: Duration = Duration::from_secs(20);

/// A server on this machine, answering `/a.html` with a page whose link goes
/// to `/b.html` on the other name, `/b.html` with a page of its own, and
/// `/itself.html` with a page that goes to `/b.html` on its own, and
/// `/part.html` with a page whose link goes to a part of itself and whose
/// button moves it to another address of its own by script: its port.
fn serve() -> Option<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                if socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .is_err()
                {
                    return;
                }
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let (status, body) = match asked.split(' ').nth(1) {
                    Some("/a.html") => (
                        "200 OK",
                        format!(
                            "<!doctype html><title>First</title><h1>First</h1>\
                             <a href=\"http://localhost:{port}/b.html\">Elsewhere</a>"
                        ),
                    ),
                    Some("/b.html") => (
                        "200 OK",
                        "<!doctype html><title>Second</title><h1>Second</h1>".to_owned(),
                    ),
                    Some("/part.html") => (
                        "200 OK",
                        "<!doctype html><title>Parts</title><h1>Parts</h1>\
                         <a href=\"#second\">To the second part</a>\
                         <button onclick=\"history.pushState(null, '', 'moved.html')\">Move</button>\
                         <div style=\"height:3000px\"></div><h2 id=\"second\">Second part</h2>"
                            .to_owned(),
                    ),
                    Some("/itself.html") => (
                        "200 OK",
                        format!(
                            "<!doctype html><title>Going</title><h1>Going</h1><script>\
                             setTimeout(() => location = 'http://localhost:{port}/b.html', 300)\
                             </script>"
                        ),
                    ),
                    // A connection Chrome opened ahead of need asked nothing.
                    None | Some("") => return,
                    Some(_) => ("404 Not Found", String::new()),
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
    Some(port)
}

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-where-{}-{name}", std::process::id()));
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

/// What the tabs listed are, as the answer to a `tabs` request says.
fn tabs_answer(tabs: &[String]) -> String {
    let tabs: Vec<String> = tabs.iter().map(|tab| format!("{tab:?}")).collect();
    format!(
        r#"{{"answer":"tabs","id":1,"tabs":[{}],"v":1}}"#,
        tabs.join(",")
    )
}

/// A tab in Chrome at the first page, kept to `sites`, reached through a
/// socket of its own; the link on it followed through the socket; and the
/// tabs asked for until they are `then`. What was answered to `after`,
/// asked last — `PORT`, in `then` and `after` and in what is answered, is
/// the server's port.
fn followed(
    name: &str,
    sites: &[&str],
    then: &[&str],
    after: &[String],
) -> Result<Vec<String>, String> {
    let port = serve().ok_or("no server")?;
    let first = format!("http://127.0.0.1:{port}/a.html");
    let address = alo_url::parse(&first).map_err(|why| format!("{why:?}"))?;
    let sites = AgentSites::named(sites.iter().copied()).map_err(|why| format!("{why:?}"))?;
    let serving = ChromeServing::start(Opening::Network(address), SIZE, sites)
        .map_err(|why| format!("no tab in Chrome: {why}"))?;
    let path = socket(name)?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let port_named = |line: &str| line.replace("PORT", &port.to_string());
    let then: Vec<String> = then.iter().map(|tab| port_named(tab)).collect();
    let result = (|| {
        let tabs = ask(&path, &[r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned()])?;
        if tabs != [tabs_answer(std::slice::from_ref(&first))] {
            return Err(format!("the first page is not listed: {tabs:?}"));
        }
        let follow = format!(
            r#"{{"v":1,"id":2,"ask":"act","tab":"{first}","target":{{"role":"link","name":"Elsewhere"}},"verb":"activate"}}"#
        );
        let followed = ask(&path, &[follow])?;
        let to = format!(r#""to":"http://localhost:{port}/b.html""#);
        if !followed
            .first()
            .is_some_and(|answer| answer.contains(r#""kind":"Followed""#) && answer.contains(&to))
        {
            return Err(format!("the link was not followed: {followed:?}"));
        }
        let until = Instant::now() + AT_MOST;
        loop {
            let tabs = ask(&path, &[r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned()])?;
            if tabs == [tabs_answer(&then)] {
                break;
            }
            if Instant::now() > until {
                return Err(format!("the tabs are still {tabs:?}"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let after: Vec<String> = after.iter().map(|line| port_named(line)).collect();
        ask(&path, &after)
    })();
    listening.stop();
    let _ = serving.records();
    Ok(result?
        .into_iter()
        .map(|answer| answer.replace(&port.to_string(), "PORT"))
        .collect())
}

/// A `read` of `tab`, where `PORT` is the server's port.
fn read(id: u32, tab: &str) -> String {
    format!(r#"{{"v":1,"id":{id},"ask":"read","tab":"{tab}"}}"#)
}

#[test]
fn a_tab_whose_page_went_elsewhere_is_named_by_where_it_is_now() {
    // Every site may be reached: the tab is listed at the second page, read
    // there, and not found at the first.
    let answers = followed(
        "every",
        &[],
        &["http://localhost:PORT/b.html"],
        &[
            read(3, "http://localhost:PORT/b.html"),
            read(4, "http://127.0.0.1:PORT/a.html"),
        ],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(
        answers.first().is_some_and(|tree| tree
            .starts_with(r#"{"answer":"tree","id":3,"tab":"http://localhost:PORT/b.html","#)
            && tree.contains(r#""name":"Second""#)),
        "{answers:#?}"
    );
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(
            r#"{"answer":"refused","id":4,"refusal":{"kind":"NoSuchTab","tab":"http://127.0.0.1:PORT/a.html"},"said":"no tab is open at http://127.0.0.1:PORT/a.html","v":1}"#
        )
    );
}

#[test]
fn a_tab_whose_page_went_to_a_site_not_named_is_refused_there() {
    let answers = followed(
        "kept",
        &["http://127.0.0.1"],
        // Listed nowhere: the one tab is on a site agents are kept from.
        &[],
        &[
            read(3, "http://localhost:PORT/b.html"),
            read(4, "http://127.0.0.1:PORT/a.html"),
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
    assert_eq!(kinds, ["NotGranted", "NoSuchTab"], "{answers:#?}");
    assert!(
        answers
            .first()
            .is_some_and(|answer| answer.contains(r#""site":"http://localhost""#)),
        "{answers:#?}"
    );
}

#[test]
fn a_tab_whose_page_went_elsewhere_on_its_own_is_named_by_where_it_went() {
    let port = serve().unwrap_or_else(|| panic!("no server"));
    let first = format!("http://127.0.0.1:{port}/itself.html");
    let second = format!("http://localhost:{port}/b.html");
    let address = alo_url::parse(&first).unwrap_or_else(|why| panic!("{why:?}"));
    let serving = ChromeServing::start(Opening::Network(address), SIZE, AgentSites::Every)
        .unwrap_or_else(|why| panic!("no tab in Chrome: {why}"));
    let path = socket("itself").unwrap_or_else(|why| panic!("{why}"));
    let listening =
        Listening::at(&path, serving.orders()).unwrap_or_else(|why| panic!("not listening: {why}"));
    let listed = |tabs: &[String]| tabs == [tabs_answer(std::slice::from_ref(&second))];
    let until = Instant::now() + AT_MOST;
    let mut tabs = Vec::new();
    while Instant::now() < until {
        tabs = ask(&path, &[r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned()])
            .unwrap_or_else(|why| panic!("{why}"));
        if listed(&tabs) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let read = ask(&path, &[read(2, &second), read(3, &first)]);
    listening.stop();
    let _ = serving.records();
    assert!(listed(&tabs), "named where it went: {tabs:?}");
    let read = read.unwrap_or_else(|why| panic!("{why}"));
    assert!(
        read.first()
            .is_some_and(|tree| tree.contains(r#""name":"Second""#)),
        "{read:#?}"
    );
    assert!(
        read.get(1)
            .is_some_and(|answer| answer.contains(r#""kind":"NoSuchTab""#)),
        "{read:#?}"
    );
}

/// A tab in Chrome at `/part.html`, every site reachable, reached through a
/// socket of its own; `asked`, each line sent and answered before the next;
/// and what was answered — `PORT`, in `asked` and in what is answered, is
/// the server's port.
fn in_parts(name: &str, asked: &[&str]) -> Result<Vec<String>, String> {
    let port = serve().ok_or("no server")?;
    let first = format!("http://127.0.0.1:{port}/part.html");
    let address = alo_url::parse(&first).map_err(|why| format!("{why:?}"))?;
    let serving = ChromeServing::start(Opening::Network(address), SIZE, AgentSites::Every)
        .map_err(|why| format!("no tab in Chrome: {why}"))?;
    let path = socket(name)?;
    let listening =
        Listening::at(&path, serving.orders()).map_err(|why| format!("not listening: {why}"))?;
    let mut answers = Vec::new();
    let mut result = Ok(());
    for line in asked {
        match ask(&path, &[line.replace("PORT", &port.to_string())]) {
            Ok(mut answered) => answers.append(&mut answered),
            Err(why) => {
                result = Err(why);
                break;
            }
        }
    }
    listening.stop();
    let _ = serving.records();
    result?;
    Ok(answers
        .into_iter()
        .map(|answer| answer.replace(&port.to_string(), "PORT"))
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
fn a_tab_whose_page_went_to_a_part_of_itself_is_named_with_the_part() {
    let answers = in_parts(
        "part",
        &[
            r#"{"v":1,"id":1,"ask":"act","tab":"http://127.0.0.1:PORT/part.html","target":{"role":"link","name":"To the second part"},"verb":"activate"}"#,
            r#"{"v":1,"id":1,"ask":"tabs"}"#,
            r#"{"v":1,"id":3,"ask":"read","tab":"http://127.0.0.1:PORT/part.html#second"}"#,
            r#"{"v":1,"id":4,"ask":"read","tab":"http://127.0.0.1:PORT/part.html"}"#,
        ],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(
        answers.first().is_some_and(
            |answer| kind(answer) == "Followed" && answer.contains(r##""to":"#second""##)
        ),
        "{answers:#?}"
    );
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(tabs_answer(&["http://127.0.0.1:PORT/part.html#second".to_owned()]).as_str()),
        "{answers:#?}"
    );
    assert!(
        answers.get(2).is_some_and(|tree| tree.starts_with(
            r#"{"answer":"tree","id":3,"tab":"http://127.0.0.1:PORT/part.html#second","#
        ) && tree.contains(r#""name":"Second part""#)),
        "{answers:#?}"
    );
    assert_eq!(
        answers.get(3).map(|answer| kind(answer)),
        Some("NoSuchTab"),
        "{answers:#?}"
    );
}

#[test]
fn a_tab_whose_page_moved_itself_by_script_is_named_where_it_moved() {
    let answers = in_parts(
        "moved",
        &[
            r#"{"v":1,"id":1,"ask":"act","tab":"http://127.0.0.1:PORT/part.html","target":{"role":"button","name":"Move"},"verb":"activate"}"#,
            r#"{"v":1,"id":1,"ask":"tabs"}"#,
            r#"{"v":1,"id":3,"ask":"read","tab":"http://127.0.0.1:PORT/moved.html"}"#,
        ],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        answers.first().map(|answer| kind(answer)),
        Some("Activated"),
        "{answers:#?}"
    );
    assert_eq!(
        answers.get(1).map(String::as_str),
        Some(tabs_answer(&["http://127.0.0.1:PORT/moved.html".to_owned()]).as_str()),
        "{answers:#?}"
    );
    assert!(
        answers
            .get(2)
            .is_some_and(|tree| tree.contains(r#""name":"Parts""#)),
        "{answers:#?}"
    );
}
