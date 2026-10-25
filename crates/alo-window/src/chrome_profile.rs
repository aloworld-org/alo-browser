/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The profile one Chrome is started on, and what it holds before Chrome
//! reads it (ADR 0058 § 3, ADR 0008, queue item 635).
//!
//! Each Chrome runs on a new, private, empty profile, never the person's
//! ([`crate::chrome`]). Empty is not quite what Chrome is given: one file is
//! written in it first, `Local State`, which is where Chrome keeps what holds
//! for the whole browser rather than one person's profile — and where it
//! reads which resolver it uses.
//!
//! # The machine's resolver, never silently replaced
//!
//! Measured against Chrome 155.0.8059.86 on this machine, from Chrome's own
//! net log: on a profile nobody has touched, Chrome's DNS configuration is in
//! **secure DNS mode 1, automatic** — every lookup asked in that mode. In it
//! Chrome sends a name to the resolver the machine names, unless that
//! resolver is one Chrome recognises as also answering encrypted DNS, when it
//! sends the name there encrypted instead, and back in the clear if that
//! fails. Nobody chose that, and nobody is told when it happens: the silent
//! replacement, and the silent falling back, that ADR 0008 forbids. This
//! machine's resolver is its router, which Chrome does not recognise, so here
//! automatic mode had nothing to upgrade to; the mode is what is pinned,
//! because the resolver a machine names changes with the network it joins.
//!
//! [`LOCAL_STATE`] sets `dns_over_https.mode` to `off`, and Chrome's
//! configuration is then in mode 0 and every lookup asked in it — measured
//! the same way, and asserted by `tests/chromes_resolver_is_the_machines.rs`.
//! The same file set to `secure`, with a resolver's address, puts Chrome in
//! mode 2 and asks that resolver, so it is the file Chrome reads rather than
//! a default that happened to agree. `--disable-features=DnsOverHttps`, the
//! flag's name in older Chromes, changed nothing in 155.
//!
//! Encrypted DNS as somebody's choice — ADR 0008's named setting — is not
//! built on this path; when it is, it is this file's to write.

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// What Chrome finds in its `Local State` the first time it starts: its own
/// encrypted DNS turned off, so that it asks the machine's resolver and no
/// other (ADR 0008).
pub const LOCAL_STATE: &str = r#"{"dns_over_https":{"mode":"off"}}"#;

/// The name Chrome gives that file, in the top of the profile.
pub const LOCAL_STATE_FILE: &str = "Local State";

/// A new directory only this user can read, for one Chrome's profile, with
/// [`LOCAL_STATE`] written in it.
///
/// Made rather than reused: a directory that is already there may be one
/// somebody else put there for Chrome to read. A directory made and then not
/// written is removed before the error is answered.
///
/// # Errors
///
/// What making the directory or writing the file failed with.
pub fn new_profile() -> std::io::Result<PathBuf> {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "alo-chrome-{}-{}-{since}",
        std::process::id(),
        MADE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    if let Err(why) = write_local_state(&path) {
        let _ = std::fs::remove_dir_all(&path);
        return Err(why);
    }
    Ok(path)
}

/// Write [`LOCAL_STATE`] into the profile at `profile`, which must not have
/// one already.
fn write_local_state(profile: &Path) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(profile.join(LOCAL_STATE_FILE))?;
    file.write_all(LOCAL_STATE.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_new_profile_is_private_and_holds_only_the_local_state() {
        let made = new_profile();
        assert!(made.is_ok(), "{made:?}");
        let Ok(profile) = made else { return };
        let directory_mode =
            std::fs::metadata(&profile).map(|about| about.permissions().mode() & 0o777);
        let names: Vec<_> = std::fs::read_dir(&profile)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name())
                    .collect()
            })
            .unwrap_or_default();
        let written = std::fs::read_to_string(profile.join(LOCAL_STATE_FILE));
        let file_mode = std::fs::metadata(profile.join(LOCAL_STATE_FILE))
            .map(|about| about.permissions().mode() & 0o777);
        let _ = std::fs::remove_dir_all(&profile);
        assert_eq!(directory_mode.ok(), Some(0o700));
        assert_eq!(names, vec![std::ffi::OsString::from(LOCAL_STATE_FILE)]);
        assert_eq!(written.ok().as_deref(), Some(LOCAL_STATE));
        assert_eq!(file_mode.ok(), Some(0o600));
    }

    #[test]
    fn two_profiles_are_two_directories() {
        let (first, second) = (new_profile(), new_profile());
        let (Ok(first), Ok(second)) = (first, second) else {
            panic!("a profile was not made");
        };
        let _ = std::fs::remove_dir_all(&first);
        let _ = std::fs::remove_dir_all(&second);
        assert_ne!(first, second);
    }

    #[test]
    fn a_profile_that_already_has_a_local_state_is_not_written_over() {
        let made = new_profile();
        let Ok(profile) = made else {
            panic!("a profile was not made: {made:?}");
        };
        let again = write_local_state(&profile);
        let _ = std::fs::remove_dir_all(&profile);
        assert_eq!(
            again.map_err(|why| why.kind()),
            Err(std::io::ErrorKind::AlreadyExists)
        );
    }
}
