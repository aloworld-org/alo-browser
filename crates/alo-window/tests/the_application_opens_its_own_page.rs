/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo` inside the macOS application, started the way macOS starts it
//! (queue items 477 and 719; ADR 0067 § 1, ADR 0068, ADR 0077).
//!
//! The real `alo` and `alo-render`, copied into `alo browser.app` shaped as
//! the application is, with the application's page from the corpus in its
//! `Contents/Resources`, are started with **no arguments and `/dev/null`
//! for input** — what macOS gives an application opened from the Finder,
//! measured. Before this, `alo` given that opened an empty tab, and `alo
//! --chrome` closed as it opened.
//!
//! The application carries **a Chromium of its own** at
//! `Contents/Frameworks/alo Chromium.app/Contents/MacOS/alo Chromium` (ADR
//! 0077). **With a CEF distribution named by `CEF_PATH`** it is the real
//! one (queue item 720): alo's CEF host is built against it, and the
//! application is the one `alo-bundle` makes, carrying the host, CEF's
//! framework and its licences, sealed — run as it is. **Without one** the
//! host cannot be built, and so that the application's own path is still
//! run, its Chromium is this machine's Chrome cloned whole into that place
//! — `cp -c`, a copy-on-write clone that costs no disk — with its program
//! reached through a link named `alo Chromium` beside it, since a Chrome
//! finds its framework and helpers from the path it was started at. It
//! speaks the same protocol, and it is a different program at a different
//! path from the machine's Chrome, which is never touched. Which of the two
//! ran is said on the test's error output.
//!
//! Here it opens the application's page in that Chromium, says so on the
//! terminal with the way it is closed, and is still running after its
//! input has ended. The Chrome process it started is, by the process
//! table's own word, the program inside the application, and no process of
//! the machine's Chrome is among what it started. An agent, through
//! `alo-ask`, finds the page at its `file:` address inside the application
//! and is told the Chromium alo carries draws it.
//! Then the application is **quit** as the Dock and ⌘Q quit it, by the
//! Apple Event `osascript` sends it, which ends the process inside `AppKit`
//! without returning from its event loop (ADR 0067 § 1). Nothing it started
//! is left running — no process of any program inside the application,
//! the Chromium's helpers among them — and nothing it made is left on the
//! disk: not the Chromium's profile, which Quit used to leave (queue item
//! 688), and not the socket an agent reached it at. And no program inside
//! it left a crash report.

#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for to say where an agent reaches it,
/// and what it started for to end.
const AT_MOST: Duration = Duration::from_secs(60);

/// Where Chrome's installer puts it on a Mac: what the test's Chromium is
/// cloned from when there is no CEF distribution, and what the application
/// must never start.
const MACHINES_CHROME: &str = "/Applications/Google Chrome.app";

/// The CEF distribution `CEF_PATH` names, or why there is none.
fn distribution() -> Result<PathBuf, String> {
    let Some(named) = std::env::var_os("CEF_PATH") else {
        return Err("CEF_PATH is not set".to_owned());
    };
    let named = PathBuf::from(named);
    if named.join(alo_window::cef_host::FRAMEWORK).is_dir()
        && named.join(alo_window::cef_pin::ARCHIVE_JSON).is_file()
    {
        Ok(named)
    } else {
        Err(format!(
            "CEF_PATH names {}, which holds no unpacked CEF distribution",
            named.display()
        ))
    }
}

/// alo's CEF host built against the distribution at `cef`: its program.
fn built(cef: &Path) -> Result<PathBuf, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = repository.join("target/alo-cef-host");
    let status = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repository.join("crates/alo-cef-host/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CEF_PATH", cef)
        .status()
        .map_err(|why| format!("cargo did not start: {why}"))?;
    if !status.success() {
        return Err(format!("the host did not build: {status}"));
    }
    Ok(target.join("debug/alo-cef-host"))
}

/// The application `alo-bundle` makes in `root` from the programs this test
/// was built with, carrying the host built against `cef`: its `Contents`.
fn made_by_alo_bundle(root: &Path, cef: &Path) -> Result<PathBuf, String> {
    let host = built(cef)?;
    let alo = Path::new(env!("CARGO_BIN_EXE_alo"));
    let programs = alo.parent().ok_or("alo is in no directory")?;
    let made = Command::new(env!("CARGO_BIN_EXE_alo-bundle"))
        .arg("--programs")
        .arg(programs)
        .arg("--into")
        .arg(root)
        .arg("--host")
        .arg(&host)
        .env("CEF_PATH", cef)
        .output()
        .map_err(|why| format!("alo-bundle did not run: {why}"))?;
    if !made.status.success() {
        return Err(format!(
            "alo-bundle did not make the application: {}",
            String::from_utf8_lossy(&made.stderr)
        ));
    }
    Ok(root.join("alo browser.app").join("Contents"))
}

/// Put a Chromium in the application whose `Contents` is `contents`, at
/// the place it carries one: this machine's Chrome cloned, its program
/// reached through a link named `alo Chromium` beside it. The program's
/// path.
fn carry_a_chromium(contents: &Path) -> Result<PathBuf, String> {
    let program = contents.join(alo_window::bundle::CHROMIUM);
    let carried = program
        .ancestors()
        .nth(3)
        .ok_or("the Chromium's place is in no application")?;
    let frameworks = carried.parent().ok_or("no Frameworks")?;
    std::fs::create_dir_all(frameworks).map_err(|why| why.to_string())?;
    let cloned = Command::new("/bin/cp")
        .arg("-cR")
        .arg(MACHINES_CHROME)
        .arg(carried)
        .output()
        .map_err(|why| format!("cp did not run: {why}"))?;
    if !cloned.status.success() {
        return Err(format!(
            "{MACHINES_CHROME} could not be cloned into the application: {}",
            String::from_utf8_lossy(&cloned.stderr)
        ));
    }
    std::os::unix::fs::symlink("Google Chrome", &program)
        .map_err(|why| format!("{} could not be made: {why}", program.display()))?;
    Ok(program)
}

