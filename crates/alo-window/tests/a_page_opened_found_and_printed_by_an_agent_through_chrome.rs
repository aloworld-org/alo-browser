/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 956, ADR 0112 § 4: the socket's `find`, `print_pdf` and
//! `open` in a tab, asked by `alo-ask` — a process of its own — of a tab in
//! the machine's Chrome and in alo's CEF host, each the same test, kept to
//! the sites and the directory a person named.
//!
//! Asserted from what `alo-ask` printed:
//!
//! 1. **`find`** answers the link that holds the words by its number, role
//!    and name, and **no position** — not a `rect` in the line — and
//!    `alo-ask --plain` writes it as a tree's node is written.
//! 2. **`print_pdf`** writes a new file in the directory allowed that is a
//!    PDF, says how many bytes it holds, refuses a path outside every
//!    directory allowed before anything is written, and never writes over
//!    the file it made.
//! 3. **`open`** sends the tab to the second page, answered by where it
//!    arrived, after which the tab is named there and read there; an
//!    address on a site not named is refused by its site; with no tab
//!    named, and `new_tab`, are not offered outside the conversation.
//!
//! Never the real network: every address is `http://127.0.0.1:PORT/…` or
//! `http://localhost:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_saves::AgentSaves;
use alo_window::agent_scope::AgentScope;
use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::beside::Chromium;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::message::Order;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The size the page is laid out at.
const SIZE: (u32, u32) = (800, 600);

/// A server on this machine answering `/one.html`, whose link goes to
/// `/two.html`, and `/two.html`: its port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut read = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    read.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || read.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let read = String::from_utf8_lossy(&read);
                let path = read.split(' ').nth(1).unwrap_or_default();
                let (status, body) = match path {
                    "/one.html" => (
                        "200 OK",
                        "<!doctype html><title>One</title><h1>Page one</h1>\
                         <p>Some words.</p><a href=\"/two.html\">Next page</a>",
                    ),
                    "/two.html" => (
                        "200 OK",
                        "<!doctype html><title>Two</title><h1>Page two</h1>",
                    ),
                    _ => ("404 Not Found", ""),
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
    Ok(port)
}

/// A directory of this test's own, gone with the value.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!("alo-956-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("saved")).map_err(|why| why.to_string())?;
        let root = std::fs::canonicalize(&root).map_err(|why| why.to_string())?;
        Ok(Self(root))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Send `lines` through `alo-ask`, with `--plain` when `plain`, and what it
/// printed, a line per answer.
fn ask(socket: &Path, plain: bool, lines: &[String]) -> Result<Vec<String>, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_alo-ask"));
    if plain {
        command.arg("--plain");
    }
    let mut asking = command
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

/// The answer at `at`, or why there is none.
fn answer(answers: &[String], at: usize) -> Result<&str, String> {
    answers
        .get(at)
        .map(String::as_str)
        .ok_or_else(|| format!("no answer {at} in {answers:?}"))
}

