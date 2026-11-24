/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo browser.app`, started the way macOS starts it with nothing named,
//! opens **the conversation window**, and Quit leaves nothing behind (queue
//! item 993: the owner's decision of 15 November 2026 that alo behaves as
//! any other Mac application; ADR 0068's and ADR 0077's addenda).
//!
//! This replaces what `the_application_opens_its_own_page.rs` asserted of
//! the application opened from the Finder until 993 — its own page in its
//! Chromium, with our frame above it — which that file now asserts only of
//! `alo --chrome` naming the page, the developer's tool that still shows it.
//!
//! The application is made as that file makes it: **with a CEF
//! distribution named by `CEF_PATH`**, the one `alo-bundle` makes, carrying
//! alo's CEF host, CEF's framework and the licences, sealed; **without
//! one**, shaped by hand with this machine's Chrome cloned (`cp -c`) into
//! the place it carries its Chromium, reached through a link named `alo
//! Chromium`. Which of the two ran is said on the test's error output.
//!
//! Its `alo` is started with `/dev/null` for input and `/` for its
//! directory, as macOS starts it, and with one option only: `--data DIR`, a
//! directory of this test's own, so that the conversation keeps nothing in
//! the person's own places and never asks their keychain (ADR 0112 § 5,
//! queue item 957) — a test never touches the real keychain, settings,
//! chats or sign-ins. `--data` names no page, no `--chrome` and no
//! `--conversation`, so it decides what opens exactly as no argument at
//! all does (`Bundle::opening`, whose unit tests take both).
//!
//! Asserted: it says the browser's own interface is drawn by this engine
//! and **no page is drawn by a Chromium** (no `pages are drawn by` line),
//! and runs on after its input has ended; an agent, through `alo-ask`,
//! finds no tab — the conversation opens no page — and then has a site, on
//! a server of this test's own, opened in a new chat; the Chromium that
//! shows it is, by the process table's word, the program inside the
//! application, and no process the application started is the machine's
//! Chrome. The licences are reachable: made by `alo-bundle`, the
//! application's `Credits.html` — what macOS's *About alo* panel shows —
//! links every file in its licences folder, and each link names a file
//! there. Then it is **quit** as the Dock and ⌘Q quit it, by the Apple
//! Event `osascript` sends, which ends the process inside `AppKit` (ADR
//! 0068's amendment, item 688): nothing it started is left running — no
//! process of any program inside the application, the Chromium's helpers
//! among them — and nothing it made is left on the disk: not the card's
//! Chromium's profile, and not the socket. And no program inside it left a
//! crash report.

#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for to say where an agent reaches it,
/// and what it started for to end; and a card's Chromium to start and draw.
const AT_MOST: Duration = Duration::from_secs(90);

/// Where Chrome's installer puts it on a Mac: what the test's Chromium is
/// cloned from when there is no CEF distribution, and what the application
/// must never start.
const MACHINES_CHROME: &str = "/Applications/Google Chrome.app";

/// A server on this machine answering `/one.html`: its port. Never the
/// real network.
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
    // Short: the agent's socket is made under it, in the data directory,
    // and a socket's path has to be shorter than `SUN_LEN`.
    let root = std::env::temp_dir().join(format!("alo-993-{}", std::process::id()));
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
    std::fs::copy(
        cases.join("the-applications-page/page.html"),
        resources.join(alo_window::bundle::PAGE),
    )
    .map_err(|why| format!("the application's page could not be copied: {why}"))?;
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

