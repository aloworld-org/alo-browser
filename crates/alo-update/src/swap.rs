/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Two bundles exchanged in one call, so that either the old application is
//! at its path or the new one is, never neither (ADR 0116 § 3).
//!
//! # Rented, behind this file
//!
//! `rustix::fs::renameat_with(…, RenameFlags::EXCHANGE)`: `renameatx_np`
//! with `RENAME_SWAP` on macOS, `renameat2` with `RENAME_EXCHANGE` on
//! Linux — a safe function, whose `unsafe` is the crate's, not ours (ADR
//! 0010). `rustix` is already in the workspace for `statvfs` (ADR 0025 §
//! 3); this is the second file that names it. No `unsafe` of alo's.

use std::path::Path;

/// Exchange what is at `a` with what is at `b`, atomically: both must be
/// there, on one volume.
///
/// # Errors
///
/// Why not, in words; nothing is moved then.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn exchange(a: &Path, b: &Path) -> Result<(), String> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        a,
        rustix::fs::CWD,
        b,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|why| {
        format!(
            "{} and {} could not be exchanged ({why})",
            a.display(),
            b.display()
        )
    })
}

/// As above, on a platform that has no exchange: never.
///
/// # Errors
///
/// Always: there is no way to swap here without a moment of neither.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn exchange(a: &Path, b: &Path) -> Result<(), String> {
    Err(format!(
        "{} and {} cannot be exchanged on this platform",
        a.display(),
        b.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn two_folders_are_exchanged_whole_and_a_missing_one_moves_nothing() {
        let root = std::env::temp_dir().join(format!("alo-update-swap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (old, new) = (
            root.join("alo browser.app"),
            root.join(".alo-update/0.4.2/alo browser.app"),
        );
        fs::create_dir_all(&old).expect("made");
        fs::create_dir_all(&new).expect("made");
        fs::write(old.join("which"), "old").expect("written");
        fs::write(new.join("which"), "new").expect("written");
        exchange(&new, &old).expect("exchanged");
        assert_eq!(
            fs::read_to_string(old.join("which")).ok().as_deref(),
            Some("new")
        );
        assert_eq!(
            fs::read_to_string(new.join("which")).ok().as_deref(),
            Some("old")
        );
        let refused = exchange(&root.join("not here"), &old);
        assert!(refused.is_err());
        assert_eq!(
            fs::read_to_string(old.join("which")).ok().as_deref(),
            Some("new")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