fn through(chromium: Chromium, who: &str) -> Result<(), String> {
    let port = serve()?;
    let one = format!("http://127.0.0.1:{port}/one.html");
    let two = format!("http://127.0.0.1:{port}/two.html");
    let elsewhere = format!("http://localhost:{port}/one.html");
    let scratch = Scratch::new(if who == "Chrome" { "chrome" } else { "host" })?;
    let kept_in = scratch.0.join("saved");
    let sites = AgentSites::named([one.as_str()]).map_err(|why| why.to_string())?;
    let saves = AgentSaves::named([kept_in.clone()]).map_err(|why| why.to_string())?;
    let address = alo_url::parse(&one).map_err(|why| format!("{why:?}"))?;
    let serving = ChromeServing::start_from(
        chromium,
        Opening::Network(address),
        SIZE,
        AgentScope::new(sites, saves),
    )
    .map_err(|why| format!("{who}: {why}"))?;
    // Under the temporary directory as the system spells it — shorter than
    // with its links followed, for a socket's path is bounded.
    let place = std::env::temp_dir().join(scratch.0.file_name().unwrap_or_default());
    let socket = alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&place))
        .map_err(|why| format!("no place for the socket: {why}"))?;
    let listening =
        Listening::at(&socket, serving.orders()).map_err(|why| format!("not listening: {why}"))?;

    let pdf = kept_in.join("one.pdf");
    let outside = scratch.0.join("outside.pdf");
    let lines = [
        format!(r#"{{"v":1,"id":1,"ask":"find","tab":"{one}","text":"next"}}"#),
        format!(
            r#"{{"v":1,"id":2,"ask":"print_pdf","tab":"{one}","path":"{}"}}"#,
            pdf.display()
        ),
        format!(
            r#"{{"v":1,"id":3,"ask":"print_pdf","tab":"{one}","path":"{}"}}"#,
            outside.display()
        ),
        format!(
            r#"{{"v":1,"id":4,"ask":"print_pdf","tab":"{one}","path":"{}"}}"#,
            pdf.display()
        ),
        format!(r#"{{"v":1,"id":5,"ask":"open","tab":"{one}","address":"{elsewhere}"}}"#),
        format!(r#"{{"v":1,"id":6,"ask":"open","tab":"{one}","address":"{two}"}}"#),
        r#"{"v":1,"id":7,"ask":"tabs"}"#.to_owned(),
        format!(r#"{{"v":1,"id":8,"ask":"read","tab":"{two}"}}"#),
        format!(r#"{{"v":1,"id":9,"ask":"open","address":"{two}"}}"#),
        r#"{"v":1,"id":10,"ask":"new_tab"}"#.to_owned(),
    ];
    let answers = ask(&socket, false, &lines)?;
    found_by_name(&answers, who)?;
    printed_where_allowed(&answers, (&pdf, &outside), who)?;
    opened_in_the_tab(&answers, &two, who)?;
    written_plainly(&socket, &two, who)?;
    listening.stop();
    recorded(serving, who)
}

/// One: found by name, and no position crosses.
fn found_by_name(answers: &[String], who: &str) -> Result<(), String> {
    // 1. Found by name, and no position crosses.
    let found = answer(answers, 0)?;
    assert!(
        found.starts_with(r#"{"answer":"found","found":[{"name":"Next page","node":"#),
        "{who}: {found}"
    );
    assert!(found.contains(r#""role":"link""#), "{who}: {found}");
    assert!(found.contains(r#""more":0"#), "{who}: {found}");
    assert!(
        !found.contains("rect"),
        "{who}: a position crossed: {found}"
    );
    Ok(())
}

/// Two: printed where allowed, as a PDF, at `pdf`; refused at `outside`;
/// never written over.
fn printed_where_allowed(
    answers: &[String],
    (pdf, outside): (&Path, &Path),
    who: &str,
) -> Result<(), String> {
    let printed = answer(answers, 1)?;
    assert!(
        printed.starts_with(r#"{"answer":"printed","bytes":"#),
        "{who}: {printed}"
    );
    let bytes = std::fs::read(pdf).map_err(|why| format!("{who}: no PDF: {why}"))?;
    assert!(bytes.starts_with(b"%PDF-"), "{who}: not a PDF");
    assert!(
        printed.contains(&format!(r#""bytes":{},"#, bytes.len())),
        "{who}: {printed} says another length than {}",
        bytes.len()
    );
    assert!(
        answer(answers, 2)?
            .starts_with(r#"{"answer":"refused","id":3,"refusal":{"kind":"NotAllowedThere","#),
        "{who}: {answers:?}"
    );
    assert!(!outside.exists(), "{who}: written outside");
    assert!(
        answer(answers, 3)?
            .starts_with(r#"{"answer":"refused","id":4,"refusal":{"kind":"Exists","#),
        "{who}: {answers:?}"
    );
    assert_eq!(
        std::fs::read(pdf).map_err(|why| why.to_string())?,
        bytes,
        "{who}: written over"
    );
    Ok(())
}

/// Three: opened in the tab, named at `two` where it arrived; another site
/// refused; a new tab not offered here.
fn opened_in_the_tab(answers: &[String], two: &str, who: &str) -> Result<(), String> {
    assert!(
        answer(answers, 4)?
            .starts_with(r#"{"answer":"refused","id":5,"refusal":{"kind":"NotGranted","#),
        "{who}: {answers:?}"
    );
    assert_eq!(
        answer(answers, 5)?,
        format!(r#"{{"answer":"opened","id":6,"said":"opened {two}","tab":"{two}","v":1}}"#),
        "{who}"
    );
    assert_eq!(
        answer(answers, 6)?,
        format!(r#"{{"answer":"tabs","id":7,"tabs":["{two}"],"v":1}}"#),
        "{who}"
    );
    let read = answer(answers, 7)?;
    assert!(
        read.starts_with(r#"{"answer":"tree","id":8,"#) && read.contains("Page two"),
        "{who}: {read}"
    );
    assert!(
        answer(answers, 8)?.starts_with(
            r#"{"answer":"refused","id":9,"refusal":{"ask":"open","kind":"NotOffered"}"#
        ),
        "{who}: {answers:?}"
    );
    assert!(
        answer(answers, 9)?.starts_with(
            r#"{"answer":"refused","id":10,"refusal":{"ask":"new_tab","kind":"NotOffered"}"#
        ),
        "{who}: {answers:?}"
    );
    Ok(())
}

/// Written plainly for a person: the node a line, as a tree's.
fn written_plainly(socket: &Path, two: &str, who: &str) -> Result<(), String> {
    let plain = ask(
        socket,
        true,
        &[format!(
            r#"{{"v":1,"id":1,"ask":"find","tab":"{two}","text":"page two"}}"#
        )],
    )?;
    assert_eq!(
        plain.first().map(String::as_str),
        Some("\"page two\" is in heading \"Page two\""),
        "{who}: {plain:?}"
    );
    assert!(
        plain
            .get(1)
            .is_some_and(|line| line.starts_with("  [") && line.ends_with("] heading \"Page two\"")),
        "{who}: {plain:?}"
    );
    Ok(())
}

/// The page saved and the page sent, in the record, once Chrome is closed.
fn recorded(serving: ChromeServing, who: &str) -> Result<(), String> {
    let _ = serving.orders().send(Order::CloseEverything);
    let records = serving
        .records()
        .ok_or_else(|| format!("{who}: Chrome's thread did not finish"))?;
    assert!(
        records
            .entries()
            .any(|entry| entry.to_string().contains("as a PDF of")),
        "{who}: the PDF saved is in the record"
    );
    assert!(
        records
            .entries()
            .any(|entry| entry.to_string().contains("sent the page to")),
        "{who}: the page sent is in the record"
    );
    Ok(())
}

#[test]
fn a_page_opened_found_and_printed_by_an_agent_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_page_opened_found_and_printed_by_an_agent_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("956")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
