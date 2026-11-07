/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The system's clipboard, written (queue item 798's *Copy link*).
//!
//! On macOS the clipboard is the general pasteboard, which `pbcopy` — a
//! program of the system's own, at one place — writes from its standard
//! input as plain text: no `unsafe`, nothing rented, and nothing alo keeps.
//! Only a person's press on *Copy link* in alo's menu writes it; a test
//! hands the window a writer of its own
//! (`ConversationWindow::copy_with`), so no test touches the person's
//! clipboard. Off macOS nothing is written yet, and the press says so.

use std::io::Write as _;
use std::process::{Command, Stdio};

/// Where macOS keeps `pbcopy`.
pub const PBCOPY: &str = "/usr/bin/pbcopy";

/// Put `text` on the system's clipboard as plain text, or say why not.
///
/// # Errors
///
/// Why the clipboard was not written: `pbcopy` not started, not handed the
/// text, or ending with a failure.
pub fn copy(text: &str) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("only macOS's clipboard is written so far".to_owned());
    }
    let mut child = Command::new(PBCOPY)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| format!("pbcopy did not start: {why}"))?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| "pbcopy took no text".to_owned())
        .and_then(|mut to| {
            to.write_all(text.as_bytes())
                .map_err(|why| format!("pbcopy was not handed the text: {why}"))
        });
    let ended = child
        .wait()
        .map_err(|why| format!("pbcopy was not waited for: {why}"))?;
    written?;
    if ended.success() {
        Ok(())
    } else {
        Err(format!("pbcopy failed: {ended}"))
    }
}
