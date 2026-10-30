/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application with no Chromium of its own, or a damaged one,
//! draws its page with this engine and says so — never with the machine's
//! Chrome (queue item 719, ADR 0077).
//!
//! The real `alo` and `alo-render`, copied into `alo browser.app` shaped as
//! the application is, are started as macOS starts an application: no
//! arguments, `/dev/null` for input, `/` for its directory. In the first
//! test nothing is at `Contents/Frameworks/Chromium.app/Contents/MacOS/
//! Chromium`; in the second a program is there that ends as it starts, as a
//! Chromium that is damaged would. Either way the machine's Chrome may be
//! installed — on the Mac this runs on, it is — and is never started: the
//! owner's words, *as if the user only has alo-browser on his PC*.
//!
//! The terminal says why this engine draws the page, and an agent, through
//! `alo-ask`, is told this engine draws it and reads the sentence the page
//! says. No process the application started is a Chrome.
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

/// What the page this engine draws says, in the corpus case
/// `the-applications-page-without-chrome`.
const SAID_ON_THE_PAGE: &str = "alo browser's own Chromium is missing or damaged, so this page is drawn by alo browser's \
     own engine.";

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
/// macOS starts it; check that this engine draws its page, saying `why` on
/// its terminal and what the page says to an agent, and that nothing it
/// started is a Chrome; then end it.
fn drawn_by_this_engine(root: PathBuf, contents: &Path, why: &str) -> Result<(), String> {
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
    let mut started = Started { child, root };
    let pid = started.child.id();
    let (lines, socket) = until_reachable(&said)?;
    assert!(
        lines.iter().any(|line| line == why),
        "{why:?} not among {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "alo: pages are drawn by this browser's engine"),
        "{lines:?}"
    );

    let answers = ask(&socket, &[r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned()])?;
    let address = answers
        .first()
        .and_then(|tabs| tabs.split_once(r#""tabs":[""#))
        .and_then(|(_, after)| after.split_once('"'))
        .map(|(address, _)| address.to_owned())
        .ok_or_else(|| format!("no tab in {answers:?}"))?;
    assert!(
        address.starts_with("file://") && address.ends_with("/Resources/without-chrome.html"),
        "{address}"
    );
    // The tab is named as it opens; its page is read once this engine has
    // loaded it.
    let until = Instant::now() + AT_MOST;
    let answers = loop {
        let answers = ask(
            &socket,
            &[
                format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{address}"}}"#),
                format!(r#"{{"v":1,"id":3,"ask":"renderer","tab":"{address}"}}"#),
            ],
        )?;
        let unloaded = answers
            .first()
            .is_some_and(|tree| tree.contains(r#""kind":"NothingLoaded""#));
        if !unloaded || Instant::now() >= until {
            break answers;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let [tree, renderer] = answers.as_slice() else {
        return Err(format!("two answers, not {answers:?}"));
    };
    assert!(tree.contains(SAID_ON_THE_PAGE), "{tree}");
    assert!(
        renderer.contains(r#""renderer":{"kind":"ThisEngine"}"#),
        "{renderer}"
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
fn with_no_chromium_of_its_own_the_application_draws_with_this_engine_and_says_so()
-> Result<(), String> {
    let (root, contents) = application("gone")?;
    let carried = contents.join(alo_window::bundle::CHROMIUM);
    let why = format!(
        "alo: alo's own Chromium is not in the application: it was looked for at {}, so this \
         browser's own engine draws the application's page",
        carried.display()
    );
    drawn_by_this_engine(root, &contents, &why)
}

#[test]
fn with_a_damaged_chromium_the_application_draws_with_this_engine_and_says_so() -> Result<(), String>
{
    let (root, contents) = application("bad")?;
    let carried = match carry_a_damaged_chromium(&contents) {
        Ok(carried) => carried,
        Err(why) => {
            let _ = std::fs::remove_dir_all(&root);
            return Err(why);
        }
    };
    let why = format!(
        "alo: alo's own Chromium at {} could not draw the application's page — Chrome did not \
         say which it is: Chrome has gone — so this browser's own engine draws it",
        carried.display()
    );
    drawn_by_this_engine(root, &contents, &why)
}
