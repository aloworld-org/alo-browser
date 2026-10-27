/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The frame of ours around Chrome's window, as the program a person
//! starts makes it (ADR 0062, queue item 642): `alo --chrome` itself, run
//! as a process of its own, and both windows read from the window server
//! in numbers — never from Chrome's word alone, and never from ours.
//!
//! **What moves Chrome's window.** The page does, with `window.moveTo` and
//! `window.resizeTo`, which Chrome 155 carries out for its `--app` window
//! and reports over the pipe exactly as it reports a person's drag of the
//! same window (measured on 2026-10-26: `chrome_frame`'s documentation; and
//! by item 642, `moveTo(300, 260)` read back as 300,260 and `resizeTo(700,
//! 500)` as a window of 700×500 holding a page of 700×468). A test cannot
//! drag a window on a locked screen, and a page can move its own on any.
//!
//! **What is not assumed.** Under the suite's other shown Chromes a window
//! placed is left in Stage Manager's strip in about one run in four (item
//! 643). So this asserts the rule rather than that Chrome's window is in
//! front: in every settled reading, either the window server shows
//! Chrome's window and ours directly above it, as wide as it is, or it
//! shows Chrome's nowhere a person can see it and ours not at all.
//!
//! **Closing.** The program's input ending closes Chrome's window and the
//! frame, through exactly what closing the frame does — the order the
//! thread holding Chrome is sent — and Chrome going closes the frame and
//! the program, each read from the window server and the process table.
//!
//! Chrome's process is found as the program's child, and the frame as the
//! program's own floating window, so the two tests here do not see each
//! other's.

#![cfg(target_os = "macos")]

use alo_window::chrome::LONGEST_SILENCE;
use alo_window::chrome_follow::FOLLOW_EVERY;
use alo_window::chrome_frame::FRAME_HEIGHT;
use alo_window::chrome_protocol::Bounds;
use alo_window::window_server::{self, Listed};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long the program is waited for: to say where an agent reaches it,
/// and to close.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the windows are watched after the page's last move: a window
/// brought out of the strip, or a frame catching up, within Chrome's own
/// patience.
const SETTLE: Duration = Duration::from_secs(10);

/// A page that moves its own window once it has loaded, and then resizes
/// it, a few seconds apart.
const MOVING: &str = "<!doctype html><title>Moving</title><h1>Moving</h1><script>\
    setTimeout(() => window.moveTo(300, 260), 2000);\
    setTimeout(() => window.resizeTo(700, 500), 4000);\
    </script>";

/// A page that stays where it is put.
const STILL: &str = "<!doctype html><title>Still</title><h1>Still</h1>";

/// Where the page moves its window.
const MOVED: (i64, i64) = (300, 260);

/// The size the page resizes its window to.
const RESIZED: (i64, i64) = (700, 500);

/// The title bar of Chrome's `--app` window, in points: measured by item
/// 642 as a window of 700×500 holding a page of 700×468.
const TITLE_BAR: i64 = 32;

/// The narrowest window of Chrome's a person sees as a window: Stage
/// Manager's thumbnails were measured at 97 to 164 points wide.
const SEEN_WIDE: i64 = 400;

/// A runtime directory of this test's own, empty, so the program's socket
/// is placed under it rather than under the person's.
fn runtime(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-frame-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    Ok(root)
}

/// `alo --chrome`, running, with its input held open and what it says on
/// the terminal arriving a line at a time.
struct Running {
    child: Child,
    input: Option<ChildStdin>,
    said: Receiver<String>,
}

impl Running {
    /// Start `alo --chrome` on `page`, its socket under `runtime`.
    fn start(page: &Path, runtime: &Path) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alo"))
            .arg("--chrome")
            .arg(page)
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

    /// Wait until it says it runs, and what it said until then.
    fn until_running(&self) -> Result<Vec<String>, String> {
        let until = Instant::now() + AT_MOST;
        let mut lines = Vec::new();
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let line = self
                .said
                .recv_timeout(left)
                .map_err(|_| format!("alo never said it runs: {lines:?}"))?;
            let runs = line.starts_with("alo: this browser runs until ");
            lines.push(line);
            if runs {
                return Ok(lines);
            }
        }
    }

    /// The process of the Chrome it started: its one child of that name.
    fn chrome(&self) -> Result<u32, String> {
        let found = Command::new("pgrep")
            .args(["-P", &self.child.id().to_string(), "-x", "Google Chrome"])
            .output()
            .map_err(|why| format!("pgrep could not be run: {why}"))?;
        let found = String::from_utf8_lossy(&found.stdout);
        let mut lines = found.lines();
        match (lines.next(), lines.next()) {
            (Some(one), None) => one
                .trim()
                .parse()
                .map_err(|_| format!("not a process: {one}")),
            _ => Err(format!("not one Chrome under alo: {found:?}")),
        }
    }

    /// How it exited, waiting at most [`AT_MOST`].
    fn exited(&mut self) -> Result<ExitStatus, String> {
        let until = Instant::now() + AT_MOST;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => return Err("alo did not close".to_owned()),
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

/// What the window server shows of both programs at one moment: Chrome's
/// ordinary windows a person sees as windows, and our floating ones.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    chrome: Vec<Bounds>,
    frames: Vec<Option<Bounds>>,
}

