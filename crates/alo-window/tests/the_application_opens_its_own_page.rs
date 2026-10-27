/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo` inside the macOS application, started the way macOS starts it
//! (queue item 477; ADR 0067 § 1, ADR 0068).
//!
//! The real `alo` and `alo-render`, copied into `alo browser.app` shaped as
//! the application is, with the application's page from the corpus in its
//! `Contents/Resources`, are started with **no arguments and `/dev/null`
//! for input** — what macOS gives an application opened from the Finder,
//! measured. Before this, `alo` given that opened an empty tab, and `alo
//! --chrome` closed as it opened.
//!
//! Here it opens the application's page in the machine's Chrome, says so
//! on the terminal with the way it is closed, and is still running after
//! its input has ended. An agent, through `alo-ask`, finds the page at its
//! `file:` address inside the application and is told Chrome draws it.
//! Then the application is **quit** as the Dock and ⌘Q quit it, by the
//! Apple Event `osascript` sends it, which ends the process inside `AppKit`
//! without returning from its event loop (ADR 0067 § 1). Nothing it started
//! is left running, and nothing it made is left on the disk: not Chrome's
//! profile, which Quit used to leave (queue item 688), and not the socket
//! an agent reached it at.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for to say where an agent reaches it,
/// and what it started for to end.
const AT_MOST: Duration = Duration::from_secs(60);

/// How long after its input ended the program is asked to be running still:
/// `alo --chrome` started this way had ended two seconds later (ADR 0067
/// § 1).
const STILL_AFTER: Duration = Duration::from_secs(4);

/// The application, made under a directory of this test's own: the
/// directory, and the application's `Contents`.
fn application() -> Result<(PathBuf, PathBuf), String> {
    let root = std::env::temp_dir().join(format!("alo-application-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let contents = root.join("alo browser.app").join("Contents");
    let programs = contents.join("MacOS");
    let resources = contents.join("Resources");
    for directory in [&programs, &resources] {
        std::fs::create_dir_all(directory).map_err(|why| why.to_string())?;
    }
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let renderer = alo_window::beside::renderer_beside(alo);
    for program in [alo, renderer.as_path()] {
        let name = program.file_name().ok_or("a program with no name")?;
        std::fs::copy(program, programs.join(name))
            .map_err(|why| format!("{} could not be copied: {why}", program.display()))?;
    }
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases");
    for (case, name) in [
        ("the-applications-page", alo_window::bundle::PAGE),
        (
            "the-applications-page-without-chrome",
            alo_window::bundle::WITHOUT_CHROME,
        ),
    ] {
        std::fs::copy(cases.join(case).join("page.html"), resources.join(name))
            .map_err(|why| format!("{case} could not be copied: {why}"))?;
    }
    Ok((root, contents))
}

/// How long Quit is given to end the program: Chrome is asked to close and
/// waited for first, which takes about a second (ADR 0068 § 2).
const QUIT_WITHIN: Duration = Duration::from_secs(30);

/// Quit the application at `application`, as the Dock and ⌘Q do.
fn quit(application: &Path) -> Result<(), String> {
    let path = application
        .to_str()
        .ok_or("not a path AppleScript can name")?;
    let asked = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!("tell application {path:?} to quit"))
        .output()
        .map_err(|why| format!("osascript did not run: {why}"))?;
    if asked.status.success() {
        Ok(())
    } else {
        Err(format!(
            "the application was not quit: {}",
            String::from_utf8_lossy(&asked.stderr)
        ))
    }
}

/// What a process says on its terminal, a line at a time.
fn lines_of(child: &mut Child) -> Result<Receiver<String>, String> {
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
    Ok(said)
}

/// Every line said up to the one saying where an agent reaches it, and that
/// place.
fn until_reachable(said: &Receiver<String>) -> Result<(Vec<String>, PathBuf), String> {
    let until = Instant::now() + AT_MOST;
    let mut lines = Vec::new();
    loop {
        let left = until.saturating_duration_since(Instant::now());
        let line = said
            .recv_timeout(left)
            .map_err(|_| format!("alo never said where an agent reaches it: {lines:?}"))?;
        if let Some(path) = line.strip_prefix("alo: an agent reaches this browser at ") {
            return Ok((lines, PathBuf::from(path)));
        }
        lines.push(line);
    }
}

/// Send `lines` through `alo-ask`, and what it printed, a line per answer.
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

/// The processes whose parent is `parent`: each one's number and command
/// line, from the process table.
fn children_of(parent: u32) -> Result<Vec<(u32, String)>, String> {
    let table = Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid=,args="])
        .output()
        .map_err(|why| format!("ps did not run: {why}"))?;
    Ok(String::from_utf8_lossy(&table.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let process = words.next()?.parse().ok()?;
            let its_parent: u32 = words.next()?.parse().ok()?;
            let args = words.collect::<Vec<_>>().join(" ");
            (its_parent == parent).then_some((process, args))
        })
        .collect())
}

