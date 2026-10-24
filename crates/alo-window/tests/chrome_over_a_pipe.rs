/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome is spoken to over a pipe (queue item 619, ADR 0058 § 3).
//!
//! The Chrome installed on this machine is started, answers
//! `Browser.getVersion` with the version its own program says when asked on
//! the command line — never a number written here — and is gone, with every
//! process it started and the profile it was given, once the browser process
//! is: whether it closed Chrome itself or died without a word. A Chrome that is
//! not there is refused by name, and a far side that sends what is not the
//! protocol is refused as a kind, never a panic; those are played by small
//! shell programs standing where Chrome would.

#![cfg(unix)]

use alo_window::beside::{NotInstalled, chrome_in, chrome_places};
use alo_window::chrome::{Chrome, Failed, Unstarted};
use alo_window::chrome_protocol::{Declined, Malformed, Version};
use alo_window::chrome_wire::Unframed;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Set in a copy of this test binary that plays the browser process.
const PLAYING: &str = "ALO_TEST_PLAYS_THE_BROWSER_PROCESS";

/// The Chrome on this machine, where the browser looks for it. These tests
/// drive it (ADR 0058), so one that is not there fails them.
fn installed() -> Result<PathBuf, NotInstalled> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    chrome_in(&chrome_places(home.as_deref()))
}

/// What the program says its version is, asked on the command line rather
/// than over the pipe: `Google Chrome 155.0.8059.86` read as `155.0.8059.86`.
fn version_said_by(program: &Path) -> Option<String> {
    let said = Command::new(program).arg("--version").output().ok()?;
    let said = String::from_utf8(said.stdout).ok()?;
    said.split_whitespace().last().map(str::to_owned)
}

