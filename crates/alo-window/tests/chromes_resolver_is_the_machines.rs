/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome asks the machine's resolver and never upgrades it to encrypted DNS
//! on its own (queue item 635, ADR 0008, ADR 0060 § 4).
//!
//! What Chrome resolves through is read from Chrome itself: its net log, which
//! writes the secure DNS mode its configuration is in and the mode each lookup
//! is asked in, as `"secure_dns_mode":` and a number — 0 off, 1 automatic, 2
//! secure. The installed Chrome is started as the browser starts it, through a
//! program standing where Chrome would that adds the net log's two flags and
//! runs the real one. No page is opened and nothing here asks the network for
//! anything: Chrome reads its configuration, and asks for its own things, as it
//! starts.
//!
//! A second Chrome, whose stand-in writes `automatic` over the profile's file
//! before Chrome reads it, is read as mode 1 — so the reading tells the modes
//! apart, and it is the file the browser writes that keeps Chrome in mode 0.

#![cfg(unix)]

use alo_window::beside::{NotInstalled, chrome_in, chrome_places};
use alo_window::chrome::{Chrome, LONGEST_SILENCE};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The Chrome on this machine, where the browser looks for it. These tests
/// drive it (ADR 0058), so one that is not there fails them.
fn installed() -> Result<PathBuf, NotInstalled> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    chrome_in(&chrome_places(home.as_deref()))
}

/// A directory of the test's own holding a program that runs `chrome` with
/// its net log written to `netlog.json` beside it, after running `before`.
struct LoggedChrome {
    directory: PathBuf,
}

impl LoggedChrome {
    fn new(name: &str, chrome: &Path, before: &str) -> Result<Self, String> {
        let directory =
            std::env::temp_dir().join(format!("alo-chrome-resolver-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|why| format!("{why}"))?;
        let quoted = |path: &Path| -> Result<String, String> {
            let text = path.to_str().ok_or("a path that is not text")?;
            if text.contains('\'') {
                return Err(format!("a path with a quote in it: {text}"));
            }
            Ok(format!("'{text}'"))
        };
        let program = directory.join("chrome");
        let script = format!(
            "#!/bin/sh\n{before}\nexec {} \"$@\" --log-net-log={} --net-log-capture-mode=Default\n",
            quoted(chrome)?,
            quoted(&directory.join("netlog.json"))?
        );
        std::fs::write(&program, script).map_err(|why| format!("{why}"))?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
            .map_err(|why| format!("{why}"))?;
        Ok(Self { directory })
    }

    fn program(&self) -> PathBuf {
        self.directory.join("chrome")
    }

    fn netlog(&self) -> String {
        std::fs::read_to_string(self.directory.join("netlog.json")).unwrap_or_default()
    }
}

impl Drop for LoggedChrome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Every secure DNS mode the net log `text` names, in the order it names them.
fn modes(text: &str) -> Vec<String> {
    const KEY: &str = "\"secure_dns_mode\":";
    text.match_indices(KEY)
        .map(|(at, _)| {
            text.get(at + KEY.len()..)
                .unwrap_or_default()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect()
        })
        .collect()
}

/// Start the Chrome at `program` as the browser does, keep it until its net log
/// names a secure DNS mode, close it, and answer every mode the whole net log
/// names — or why there were none.
fn modes_chrome_ran_in(logged: &LoggedChrome) -> Result<Vec<String>, String> {
    let mut chrome = Chrome::start_at(&logged.program()).map_err(|why| format!("{why}"))?;
    chrome.version().map_err(|why| format!("{why}"))?;
    let until = Instant::now() + LONGEST_SILENCE;
    while modes(&logged.netlog()).is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    // Closed, so that the net log is written to its end.
    drop(chrome);
    let named = modes(&logged.netlog());
    if named.is_empty() {
        return Err(format!(
            "Chrome's net log named no secure DNS mode in {} s",
            LONGEST_SILENCE.as_secs()
        ));
    }
    Ok(named)
}

#[test]
fn chrome_asks_the_machines_resolver_and_never_its_own_encrypted_dns() {
    let chrome = installed().unwrap();
    let logged = LoggedChrome::new("pinned", &chrome, "").unwrap();
    let named = modes_chrome_ran_in(&logged).unwrap();
    assert!(
        named.iter().all(|mode| mode == "0"),
        "Chrome ran in secure DNS modes {named:?}, where 0 is off"
    );
}

#[test]
fn the_reading_tells_chromes_automatic_mode_from_off() {
    let chrome = installed().unwrap();
    // Writes `automatic` over the file the browser wrote, in the profile
    // Chrome is handed as `--user-data-dir=`, before Chrome reads it.
    let before = r#"for given in "$@"; do
  case "$given" in --user-data-dir=*)
    printf '{"dns_over_https":{"mode":"automatic"}}' > "${given#--user-data-dir=}/Local State";;
  esac
done"#;
    let logged = LoggedChrome::new("automatic", &chrome, before).unwrap();
    let named = modes_chrome_ran_in(&logged).unwrap();
    assert!(
        named.iter().any(|mode| mode == "1"),
        "Chrome told to be automatic ran in secure DNS modes {named:?}"
    );
}

#[test]
fn a_mode_is_read_as_its_digits() {
    assert_eq!(
        modes(r#"{"a":1,"secure_dns_mode":0,"b":{"secure_dns_mode":12}},"secure_dns_mode":"#),
        vec!["0", "12", ""]
    );
    assert!(modes("{}").is_empty());
}
