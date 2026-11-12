/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 964, ADR 0112 requirement 7 (*read page text*): the socket's
//! `page_text`, asked by `alo-ask` — a process of its own — of a tab in the
//! machine's Chrome and in alo's CEF host, each the same test, kept to the
//! sites a person named.
//!
//! Asserted from what `alo-ask` printed:
//!
//! 1. **The page's words, in reading order, a line each** for a heading, a
//!    paragraph — a link and its punctuation inside it on its line — each
//!    list item with its marker, and each of two `<div>`s; a field as its
//!    label and what it holds. **Nothing hidden**: not `display: none`,
//!    `visibility: hidden` or `aria-hidden`, not the page's title, not a
//!    password. **No position**: no `rect` in the line. Not cut.
//! 2. **A page longer than the bound** is answered to it, cut between
//!    lines, `cut` true and `more` saying how many characters were not
//!    answered, which with those answered are the page's whole text.
//! 3. `alo-ask --plain` writes the words as they are, a line of the page
//!    to a line, and says where a page's words were cut.
//! 4. An address on a site not named is refused by its site, and the read
//!    is in the record.
//!
//! Never the real network: every address is `http://127.0.0.1:PORT/…` or
//! `http://localhost:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_scope::AgentScope;
use alo_window::agent_sites::AgentSites;
use alo_window::agent_text::MOST_CHARACTERS;
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

/// The page read, as served.
const WORDS: &str = "<!doctype html><title>The title</title>\
    <h1>Invoices</h1>\
    <p>Three are <a href=\"/long.html\">unpaid</a>, and one is late.</p>\
    <ul><li>Invoice 3</li><li>Invoice 4</li></ul>\
    <div>Loose words</div><div>More loose words</div>\
    <p style=\"display:none\">Never shown</p>\
    <p style=\"visibility:hidden\">Not visible</p>\
    <p aria-hidden=\"true\">Hidden from readers</p>\
    <p><label>Customer name: <input value=\"Ada\"></label></p>\
    <p><label>Password <input type=\"password\" value=\"hunter2\"></label></p>";

/// What it reads as: the title, the hidden and the password never.
const READ: &str = "Invoices\\nThree are unpaid, and one is late.\\n• Invoice 3\\n• Invoice 4\\n\
    Loose words\\nMore loose words\\nCustomer name: Ada\\nPassword";

/// One paragraph of the long page: a thousand characters.
fn paragraph(at: usize) -> String {
    format!("{at:05} {}", "word ".repeat(199))
        .trim_end()
        .to_owned()
}

/// How many paragraphs the long page holds: past the bound.
const PARAGRAPHS: usize = MOST_CHARACTERS / 1_000 + 20;

