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
//! Mode 0 still leaves Chrome asking the machine's nameservers through a DNS
//! client of its own; that is switched off by a flag rather than this file,
//! so that every name is asked of the system's resolver ([`crate::chrome`],
//! item 650).
//!
//! Encrypted DNS as somebody's choice — ADR 0008's named setting — is not
//! built on this path; when it is, it is this file's to write.
//!
//! # Held while its browser lives, and removed by the next start if not
//!
//! A browser that is quit removes its profile ([`Profile`]'s drop, item
//! 688). One that is killed, crashes or loses its power removes nothing, and
//! its profile stays in the temporary directory (item 690). So each profile
//! holds a file, [`LOCK_FILE`], locked by the browser that made it for as
//! long as that browser lives — made as [`MAKING_FILE`], locked, then
//! renamed, so a file only ever has the lock's name once it is locked — and
//! let go only after the directory is removed, or by the kernel as the
//! browser dies (ADR 0069 § 1).
//!
//! Every profile made then sweeps the directory it was made in
//! ([`Swept`]): another `alo-chrome-*` directory, not a link and of the same
//! owner, whose [`LOCK_FILE`] is a file this process can lock, is a dead
//! browser's, and is removed while the lock is held. A living browser's lock
//! refuses — another process's, or another of this one's, since two opens
//! of one file refuse each other — and a directory with no [`LOCK_FILE`] is
//! never removed: one being made, or one a build before this left, about
//! which nothing says whether its browser is alive (ADR 0069 § 2). Measured
//! on macOS 26.5.2 before it was decided: a `SIGKILL`ed holder's lock is let
//! go, a rename keeps it, and Chrome, a child, does not inherit it.

use std::fs::File;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// What Chrome finds in its `Local State` the first time it starts: its own
/// encrypted DNS turned off, so that it asks the machine's resolver and no
/// other (ADR 0008).
pub const LOCAL_STATE: &str = r#"{"dns_over_https":{"mode":"off"}}"#;

/// The name Chrome gives that file, in the top of the profile.
pub const LOCAL_STATE_FILE: &str = "Local State";

/// The file a profile's browser holds locked for as long as it lives.
pub const LOCK_FILE: &str = "alo.lock";

/// What [`LOCK_FILE`] is called until it is locked.
pub const MAKING_FILE: &str = "alo.making";

/// What every profile's directory is named beginning with.
pub const NAMED: &str = "alo-chrome-";

/// One Chrome's profile, held: a directory only this user can read, with
/// [`LOCAL_STATE`] written in it and [`LOCK_FILE`] locked.
///
/// Dropped, the directory is removed and the lock let go after it, so no
/// other browser's sweep can take a profile this one is still removing.
#[derive(Debug)]
pub struct Profile {
    path: PathBuf,
    swept: Swept,
    /// [`LOCK_FILE`], locked while this value lives: never read, only held,
    /// and dropped after [`Drop::drop`] has removed the directory.
    _held: File,
}

impl Profile {
    /// Where it is: what Chrome is given as its `--user-data-dir`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What making it removed of browsers that died, and what it could not.
    pub fn swept(&self) -> &Swept {
        &self.swept
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        // The lock is let go after this, as the value's fields are dropped.
        let _ = remove_lock_last(&self.path);
    }
}

/// What a profile's making found of profiles a dead browser left
/// (ADR 0069 §§ 2–3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Swept {
    /// The profiles removed, which no living browser held.
    pub removed: Vec<PathBuf>,
    /// The profiles no living browser held that could not be removed, and
    /// why — left for the next start, whose sweep finds them again.
    pub unremoved: Vec<(PathBuf, ErrorKind)>,
}

/// A new directory only this user can read, for one Chrome's profile, with
/// [`LOCAL_STATE`] written in it and [`LOCK_FILE`] locked — after which
/// every profile in the temporary directory that no living browser holds is
/// removed, and said in [`Profile::swept`].
///
/// Made rather than reused: a directory that is already there may be one
/// somebody else put there for Chrome to read. A directory made and then not
/// written or not locked is removed before the error is answered. What the
/// sweep could not remove does not fail the making.
///
/// # Errors
///
/// What making the directory, writing the file or taking the lock failed
/// with.
pub fn new_profile() -> std::io::Result<Profile> {
    new_profile_in(&std::env::temp_dir())
}

