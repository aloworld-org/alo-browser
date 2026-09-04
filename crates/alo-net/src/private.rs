/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Files this browser keeps for one person, on their own disk, private to them.
//!
//! ADR 0011 section 3, which ADR 0012 § 6 then takes *unchanged* for a different
//! kind of file: *"the directory is created private to its owner, and so is every
//! file in it. Nothing in it is readable by another user account by default."*
//!
//! Two things in this crate keep something durably — the cache
//! ([`crate::disk`]) and what an agent did ([`crate::kept`]) — and that promise
//! is the same promise in both. It is here rather than in each of them because a
//! promise made twice is a promise that can come to be kept once.
//!
//! # The boundary, said plainly
//!
//! ADR 0011: *"against another user on the machine, these are protected; against
//! a program running as the person themselves, they are not."* Neither is
//! anything else that person owns, and pretending otherwise would be selling a
//! protection we do not have. **No encryption of ours** — ADR 0001 rents the
//! physics, disk encryption belongs to the operating system, and a key that has
//! to live next to the data is not a key.
//!
//! # A platform that cannot promise it gets no file
//!
//! On anything that is not Unix, both of these fail rather than doing the thing
//! and hoping. The honest answer is no cache and no durable record, not a
//! promise this engine does not keep.

use std::fs;
use std::path::Path;

/// Make this directory, and make it readable by nobody else.
///
/// # Errors
///
/// A sentence, when it cannot be made or cannot be made private. A caller that
/// gets one keeps nothing on a disk rather than keeping it unprotected.
#[cfg(unix)]
pub fn make_the_directory_private(directory: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !directory.is_dir() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|why| format!("the directory cannot be made: {why}"))?;
    }
    // Set rather than assume: a directory that already existed may have been
    // made by something else, and what is promised is that this one *is*
    // private rather than that we would have made it that way.
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
        .map_err(|why| format!("the directory cannot be made private: {why}"))
}

/// As above, on a platform that cannot say it.
///
/// # Errors
///
/// Always.
#[cfg(not(unix))]
pub fn make_the_directory_private(_directory: &Path) -> Result<(), String> {
    Err("this platform has no way to make a directory private to its owner".to_owned())
}

/// Write these bytes to a file readable by nobody else, and get them onto the
/// disk before returning.
///
/// # Errors
///
/// Whatever the filesystem says.
#[cfg(unix)]
pub fn write_privately(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    // The bytes, then the rename. Without this the rename can land before the
    // contents do, and what survives a power cut is a file full of zeroes —
    // which a checksum would catch, and which there is no reason to create.
    file.sync_all()
}

/// As above, on a platform that cannot say it.
///
/// # Errors
///
/// Always.
#[cfg(not(unix))]
pub fn write_privately(_path: &Path, _bytes: &[u8]) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "this platform has no way to make a file private to its owner",
    ))
}

/// A file's contents, when it is a file and is not larger than this.
///
/// The length is asked of the filesystem **before anything is allocated**, for
/// the reason every length in [`crate::bytes`] is checked: a number somebody
/// else chose is not a size to reserve. [`None`] for anything else at all,
/// because every way of failing to read one of these has the same answer.
pub fn read_at_most(path: &Path, most: u64) -> Option<Vec<u8>> {
    let about = fs::metadata(path).ok()?;
    if !about.is_file() || about.len() > most {
        return None;
    }
    fs::read(path).ok()
}

/// This profile's directory of a kind of thing, inside a place the system keeps
/// that kind.
///
/// *Which* place is each caller's to choose, because the answer differs: a
/// cache belongs where the system keeps caches and a record a person is
/// entitled to consult does not (see [`crate::kept`]). What is shared is the
/// shape — `<somewhere>/alo-browser/<profile>/<kind>` — and the refusal below.
///
/// [`None`] for a profile name that is not a single plain path component. **A
/// profile name is not a place to be lenient**: one containing a separator would
/// put a person's files somewhere nobody chose.
pub fn for_profile(place: &Path, profile: &str, kind: &str) -> Option<std::path::PathBuf> {
    if profile.is_empty()
        || !profile
            .chars()
            .all(|glyph| glyph.is_ascii_alphanumeric() || glyph == '-' || glyph == '_')
    {
        return None;
    }
    Some(place.join("alo-browser").join(profile).join(kind))
}

/// The home directory, when this machine has one worth the name.
pub fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(std::path::PathBuf::from)
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn somewhere(called: &str) -> std::path::PathBuf {
        let place = std::env::temp_dir().join(format!(
            "alo-private-{}-{called}",
            std::process::id().wrapping_mul(2_654_435_761)
        ));
        let _ = fs::remove_dir_all(&place);
        place
    }

    #[test]
    fn a_directory_and_a_file_in_it_are_readable_by_nobody_else() {
        let place = somewhere("modes");
        make_the_directory_private(&place).expect("a directory");
        let file = place.join("something");
        write_privately(&file, b"what is kept").expect("a write");

        let directory = fs::metadata(&place).expect("the directory");
        assert_eq!(directory.permissions().mode() & 0o777, 0o700);
        let written = fs::metadata(&file).expect("the file");
        assert_eq!(written.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            read_at_most(&file, 1024).as_deref(),
            Some(&b"what is kept"[..])
        );
        let _ = fs::remove_dir_all(&place);
    }

    /// A directory somebody else made, or one this browser made before the
    /// promise existed, is made private rather than assumed to be.
    #[test]
    fn a_directory_that_was_already_there_is_made_private_anyway() {
        let place = somewhere("loose");
        fs::create_dir_all(&place).expect("a directory");
        fs::set_permissions(&place, fs::Permissions::from_mode(0o755)).expect("loosened");

        make_the_directory_private(&place).expect("a directory");

        let about = fs::metadata(&place).expect("the directory");
        assert_eq!(about.permissions().mode() & 0o777, 0o700);
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_file_larger_than_the_bound_is_not_read_at_all() {
        let place = somewhere("large");
        make_the_directory_private(&place).expect("a directory");
        let file = place.join("big");
        write_privately(&file, &vec![b'x'; 4096]).expect("a write");

        assert_eq!(read_at_most(&file, 1024), None);
        assert!(read_at_most(&file, 4096).is_some());
        assert_eq!(
            read_at_most(&place, 4096),
            None,
            "a directory is not a file"
        );
        assert_eq!(read_at_most(&place.join("nothing"), 4096), None);
        let _ = fs::remove_dir_all(&place);
    }

    /// A profile name is a single plain path component, and anything else is
    /// nowhere rather than somewhere surprising.
    #[test]
    fn a_profile_name_that_could_point_elsewhere_is_refused() {
        let place = std::path::PathBuf::from("/somewhere");
        assert_eq!(
            for_profile(&place, "default", "http"),
            Some(std::path::PathBuf::from(
                "/somewhere/alo-browser/default/http"
            )),
        );
        assert!(for_profile(&place, "a-person_2", "agent").is_some());
        for refused in ["", "..", "a/b", "a b", "~", "a\0b", "../../etc"] {
            assert_eq!(for_profile(&place, refused, "agent"), None, "{refused:?}");
        }
    }
}
