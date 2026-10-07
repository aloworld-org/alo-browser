/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the grant table is kept, and what happens to one that does not read.
//!
//! ```text
//! <profile>/grants/
//!   table             the remembered answers, the refusals for every site, the histories
//!   table.aside.<n>   a table that failed its check, kept and never believed
//! ```
//!
//! # Set aside, never deleted
//!
//! ADR 0026 § 8: *"A table that fails its check grants nothing. It is set
//! aside, not deleted, and the person is told."* It is renamed out of the way,
//! and the table starts empty: every site asks again. [`Opened::SetAside`]
//! carries the sentence the person is told.
//!
//! # Private, and with application data
//!
//! The directory is private to its owner, as every file this browser keeps for
//! a person is (`alo_net::private`). It is beside storage rather than beside the
//! cache, because a system freeing space must not take a person's answers with
//! it.
//!
//! # This runs in the browser process
//!
//! ADR 0005: a renderer holds no grant and never this directory.

use crate::file::{self, Saved};
use alo_net::private::{for_profile, make_the_directory_private, read_at_most, write_privately};
use std::fs;
use std::path::{Path, PathBuf};

/// What the table is kept in.
const TABLE: &str = "table";

/// What the table is called until the rename makes it the table.
const WRITING: &str = "table.writing";

/// What a set-aside table carries after its name.
const ASIDE: &str = "aside";

/// The most bytes a table may be.
///
/// Far above any honest table: a row is a few hundred bytes and a key's
/// history at most sixty-four entries of a few dozen, so this is tens of
/// thousands of sites with a full history each. A file larger than this was
/// not written by this engine, and reading it would be allocating on a
/// stranger's say-so.
pub const LARGEST: u64 = 256 * 1024 * 1024;

/// A grant table's directory.
#[derive(Debug)]
pub struct Disk {
    directory: PathBuf,
}

/// What was found on opening one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// No table: a profile that has never kept an answer.
    Nothing,
    /// A table that passed its check.
    Table(Saved),
    /// A table that failed its check, set aside, and why.
    SetAside(String),
}

impl Disk {
    /// The grant table's directory at this path, made private to its owner.
    ///
    /// # Errors
    ///
    /// A sentence, when it cannot be made or made private. A caller that gets
    /// one keeps nothing on a disk rather than keeping it unprotected.
    pub fn at(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = directory.as_ref().to_path_buf();
        make_the_directory_private(&directory)?;
        // A crash between writing and renaming leaves this. It was never the
        // table, and the table beside it is whole.
        let _ = fs::remove_file(directory.join(WRITING));
        Ok(Self { directory })
    }

    /// Where this is.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The table, read as a stranger's and set aside whole when it fails.
    pub fn open(&self) -> Opened {
        let path = self.directory.join(TABLE);
        if !path.exists() {
            return Opened::Nothing;
        }
        let read = read_at_most(&path, LARGEST)
            .ok_or_else(|| "a grant table larger than this engine writes, or unreadable".to_owned())
            .and_then(|bytes| file::decode(&bytes).map_err(|why| why.why));
        match read {
            Ok(saved) => Opened::Table(saved),
            Err(why) => Opened::SetAside(self.put_aside(&path, why)),
        }
    }

    /// Write the table, replacing what was there.
    ///
    /// # Errors
    ///
    /// A sentence, when the filesystem refuses. The table that was there is
    /// still there: the new one is written beside it and renamed over it.
    pub fn write(&self, saved: &Saved) -> Result<(), String> {
        let beside = self.directory.join(WRITING);
        let done = write_privately(&beside, &file::encode(saved))
            .and_then(|()| fs::rename(&beside, self.directory.join(TABLE)));
        done.map_err(|why| {
            let _ = fs::remove_file(&beside);
            format!("the grant table could not be written: {why}")
        })
    }