/// One reading of what the window server shows of `chrome` and `ours`.
fn seen(chrome: u32, ours: u32) -> Result<Seen, String> {
    let listed = |owner| window_server::on_the_screen(owner).map_err(|why| why.to_string());
    let chrome = listed(chrome)?
        .into_iter()
        .filter(|window: &Listed| window.layer == 0)
        .filter_map(|window| window.bounds)
        .filter(|bounds| bounds.width >= SEEN_WIDE)
        .collect();
    let frames = listed(ours)?
        .into_iter()
        .filter(|window| window.layer == 3)
        .map(|window| window.bounds)
        .collect();
    Ok(Seen { chrome, frames })
}

/// Read until two readings a moment apart agree, so a frame still
/// catching up with a window that just moved is read again rather than
/// believed — or the last reading, when they never do within [`SETTLE`].
///
/// Two readings agreeing is not the frame having looked. The frame is
/// placed by its own reading — [`FOLLOW_EVERY`] apart, each one question
/// to Chrome and one listing, and since item 681 a second question, what
/// Chrome calls the page, after the frame is told where to stand and
/// before the next reading — and the questions to Chrome are the part
/// this test never makes: under memory pressure on 2026-10-26 it was what
/// slowed, so two of this test's readings 300 ms apart both fell before
/// the frame's first look and read *Chrome's window shown, no frame*
/// (item 682; made to fail three runs in three by a 1.5 s pause in the
/// frame's reading alone, and not by the same pause in every listing,
/// which slows this test's readings with the frame's). So a reading that
/// keeps the rule is believed when two agree, and one that breaks it only
/// once it has stood unchanged longer than the frame's longest cycle: the
/// wait between readings, Chrome's patience for each of its two questions
/// ([`LONGEST_SILENCE`]), and a listing as slow as the slowest this test
/// timed. A frame that never comes still fails, by that bound.
fn settled(chrome: u32, ours: u32) -> Result<(Seen, bool), String> {
    let until = Instant::now() + SETTLE;
    let mut slowest = Duration::ZERO;
    let mut since = Instant::now();
    let mut last = timed(chrome, ours, &mut slowest)?;
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let now = timed(chrome, ours, &mut slowest)?;
        if now == last {
            let frames_longest = FOLLOW_EVERY + LONGEST_SILENCE * 2 + slowest;
            if the_rule_holds(&now).is_ok() || since.elapsed() > frames_longest {
                return Ok((now, true));
            }
        } else {
            if Instant::now() >= until {
                return Ok((now, false));
            }
            since = Instant::now();
            last = now;
        }
    }
}

/// One reading, with `slowest` raised to how long it took if it was longer.
fn timed(chrome: u32, ours: u32, slowest: &mut Duration) -> Result<Seen, String> {
    let began = Instant::now();
    let reading = seen(chrome, ours)?;
    *slowest = (*slowest).max(began.elapsed());
    Ok(reading)
}

