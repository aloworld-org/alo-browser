/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 976, ADR 0114 (ADR 0112, *Answered* 4): a second `alo`
//! started on the **same** profile with an address hands it to the alo
//! already running, which opens it in a new chat and comes forward, and the
//! second ends; one started with a **different** `--data DIR` is an
//! instance of its own.
//!
//! Every `alo` is the built program, run as a person runs it, its window
//! shown, each card's page in the machine's Chrome. Asserted, from what
//! each said on its terminal, how each ended, and `alo-ask`'s answers:
//!
//! 1. The second `alo --conversation --data ONE ADDRESS` ends by itself,
//!    successfully, having opened no socket of its own, and says the first
//!    answered that the page opened.
//! 2. The first says it was started again with the address, opens it in a
//!    new chat and comes forward; its one tab is the address.
//! 3. A third, `--data TWO` with another address, keeps running as an
//!    instance of its own: at its own socket, its one tab is its address,
//!    and the first's is still the first's.
//!
//! Neither touches the person's keychain, chats or sign-ins (`--data`
//! keeps no key, ADR 0112 § 5). Never the real network: every address is
//! `http://127.0.0.1:PORT/…`. That the window came forward is asserted in
//! `conversation_window/outside.rs`, where the front is asked of the
//! event loop; which program macOS put in front is not read here.

#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: an alo starting, a card's Chrome
/// starting and drawing its page.
const AT_MOST: Duration = Duration::from_secs(120);

/// What an alo says as its socket is placed.
const AT: &str = "alo: an agent reaches this browser at ";

/// A server on this machine answering `/one.html` and `/two.html`: its
/// port.
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
                        "<!doctype html><title>One</title><h1>Page one</h1>",
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

/// An `alo` started, closed when dropped.
struct Alo {
    child: Child,
    said: Receiver<String>,
    heard: Vec<String>,
}

impl Drop for Alo {
    fn drop(&mut self) {
        // Its cards' Chromes end as their pipes close with it.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Alo {
    /// `alo --conversation --data DATA` and `more`.
    fn start(data: &Path, more: &[&str]) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alo"))
            .args(["--conversation", "--data"])
            .arg(data)
            .args(more)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|why| format!("alo did not start: {why}"))?;
        let stderr = child.stderr.take().ok_or("alo has no error output")?;
        let (tell, said) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    return;
                };
                if tell.send(line).is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            child,
            said,
            heard: Vec::new(),
        })
    }

    /// Until it has said a line `wanted` holds of, within [`AT_MOST`]: that
    /// line.
    fn until_said(&mut self, what: &str, wanted: impl Fn(&str) -> bool) -> Result<String, String> {
        if let Some(line) = self.heard.iter().find(|line| wanted(line)) {
            return Ok(line.clone());
        }
        let by = Instant::now() + AT_MOST;
        while let Some(left) = by.checked_duration_since(Instant::now()) {
            let Ok(line) = self.said.recv_timeout(left) else {
                break;
            };
            self.heard.push(line.clone());
            if wanted(&line) {
                return Ok(line);
            }
        }
        Err(format!("alo never said {what}: {:?}", self.heard))
    }

    /// The path it said an agent reaches it at.
    fn socket(&mut self) -> Result<PathBuf, String> {
        let line = self.until_said("where its socket is", |line| line.starts_with(AT))?;
        Ok(PathBuf::from(line.trim_start_matches(AT)))
    }

    /// How it ended, once it has — within [`AT_MOST`] — and everything it
    /// said.
    fn ended(&mut self) -> Result<(std::process::ExitStatus, Vec<String>), String> {
        let by = Instant::now() + AT_MOST;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    // What it said to the last line, its output closed.
                    while let Ok(line) = self.said.recv_timeout(Duration::from_secs(5)) {
                        self.heard.push(line);
                    }
                    return Ok((status, self.heard.clone()));
                }
                Ok(None) if Instant::now() < by => {
                    while let Ok(line) = self.said.recv_timeout(Duration::from_millis(100)) {
                        self.heard.push(line);
                    }
                }
                Ok(None) => return Err(format!("alo did not end: {:?}", self.heard)),
                Err(why) => return Err(format!("alo could not be waited for: {why}")),
            }
        }
    }
}

