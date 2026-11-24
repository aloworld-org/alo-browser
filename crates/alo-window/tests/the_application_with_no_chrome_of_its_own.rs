/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application with no Chromium of its own, or a damaged one,
//! still opens the conversation window, and says why a site chosen in it
//! cannot open — never with the machine's Chrome (queue items 719 and 993,
//! ADR 0077 and its addendum of 15 November 2026).
//!
//! Until 993 it drew a page of its own with this engine saying its
//! Chromium was missing or damaged; the application now opens the
//! conversation, which this engine draws with or without a Chromium, so
//! what was said on that page is said where a site is chosen instead.
//!
//! The real `alo` and `alo-render`, copied into `alo browser.app` shaped as
//! the application is, are started as macOS starts an application —
//! `/dev/null` for input, `/` for its directory — naming nothing but `--data
//! DIR`, a directory of this test's own, so that the conversation never
//! touches the person's keychain, chats, settings or sign-ins (it decides
//! what opens as no argument at all does: `Bundle::opening`). In the first
//! test nothing is at `Contents/Frameworks/alo Chromium.app/Contents/MacOS/
//! alo Chromium`; in the second a program is there that ends as it starts,
//! as a Chromium that is damaged would. Either way the machine's Chrome may
//! be installed — on the Mac this runs on, it is — and is never started:
//! the owner's words, *as if the user only has alo-browser on his PC*.
//!
//! With none there, the terminal says no site chosen can open. In both, an
//! agent, through `alo-ask`, finds no tab, and a site it asks to open is
//! refused as not opened with why — the sentence the conversation's own
//! turn says to the person (*… could not be opened: …*, `Went::Unopened`,
//! drawn in `a_conversation_scrolled_in_the_window.rs`). No process the
//! application started is a Chrome.
//!
//! This file has "chrome" in its name so that it is in the `chrome` test
//! group: a regression here would start a Chrome, and Chromes take turns.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for to say where an agent reaches it,
/// and what it started for to end.
const AT_MOST: Duration = Duration::from_secs(60);

/// Where Chrome's installer puts it on a Mac: never to be started by the
/// application.
const MACHINES_CHROME: &str = "/Applications/Google Chrome.app";

/// The application, made under a directory of this test's own called
/// `name`: the directory, and the application's `Contents`. Its name is
/// short: the agent's socket is made under it, and a socket's path has to
/// be shorter than `SUN_LEN`.
fn application(name: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = std::env::temp_dir().join(format!("alo-unc-{}-{name}", std::process::id()));
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
    std::fs::copy(
        cases.join("the-applications-page/page.html"),
        resources.join(alo_window::bundle::PAGE),
    )
    .map_err(|why| format!("the application's page could not be copied: {why}"))?;
    Ok((root, contents))
}