/// The path the process table gives the program `pid` runs.
fn program_of(pid: u32) -> Result<String, String> {
    let read = Command::new("/bin/ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .map_err(|why| format!("ps did not run: {why}"))?;
    Ok(String::from_utf8_lossy(&read.stdout).trim().to_owned())
}

/// The Chrome among `children`, a program's processes, checked to be the
/// program at `carried` inside the application by the process table's word
/// — and no process among them the machine's Chrome.
fn only_its_own_chromium<'c>(
    children: &'c [(u32, String)],
    carried: &Path,
) -> Result<&'c (u32, String), String> {
    let chrome = children
        .iter()
        .find(|(_, args)| args.contains("--remote-debugging-pipe"))
        .ok_or_else(|| format!("no Chrome among {children:?}"))?;
    let carried = carried.to_str().ok_or("not a path ps can name")?;
    assert_eq!(program_of(chrome.0)?, carried, "{children:?}");
    for (child, args) in children {
        let program = program_of(*child)?;
        assert!(
            !program.starts_with(MACHINES_CHROME),
            "the machine's Chrome was started: {program} ({args})"
        );
    }
    Ok(chrome)
}

/// How long after its input ended the program is asked to be running still:
/// `alo --chrome` started this way had ended two seconds later (ADR 0067
/// § 1).
const STILL_AFTER: Duration = Duration::from_secs(4);

/// The application, made under a directory of this test's own, carrying
/// a Chromium — CEF's, made by `alo-bundle`, when `CEF_PATH` names a
/// distribution; this machine's Chrome cloned when not: the directory, the
/// application's `Contents`, and its Chromium's program.
fn application() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let root = std::env::temp_dir().join(format!("alo-application-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    let contents = match distribution() {
        Ok(cef) => {
            eprintln!(
                "the application carries CEF's Chromium, made by alo-bundle against {}",
                cef.display()
            );
            made_by_alo_bundle(&root, &cef)?
        }
        Err(why) => {
            eprintln!(
                "the application carries this machine's Chrome cloned, not CEF's Chromium: \
                 {why} (queue item 720)"
            );
            let contents = shaped(&root)?;
            carry_a_chromium(&contents)?;
            contents
        }
    };
    let carried = contents.join(alo_window::bundle::CHROMIUM);
    Ok((root, contents, carried))
}

/// The application shaped by hand under `root`, carrying no Chromium yet:
/// its `Contents`.
fn shaped(root: &Path) -> Result<PathBuf, String> {
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
    Ok(contents)
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

/// Whether any process is running a program inside `application`.
fn running_inside(application: &Path) -> bool {
    Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(application)
        .output()
        .is_ok_and(|found| !found.stdout.is_empty())
}

/// The crash reports macOS keeps for this person, by name.
fn crash_reports() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    std::fs::read_dir(Path::new(&home).join("Library/Logs/DiagnosticReports"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The test's own directory — the application and its Chromium in it —
/// removed when the test ends however it ends.
struct RemovedAtTheEnd(PathBuf);

impl Drop for RemovedAtTheEnd {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
/// reads it laid out where the corpus case has it, and is told the
/// Chromium alo carries draws it.
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
        tree.contains(
            "This page is drawn by the Chromium alo browser carries, driven by alo browser."
        ),
        "{tree}"
    );
    assert!(
        renderer.contains(r#""renderer":{"kind":"CarriedChromium","product":"Chrome/"#),
        "{renderer}"
    );

    Ok(())
}

/// Assert nothing `alo` started — `children` — is running once it ended,
/// no program inside `application` is, and no crash report beside
/// `reports` is any of theirs.
fn nothing_is_left_running(children: &[(u32, String)], application: &Path, reports: &[String]) {
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
    while running_inside(application) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !running_inside(application),
        "a program inside the application outlived it"
    );
    let reported: Vec<String> = crash_reports()
        .into_iter()
        .filter(|report| !reports.contains(report) && report.starts_with("alo"))
        .collect();
    assert_eq!(reported, Vec::<String>::new(), "crash reports");
}

#[test]
fn the_application_opens_its_page_in_its_own_chromium_and_quitting_leaves_nothing_running()
-> Result<(), String> {
    let reports = crash_reports();
    let (root, contents, carried) = application()?;
    let _removed = RemovedAtTheEnd(root.clone());
    let application = root.join("alo browser.app");
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
                && line.ends_with(", the Chromium alo carries, driven by this browser")),
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
    let chrome = only_its_own_chromium(&children, &carried)?;
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

    quit(&application)?;
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
    nothing_is_left_running(&children, &application, &reports);

    // Item 688: nothing of the session is left on the disk.
    let profile_left = profile.exists();
    let socket_left = socket.exists();
    let _ = std::fs::remove_dir_all(&profile);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        !profile_left,
        "the Chromium's profile was left: {}",
        profile.display()
    );
    assert!(!socket_left, "the socket was left: {}", socket.display());
    Ok(())
}