    /// How many tables are set aside here, for a test or a settings page to
    /// ask.
    pub fn set_aside_count(&self) -> usize {
        let Ok(listing) = fs::read_dir(&self.directory) else {
            return 0;
        };
        listing
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| is_set_aside(name))
            .count()
    }

    /// Rename a table out of the way, and say so.
    fn put_aside(&self, path: &Path, why: String) -> String {
        let mut n: u64 = 0;
        loop {
            let aside = self.directory.join(format!("{TABLE}.{ASIDE}.{n}"));
            if !aside.exists() {
                return if fs::rename(path, &aside).is_ok() {
                    why
                } else {
                    format!("{why}, and it could not be moved out of the way")
                };
            }
            match n.checked_add(1) {
                Some(next) => n = next,
                None => return format!("{why}, and there was nowhere to move it"),
            }
        }
    }
}

/// Where this operating system keeps application data, and this profile's
/// grant table inside it.
///
/// [`None`] when there is nowhere to put it: no home directory, or a profile
/// name that is not a single plain path component.
pub fn where_the_system_keeps_grants(profile: &str) -> Option<PathBuf> {
    for_profile(&alo_net::private::application_data()?, profile, "grants")
}

/// Whether a name is a set-aside table's.
fn is_set_aside(name: &str) -> bool {
    let mut parts = name.splitn(3, '.');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(TABLE), Some(ASIDE), Some(n)) if n.parse::<u64>().is_ok()
    )
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::capability::Capability;

    fn somewhere(called: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!(
            "alo-grants-disk-{}-{called}",
            std::process::id().wrapping_mul(2_654_435_761)
        ));
        let _ = fs::remove_dir_all(&place);
        place
    }

    fn a_table() -> Saved {
        Saved {
            everywhere: [Capability::Notifications].into_iter().collect(),
            ..Saved::default()
        }
    }

    #[test]
    fn the_table_and_its_directory_are_readable_by_nobody_else() {
        use std::os::unix::fs::PermissionsExt;
        let place = somewhere("private");
        let disk = Disk::at(&place).expect("a directory");
        assert_eq!(disk.open(), Opened::Nothing);
        disk.write(&a_table()).expect("a write");
        let mode = |path: &Path| fs::metadata(path).expect("there").permissions().mode() & 0o777;
        assert_eq!(mode(&place), 0o700);
        assert_eq!(mode(&place.join(TABLE)), 0o600);
        assert_eq!(disk.open(), Opened::Table(a_table()));
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_table_that_fails_its_check_is_set_aside_and_never_deleted() {
        let place = somewhere("aside");
        let disk = Disk::at(&place).expect("a directory");
        for round in 0..2 {
            fs::write(place.join(TABLE), b"alogrant and then rubbish").expect("damage");
            let Opened::SetAside(why) = disk.open() else {
                panic!("a damaged table was read in round {round}");
            };
            assert!(!why.is_empty());
            assert!(!place.join(TABLE).exists());
        }
        assert_eq!(disk.set_aside_count(), 2);
        assert_eq!(
            fs::read(place.join("table.aside.1")).expect("kept"),
            b"alogrant and then rubbish"
        );
        assert_eq!(disk.open(), Opened::Nothing);
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn what_a_crash_left_half_written_is_removed_and_the_table_is_whole() {
        let place = somewhere("crash");
        let disk = Disk::at(&place).expect("a directory");
        disk.write(&a_table()).expect("a write");
        fs::write(place.join(WRITING), b"half").expect("a leftover");
        let disk = Disk::at(&place).expect("the same directory");
        assert!(!place.join(WRITING).exists());
        assert_eq!(disk.open(), Opened::Table(a_table()));
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn grants_are_kept_with_application_data_beside_storage() {
        let Some(grants) = where_the_system_keeps_grants("default") else {
            return;
        };
        assert!(grants.ends_with("alo-browser/default/grants"));
        assert_eq!(
            grants.parent(),
            alo_storage::where_the_system_keeps_storage("default")
                .as_deref()
                .and_then(Path::parent)
        );
        assert_eq!(where_the_system_keeps_grants("../elsewhere"), None);
        assert!(is_set_aside("table.aside.0"));
        assert!(!is_set_aside("table.aside"));
        assert!(!is_set_aside("notes.aside.0"));
    }
}
