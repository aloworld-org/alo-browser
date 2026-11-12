/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application refuses `--sign-ins DIR` (queue item 973, ADR
//! 0111 § 6): a developer's hand run keeps its sign-ins' key in a keychain
//! file under a password written in alo's source, and no product path may
//! keep anything so.
//!
//! The real `alo`, copied into `alo browser.app` shaped as the application
//! is, is started from inside it with `--conversation --sign-ins DIR`, its
//! `HOME` a directory of the test's own. It ends at once with the usage
//! error's status, says why and names `--data DIR` instead, and makes
//! nothing: no `DIR`, no keychain file in it, nothing under `HOME` — so
//! neither a hand run's keychain nor the person's own is ever touched. It
//! opens no window and starts no Chromium, so this file is in no Chrome
//! group.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Every entry under `root`, relative to it.
fn entries_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut left = vec![root.to_path_buf()];
    while let Some(directory) = left.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if let Ok(relative) = path.strip_prefix(root) {
                found.push(relative.display().to_string());
            }
            if path.is_dir() {
                left.push(path);
            }
        }
    }
    found
}

/// The application, made under a directory of this test's own: the
/// directory, and the `alo` inside the application.
fn application() -> Result<(PathBuf, PathBuf), String> {
    let root = std::env::temp_dir().join(format!("alo-sign-ins-refused-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let contents = root.join("alo browser.app").join("Contents");
    let programs = contents.join("MacOS");
    let resources = contents.join("Resources");
    for directory in [&programs, &resources] {
        std::fs::create_dir_all(directory).map_err(|why| why.to_string())?;
    }
    let alo = programs.join("alo");
    std::fs::copy(env!("CARGO_BIN_EXE_alo"), &alo)
        .map_err(|why| format!("alo could not be copied: {why}"))?;
    let page = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/cases/the-applications-page/page.html");
    std::fs::copy(page, resources.join(alo_window::bundle::PAGE))
        .map_err(|why| format!("the application's page could not be copied: {why}"))?;
    Ok((root, alo))
}

#[test]
fn the_application_refuses_a_hand_runs_sign_ins_and_makes_nothing() -> Result<(), String> {
    let (root, alo) = application()?;
    let home = root.join("home");
    std::fs::create_dir_all(&home).map_err(|why| why.to_string())?;
    let named = root.join("hand run").join("sign-ins");
    let ran = Command::new(&alo)
        .args(["--conversation", "--sign-ins"])
        .arg(&named)
        .env("HOME", &home)
        .current_dir("/")
        .stdin(Stdio::null())
        .output()
        .map_err(|why| format!("alo did not run: {why}"))?;
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    assert_eq!(
        ran.status.code(),
        Some(2),
        "refused as a usage error: {said}"
    );
    assert!(
        said.starts_with(
            "alo: --sign-ins is for a developer's hand run, and alo browser refuses it"
        ),
        "{said}"
    );
    assert!(said.contains("Use --data DIR"), "names what to use: {said}");
    assert_eq!(said.lines().count(), 1, "says nothing else: {said}");
    assert!(ran.stdout.is_empty());
    assert!(!named.exists(), "no directory made at {}", named.display());
    assert!(
        !named.join("alo-sign-ins.keychain-db").exists(),
        "no keychain file made"
    );
    assert_eq!(entries_under(&home), Vec::<String>::new(), "nothing kept");
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
