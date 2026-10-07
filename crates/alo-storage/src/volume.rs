/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How much of a volume is free, asked of the operating system.
//!
//! ADR 0025 § 3: the profile's bound is the smaller of 8 GiB and *"a fifth of
//! the volume's free space when the browser starts"*. The standard library has
//! no call for that number, and law 4 forbids writing the system call here, so
//! it is rented: `rustix`'s `statvfs`, a safe function over the C library's
//! (or, on Linux, the kernel's) own call. The `unsafe` that makes the call is
//! inside `rustix` — ADR 0010's *the crate's, not ours* — and this is the one
//! file that names it (`scripts/gate.sh`).
//!
//! # Free means free to us
//!
//! The figure is the blocks available to a process that is not the
//! superuser, times the size the volume counts them in: what `df` calls
//! *available*, not the larger *free*, which includes the blocks a volume
//! keeps back for its administrator. A browser that sized itself by space it
//! could never write would be sizing itself by a promise the disk will not
//! keep.
//!
//! # A volume that cannot be asked has nothing free
//!
//! A path that is not there, a platform without the call, and a figure too
//! large to be a number of bytes all give [`None`], and the profile's bound
//! built from that is zero (`Limits::for_the_volume_at`), which refuses every
//! write. It is never an unbounded one: a store that could not measure its
//! disk must not be the store that fills it.

use std::path::Path;

/// The bytes free to this process on the volume this path is on, or [`None`]
/// when the volume cannot be asked.
#[cfg(unix)]
pub fn free_space(path: &Path) -> Option<u64> {
    let volume = rustix::fs::statvfs(path).ok()?;
    volume.f_bavail.checked_mul(volume.f_frsize)
}

/// As above, on a platform this crate cannot ask: never a figure.
#[cfg(not(unix))]
pub fn free_space(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_is_not_there_has_no_volume_to_ask() {
        let nowhere = std::env::temp_dir().join("alo-storage-volume-nowhere/not/here");
        assert_eq!(free_space(&nowhere), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_that_is_there_has_a_figure() {
        assert!(free_space(&std::env::temp_dir()).is_some());
    }
}