/// [`new_profile`], made in and sweeping `directory`.
fn new_profile_in(directory: &Path) -> std::io::Result<Profile> {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let path = directory.join(format!(
        "{NAMED}{}-{}-{since}",
        std::process::id(),
        MADE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    let held = match write_local_state(&path).and_then(|()| hold(&path)) {
        Ok(held) => held,
        Err(why) => {
            let _ = std::fs::remove_dir_all(&path);
            return Err(why);
        }
    };
    let swept = sweep(directory, &path);
    Ok(Profile {
        path,
        swept,
        _held: held,
    })
}

/// Make the profile at `profile`'s [`LOCK_FILE`], locked: made under
/// another name, locked, and only then given the lock's.
fn hold(profile: &Path) -> std::io::Result<File> {
    let making = profile.join(MAKING_FILE);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&making)?;
    file.try_lock().map_err(std::io::Error::from)?;
    std::fs::rename(&making, profile.join(LOCK_FILE))?;
    Ok(file)
}

/// Remove every profile in `directory` but `own` that no living browser
/// holds.
fn sweep(directory: &Path, own: &Path) -> Swept {
    let mut swept = Swept::default();
    // The profile just made is this user's by construction, so whose a
    // directory is can be asked without asking who this user is.
    let Ok(owner) = std::fs::symlink_metadata(own).map(|about| about.uid()) else {
        return swept;
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return swept;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let named = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(NAMED));
        if !named || path == own {
            continue;
        }
        match removed_if_nobodys(&path, owner) {
            Found::Removed => swept.removed.push(path),
            Found::Left => {}
            Found::Unremoved(kind) => swept.unremoved.push((path, kind)),
        }
    }
    swept
}

/// What became of one directory a sweep looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    /// A dead browser's profile, removed.
    Removed,
    /// Somebody's: a living browser's, another user's, not a profile at
    /// all, or gone already.
    Left,
    /// A dead browser's profile, which could not be removed.
    Unremoved(ErrorKind),
}

/// Remove `profile` if it is a directory of `owner`'s whose [`LOCK_FILE`]
/// nobody holds — holding the lock while it is removed, so two starts
/// sweeping at once cannot both remove it.
fn removed_if_nobodys(profile: &Path, owner: u32) -> Found {
    // Its own entry, never what a link names.
    let Ok(about) = std::fs::symlink_metadata(profile) else {
        return Found::Left;
    };
    if !about.file_type().is_dir() || about.uid() != owner {
        return Found::Left;
    }
    // The directory is this user's and only this user's, so nobody else can
    // put another file at this name between this look and the open.
    let lock = profile.join(LOCK_FILE);
    let is_file = std::fs::symlink_metadata(&lock).is_ok_and(|about| about.file_type().is_file());
    if !is_file {
        return Found::Left;
    }
    let Ok(file) = File::open(&lock) else {
        return Found::Left;
    };
    if file.try_lock().is_err() {
        return Found::Left;
    }
    match remove_lock_last(profile) {
        Ok(()) => Found::Removed,
        // Its browser removed it and let go of its lock between the open and
        // the lock taken: it was never a dead browser's.
        Err(why) if why.kind() == ErrorKind::NotFound => Found::Left,
        Err(why) => Found::Unremoved(why.kind()),
    }
}