/// Whether any process is still running with `profile` in its command line —
/// Chrome, or any helper it started.
fn anything_runs_on(profile: &Path) -> Option<bool> {
    let looked = Command::new("pgrep").arg("-f").arg(profile).output().ok()?;
    // 0 found one, 1 found none; anything else is pgrep failing.
    match looked.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// Whether process `id` is still running.
fn runs(id: u32) -> Option<bool> {
    let looked = Command::new("ps")
        .args(["-o", "stat=", "-p", &id.to_string()])
        .output()
        .ok()?;
    // `ps` finds nothing for a process that has gone, and a zombie — gone
    // but never waited for — is not running either.
    let stat = String::from_utf8(looked.stdout).ok()?;
    Some(!stat.trim().is_empty() && !stat.trim().starts_with('Z'))
}

/// How long a wait for a process to go is given before it is called a
/// failure.
///
/// **The number is an instrument, not the claim.** What these tests assert is
/// that Chrome *ends*, not that it ends inside any particular time; no part of
/// ADR 0058 promises a speed, and a test that fails because the machine was
/// busy has measured the machine. It was ten seconds until 2026-10-24, when
/// the full gate failed on
/// `chrome_is_gone_when_the_browser_process_dies_without_a_word` at 10.9
/// seconds while `a_few_million_pictures_say_a_ceilings_worth_and_how_many_more`
/// had been running for over 420 seconds beside it. The same test passes alone
/// in 3.3 seconds. So the budget is set far above anything a healthy machine
/// needs and the failure says how long it actually waited, which is the number
/// worth reading when it fires.
const LONG_ENOUGH: Duration = Duration::from_secs(90);

/// Wait up to [`LONG_ENOUGH`] for `done`, saying how long it took, or
/// [`None`] if it never came.
fn waiting_for(mut done: impl FnMut() -> bool) -> Option<Duration> {
    let began = Instant::now();
    while began.elapsed() < LONG_ENOUGH {
        if done() {
            return Some(began.elapsed());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done().then(|| began.elapsed())
}

#[test]
fn chrome_answers_its_own_version_and_is_gone_when_closed() {
    let program = installed().unwrap();
    let mut chrome = Chrome::start_at(&program).unwrap();
    let version = chrome.version().unwrap();
    assert_eq!(
        version.product,
        format!("Chrome/{}", version_said_by(&program).unwrap()),
        "the version over the pipe is the program's own"
    );
    assert_eq!(version.protocol, "1.3");
    assert!(
        version.user_agent.contains("HeadlessChrome/"),
        "{version:?}"
    );
    // Asked twice, answered twice: each command is matched to its reply.
    assert_eq!(chrome.version().unwrap(), version);

    let (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
    assert_eq!(runs(process), Some(true));
    assert_eq!(anything_runs_on(&profile), Some(true));
    drop(chrome);
    assert_eq!(runs(process), Some(false), "waited for, so gone at once");
    assert!(
        waiting_for(|| anything_runs_on(&profile) == Some(false)).is_some(),
        "every helper Chrome started has gone, waited {LONG_ENOUGH:?}"
    );
    assert!(!profile.exists(), "its profile is removed");
}

#[test]
fn chrome_is_gone_when_the_browser_process_dies_without_a_word() {
    if std::env::var_os(PLAYING).is_some() {
        // The browser process: start Chrome, say where it is, and die
        // without closing it, as a crashed or killed browser would.
        let mut chrome = Chrome::start().unwrap();
        let version = chrome.version().unwrap();
        println!(
            "started {} {} {}",
            chrome.process(),
            version.product,
            chrome.profile().display()
        );
        std::process::exit(0);
    }
    let played = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "chrome_is_gone_when_the_browser_process_dies_without_a_word",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PLAYING, "1")
        .output()
        .unwrap();
    let said = String::from_utf8(played.stdout).unwrap();
    let started = said
        .lines()
        // The harness may have begun the line with the test's name.
        .find_map(|line| line.split_once("started ").map(|(_, rest)| rest))
        .unwrap_or_else(|| panic!("the browser process did not start Chrome: {said}"));
    let mut words = started.splitn(3, ' ');
    let process: u32 = words.next().unwrap().parse().unwrap();
    let product = words.next().unwrap();
    let profile = PathBuf::from(words.next().unwrap());
    assert_eq!(
        product,
        format!("Chrome/{}", version_said_by(&installed().unwrap()).unwrap())
    );
    assert!(
        waiting_for(|| runs(process) == Some(false) && anything_runs_on(&profile) == Some(false))
            .is_some(),
        "Chrome ends when its pipe closes with the browser process, waited {LONG_ENOUGH:?}"
    );
    // Nobody was left to remove it; it is in the system's temporary
    // directory, which is the system's to clear.
    let _ = std::fs::remove_dir_all(&profile);
}

#[test]
fn a_chrome_that_is_not_there_is_refused_by_name() {
    let nowhere = PathBuf::from("/nowhere/Google Chrome.app/Contents/MacOS/Google Chrome");
    let Err(refused) = Chrome::start_at(&nowhere) else {
        panic!("a Chrome that is not there was started");
    };
    assert_eq!(
        refused.to_string(),
        "Chrome is not installed: it was looked for at /nowhere/Google Chrome.app/Contents/MacOS/Google Chrome"
    );
    assert!(matches!(refused, Unstarted::NotInstalled(_)));
    // A directory is not a program either, and is not started to find out.
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(matches!(
        Chrome::start_at(&directory),
        Err(Unstarted::NotInstalled(_))
    ));
}

/// A program standing where Chrome would, running `script` with Chrome's
/// pipe on 3 and 4, in a directory of its own.
struct FarSide {
    directory: PathBuf,
}

impl FarSide {
    fn running(name: &str, script: &str) -> std::io::Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("alo-chrome-far-side-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory)?;
        let program = directory.join("chrome");
        std::fs::write(&program, format!("#!/bin/sh\n{script}\n"))?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
        Ok(Self { directory })
    }

    /// Ask it Chrome's version, waiting at most two seconds.
    fn asked(&self) -> Result<Result<Version, Failed>, Unstarted> {
        Ok(Chrome::start_at(&self.directory.join("chrome"))?
            .waiting_at_most(Duration::from_secs(2))
            .version())
    }
}

impl Drop for FarSide {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn a_far_side_that_does_not_speak_the_protocol_is_refused_as_a_kind() {
    // Each says its piece on 4, then reads 3 until the pipe closes.
    let saying = |name, bytes: &str| {
        FarSide::running(name, &format!("printf '{bytes}' >&4\ncat <&3 >/dev/null"))
            .unwrap()
            .asked()
            .unwrap()
    };
    assert_eq!(
        saying("not-json", "not json\\000"),
        Err(Failed::Malformed(Malformed::NotJson))
    );
    assert_eq!(
        saying("not-an-object", "[1,2]\\000"),
        Err(Failed::Malformed(Malformed::NotAnObject))
    );
    assert_eq!(
        saying("not-utf-8", "\\377\\376\\000"),
        Err(Failed::Malformed(Malformed::NotJson))
    );
    assert_eq!(
        saying("too-deep", &format!("{}\\000", "[".repeat(10_000))),
        Err(Failed::Malformed(Malformed::TooDeep { most: 16 }))
    );
    assert_eq!(
        saying("no-version", "{\"id\":1,\"result\":{\"product\":5}}\\000"),
        Err(Failed::Malformed(Malformed::BadField { field: "product" }))
    );
    assert_eq!(
        saying(
            "declined",
            "{\"id\":1,\"error\":{\"code\":-32601,\"message\":\"no\"}}\\000"
        ),
        Err(Failed::Declined(Declined {
            code: -32601,
            message: "no".to_owned()
        }))
    );
    // A message the pipe closed in the middle of: its 4 closed, and still
    // reading 3.
    assert_eq!(
        FarSide::running(
            "cut-off",
            "printf '{\"id\":1,\"res' >&4\nexec 4>&-\ncat <&3 >/dev/null"
        )
        .unwrap()
        .asked()
        .unwrap(),
        Err(Failed::Unframed(Unframed::Truncated))
    );
}

#[test]
fn a_far_side_that_goes_or_says_nothing_is_said_to_have() {
    assert_eq!(
        FarSide::running("gone", "exit 0").unwrap().asked().unwrap(),
        Err(Failed::Gone)
    );
    assert_eq!(
        FarSide::running("silent", "exec sleep 30")
            .unwrap()
            .asked()
            .unwrap(),
        Err(Failed::Silent {
            waited: Duration::from_secs(2)
        })
    );
    // Events are passed over, and a reply to another command is not this
    // one's: the reply that counts is the one with the command's own id.
    let version = FarSide::running(
        "chatty",
        "printf '{\"method\":\"Target.targetCreated\",\"params\":{}}\\000' >&4\n\
         printf '{\"id\":99,\"result\":{}}\\000' >&4\n\
         printf '{\"id\":1,\"result\":{\"product\":\"Chrome/0\",\"protocolVersion\":\"1.3\",\
         \"revision\":\"r\",\"userAgent\":\"u\",\"jsVersion\":\"j\"}}\\000' >&4\n\
         cat <&3 >/dev/null",
    )
    .unwrap()
    .asked()
    .unwrap()
    .unwrap();
    assert_eq!(version.product, "Chrome/0");
}
