/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a killed browser left is removed at the next start, and nothing a
//! living browser holds is (queue item 690, ADR 0069).
//!
//! Three browser processes, each a copy of this test binary holding the
//! machine's Chrome, all making their profiles in one temporary directory of
//! this test's own — `TMPDIR` is set for them alone, so no other test's
//! start sweeps it and this one's sweeps nothing else. The first is killed
//! with `SIGKILL`, which runs nothing, and its profile is left; the second
//! goes on living; the third is started and removes the first's profile, and
//! only it.

#![cfg(unix)]

use alo_window::chrome::Chrome;
use alo_window::chrome_profile::LOCK_FILE;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// Set in a copy of this test binary that plays a browser process, to what
/// it does once its Chrome has started.
const PLAYING: &str = "ALO_TEST_PLAYS_A_BROWSER_PROCESS";

/// A browser that holds its Chrome until its input ends, or it is killed.
const HOLDS: &str = "holds";

/// A browser that says what its start removed, and closes.
const STARTS: &str = "starts";

/// How long a Chrome is given to end once its browser was killed — an
/// instrument, not the claim, as in `chrome_over_a_pipe.rs`.
const LONG_ENOUGH: Duration = Duration::from_secs(90);

/// The browser process, when this binary is run as one.
fn play(what: &str) {
    let chrome = match Chrome::start() {
        Ok(chrome) => chrome,
        // Said rather than panicked: the test reading this finds no
        // `started` line, and says what it found instead.
        Err(why) => {
            println!("unstarted {why}");
            return;
        }
    };
    println!(
        "started {} {}",
        chrome.process(),
        chrome.profile().display()
    );
    for removed in &chrome.swept().removed {
        println!("removed {}", removed.display());
    }
    for (unremoved, why) in &chrome.swept().unremoved {
        println!("unremoved {} {why:?}", unremoved.display());
    }
    println!("said");
    if what == HOLDS {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
    }
    drop(chrome);
}

/// A browser process started with its temporary directory at `directory`.
fn browser(directory: &Path, what: &str) -> Option<(Child, BufReader<ChildStdout>)> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .args([
            "--exact",
            "a_killed_browsers_profile_is_removed_by_the_next_start_and_a_living_ones_is_not",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PLAYING, what)
        .env("TMPDIR", directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let said = BufReader::new(child.stdout.take()?);
    Some((child, said))
}

/// What a browser process said about its start, up to its `said` line:
/// its Chrome's process, its profile, and what its start removed and could
/// not.
#[derive(Debug, Default)]
struct Said {
    chrome: u32,
    profile: PathBuf,
    removed: Vec<PathBuf>,
    unremoved: Vec<String>,
}

fn said_by(lines: &mut BufReader<ChildStdout>) -> Option<Said> {
    let mut said = Said::default();
    let mut line = String::new();
    loop {
        line.clear();
        if lines.read_line(&mut line).ok()? == 0 {
            return None;
        }
        // The harness may have begun the line with the test's name.
        let line = line.trim_end();
        if line.ends_with("said") {
            return Some(said);
        }
        if let Some((_, started)) = line.split_once("started ") {
            let (process, profile) = started.split_once(' ')?;
            said.chrome = process.parse().ok()?;
            said.profile = PathBuf::from(profile);
        } else if let Some((_, removed)) = line.split_once("removed ") {
            if line.contains("unremoved ") {
                said.unremoved.push(removed.to_owned());
            } else {
                said.removed.push(PathBuf::from(removed));
            }
        }
    }
}

/// Whether process `id` is still running; a zombie is not.
fn runs(id: u32) -> Option<bool> {
    let looked = Command::new("ps")
        .args(["-o", "stat=", "-p", &id.to_string()])
        .output()
        .ok()?;
    let stat = String::from_utf8(looked.stdout).ok()?;
    Some(!stat.trim().is_empty() && !stat.trim().starts_with('Z'))
}

/// Whether any process is still running with `profile` in its command line.
fn anything_runs_on(profile: &Path) -> Option<bool> {
    let looked = Command::new("pgrep").arg("-f").arg(profile).output().ok()?;
    match looked.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// Wait up to [`LONG_ENOUGH`] for `done`.
fn waiting_for(mut done: impl FnMut() -> bool) -> bool {
    let began = Instant::now();
    while began.elapsed() < LONG_ENOUGH {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

/// This test's own temporary directory, removed when it is done.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_killed_browsers_profile_is_removed_by_the_next_start_and_a_living_ones_is_not() {
    if let Some(what) = std::env::var_os(PLAYING) {
        play(&what.to_string_lossy());
        return;
    }
    let scratch = Scratch(
        std::env::temp_dir().join(format!("alo-killed-browser-test-{}", std::process::id())),
    );
    let _ = std::fs::remove_dir_all(&scratch.0);
    std::fs::create_dir(&scratch.0).unwrap();
    let directory = scratch.0.canonicalize().unwrap();

    let (mut killed, mut killed_said) = browser(&directory, HOLDS).unwrap();
    let killed_said = said_by(&mut killed_said).unwrap();
    let (mut living, mut living_said) = browser(&directory, HOLDS).unwrap();
    let living_said = said_by(&mut living_said).unwrap();
    assert!(killed_said.profile.join(LOCK_FILE).is_file());
    assert!(living_said.profile.join(LOCK_FILE).is_file());
    assert_eq!(
        living_said.removed,
        Vec::<PathBuf>::new(),
        "a living browser's profile is not removed"
    );

    // `SIGKILL`: the browser runs nothing as it ends.
    killed.kill().unwrap();
    killed.wait().unwrap();
    assert!(
        waiting_for(|| runs(killed_said.chrome) == Some(false)
            && anything_runs_on(&killed_said.profile) == Some(false)),
        "the killed browser's Chrome ends when its pipe closes, waited {LONG_ENOUGH:?}"
    );
    assert!(
        killed_said.profile.join(LOCK_FILE).is_file(),
        "a killed browser leaves its profile"
    );

    let (mut next, mut next_said) = browser(&directory, STARTS).unwrap();
    let next_said = said_by(&mut next_said).unwrap();
    assert!(next.wait().unwrap().success());
    assert_eq!(next_said.removed, vec![killed_said.profile.clone()]);
    assert_eq!(next_said.unremoved, Vec::<String>::new());
    assert!(
        !killed_said.profile.exists(),
        "the killed browser's profile is removed"
    );
    assert!(
        living_said.profile.join(LOCK_FILE).is_file(),
        "the living browser's profile is left"
    );
    assert!(
        !next_said.profile.exists(),
        "the next browser, closed, removed its own"
    );

    // The living browser closed, its own profile goes with it.
    drop(living.stdin.take());
    assert!(living.wait().unwrap().success());
    assert!(!living_said.profile.exists());
}