/// A server on this machine answering `/words.html` and `/long.html`: its
/// port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let long: String = core::iter::once("<!doctype html><title>Long</title>".to_owned())
        .chain((0..PARAGRAPHS).map(|at| format!("<p>{}</p>", paragraph(at))))
        .collect();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let long = long.clone();
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
                    "/words.html" => ("200 OK", WORDS),
                    "/long.html" => ("200 OK", long.as_str()),
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
        let root = std::env::temp_dir().join(format!("alo-964-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
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
/// printed, a line at a time.
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

/// The whole number after `"field":` in `line`.
fn number(line: &str, field: &str) -> Option<usize> {
    let (_, after) = line.split_once(&format!("\"{field}\":"))?;
    after
        .split(|each: char| !each.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn through(chromium: Chromium, who: &str) -> Result<(), String> {
    let port = serve()?;
    let words = format!("http://127.0.0.1:{port}/words.html");
    let long = format!("http://127.0.0.1:{port}/long.html");
    let elsewhere = format!("http://localhost:{port}/words.html");
    let scratch = Scratch::new(if who == "Chrome" { "chrome" } else { "host" })?;
    let sites = AgentSites::named([words.as_str()]).map_err(|why| why.to_string())?;
    let address = alo_url::parse(&words).map_err(|why| format!("{why:?}"))?;
    let serving = ChromeServing::start_from(
        chromium,
        Opening::Network(address),
        SIZE,
        AgentScope::from(sites),
    )
    .map_err(|why| format!("{who}: {why}"))?;
    // Under the temporary directory as the system spells it — shorter than
    // with its links followed, for a socket's path is bounded.
    let place = std::env::temp_dir().join(scratch.0.file_name().unwrap_or_default());
    let socket = alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&place))
        .map_err(|why| format!("no place for the socket: {why}"))?;
    let listening =
        Listening::at(&socket, serving.orders()).map_err(|why| format!("not listening: {why}"))?;

    let answers = ask(
        &socket,
        false,
        &[
            format!(r#"{{"v":1,"id":1,"ask":"page_text","tab":"{words}"}}"#),
            format!(r#"{{"v":1,"id":2,"ask":"page_text","tab":"{elsewhere}"}}"#),
            format!(r#"{{"v":1,"id":3,"ask":"open","tab":"{words}","address":"{long}"}}"#),
            format!(r#"{{"v":1,"id":4,"ask":"page_text","tab":"{long}"}}"#),
        ],
    )?;
    read_in_order(&answers, &words, who)?;
    cut_at_the_bound(&answers, who)?;
    written_plainly(&socket, &long, who)?;
    listening.stop();
    recorded(serving, who)
}

/// One, and four's refusal: the page's words in reading order, nothing
/// hidden, no position; another site refused by its site.
fn read_in_order(answers: &[String], words: &str, who: &str) -> Result<(), String> {
    assert_eq!(
        answer(answers, 0)?,
        format!(
            "{{\"answer\":\"page_text\",\"cut\":false,\"id\":1,\"more\":0,\
             \"said\":\"read the page's words, 8 lines\",\"tab\":\"{words}\",\
             \"text\":\"{READ}\",\"v\":1}}"
        ),
        "{who}"
    );
    assert!(
        answer(answers, 1)?
            .starts_with(r#"{"answer":"refused","id":2,"refusal":{"kind":"NotGranted","#),
        "{who}: {answers:?}"
    );
    Ok(())
}

/// Two: a page longer than the bound, cut between its paragraphs and said
/// to have been, with how many characters more.
fn cut_at_the_bound(answers: &[String], who: &str) -> Result<(), String> {
    assert!(
        answer(answers, 2)?.starts_with(r#"{"answer":"opened","id":3,"#),
        "{who}: {answers:?}"
    );
    let cut = answer(answers, 3)?;
    assert!(
        cut.starts_with(r#"{"answer":"page_text","cut":true,"id":4,"more":"#),
        "{who}: {}",
        cut.get(..200).unwrap_or(cut)
    );
    assert!(!cut.contains("rect"), "{who}: a position crossed");
    let more = number(cut, "more").ok_or_else(|| format!("{who}: no more"))?;
    let (_, text) = cut
        .split_once("\"text\":\"")
        .ok_or_else(|| format!("{who}: no text"))?;
    let text = text.strip_suffix("\",\"v\":1}").unwrap_or(text);
    let answered: Vec<&str> = text.split("\\n").collect();
    assert!(
        answered
            .iter()
            .enumerate()
            .all(|(at, line)| *line == paragraph(at)),
        "{who}: a paragraph cut in two, or out of order"
    );
    // Each line break is written `\n` on the wire, two characters for one.
    let characters = text.chars().count() - (answered.len() - 1);
    assert!(characters <= MOST_CHARACTERS, "{who}: {characters}");
    // Those answered, with the line breaks between them, and those not:
    // the page's whole text.
    let whole = PARAGRAPHS * 1_000 + (PARAGRAPHS - 1);
    assert_eq!(characters + more, whole, "{who}");
    Ok(())
}

/// Three: written plainly, the words as they are, and where they were cut.
fn written_plainly(socket: &Path, long: &str, who: &str) -> Result<(), String> {
    let plain = ask(
        socket,
        true,
        &[format!(
            r#"{{"v":1,"id":1,"ask":"page_text","tab":"{long}"}}"#
        )],
    )?;
    assert_eq!(
        plain.first().map(String::as_str),
        Some(paragraph(0).as_str()),
        "{who}"
    );
    assert!(
        plain.last().is_some_and(|line| {
            line.starts_with("(cut here: ") && line.ends_with(" more characters were not answered)")
        }),
        "{who}: {:?}",
        plain.last()
    );
    Ok(())
}

/// The words read, in the record, once Chrome is closed.
fn recorded(serving: ChromeServing, who: &str) -> Result<(), String> {
    let _ = serving.orders().send(Order::CloseEverything);
    let records = serving
        .records()
        .ok_or_else(|| format!("{who}: Chrome's thread did not finish"))?;
    assert!(
        records
            .entries()
            .any(|entry| entry.to_string().contains("read the words")),
        "{who}: the words read are in the record"
    );
    Ok(())
}

#[test]
fn a_pages_text_read_by_an_agent_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_pages_text_read_by_an_agent_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("964")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