/// What `alo-ask` printed for the tabs, asked at `socket`.
fn tabs_at(socket: &Path) -> Result<String, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(br#"{"v":1,"id":1,"ask":"tabs"}"#)
        .map_err(|why| why.to_string())?;
    drop(stdin);
    let finished = asking.wait_with_output().map_err(|why| why.to_string())?;
    Ok(String::from_utf8_lossy(&finished.stdout).trim().to_owned())
}

/// The tabs answer naming `tabs`.
fn tabs(tabs: &[&str]) -> String {
    let named: Vec<String> = tabs.iter().map(|tab| format!("\"{tab}\"")).collect();
    format!(
        r#"{{"answer":"tabs","id":1,"tabs":[{}],"v":1}}"#,
        named.join(",")
    )
}

/// Until the alo at `socket` answers `wanted` for its tabs, within
/// [`AT_MOST`].
fn until_tabs(socket: &Path, wanted: &str) -> Result<(), String> {
    let by = Instant::now() + AT_MOST;
    let mut last = String::new();
    while Instant::now() < by {
        last = tabs_at(socket)?;
        if last == wanted {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "{} answered {last}, not {wanted}",
        socket.display()
    ))
}

#[test]
fn a_second_alo_hands_its_address_to_the_first_through_chrome() -> Result<(), String> {
    let port = serve()?;
    let one_page = format!("http://127.0.0.1:{port}/one.html");
    let two_page = format!("http://127.0.0.1:{port}/two.html");
    let root = std::env::temp_dir().join(format!("alo-976-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (one, two) = (root.join("one"), root.join("two"));

    let mut first = Alo::start(&one, &["--frozen-fonts"])?;
    let at_one = first.socket()?;
    assert_eq!(at_one, one.join("alo-browser").join("agent.sock"));
    assert_eq!(tabs_at(&at_one)?, tabs(&[]), "nothing open yet");

    // 1. The same profile: handed over, and the second ends by itself.
    let mut second = Alo::start(&one, &[one_page.as_str()])?;
    let (ended, said) = second.ended()?;
    assert!(ended.success(), "{ended}: {said:?}");
    assert!(
        said.iter().any(
            |line| line.starts_with("alo: an alo is already running on this profile")
                && line.contains(&format!("{one_page} was handed to it"))
        ),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|line| line == &format!("alo: it answered: opened {one_page}")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|line| line.starts_with(AT)),
        "the second opened no socket of its own: {said:?}"
    );

    // 2. The first opened it in a new chat, and came forward.
    let started_again = format!(
        "alo: alo was started again with {one_page}: it opens in a new chat, and alo comes forward"
    );
    first.until_said("it was started again", |line| line == started_again)?;
    assert_eq!(tabs_at(&at_one)?, tabs(&[&one_page]));

    // 3. Another profile: an instance of its own, opening its own address.
    let mut third = Alo::start(&two, &["--frozen-fonts", two_page.as_str()])?;
    let at_two = third.socket()?;
    assert_eq!(at_two, two.join("alo-browser").join("agent.sock"));
    until_tabs(&at_two, &tabs(&[&two_page]))?;
    assert!(
        matches!(third.child.try_wait(), Ok(None)),
        "the third keeps running: {:?}",
        third.heard
    );
    assert!(
        !third
            .heard
            .iter()
            .any(|line| line.starts_with("alo: an alo is already running")),
        "{:?}",
        third.heard
    );
    assert_eq!(tabs_at(&at_one)?, tabs(&[&one_page]), "the first's own");

    drop(third);
    drop(first);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