/// Whether the process `pid` is in the process table.
fn running(pid: u32) -> bool {
    Command::new("/bin/ps")
        .args(["-p", &pid.to_string()])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The program, killed and waited for when the test ends however it ends.
struct Started(Child);

impl Drop for Started {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// An agent, through `alo-ask`, finds the application's page as its one tab,
/// reads it laid out where the corpus case has it, and is told Chrome
/// draws it.
fn reads_the_page_in_chrome(socket: &Path, contents: &Path) -> Result<(), String> {
    let page = contents.join("Resources").join("page.html");
    let address = format!(
        "file://{}",
        page.to_str()
            .ok_or("not a path a URL can name")?
            .replace(' ', "%20")
    );
    let answers = ask(
        socket,
        &[
            r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{address}"}}"#),
            format!(r#"{{"v":1,"id":3,"ask":"renderer","tab":"{address}"}}"#),
        ],
    )?;
    let [tabs, tree, renderer] = answers.as_slice() else {
        return Err(format!("three answers, not {answers:?}"));
    };
    assert_eq!(
        tabs,
        &format!(r#"{{"answer":"tabs","id":1,"tabs":["{address}"],"v":1}}"#)
    );
    // The page's heading, laid out where the corpus case has it. Chrome
    // numbers its nodes as it pleases, so the number is passed over.
    let heading = tree
        .split_once(r#""name":"alo browser","node":"#)
        .map(|(_, after)| after.trim_start_matches(|letter: char| letter.is_ascii_digit()));
    assert!(
        heading.is_some_and(|after| after.starts_with(
            r#","rect":{"height":40.0,"width":640.0,"x":96.0,"y":64.0},"role":"heading""#
        )),
        "{tree}"
    );
    assert!(
        tree.contains("This page is drawn by this Mac's Chrome, driven by alo browser."),
        "{tree}"
    );
    assert!(
        renderer.contains(r#""renderer":{"kind":"Chrome","product":"Chrome/"#),
        "{renderer}"
    );

    Ok(())
}

#[test]
fn the_application_opens_its_page_in_chrome_and_quitting_leaves_nothing_running()
-> Result<(), String> {
    let (root, contents) = application()?;
    let runtime = root.join("runtime");
    std::fs::create_dir_all(&runtime).map_err(|why| why.to_string())?;
    let mut child = Command::new(contents.join("MacOS").join("alo"))
        .current_dir("/")
        .env("XDG_RUNTIME_DIR", &runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("the application's alo did not start: {why}"))?;
    let said = lines_of(&mut child)?;
    let mut started = Started(child);
    let pid = started.0.id();
    let (lines, socket) = until_reachable(&said)?;
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("alo: pages are drawn by Chrome/")
                && line.ends_with(", driven by this browser")),
        "{lines:?}"
    );

    // Its input ended as it started, and it runs on.
    let until = Instant::now() + STILL_AFTER;
    while Instant::now() < until {
        assert!(
            matches!(started.0.try_wait(), Ok(None)),
            "alo closed with its input: {:?}",
            said.try_iter().collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let closed = said
        .try_iter()
        .find(|line| line.starts_with("alo: this browser runs until"));
    assert_eq!(
        closed.as_deref(),
        Some(
            "alo: this browser runs until Chrome's window or the frame above it is closed, \
             or it is quit"
        )
    );

    reads_the_page_in_chrome(&socket, &contents)?;

    // What it started: Chrome, and the tab strip's renderer.
    let children = children_of(pid)?;
    let chrome = children
        .iter()
        .find(|(_, args)| args.contains("--remote-debugging-pipe"))
        .ok_or_else(|| format!("no Chrome among {children:?}"))?;
    let profile = chrome
        .1
        .split(" --")
        .find_map(|flag| flag.strip_prefix("user-data-dir="))
        .map(PathBuf::from)
        .ok_or_else(|| format!("Chrome has no profile: {}", chrome.1))?;
    assert!(
        children
            .iter()
            .any(|(_, args)| args.contains("/alo-render")),
        "no tab strip renderer among {children:?}"
    );

    assert!(profile.is_dir(), "{}", profile.display());
    assert!(socket.exists(), "{}", socket.display());

    quit(&root.join("alo browser.app"))?;
    let until = Instant::now() + QUIT_WITHIN;
    let ended = loop {
        match started.0.try_wait() {
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            Ok(ended) => break ended,
            Err(why) => return Err(format!("alo could not be waited for: {why}")),
        }
    };
    assert!(
        ended.is_some(),
        "alo was still running {QUIT_WITHIN:?} after it was quit"
    );
    let until = Instant::now() + AT_MOST;
    let mut left: Vec<_> = children.iter().map(|(pid, _)| *pid).collect();
    while !left.is_empty() && Instant::now() < until {
        left.retain(|pid| running(*pid));
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        left.is_empty(),
        "still running after the program ended: {left:?} of {children:?}"
    );

    // Item 688: nothing of the session is left on the disk.
    let profile_left = profile.exists();
    let socket_left = socket.exists();
    let _ = std::fs::remove_dir_all(&profile);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        !profile_left,
        "Chrome's profile was left: {}",
        profile.display()
    );
    assert!(!socket_left, "the socket was left: {}", socket.display());
    Ok(())
}