/// Remove the profile at `profile`, its [`LOCK_FILE`] last of all.
///
/// So a removal that fails part of the way — something in it that may not
/// be taken out, or a Chrome still ending that writes into it — never leaves
/// what is left without the file the next start's sweep looks for. Only the
/// directory's own removal can fail once the lock file is gone, and then a
/// new one is put back, held by nobody, for that sweep to find. Nothing is
/// followed out of it: a link in it is removed, never what it names.
fn remove_lock_last(profile: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(profile)? {
        let entry = entry?;
        if entry.file_name() == LOCK_FILE {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    let lock = profile.join(LOCK_FILE);
    match std::fs::remove_file(&lock) {
        Err(why) if why.kind() != ErrorKind::NotFound => return Err(why),
        _ => {}
    }
    std::fs::remove_dir(profile).inspect_err(|_| {
        let _ = std::fs::write(&lock, "");
    })
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

    /// A directory of its own for one test to make and sweep profiles in,
    /// named so that no real sweep looks inside it, and removed when done.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static MADE: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "alo-profile-test-{}-{}",
                std::process::id(),
                MADE.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            let made = std::fs::create_dir(&path);
            assert!(made.is_ok(), "{made:?}");
            Self(path)
        }

        /// A profile as a killed browser leaves it: its lock file there,
        /// held by nobody.
        fn dead(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            let made = std::fs::create_dir(&path)
                .and_then(|()| std::fs::write(path.join(LOCAL_STATE_FILE), LOCAL_STATE))
                .and_then(|()| std::fs::write(path.join(LOCK_FILE), ""));
            assert!(made.is_ok(), "{made:?}");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn names_in(directory: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(directory)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn a_new_profile_is_private_and_holds_only_the_local_state_and_its_lock() {
        let made = new_profile();
        assert!(made.is_ok(), "{made:?}");
        let Ok(profile) = made else { return };
        let path = profile.path().to_path_buf();
        let directory_mode =
            std::fs::metadata(&path).map(|about| about.permissions().mode() & 0o777);
        let names = names_in(&path);
        let written = std::fs::read_to_string(path.join(LOCAL_STATE_FILE));
        let file_mode = std::fs::metadata(path.join(LOCAL_STATE_FILE))
            .map(|about| about.permissions().mode() & 0o777);
        let lock_mode =
            std::fs::metadata(path.join(LOCK_FILE)).map(|about| about.permissions().mode() & 0o777);
        drop(profile);
        assert_eq!(directory_mode.ok(), Some(0o700));
        assert_eq!(
            names,
            vec![LOCAL_STATE_FILE.to_owned(), LOCK_FILE.to_owned()]
        );
        assert_eq!(written.ok().as_deref(), Some(LOCAL_STATE));
        assert_eq!(file_mode.ok(), Some(0o600));
        assert_eq!(lock_mode.ok(), Some(0o600));
        assert!(!path.exists(), "dropped, the profile is removed");
    }

    #[test]
    fn two_profiles_are_two_directories() {
        let (first, second) = (new_profile(), new_profile());
        let (Ok(first), Ok(second)) = (first, second) else {
            panic!("a profile was not made");
        };
        assert_ne!(first.path(), second.path());
    }

    #[test]
    fn a_profile_that_already_has_a_local_state_is_not_written_over() {
        let made = new_profile();
        let Ok(profile) = made else {
            panic!("a profile was not made: {made:?}");
        };
        let again = write_local_state(profile.path());
        assert_eq!(
            again.map_err(|why| why.kind()),
            Err(std::io::ErrorKind::AlreadyExists)
        );
    }

    #[test]
    fn a_living_profiles_lock_is_held_and_refuses_another_open() {
        let scratch = Scratch::new();
        let made = new_profile_in(&scratch.0);
        let Ok(profile) = made else {
            panic!("a profile was not made: {made:?}");
        };
        let opened = File::open(profile.path().join(LOCK_FILE));
        let Ok(opened) = opened else {
            panic!("the lock was not there: {opened:?}");
        };
        assert!(
            matches!(opened.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
            "a living profile's lock is held"
        );
        assert!(!profile.path().join(MAKING_FILE).exists());
    }

    #[test]
    fn a_dead_browsers_profile_is_removed_and_a_living_ones_is_left() {
        let scratch = Scratch::new();
        let living = new_profile_in(&scratch.0);
        let Ok(living) = living else {
            panic!("a profile was not made: {living:?}");
        };
        let dead = scratch.dead("alo-chrome-1-0-1");
        let made = new_profile_in(&scratch.0);
        let Ok(made) = made else {
            panic!("a profile was not made: {made:?}");
        };
        assert_eq!(
            made.swept(),
            &Swept {
                removed: vec![dead.clone()],
                unremoved: Vec::new(),
            }
        );
        assert!(!dead.exists(), "the dead browser's profile is removed");
        assert!(
            living.path().join(LOCK_FILE).exists(),
            "the living one's is left"
        );
        assert!(made.path().join(LOCK_FILE).exists(), "and its own");
        // The living one swept before the dead one was there, and found
        // nothing.
        assert_eq!(living.swept(), &Swept::default());
    }

    #[test]
    fn what_is_not_a_held_profile_is_never_removed() {
        let scratch = Scratch::new();
        let outside = Scratch::new();
        // A dead profile, but outside this directory and reached by a link.
        let far = outside.dead("alo-chrome-far");
        let linked = std::os::unix::fs::symlink(&far, scratch.0.join("alo-chrome-a-link"));
        assert!(linked.is_ok(), "{linked:?}");
        // Being made, or made by a build before this: no lock file.
        let unlocked = scratch.0.join("alo-chrome-no-lock");
        let making = scratch.0.join("alo-chrome-making");
        let made = std::fs::create_dir(&unlocked)
            .and_then(|()| std::fs::create_dir(&making))
            .and_then(|()| std::fs::write(making.join(MAKING_FILE), ""));
        assert!(made.is_ok(), "{made:?}");
        // A lock that is a link, and a lock that is a directory.
        let lock_linked = scratch.0.join("alo-chrome-lock-a-link");
        let lock_directory = scratch.0.join("alo-chrome-lock-a-directory");
        let made = std::fs::create_dir(&lock_linked)
            .and_then(|()| {
                std::os::unix::fs::symlink(far.join(LOCK_FILE), lock_linked.join(LOCK_FILE))
            })
            .and_then(|()| std::fs::create_dir_all(lock_directory.join(LOCK_FILE)));
        assert!(made.is_ok(), "{made:?}");
        // A file with a profile's name, and a dead profile by another name.
        let made = std::fs::write(scratch.0.join("alo-chrome-a-file"), "");
        assert!(made.is_ok(), "{made:?}");
        scratch.dead("not-alo-chrome-dead");
        let before = names_in(&scratch.0);

        let made = new_profile_in(&scratch.0);
        let Ok(made) = made else {
            panic!("a profile was not made: {made:?}");
        };
        assert_eq!(made.swept(), &Swept::default());
        let own = made
            .path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let mut after = names_in(&scratch.0);
        after.retain(|name| Some(name) != own.as_ref());
        assert_eq!(after, before, "nothing but its own was added or taken");
        assert!(far.join(LOCK_FILE).exists(), "what a link names is left");
    }

    #[test]
    fn a_dead_profile_that_cannot_be_removed_is_said_and_does_not_stop_the_start() {
        let scratch = Scratch::new();
        let dead = scratch.dead("alo-chrome-stuck");
        // A directory inside it that nothing may be taken out of.
        let stuck = dead.join("Default");
        let made = std::fs::create_dir(&stuck)
            .and_then(|()| std::fs::write(stuck.join("Cookies"), ""))
            .and_then(|()| {
                std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o500))
            });
        assert!(made.is_ok(), "{made:?}");
        let made = new_profile_in(&scratch.0);
        let _ = std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o700));
        let Ok(made) = made else {
            panic!("a profile was not made: {made:?}");
        };
        assert_eq!(
            made.swept(),
            &Swept {
                removed: Vec::new(),
                unremoved: vec![(dead.clone(), ErrorKind::PermissionDenied)],
            }
        );
        assert!(
            dead.join(LOCK_FILE).exists(),
            "left for the next start, with its lock file"
        );
        // Which, once it can, removes it.
        let again = new_profile_in(&scratch.0);
        let Ok(again) = again else {
            panic!("a profile was not made: {again:?}");
        };
        assert_eq!(again.swept().removed, vec![dead.clone()]);
        assert!(!dead.exists());
    }

    #[test]
    fn a_dead_profile_emptied_but_not_removed_keeps_a_lock_file_for_the_next_start() {
        let scratch = Scratch::new();
        let living = new_profile_in(&scratch.0);
        let Ok(living) = living else {
            panic!("a profile was not made: {living:?}");
        };
        let dead = scratch.dead("alo-chrome-emptied");
        // Everything inside the dead profile may be taken out, and the
        // directory itself may not be.
        let closed = std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o500));
        assert!(closed.is_ok(), "{closed:?}");
        let swept = sweep(&scratch.0, living.path());
        let _ = std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o700));
        assert_eq!(
            swept,
            Swept {
                removed: Vec::new(),
                unremoved: vec![(dead.clone(), ErrorKind::PermissionDenied)],
            }
        );
        assert_eq!(names_in(&dead), vec![LOCK_FILE.to_owned()]);
        let again = sweep(&scratch.0, living.path());
        assert_eq!(again.removed, vec![dead.clone()]);
        assert!(!dead.exists());
    }

    #[test]
    fn two_starts_at_once_remove_a_dead_profile_once() {
        let scratch = Scratch::new();
        let dead = scratch.dead("alo-chrome-raced");
        let directory = scratch.0.clone();
        let starts: Vec<_> = (0..8)
            .map(|_| {
                let directory = directory.clone();
                std::thread::spawn(move || {
                    new_profile_in(&directory).map(|made| made.swept().clone())
                })
            })
            .collect();
        let swept: Vec<_> = starts
            .into_iter()
            .map(std::thread::JoinHandle::join)
            .collect();
        let mut removed = Vec::new();
        for one in swept {
            let Ok(Ok(one)) = one else {
                panic!("a start failed: {one:?}");
            };
            assert_eq!(one.unremoved, Vec::new());
            removed.extend(one.removed);
        }
        // The other starts' own profiles may be dead by the time a later one
        // sweeps, since each is dropped as its thread ends — but each is
        // removed once at most, and the dead one exactly once.
        assert_eq!(removed.iter().filter(|path| **path == dead).count(), 1);
        let mut unique = removed.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), removed.len(), "{removed:?}");
        assert!(!dead.exists());
    }
}
