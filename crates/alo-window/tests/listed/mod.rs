/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What macOS says a process is among a person's applications (queue items
//! 781 and 739): `Foreground` — an ordinary application, with a Dock icon
//! and a place in ⌘-Tab, its windows Stage Manager's to put in its strip —
//! or `UIElement`, an accessory one, in none of them. Read with
//! `lsappinfo`, macOS's own tool for what Launch Services knows of a
//! running application, by the process's number; the answer is read as
//! strictly as anything from outside, one line of exactly one key.

use std::process::Command;

/// What `lsappinfo` says `process` is: `Foreground`, `UIElement`,
/// `BackgroundOnly` — or why it was not read.
pub fn application_type(process: u32) -> Result<String, String> {
    let ran = Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "ApplicationType"])
        .arg(process.to_string())
        .output()
        .map_err(|why| format!("lsappinfo could not be run: {why}"))?;
    let said = String::from_utf8_lossy(&ran.stdout);
    let said = said.trim();
    said.strip_prefix("\"ApplicationType\"=\"")
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|kind| !kind.is_empty() && kind.chars().all(char::is_alphanumeric))
        .map(str::to_owned)
        .ok_or_else(|| format!("lsappinfo said no application type of {process}: {said:?}"))
}
