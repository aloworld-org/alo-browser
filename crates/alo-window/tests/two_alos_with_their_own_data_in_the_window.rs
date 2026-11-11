/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 957, ADR 0112 § 5: two `alo --conversation` started at once,
//! each with `--data` naming a directory of its own, are two instances —
//! each keeps its chats, sign-ins, keys and socket in its own directory,
//! and an agent reaches each through its own socket alone.
//!
//! Each `alo` is the built program, run as a person would run it, with its
//! window shown; neither touches the person's keychain, chats or sign-ins,
//! and neither writes anything secret under its directory: with no secret
//! store yet, no key is kept at all (ADR 0112 § 5). No card is
//! opened, so no Chromium is started. Both are closed by the test.

#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long an `alo` is waited for to say where its socket is.
const AT_MOST: Duration = Duration::from_secs(60);

/// An `alo` started, closed when dropped.
struct Alo {
    child: Child,
    said: Receiver<String>,
}

impl Drop for Alo {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Alo {
    /// `alo --conversation --frozen-fonts --data DATA`.
    fn start(data: &Path) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alo"))
            .args(["--conversation", "--frozen-fonts", "--data"])
            .arg(data)
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
        Ok(Self { child, said })
    }

    /// The path it said an agent reaches it at, and what it said before.
    fn socket(&self) -> Result<(PathBuf, Vec<String>), String> {
        const AT: &str = "alo: an agent reaches this browser at ";
        let by = Instant::now() + AT_MOST;
        let mut heard = Vec::new();
        while let Some(left) = by.checked_duration_since(Instant::now()) {
            let Ok(line) = self.said.recv_timeout(left) else {
                break;
            };
            if let Some(path) = line.strip_prefix(AT) {
                return Ok((PathBuf::from(path), heard));
            }
            heard.push(line);
        }
        Err(format!("alo never said where its socket is: {heard:?}"))
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

/// Every file under `root`, however deep.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut waiting = vec![root.to_path_buf()];
    while let Some(at) = waiting.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                waiting.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn two_alos_with_their_own_data_keep_apart_and_are_reached_apart() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("alo-957-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (one, two) = (root.join("one"), root.join("two"));
    let first = Alo::start(&one)?;
    let second = Alo::start(&two)?;
    let ((at_one, said_one), (at_two, said_two)) = (first.socket()?, second.socket()?);
    assert_eq!(at_one, one.join("alo-browser").join("agent.sock"));
    assert_eq!(at_two, two.join("alo-browser").join("agent.sock"));
    for socket in [&at_one, &at_two] {
        assert_eq!(
            tabs_at(socket)?,
            r#"{"answer":"tabs","id":1,"tabs":[],"v":1}"#,
            "each answers at its own socket"
        );
    }
    for (data, said) in [(&one, &said_one), (&two, &said_two)] {
        let chats = format!("alo: chats are kept in {}", data.join("Chats").display());
        assert!(said.contains(&chats), "{said:?}");
        assert!(
            said.iter()
                .any(|line| line == "alo: sign-ins are kept only until alo quits"),
            "{said:?}"
        );
        // Nothing secret is kept without a secret store (ADR 0112 § 5):
        // no keychain file, no sealed file, no sign-ins.
        for file in files_under(data) {
            let name = file.to_string_lossy().into_owned();
            assert!(
                !name.contains("keychain")
                    && !name.ends_with(".sealed")
                    && !name.contains("Sign-ins"),
                "{name} written under the data directory"
            );
        }
    }
    drop(first);
    drop(second);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
