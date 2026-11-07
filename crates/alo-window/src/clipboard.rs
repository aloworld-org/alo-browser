/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The system's clipboard, written (queue item 798's *Copy link*) and read
//! (queue item 861's paste).
//!
//! On macOS the clipboard is the general pasteboard, which `pbcopy` — a
//! program of the system's own, at one place — writes from its standard
//! input as plain text, and `pbpaste` reads to its standard output: no
//! `unsafe`, nothing rented, and nothing alo keeps. Only a person writes it
//! — *Copy link* in alo's menu, ⌘C or ⌘X in one of alo's fields — and only
//! a person's ⌘V reads it. A test hands the window a writer and a reader of
//! its own (`ConversationWindow::copy_with`, `ConversationWindow::paste_with`),
//! so no test touches the person's clipboard. Off macOS nothing is written
//! or read yet, and the key says so.
//!
//! # How much is read
//!
//! At most [`MOST_PASTED`] bytes of what `pbpaste` writes: a field holds at
//! most 2,048 characters, so more could never go in, and a clipboard
//! holding a whole book is not read whole into the browser process for
//! one line of it. What is read is cut back to its last whole character.

use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

/// Where macOS keeps `pbcopy`.
pub const PBCOPY: &str = "/usr/bin/pbcopy";

/// Where macOS keeps `pbpaste`.
pub const PBPASTE: &str = "/usr/bin/pbpaste";

/// The most bytes of the clipboard read for a paste: four bytes for each
/// character a field holds, and twice that again.
pub const MOST_PASTED: usize = 2 * 4 * alo_renderer::typed::LONGEST_TYPED;

/// The clipboard's plain text, at most [`MOST_PASTED`] bytes of it cut back
/// to a whole character, or why not.
///
/// # Errors
///
/// Why the clipboard was not read: off macOS, `pbpaste` not started, or
/// failing.
pub fn paste() -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("only macOS's clipboard is read so far".to_owned());
    }
    let mut child = Command::new(PBPASTE)
        .args(["-Prefer", "txt"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| format!("pbpaste did not start: {why}"))?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(|| "pbpaste gave nothing to read".to_owned())
        .and_then(|out| {
            out.take(MOST_PASTED as u64)
                .read_to_end(&mut bytes)
                .map_err(|why| format!("pbpaste could not be read: {why}"))
        });
    // What is past the most is not wanted: it ends now, rather than writing
    // into a pipe nobody reads.
    let _ = child.kill();
    let _ = child.wait();
    read?;
    Ok(whole(&bytes))
}

/// `bytes` as text, cut back to the last whole character, any byte that is
/// not UTF-8 shown as U+FFFD.
pub fn whole(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(error) if error.error_len().is_none() => {
            // Cut inside the last character: what came before it.
            String::from_utf8_lossy(bytes.get(..error.valid_up_to()).unwrap_or_default())
                .into_owned()
        }
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_read_is_cut_back_to_a_whole_character() {
        assert_eq!(whole(b"kampala"), "kampala");
        let euro = "a\u{20ac}".as_bytes();
        assert_eq!(whole(euro.get(..2).unwrap_or_default()), "a", "half a euro");
        assert_eq!(whole(b"a\xffb"), "a\u{fffd}b");
        assert_eq!(MOST_PASTED, 16_384);
    }
}
