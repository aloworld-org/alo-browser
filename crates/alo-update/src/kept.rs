/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the updater keeps between runs, in alo's own state and outside any
//! profile (ADR 0116 § 3): the highest serial it has believed, and when it
//! last checked and what it found.
//!
//! # Where
//!
//! In a folder of its own, `Updates/` beside the chats in alo's directory
//! (`~/Library/Application Support/org.aloworld.Browser/Updates`), made
//! readable by this user alone. Under it, beside these two files, are the
//! downloads ([`crate::package`]).
//!
//! - `serial`: one decimal number. A manifest with a lower one is a
//!   replayed older manifest, refused ([`crate::check`]). A file that does
//!   not read is **not** zero — that would let any old manifest in — but
//!   the largest serial, so that nothing is believed until a person sees
//!   why (*Settings* says the check failed) and the file is put right.
//! - `believed`: when a manifest was last believed — signed, this
//!   platform's, not replayed and not expired — as a manifest writes a
//!   moment. More than three days ago, and alo says its Chromium may be
//!   missing security fixes (§ 4).
//! - `checked`: when the last check ended, as a manifest writes a moment,
//!   a space, and what it found in words.
//!
//! Each written whole or not at all: to a `.writing` file, flushed, and
//! renamed over the file.

use crate::instant::Instant;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The folder's name in alo's directory.
pub const FOLDER: &str = "Updates";

/// The file holding the highest serial believed.
const SERIAL: &str = "serial";

/// The file holding when a manifest was last believed.
const BELIEVED: &str = "believed";

/// The file holding the last check.
const CHECKED: &str = "checked";

/// The most bytes either file is read to.
const LARGEST: u64 = 1024;

/// The updater's own folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// It.
    folder: PathBuf,
}

/// What a serial file read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serial {
    /// None is kept: nothing was ever believed.
    None,
    /// This is the highest believed.
    Highest(u64),
    /// The file does not read, so nothing is believed.
    Unread,
}

impl Kept {
    /// The updater's folder, `folder`.
    pub fn at(folder: PathBuf) -> Self {
        Self { folder }
    }

    /// Its folder.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The highest serial believed.
    pub fn serial(&self) -> Serial {
        match self.read(SERIAL) {
            Ok(None) => Serial::None,
            Ok(Some(text)) => text
                .strip_suffix('\n')
                .and_then(|number| {
                    (!number.starts_with('0') || number == "0")
                        .then(|| number.parse().ok())
                        .flatten()
                })
                .map_or(Serial::Unread, Serial::Highest),
            Err(_) => Serial::Unread,
        }
    }

    /// Keep `serial` as the highest believed, believed `at`.
    ///
    /// # Errors
    ///
    /// Why it was not written; the file is then as it was.
    pub fn believe(&self, serial: u64, at: Instant) -> Result<(), String> {
        self.write(SERIAL, &format!("{serial}\n"))?;
        self.write(BELIEVED, &format!("{at}\n"))
    }

    /// When a manifest was last believed, if ever.
    pub fn believed(&self) -> Option<Instant> {
        let text = self.read(BELIEVED).ok()??;
        Instant::read(text.strip_suffix('\n')?)
    }

    /// When the last check ended and what it found, if any is kept.
    pub fn checked(&self) -> Option<(Instant, String)> {
        let text = self.read(CHECKED).ok()??;
        let line = text.strip_suffix('\n')?;
        let (at, found) = line.split_once(' ')?;
        Some((Instant::read(at)?, found.to_owned()))
    }

    /// Keep that a check ended `at`, finding what `found` says.
    ///
    /// # Errors
    ///
    /// Why it was not written.
    pub fn check_ended(&self, at: Instant, found: &str) -> Result<(), String> {
        let found: String = found
            .chars()
            .filter(|each| !each.is_control())
            .take(300)
            .collect();
        self.write(CHECKED, &format!("{at} {found}\n"))
    }

    /// The folder, made for this user alone if it is not there.
    ///
    /// # Errors
    ///
    /// Why it could not be made.
    pub fn made(&self) -> Result<&Path, String> {
        if !self.folder.is_dir() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.folder)
                .map_err(|why| format!("{} could not be made: {why}", self.folder.display()))?;
        }
        Ok(&self.folder)
    }

    /// `name`'s text, none if there is no such file.
    fn read(&self, name: &str) -> Result<Option<String>, String> {
        let file = self.folder.join(name);
        let opened = match File::open(&file) {
            Ok(opened) => opened,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(why) => return Err(why.to_string()),
        };
        let mut bytes = Vec::new();
        opened
            .take(LARGEST + 1)
            .read_to_end(&mut bytes)
            .map_err(|why| why.to_string())?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LARGEST {
            return Err("too long".to_owned());
        }
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "not text".to_owned())
    }

    /// Write `text` as `name`, whole or not at all.
    fn write(&self, name: &str, text: &str) -> Result<(), String> {
        let folder = self.made()?;
        let file = folder.join(name);
        let writing = folder.join(format!("{name}.writing"));
        let failed =
            |why: std::io::Error| format!("{} could not be written: {why}", file.display());
        let mut out = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&writing)
            .map_err(failed)?;
        out.write_all(text.as_bytes()).map_err(failed)?;
        out.sync_all().map_err(failed)?;
        drop(out);
        fs::rename(&writing, &file).map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn scratch(name: &str) -> Kept {
        let folder =
            std::env::temp_dir().join(format!("alo-update-kept-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        Kept::at(folder)
    }

    #[test]
    fn the_highest_serial_believed_is_kept_and_a_damaged_one_believes_nothing() {
        let kept = scratch("serial");
        assert_eq!(kept.serial(), Serial::None);
        assert_eq!(kept.believed(), None);
        kept.believe(118, Instant::at(60)).expect("kept");
        assert_eq!(kept.serial(), Serial::Highest(118));
        assert_eq!(kept.believed(), Some(Instant::at(60)));
        kept.believe(0, Instant::at(120)).expect("kept");
        assert_eq!(kept.serial(), Serial::Highest(0));
        for odd in ["", "118", "0118\n", "-1\n", "x\n", "118\n119\n"] {
            fs::write(kept.folder().join(SERIAL), odd).expect("written");
            assert_eq!(kept.serial(), Serial::Unread, "{odd:?}");
        }
        assert!(!kept.folder().join("serial.writing").exists());
        let _ = fs::remove_dir_all(kept.folder());
    }

    #[test]
    fn the_last_check_is_kept_with_what_it_found() {
        let kept = scratch("checked");
        assert_eq!(kept.checked(), None);
        let at = Instant::at(1_794_646_800);
        kept.check_ended(at, "alo is up to date.\nSpoofed line")
            .expect("kept");
        assert_eq!(
            kept.checked(),
            Some((at, "alo is up to date.Spoofed line".to_owned()))
        );
        let mode = fs::metadata(kept.folder()).map(|meta| meta.permissions().mode() & 0o777);
        assert_eq!(mode.ok(), Some(0o700));
        let _ = fs::remove_dir_all(kept.folder());
    }
}