/// Put a damaged Chromium where the application carries its own: a program
/// that ends as it starts, saying nothing. Its path.
fn carry_a_damaged_chromium(contents: &Path) -> Result<PathBuf, String> {
    let program = contents.join(alo_window::bundle::CHROMIUM);
    let directory = program.parent().ok_or("a place with no directory")?;
    std::fs::create_dir_all(directory).map_err(|why| why.to_string())?;
    std::fs::write(&program, "#!/bin/sh\nexit 1\n").map_err(|why| why.to_string())?;
    std::fs::set_permissions(
        &program,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .map_err(|why| why.to_string())?;
    Ok(program)
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

/// The processes whose parent is `parent`: each one's number and program,
/// from the process table.
fn children_of(parent: u32) -> Result<Vec<(u32, String)>, String> {
    let table = Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid=,comm="])
        .output()
        .map_err(|why| format!("ps did not run: {why}"))?;
    Ok(String::from_utf8_lossy(&table.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let process = words.next()?.parse().ok()?;
            let its_parent: u32 = words.next()?.parse().ok()?;
            let program = words.collect::<Vec<_>>().join(" ");
            (its_parent == parent).then_some((process, program))
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

/// The program, killed and waited for when the test ends however it ends,
/// and the test's own directory removed after it.
struct Started {
    child: Child,
    root: PathBuf,
}

impl Drop for Started {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Start the application made at `root`, whose `Contents` is `contents`, as
/// macOS starts it; check that it opens the conversation, saying `said` on
/// its terminal first when it is given, and that a site an agent asks it to
/// open is refused as not opened, `why`, and that
/// nothing it started is a Chrome; then end it.
fn the_conversation_says_why_no_site_opens(
    root: PathBuf,
    contents: &Path,
    said: Option<&str>,
    why: &str,
) -> Result<(), String> {
    let runtime = root.join("runtime");
    std::fs::create_dir_all(&runtime).map_err(|why| why.to_string())?;
    let mut child = Command::new(contents.join("MacOS").join("alo"))
        .arg("--data")
        .arg(root.join("d"))
        .current_dir("/")
        .env("XDG_RUNTIME_DIR", &runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("the application's alo did not start: {why}"))?;
    let terminal = lines_of(&mut child)?;
    let mut started = Started { child, root };
    let pid = started.child.id();
    let (lines, socket) = until_reachable(&terminal)?;
    if let Some(said) = said {
        assert!(
            lines.iter().any(|line| line == said),
            "{said:?} not among {lines:?}"
        );
    }
    assert!(
        lines.iter().any(
            |line| line == "alo: the browser's own interface is drawn by this browser's engine"
        ),
        "{lines:?}"
    );

    // Nothing opens by itself; a site asked for is refused with why. The
    // address is never fetched: no Chromium starts to fetch it.
    // The socket is open before the window answers what only it can — a
    // new chat — and until then `open` is refused as not offered: asked
    // again until the window answers.
    let address = "http://127.0.0.1:9/one.html";
    let until = Instant::now() + AT_MOST;
    let answers = loop {
        let answers = ask(
            &socket,
            &[
                r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
                format!(r#"{{"v":1,"id":2,"ask":"open","address":"{address}"}}"#),
            ],
        )?;
        let unoffered = answers
            .get(1)
            .is_some_and(|answer| answer.contains(r#""kind":"NotOffered""#));
        if !unoffered || Instant::now() >= until {
            break answers;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let [tabs, opened] = answers.as_slice() else {
        return Err(format!("two answers, not {answers:?}"));
    };
    assert_eq!(tabs, r#"{"answer":"tabs","id":1,"tabs":[],"v":1}"#);
    assert_eq!(
        opened,
        &format!(
            r#"{{"answer":"refused","id":2,"refusal":{{"address":"{address}","kind":"NotOpened"}},"said":"{address} was not opened: {why}","v":1}}"#
        )
    );

    // What it started: this engine's renderers, and no Chrome — not the
    // machine's, and not the damaged one, which ended as it started.
    let children = children_of(pid)?;
    assert!(
        children
            .iter()
            .any(|(_, program)| program.ends_with("/alo-render")),
        "no renderer among {children:?}"
    );
    for (_, program) in &children {
        assert!(
            !program.starts_with(MACHINES_CHROME) && !program.contains("Chromium"),
            "a Chrome was started: {program} among {children:?}"
        );
    }

    let _ = started.child.kill();
    let _ = started.child.wait();
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
    Ok(())
}

#[test]
fn with_no_chromium_of_its_own_the_application_opens_the_conversation_and_says_why_no_site_can()
-> Result<(), String> {
    let (root, contents) = application("gone")?;
    let carried = contents.join(alo_window::bundle::CHROMIUM);
    let missing = format!(
        "alo's own Chromium is not in the application: it was looked for at {}",
        carried.display()
    );
    let said = format!(
        "alo: {missing}, so no site chosen in the conversation can open, and the conversation \
         says so"
    );
    the_conversation_says_why_no_site_opens(root, &contents, Some(&said), &missing)
}

#[test]
fn with_a_damaged_chromium_the_application_opens_the_conversation_and_says_why_no_site_can()
-> Result<(), String> {
    let (root, contents) = application("bad")?;
    if let Err(why) = carry_a_damaged_chromium(&contents) {
        let _ = std::fs::remove_dir_all(&root);
        return Err(why);
    }
    the_conversation_says_why_no_site_opens(
        root,
        &contents,
        None,
        "Chrome did not say which it is: Chrome has gone",
    )
}
