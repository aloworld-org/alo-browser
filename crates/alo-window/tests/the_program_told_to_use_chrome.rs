/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo --chrome`: the program a person starts, told to have the machine's
//! Chrome draw the page (queue item 641, ADR 0058).
//!
//! Until this, the Chrome path was a library and its tests, and nothing a
//! person could run. Here the `alo` binary itself is started, as a process
//! of its own, on a page from a server of this test's own; it says on the
//! terminal which Chrome draws the page — by the version that Chrome gives
//! itself, read here from a Chrome of the test's own — where Chrome's own
//! window went on the screen and the page's size in it, in numbers (item
//! 627), and where an agent reaches it. `alo-ask`, a third process, reads the page's tabs and tree
//! and follows a link to a part of the page through the socket, and asks
//! which renderer draws it, as a person would (item 622); and the browser
//! closes Chrome and its socket when its input ends.
//!
//! The same program kept to another site lets an agent reach nothing of
//! the page, and named a page's file opens it at its `file:` address. What
//! cannot be given to Chrome is refused before it starts.

use alo_window::chrome::Chrome;
use alo_window::opening::{CHROME_AT, CHROME_SIZE};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for: to say where an agent reaches it,
/// and to close once its input has ended.
const AT_MOST: Duration = Duration::from_secs(60);

/// A server on this machine answering `/part.html` with a page whose link
/// goes to a part of itself: its port.
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
                    Some("/part.html") => ("200 OK", PAGE),
                    // A connection Chrome opened ahead of need asked nothing.
                    None | Some("") => return,
                    Some(_) => ("404 Not Found", ""),
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

/// The page served, and the one named as a file.
const PAGE: &str = "<!doctype html><title>Parts</title><h1>Parts</h1>\
                    <a href=\"#second\">To the second part</a>\
                    <div style=\"height:3000px\"></div><h2 id=\"second\">Second part</h2>";

/// A runtime directory of this test's own, empty, so the program's socket
/// is placed under it rather than under the person's.
fn runtime(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-told-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// `alo`, running, with its input held open and what it says on the
/// terminal arriving a line at a time.
struct Running {
    child: Child,
    input: Option<ChildStdin>,
    said: Receiver<String>,
}

impl Running {
    /// Start `alo` with `arguments`, its socket under `runtime`.
    fn start(arguments: &[&str], runtime: &Path) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alo"))
            .args(arguments)
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|why| format!("alo did not start: {why}"))?;
        let input = child.stdin.take();
        let terminal = child.stderr.take().ok_or("alo has no terminal")?;
        let (tell, said) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(terminal).lines() {
                let Ok(line) = line else {
                    return;
                };
                if tell.send(line).is_err() {
                    return;
                }
            }
        });
        Ok(Self { child, input, said })
    }

    /// Every line it says up to and including the one saying where an agent
    /// reaches it, and that place.
    fn until_reachable(&self) -> Result<(Vec<String>, PathBuf), String> {
        let until = Instant::now() + AT_MOST;
        let mut lines = Vec::new();
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let line = self
                .said
                .recv_timeout(left)
                .map_err(|_| format!("alo never said where an agent reaches it: {lines:?}"))?;
            if let Some(path) = line.strip_prefix("alo: an agent reaches this browser at ") {
                let path = PathBuf::from(path);
                lines.push(line);
                return Ok((lines, path));
            }
            lines.push(line);
        }
    }

    /// End its input, and how it exited once it has.
    fn close(mut self) -> Result<ExitStatus, String> {
        drop(self.input.take());
        let until = Instant::now() + AT_MOST;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err("alo did not close when its input ended".to_owned());
                }
                Err(why) => return Err(format!("alo could not be waited for: {why}")),
            }
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // A test that failed before closing it leaves no browser behind.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
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
            "alo-ask failed: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// The version the machine's Chrome gives itself, asked of a Chrome of this
/// test's own.
fn chromes_own_name() -> Result<String, String> {
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    chrome
        .version()
        .map(|version| version.product)
        .map_err(|why| why.to_string())
}

#[test]
fn the_program_opens_a_page_in_chrome_and_an_agent_reaches_it() -> Result<(), String> {
    let product = chromes_own_name()?;
    let port = serve().ok_or("no server")?;
    let address = format!("http://127.0.0.1:{port}/part.html");
    let runtime = runtime("net")?;
    let running = Running::start(&["--chrome", &address], &runtime)?;
    let (said, socket) = running.until_reachable()?;
    assert!(
        said.contains(&format!(
            "alo: pages are drawn by {product}, driven by this browser"
        )),
        "{said:?}"
    );
    assert!(product.contains("Chrome/"), "{product}");
    // Chrome's window, where this program put it and as tall as the page
    // and a title bar, the page laid out at the size it gives Chrome.
    let (left, top) = CHROME_AT;
    let (width, height) = CHROME_SIZE;
    let window =
        format!("alo: Chrome shows the page in a window of its own at {left},{top}, {width}×");
    let page = format!(" on the screen, the page laid out at {width}×{height} in it");
    let shown = said
        .iter()
        .find_map(|line| line.strip_prefix(&window)?.strip_suffix(&page))
        .ok_or_else(|| format!("no window was said: {said:?}"))?;
    let tall: u32 = shown
        .parse()
        .map_err(|_| format!("{shown} is not a height"))?;
    assert!(
        (height + 1..=height + 64).contains(&tall),
        "the page and a title bar: {tall}"
    );
    assert_eq!(
        socket,
        alo_window::socket_place::socket_under(&runtime),
        "the socket is under the runtime directory it was given"
    );

    let answers = ask(
        &socket,
        &[
            r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{address}"}}"#),
            format!(
                r#"{{"v":1,"id":3,"ask":"act","tab":"{address}","verb":"activate","target":{{"name":"To the second part"}}}}"#
            ),
            r#"{"v":1,"id":4,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":5,"ask":"read","tab":"{address}#second"}}"#),
            format!(r#"{{"v":1,"id":6,"ask":"renderer","tab":"{address}#second"}}"#),
        ],
    )?;
    let [tabs, tree, done, moved, again, renderer] = answers.as_slice() else {
        return Err(format!("six answers, not {answers:?}"));
    };
    assert_eq!(
        tabs,
        &format!(r#"{{"answer":"tabs","id":1,"tabs":["{address}"],"v":1}}"#)
    );
    assert!(tree.starts_with(r#"{"answer":"tree","id":2,"#), "{tree}");
    assert!(
        tree.contains(r#""name":"To the second part""#) && tree.contains(r#""role":"link""#),
        "{tree}"
    );
    // Laid out at the size this program gives Chrome, in numbers.
    assert!(
        tree.contains(&format!(
            r#""rect":{{"height":{height}.0,"width":{width}.0,"x":0.0,"y":0.0}},"role":"document""#
        )),
        "{tree}"
    );
    assert!(
        done.starts_with(r#"{"answer":"done","id":3,"#)
            && done.contains(r#""kind":"Followed""#)
            && done.contains(r##""to":"#second""##),
        "{done}"
    );
    assert_eq!(
        moved,
        &format!(r#"{{"answer":"tabs","id":4,"tabs":["{address}#second"],"v":1}}"#)
    );
    assert!(
        again.starts_with(r#"{"answer":"tree","id":5,"#) && again.contains("Second part"),
        "{again}"
    );
    // A person asks the running browser which renderer draws the page, and
    // is told Chrome by the version it gives itself (queue item 622).
    assert_eq!(
        renderer,
        &format!(
            r#"{{"answer":"renderer","id":6,"renderer":{{"kind":"Chrome","product":"{product}"}},"said":"{product}, driven by this browser","tab":"{address}#second","v":1}}"#
        )
    );

    let status = running.close()?;
    assert!(status.success(), "{status}");
    assert!(
        !socket.exists(),
        "the socket is gone with the browser: {}",
        socket.display()
    );
    let _ = std::fs::remove_dir_all(runtime);
    Ok(())
}

#[test]
fn the_program_kept_to_another_site_lets_an_agent_reach_nothing() -> Result<(), String> {
    let port = serve().ok_or("no server")?;
    let address = format!("http://127.0.0.1:{port}/part.html");
    let runtime = runtime("kept")?;
    let running = Running::start(
        &["--chrome", "--agent-site", "http://localhost", &address],
        &runtime,
    )?;
    let (_, socket) = running.until_reachable()?;
    let kept = running
        .said
        .recv_timeout(AT_MOST)
        .map_err(|_| "alo never said whom agents are kept to".to_owned())?;
    assert_eq!(
        kept,
        "alo: this browser lets agents reach only http://localhost: a refusal of its own, \
         which narrows what reaches it and grants nothing"
    );
    let answers = ask(
        &socket,
        &[
            r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{address}"}}"#),
            format!(r#"{{"v":1,"id":3,"ask":"renderer","tab":"{address}"}}"#),
        ],
    )?;
    let [tabs, refused @ ..] = answers.as_slice() else {
        return Err(format!("three answers, not {answers:?}"));
    };
    assert_eq!(tabs, r#"{"answer":"tabs","id":1,"tabs":[],"v":1}"#);
    assert_eq!(refused.len(), 2, "{answers:?}");
    for refused in refused {
        assert!(
            refused.contains(r#""kind":"NotGranted""#)
                && refused.contains(r#""site":"http://127.0.0.1""#),
            "{refused}"
        );
    }
    let status = running.close()?;
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(runtime);
    Ok(())
}

#[test]
fn a_file_named_is_opened_in_chrome_at_its_address() -> Result<(), String> {
    let runtime = runtime("file")?;
    let page = runtime.join("part.html");
    std::fs::write(&page, PAGE).map_err(|why| why.to_string())?;
    let named = page.to_str().ok_or("not a path a URL can name")?;
    let running = Running::start(&["--chrome", named], &runtime)?;
    let (_, socket) = running.until_reachable()?;
    let address = format!("file://{named}");
    let answers = ask(
        &socket,
        &[
            r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{address}"}}"#),
        ],
    )?;
    let [tabs, tree] = answers.as_slice() else {
        return Err(format!("two answers, not {answers:?}"));
    };
    assert_eq!(
        tabs,
        &format!(r#"{{"answer":"tabs","id":1,"tabs":["{address}"],"v":1}}"#)
    );
    assert!(tree.contains(r#""name":"Parts""#), "{tree}");
    let status = running.close()?;
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(runtime);
    Ok(())
}

#[test]
fn what_chrome_cannot_be_given_is_refused_before_it_starts() -> Result<(), String> {
    for arguments in [
        &["--chrome"][..],
        &["--chrome", "--scale", "2", "page.html"][..],
        &["--chrome", "/nowhere/at/all.html"][..],
        &["--chrome", "data:text/html,hi"][..],
    ] {
        let finished = Command::new(env!("CARGO_BIN_EXE_alo"))
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .map_err(|why| format!("alo did not start: {why}"))?;
        assert_eq!(finished.status.code(), Some(2), "{arguments:?}");
        let said = String::from_utf8_lossy(&finished.stderr);
        assert!(said.starts_with("alo: "), "{arguments:?}: {said}");
        assert!(
            !said.contains("pages are drawn by"),
            "{arguments:?}: {said}"
        );
    }
    Ok(())
}