/// Assert the application whose `Contents` is `contents` links every
/// licence it carries from the credits its *About alo* panel shows, each
/// link naming a file there.
fn the_licences_are_reachable(contents: &Path) -> Result<(), String> {
    let resources = contents.join("Resources");
    let credits = std::fs::read_to_string(resources.join(alo_window::bundle_credits::FILE))
        .map_err(|why| format!("the credits: {why}"))?;
    let folder = resources.join(alo_window::bundle_carried::LICENCES);
    let mut carried = Vec::new();
    for entry in std::fs::read_dir(&folder).map_err(|why| why.to_string())? {
        let name = entry.map_err(|why| why.to_string())?.file_name();
        carried.push(name.to_string_lossy().into_owned());
    }
    for (name, _) in alo_window::cef_pin::LICENCES {
        assert!(carried.iter().any(|held| held == name), "{name} is carried");
    }
    for name in &carried {
        assert!(
            credits.contains(&format!("<a href=\"licences/{name}\">")),
            "the credits link {name}: {credits}"
        );
    }
    for linked in credits.split("href=\"").skip(1) {
        let (address, _) = linked.split_once('"').ok_or("a link with no end")?;
        assert!(
            resources.join(address).exists(),
            "the credits link {address}, which is not in the application"
        );
    }
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

/// An agent, through `alo-ask` at `socket`, finds no tab in the conversation
/// the program `pid` opened, and has a site on the server at `port` opened
/// in a new chat: its children then, the card's Chromium checked to be the
/// one `carried` inside the application, and that Chromium's profile.
fn a_site_opens_in_its_own_chromium(
    socket: &Path,
    port: u16,
    pid: u32,
    carried: &Path,
) -> Result<(Vec<(u32, String)>, PathBuf), String> {
    // The conversation opens no page: no tab, until a site is chosen.
    let one = format!("http://127.0.0.1:{port}/one.html");
    let answers = ask(socket, &[r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned()])?;
    assert_eq!(
        answers,
        [r#"{"answer":"tabs","id":1,"tabs":[],"v":1}"#.to_owned()]
    );
    // The socket is open before the window answers what only it can — a
    // new chat — and until then `open` is refused as not offered: asked
    // again until the window answers.
    let until = Instant::now() + AT_MOST;
    let answers = loop {
        let answers = ask(
            socket,
            &[format!(
                r#"{{"v":1,"id":2,"ask":"open","address":"{one}"}}"#
            )],
        )?;
        let unoffered = answers
            .iter()
            .any(|answer| answer.contains(r#""kind":"NotOffered""#));
        if !unoffered || Instant::now() >= until {
            break answers;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(
        answers,
        [format!(
            r#"{{"answer":"opened","id":2,"said":"opened {one}","tab":"{one}","v":1}}"#
        )]
    );

    // What it started: the conversation's renderers, and the card's
    // Chromium — the application's own.
    let children = children_of(pid)?;
    let chrome = only_its_own_chromium(&children, carried)?;
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
        "no renderer among {children:?}"
    );
    Ok((children, profile))
}

#[test]
fn the_application_opens_the_conversation_window_and_quitting_leaves_nothing_behind()
-> Result<(), String> {
    let reports = crash_reports();
    let (root, contents, carried) = application()?;
    let _removed = RemovedAtTheEnd(root.clone());
    let application = root.join("alo browser.app");
    let made_by_alo_bundle = distribution().is_ok();
    if made_by_alo_bundle {
        the_licences_are_reachable(&contents)?;
    } else {
        eprintln!(
            "the application was shaped by hand, so it has no credits: whether its licences are \
             reachable is asserted only of the one alo-bundle makes"
        );
    }
    let runtime = root.join("runtime");
    std::fs::create_dir_all(&runtime).map_err(|why| why.to_string())?;
    let data = root.join("d");
    let port = serve()?;
    let mut child = Command::new(contents.join("MacOS").join("alo"))
        .arg("--data")
        .arg(&data)
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
        lines.iter().any(
            |line| line == "alo: the browser's own interface is drawn by this browser's engine"
        ),
        "{lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with("alo: pages are drawn by")),
        "no page of its own, in a Chromium or this engine: {lines:?}"
    );
    assert!(socket.starts_with(&data), "{}", socket.display());

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

    let (children, profile) = a_site_opens_in_its_own_chromium(&socket, port, pid, &carried)?;
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
        "the card's Chromium's profile was left: {}",
        profile.display()
    );
    assert!(!socket_left, "the socket was left: {}", socket.display());
    Ok(())
}