/// The rule, on one settled reading: Chrome's window seen and the frame
/// directly above it, as wide as it is and the strip's height — answered
/// with where Chrome's window is — or Chrome's seen nowhere and no frame.
fn the_rule_holds(seen: &Seen) -> Result<Option<Bounds>, String> {
    match (seen.chrome.as_slice(), seen.frames.as_slice()) {
        ([], []) => Ok(None),
        ([window], [Some(frame)])
            if *frame
                == Bounds {
                    left: window.left,
                    top: window.top - FRAME_HEIGHT,
                    width: window.width,
                    height: FRAME_HEIGHT,
                } =>
        {
            Ok(Some(*window))
        }
        _ => Err(format!(
            "neither a frame directly above Chrome's window, nor no frame beside none: {seen:?}"
        )),
    }
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line each.
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

/// Where the program said an agent reaches it.
fn socket_said(said: &[String]) -> Result<PathBuf, String> {
    said.iter()
        .find_map(|line| line.strip_prefix("alo: an agent reaches this browser at "))
        .map(PathBuf::from)
        .ok_or_else(|| format!("no socket was said: {said:?}"))
}

/// Whether the window server lists any window of `owner` on the screen.
fn any_window(owner: u32) -> Result<bool, String> {
    window_server::on_the_screen(owner)
        .map(|listed| !listed.is_empty())
        .map_err(|why| why.to_string())
}

/// Whether a process is still running.
fn running(process: u32) -> Result<bool, String> {
    Command::new("kill")
        .args(["-0", &process.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .map_err(|why| format!("kill could not be run: {why}"))
}

/// Wait until neither `chrome` nor `ours` has a window on the screen.
fn both_gone(chrome: u32, ours: u32) -> Result<(), String> {
    let until = Instant::now() + SETTLE;
    loop {
        if !running(chrome)? && !any_window(ours)? {
            return Ok(());
        }
        if Instant::now() >= until {
            return Err(format!(
                "Chrome running {}, a window of ours {}",
                running(chrome)?,
                any_window(ours)?
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_frame_stands_above_chromes_window_follows_it_and_closes_with_it() -> Result<(), String> {
    let runtime = runtime("moving")?;
    let page = runtime.join("moving.html");
    std::fs::write(&page, MOVING).map_err(|why| why.to_string())?;
    let mut alo = Running::start(&page, &runtime)?;
    let said = alo.until_running()?;
    let chrome = alo.chrome()?;
    let ours = alo.child.id();

    // Watched until the page's last move has had time to settle, each
    // settled reading held to the rule, and where the frame stood kept.
    let until = Instant::now() + Duration::from_secs(4) + SETTLE;
    let mut beside = Vec::new();
    let mut last = None;
    while Instant::now() < until {
        let (reading, agreed) = settled(chrome, ours)?;
        if agreed {
            let window = the_rule_holds(&reading)?;
            if let Some(window) = window
                && beside.last() != Some(&window)
            {
                beside.push(window);
            }
            last = Some(window);
        }
    }
    eprintln!("Chrome's window, with the frame above it: {beside:?}; {said:?}");
    // Wherever it was last seen, it was where the page left it.
    if let Some(Some(window)) = last {
        assert_eq!(
            (window.left, window.top, window.width, window.height),
            (MOVED.0, MOVED.1, RESIZED.0, RESIZED.1),
            "the frame stood above a window the page had finished with"
        );
    }
    // The page laid out again at the size its window was given: 700 wide,
    // and as tall as the window less its title bar, by the tree an agent
    // reads through the program's socket.
    let file = format!("file://{}", page.display());
    let answers = ask(
        &socket_said(&said)?,
        &[format!(r#"{{"v":1,"id":1,"ask":"read","tab":"{file}"}}"#)],
    )?;
    let tree = answers.first().ok_or("no answer")?;
    let document = format!(
        r#""rect":{{"height":{}.0,"width":{}.0,"x":0.0,"y":0.0}},"role":"document""#,
        RESIZED.1 - TITLE_BAR,
        RESIZED.0
    );
    assert!(tree.contains(&document), "{document} in {tree}");

    // Every place it stood was one this program or the page put the
    // window at: where it was placed, 1200 wide, or where it was moved.
    for window in &beside {
        assert!(
            (window.left, window.top) == (MOVED.0, MOVED.1) || window.width == 1200,
            "{beside:?}"
        );
    }

    // The input ends: Chrome's window closes, and the frame, and the
    // program, which exits as having finished.
    drop(alo.input.take());
    let status = alo.exited()?;
    assert!(status.success(), "{status}");
    both_gone(chrome, ours)?;
    let _ = std::fs::remove_dir_all(runtime);
    Ok(())
}

#[test]
fn chrome_going_closes_the_frame_and_the_program() -> Result<(), String> {
    let runtime = runtime("still")?;
    let page = runtime.join("still.html");
    std::fs::write(&page, STILL).map_err(|why| why.to_string())?;
    let mut alo = Running::start(&page, &runtime)?;
    alo.until_running()?;
    let chrome = alo.chrome()?;
    let ours = alo.child.id();
    let (reading, _) = settled(chrome, ours)?;
    the_rule_holds(&reading)?;

    // Chrome told to end, as the system ends a program: its window closes,
    // and with it the frame and the program, though its input is open.
    let ended = Command::new("kill")
        .args(["-TERM", &chrome.to_string()])
        .status()
        .map_err(|why| format!("kill could not be run: {why}"))?;
    assert!(ended.success(), "{ended}");
    let status = alo.exited()?;
    assert!(status.success(), "{status}");
    assert!(alo.input.is_some(), "its input was never ended");
    both_gone(chrome, ours)?;
    let _ = std::fs::remove_dir_all(runtime);
    Ok(())
}
