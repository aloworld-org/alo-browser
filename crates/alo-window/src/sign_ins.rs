/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The sign-ins kept on this Mac (ADR 0111, queue item 935): the cookies
//! the cards' Chromiums were holding, given to each new card's Chromium as
//! it starts and taken back from it as it runs
//! ([`crate::chrome_sign_ins`]).
//!
//! # What is kept, and where
//!
//! One jar for the browser process, shared by every card it opens. A
//! cookie that expires is kept on this Mac, **sealed** under the key in the
//! person's keychain ([`crate::sign_in_key`], [`crate::chat_seal::seal_as`])
//! in one file, [`FILE`], in a directory of alo's own under the person's
//! *Application Support* ([`for_this_person`]) — written whole, under
//! another name, then renamed over it. A session's cookie is kept while
//! alo runs and **never written** (ADR 0011 § 2): alo quit, it is gone, as
//! a browser's session cookies are. A cookie that has expired is dropped
//! as it is read. Nothing else — no site's storage, no cache, no history,
//! no password.
//!
//! # One alo at a time
//!
//! The browser process that keeps the sign-ins holds [`LOCK`] locked for
//! its life, as a Chromium holds its profile (ADR 0069 § 1). A second alo
//! opened meanwhile finds it held and keeps its own in memory alone
//! ([`Kept::ByAnother`]): it reads nothing the first wrote and writes
//! nothing over it, and says so.
//!
//! # Said, never hidden
//!
//! [`SignIns::kept_as`] says whether the jar is kept on this Mac, and if
//! not why: no key, another alo, or a file that does not open with the key
//! in this keychain — which is left as it is until *Forget all sign-ins*
//! removes it.

use crate::chat_seal::{self, Key};
use crate::kept_cookie::{Changes, KeptCookie};
use crate::keychain::Keychain;
use crate::sign_in_key;
use std::fs::File;
use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The sealed file the sign-ins are kept in.
pub const FILE: &str = "sign-ins.sealed";

/// What the file is written as before it is renamed over [`FILE`].
pub const WRITING: &str = "sign-ins.writing";

/// The file the browser process keeping them holds locked.
pub const LOCK: &str = "sign-ins.lock";

/// What the sealed file begins with: its format, and its version.
pub const MAGIC: &[u8] = b"alo sealed sign-ins 1\n";

/// The first line of what is sealed.
const HEADER: &str = "alo sign-ins 1";

/// The most cookies kept: Chromium's own store holds at most 3,300.
pub const MOST_COOKIES: usize = 4000;

/// The directory alo's own sign-ins are kept in, under *Application
/// Support* — beside the chats' (`chat_store`'s), never in the repository
/// or the temporary directory.
pub const DIRECTORY: &str = "Sign-ins";

/// Where this person's sign-ins are kept, from `HOME`.
///
/// # Errors
///
/// Why there is nowhere: no `HOME`, or one that is not an absolute path.
pub fn for_this_person() -> Result<PathBuf, String> {
    let chats = crate::chat_store::for_this_person()?;
    let alos = chats
        .parent()
        .ok_or_else(|| "the chats' directory has nothing above it".to_owned())?;
    Ok(alos.join(DIRECTORY))
}

/// Whether the jar is kept on this Mac, and if not why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    /// Kept, sealed, in this directory.
    OnThisMac(PathBuf),
    /// Kept in memory alone, until alo quits: never asked for a disk (a
    /// test's, or `--chrome`'s).
    InMemory,
    /// Kept in memory alone: another alo that is open holds them.
    ByAnother,
    /// Kept in memory alone: there is no key, for this reason.
    NoKey(String),
    /// Kept in memory alone: the file there does not open with the key in
    /// this keychain, and is left as it is.
    Unopened,
    /// Kept in memory alone: the directory could not be made or read.
    Nowhere(String),
}

/// The sign-ins of one browser process.
#[derive(Debug)]
pub struct SignIns {
    held: Mutex<Held>,
    /// How many times every sign-in was forgotten: a card's Chromium that
    /// sees it change empties its own jar.
    forgotten: AtomicU64,
    /// The keychain and account whose key is replaced as every sign-in is
    /// forgotten, if there is one.
    rekey: Option<(Keychain, String)>,
}

/// What the lock guards.
#[derive(Debug)]
struct Held {
    /// Every cookie kept, a session's too.
    cookies: Vec<KeptCookie>,
    /// Where they are written, if they are.
    disk: Option<Disk>,
    /// Whether they are, and if not why.
    kept: Kept,
}

/// The directory a browser process keeps the sign-ins in, held.
#[derive(Debug)]
struct Disk {
    directory: PathBuf,
    key: Key,
    /// [`LOCK`], locked while this lives.
    _lock: File,
}

/// The seconds since 1970 now.
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64())
}

impl SignIns {
    /// A jar kept in memory alone, for as long as it lives.
    pub fn in_memory() -> Self {
        Self::holding(Vec::new(), None, Kept::InMemory)
    }

    fn holding(cookies: Vec<KeptCookie>, disk: Option<Disk>, kept: Kept) -> Self {
        Self {
            held: Mutex::new(Held {
                cookies,
                disk,
                kept,
            }),
            forgotten: AtomicU64::new(0),
            rekey: None,
        }
    }

    /// The sign-ins of this alo: kept in `named` when a hand run named
    /// somewhere (`--sign-ins DIR`), their key in a keychain file there
    /// ([`sign_in_key::scratch_keychain`]); else in this person's own
    /// directory ([`for_this_person`]), their key in their own keychain.
    pub fn for_this_alo(named: Option<&Path>) -> Self {
        let (directory, keychain) = match named {
            Some(named) => {
                let made = std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(named);
                let keychain = made
                    .map_err(|why| format!("{} could not be made ({why})", named.display()))
                    .and_then(|()| sign_in_key::scratch_keychain(named));
                (Ok(named.to_path_buf()), keychain)
            }
            None => (for_this_person(), Ok(Keychain::Theirs)),
        };
        let directory = match directory {
            Ok(directory) => directory,
            Err(why) => return Self::holding(Vec::new(), None, Kept::Nowhere(why)),
        };
        let account = crate::keychain::this_person();
        let key = keychain
            .clone()
            .and_then(|keychain| sign_in_key::in_keychain(&keychain, &account));
        let jar = Self::kept_in(&directory, key);
        match keychain {
            Ok(keychain) => jar.rekeyed_in(keychain, account),
            Err(_) => jar,
        }
    }

    /// The same, its key replaced in `keychain`'s item of `account` as
    /// every sign-in is forgotten ([`SignIns::forget_all`]).
    #[must_use]
    pub fn rekeyed_in(mut self, keychain: Keychain, account: String) -> Self {
        self.rekey = Some((keychain, account));
        self
    }

    /// The jar kept in `directory`, sealed under `key` — read as it was
    /// left — or, where that cannot be, one in memory alone that says why
    /// ([`SignIns::kept_as`]). Never fails: a sign-in not kept is said, and
    /// alo opens pages all the same.
    pub fn kept_in(directory: &Path, key: Result<Key, String>) -> Self {
        let made = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory);
        if let Err(why) = made {
            return Self::holding(Vec::new(), None, Kept::Nowhere(why.to_string()));
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(directory.join(LOCK));
        let lock = match lock {
            Ok(lock) => lock,
            Err(why) => return Self::holding(Vec::new(), None, Kept::Nowhere(why.to_string())),
        };
        match locked(&lock) {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Self::holding(Vec::new(), None, Kept::ByAnother);
            }
            Err(std::fs::TryLockError::Error(why)) => {
                return Self::holding(Vec::new(), None, Kept::Nowhere(why.to_string()));
            }
        }
        let key = match key {
            Ok(key) => key,
            Err(why) => return Self::holding(Vec::new(), None, Kept::NoKey(why)),
        };
        let disk = Disk {
            directory: directory.to_path_buf(),
            key,
            _lock: lock,
        };
        match read(&disk) {
            Ok(cookies) => {
                let kept = Kept::OnThisMac(disk.directory.clone());
                Self::holding(cookies, Some(disk), kept)
            }
            Err(Unread::Unopened) => Self::holding(Vec::new(), Some(disk), Kept::Unopened),
            Err(Unread::Io(why)) => Self::holding(Vec::new(), None, Kept::Nowhere(why)),
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether they are kept on this Mac, and if not why.
    pub fn kept_as(&self) -> Kept {
        self.held().kept.clone()
    }

    /// Every cookie kept that has not expired: what a new card's Chromium
    /// is given.
    pub fn cookies(&self) -> Vec<KeptCookie> {
        let at = now();
        self.held()
            .cookies
            .iter()
            .filter(|cookie| !cookie.has_expired(at))
            .cloned()
            .collect()
    }

    /// How many times every sign-in was forgotten.
    pub fn forgotten(&self) -> u64 {
        self.forgotten.load(Ordering::SeqCst)
    }

    /// Make `changes` — what a card's Chromium did — to the jar, and write
    /// what expires to the disk when anything of that changed.
    ///
    /// # Errors
    ///
    /// Why the disk was not written; the jar in memory is changed all the
    /// same.
    pub fn change(&self, changes: &Changes) -> Result<(), String> {
        self.change_as_of(changes, self.forgotten())
    }

    /// [`SignIns::change`], unless every sign-in was forgotten since the
    /// changes were seen — when the jar has been forgotten — as `forgotten`
    /// counted then: so a card's look that raced *Forget all* never puts
    /// back what was forgotten.
    ///
    /// # Errors
    ///
    /// As [`SignIns::change`].
    pub fn change_as_of(&self, changes: &Changes, forgotten: u64) -> Result<(), String> {
        if changes.is_empty() {
            return Ok(());
        }
        let mut held = self.held();
        // Forgetting counts while it holds the jar, so this is read under
        // the same lock.
        if self.forgotten() != forgotten {
            return Ok(());
        }
        let before: Vec<KeptCookie> = lasting(&held.cookies);
        changes.apply(&mut held.cookies);
        let at = now();
        held.cookies.retain(|cookie| !cookie.has_expired(at));
        if held.cookies.len() > MOST_COOKIES {
            let over = held.cookies.len() - MOST_COOKIES;
            held.cookies.drain(..over);
        }
        let after = lasting(&held.cookies);
        if before == after || !matches!(held.kept, Kept::OnThisMac(_)) {
            return Ok(());
        }
        match &held.disk {
            Some(disk) => write(disk, &after),
            None => Ok(()),
        }
    }

    /// Forget every sign-in ([`SignIns::forget_all_under`]), the key
    /// replaced in the keychain it was read from, when it was.
    ///
    /// # Errors
    ///
    /// As [`SignIns::forget_all_under`].
    pub fn forget_all(&self) -> Result<(), String> {
        let key = match &self.rekey {
            Some((keychain, account)) => sign_in_key::replaced(keychain, account).map(Some),
            None => Ok(None),
        };
        self.forget_all_under(key)
    }

    /// Forget every sign-in: the jar emptied, the file removed, and — when
    /// `key` is a new key in the keychain's — sealed under it from now on,
    /// so a copy of the old file never opens (ADR 0111 § 5); with no new
    /// key, under the one it had. A file that did not open is removed too,
    /// and the jar is kept on this Mac again.
    ///
    /// # Errors
    ///
    /// Why the file could not be removed, or the key not replaced; the jar
    /// in memory is emptied all the same.
    pub fn forget_all_under(&self, key: Result<Option<Key>, String>) -> Result<(), String> {
        let mut held = self.held();
        held.cookies.clear();
        self.forgotten.fetch_add(1, Ordering::SeqCst);
        let Some(disk) = held.disk.as_mut() else {
            return Ok(());
        };
        let removed = match std::fs::remove_file(disk.directory.join(FILE)) {
            Err(why) if why.kind() != ErrorKind::NotFound => Err(format!(
                "the sign-ins kept in {} could not be removed ({why})",
                disk.directory.display()
            )),
            _ => Ok(()),
        };
        let replaced = key.map(|key| {
            if let Some(key) = key {
                disk.key = key;
            }
        });
        if removed.is_ok() {
            held.kept = Kept::OnThisMac(disk.directory.clone());
        }
        removed.and(replaced.map_err(|why| {
            format!("the sign-ins' key was not replaced, so the old one seals them still: {why}")
        }))
    }
}

/// How long a lock held by somebody else is tried again before it is
/// taken for another alo's.
///
/// A child this process starts holds a copy of every descriptor between
/// its `fork` and its `exec`, the lock's among them, for a moment: a lock
/// let go of by this process can still refuse once. Measured as a test of
/// a jar read again failing once in a lib run of a thousand tests that
/// start programs (11 November 2026); a bound of ours, not a measured one.
pub const LOCK_PATIENCE: std::time::Duration = std::time::Duration::from_millis(500);

/// `lock` locked, tried again for [`LOCK_PATIENCE`] while somebody holds
/// it.
fn locked(lock: &File) -> Result<(), std::fs::TryLockError> {
    let until = std::time::Instant::now() + LOCK_PATIENCE;
    loop {
        match lock.try_lock() {
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// Of `cookies`, those that expire: what is written.
fn lasting(cookies: &[KeptCookie]) -> Vec<KeptCookie> {
    cookies
        .iter()
        .filter(|cookie| !cookie.is_a_sessions())
        .cloned()
        .collect()
}

/// Why the file was not read.
enum Unread {
    /// It does not open with this key.
    Unopened,
    /// It could not be read.
    Io(String),
}

/// What `disk`'s file holds that has not expired — nothing, when there is
/// none.
fn read(disk: &Disk) -> Result<Vec<KeptCookie>, Unread> {
    let bytes = match std::fs::read(disk.directory.join(FILE)) {
        Ok(bytes) => bytes,
        Err(why) if why.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => return Err(Unread::Io(why.to_string())),
    };
    let opened = chat_seal::open_as(&disk.key, MAGIC, &bytes).map_err(|_| Unread::Unopened)?;
    let text = String::from_utf8(opened).map_err(|_| Unread::Unopened)?;
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err(Unread::Unopened);
    }
    let at = now();
    Ok(lines
        .filter_map(KeptCookie::of_line)
        .filter(|cookie| !cookie.is_a_sessions() && !cookie.has_expired(at))
        .take(MOST_COOKIES)
        .collect())
}

/// Write `cookies` to `disk`'s file, sealed: whole, under [`WRITING`],
/// then renamed over it.
fn write(disk: &Disk, cookies: &[KeptCookie]) -> Result<(), String> {
    let mut text = String::from(HEADER);
    for cookie in cookies {
        text.push('\n');
        text.push_str(&cookie.line());
    }
    text.push('\n');
    let sealed = chat_seal::seal_as(&disk.key, MAGIC, text.as_bytes())
        .map_err(|why| format!("the sign-ins could not be sealed: {why}"))?;
    let writing = disk.directory.join(WRITING);
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&writing)
        .and_then(|mut file| {
            file.write_all(&sealed)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&writing, disk.directory.join(FILE)));
    written.map_err(|why| {
        format!(
            "the sign-ins could not be kept in {} ({why})",
            disk.directory.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_seal::KEY_BYTES;
    use crate::kept_cookie::{Priority, SameSite, Scheme};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("alo-sign-ins-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn key(byte: u8) -> Key {
        Key::of([byte; KEY_BYTES])
    }

    fn cookie(name: &str, expires: Option<f64>) -> KeptCookie {
        KeptCookie {
            name: name.to_owned(),
            value: format!("{name}-value"),
            domain: "127.0.0.1".to_owned(),
            path: "/".to_owned(),
            expires,
            http_only: false,
            secure: false,
            same_site: Some(SameSite::Lax),
            priority: Priority::Medium,
            source_scheme: Scheme::NonSecure,
            source_port: 80,
        }
    }

    fn made(cookies: &[KeptCookie]) -> Changes {
        Changes {
            gone: Vec::new(),
            made: cookies.to_vec(),
        }
    }

    fn names(cookies: &[KeptCookie]) -> Vec<String> {
        let mut names: Vec<_> = cookies.iter().map(|cookie| cookie.name.clone()).collect();
        names.sort();
        names
    }

    const LATER: Option<f64> = Some(4_000_000_000.0);

    #[test]
    fn what_expires_is_kept_sealed_across_a_restart_and_a_sessions_is_not() {
        let scratch = Scratch::new("restart");
        {
            let jar = SignIns::kept_in(&scratch.0, Ok(key(1)));
            assert_eq!(jar.kept_as(), Kept::OnThisMac(scratch.0.clone()));
            assert_eq!(jar.cookies(), Vec::new());
            let changed = jar.change(&made(&[cookie("lasting", LATER), cookie("session", None)]));
            assert_eq!(changed, Ok(()));
            assert_eq!(names(&jar.cookies()), vec!["lasting", "session"]);
        }
        let sealed = std::fs::read(scratch.0.join(FILE)).expect("written");
        assert!(sealed.starts_with(MAGIC));
        assert!(
            !sealed.windows(7).any(|window| window == b"lasting"),
            "nothing of it in the clear"
        );
        assert!(!scratch.0.join(WRITING).exists());
        let again = SignIns::kept_in(&scratch.0, Ok(key(1)));
        assert_eq!(names(&again.cookies()), vec!["lasting"]);
    }

    #[test]
    fn a_cookie_removed_by_its_site_is_removed_from_the_disk_and_an_expired_one_dropped() {
        let scratch = Scratch::new("removed");
        let jar = SignIns::kept_in(&scratch.0, Ok(key(2)));
        let first = cookie("first", LATER);
        let past = cookie("past", Some(1.0));
        assert_eq!(jar.change(&made(&[first.clone(), past])), Ok(()));
        assert_eq!(names(&jar.cookies()), vec!["first"]);
        let signed_out = Changes {
            gone: vec![first.named()],
            made: Vec::new(),
        };
        assert_eq!(jar.change(&signed_out), Ok(()));
        drop(jar);
        let again = SignIns::kept_in(&scratch.0, Ok(key(2)));
        assert_eq!(again.cookies(), Vec::new());
    }

    #[test]
    fn a_second_alo_keeps_its_own_in_memory_and_writes_nothing() {
        let scratch = Scratch::new("second");
        let first = SignIns::kept_in(&scratch.0, Ok(key(3)));
        assert_eq!(first.change(&made(&[cookie("first", LATER)])), Ok(()));
        let second = SignIns::kept_in(&scratch.0, Ok(key(3)));
        assert_eq!(second.kept_as(), Kept::ByAnother);
        assert_eq!(
            second.cookies(),
            Vec::new(),
            "reads nothing the first wrote"
        );
        assert_eq!(second.change(&made(&[cookie("second", LATER)])), Ok(()));
        assert_eq!(names(&second.cookies()), vec!["second"]);
        drop((first, second));
        let after = SignIns::kept_in(&scratch.0, Ok(key(3)));
        assert_eq!(names(&after.cookies()), vec!["first"]);
    }

    #[test]
    fn a_file_under_another_key_is_left_until_forgotten_and_no_key_is_said() {
        let scratch = Scratch::new("other-key");
        {
            let jar = SignIns::kept_in(&scratch.0, Ok(key(4)));
            assert_eq!(jar.change(&made(&[cookie("first", LATER)])), Ok(()));
        }
        let before = std::fs::read(scratch.0.join(FILE)).expect("written");
        let other = SignIns::kept_in(&scratch.0, Ok(key(5)));
        assert_eq!(other.kept_as(), Kept::Unopened);
        assert_eq!(other.change(&made(&[cookie("new", LATER)])), Ok(()));
        assert_eq!(
            std::fs::read(scratch.0.join(FILE)).ok(),
            Some(before),
            "left as it is"
        );
        // Forgotten, it is removed and kept again, under the new key.
        assert_eq!(other.forget_all_under(Ok(Some(key(6)))), Ok(()));
        assert!(!scratch.0.join(FILE).exists());
        assert_eq!(other.kept_as(), Kept::OnThisMac(scratch.0.clone()));
        assert_eq!(other.change(&made(&[cookie("after", LATER)])), Ok(()));
        drop(other);
        assert_eq!(
            names(&SignIns::kept_in(&scratch.0, Ok(key(6))).cookies()),
            vec!["after"]
        );
        let scratch = Scratch::new("no-key");
        let unkeyed = SignIns::kept_in(&scratch.0, Err("locked".to_owned()));
        assert_eq!(unkeyed.kept_as(), Kept::NoKey("locked".to_owned()));
        assert_eq!(unkeyed.change(&made(&[cookie("first", LATER)])), Ok(()));
        assert!(!scratch.0.join(FILE).exists());
    }

    #[test]
    fn forget_all_empties_the_jar_removes_the_file_and_the_old_key_no_longer_opens_a_copy() {
        let scratch = Scratch::new("forget");
        let jar = SignIns::kept_in(&scratch.0, Ok(key(7)));
        assert_eq!(
            jar.change(&made(&[cookie("first", LATER), cookie("session", None)])),
            Ok(())
        );
        let copy = std::fs::read(scratch.0.join(FILE)).expect("written");
        assert_eq!(jar.forgotten(), 0);
        assert_eq!(jar.forget_all_under(Ok(Some(key(8)))), Ok(()));
        assert_eq!(jar.forgotten(), 1);
        // A change seen before it was forgotten is not made.
        assert_eq!(
            jar.change_as_of(&made(&[cookie("raced", LATER)]), 0),
            Ok(())
        );
        assert_eq!(jar.cookies(), Vec::new());
        assert_eq!(jar.cookies(), Vec::new());
        assert!(!scratch.0.join(FILE).exists());
        // What is kept after is sealed under the new key alone.
        assert_eq!(jar.change(&made(&[cookie("after", LATER)])), Ok(()));
        let now = std::fs::read(scratch.0.join(FILE)).expect("written");
        assert!(chat_seal::open_as(&Key::of([8; KEY_BYTES]), MAGIC, &now).is_ok());
        assert!(chat_seal::open_as(&Key::of([8; KEY_BYTES]), MAGIC, &copy).is_err());
        // A jar in memory forgets too.
        let memory = SignIns::in_memory();
        assert_eq!(memory.change(&made(&[cookie("first", LATER)])), Ok(()));
        assert_eq!(memory.forget_all(), Ok(()));
        assert_eq!(memory.cookies(), Vec::new());
        assert_eq!(memory.forgotten(), 1);
    }

    #[test]
    fn a_file_that_is_not_one_is_never_read_as_sign_ins_never_a_panic() {
        let scratch = Scratch::new("hostile");
        std::fs::create_dir_all(&scratch.0).expect("made");
        let k = Key::of([9; KEY_BYTES]);
        let sealed_text =
            |text: &str| chat_seal::seal_as(&k, MAGIC, text.as_bytes()).expect("sealed");
        let hostile: Vec<Vec<u8>> = vec![
            Vec::new(),
            b"not sealed".to_vec(),
            MAGIC.to_vec(),
            sealed_text("not the header\n"),
            sealed_text(""),
            {
                let mut cut = sealed_text("alo sign-ins 1\n");
                cut.truncate(cut.len() - 3);
                cut
            },
        ];
        for bytes in hostile {
            std::fs::write(scratch.0.join(FILE), &bytes).expect("written");
            let jar = SignIns::kept_in(&scratch.0, Ok(k.clone()));
            assert_eq!(jar.kept_as(), Kept::Unopened, "{bytes:?}");
            assert_eq!(jar.cookies(), Vec::new());
        }
        // Lines that are not cookies are passed over; the rest are read.
        let good = cookie("good", LATER).line();
        let text = format!(
            "alo sign-ins 1\n{good}\nnot a cookie\n\t\t\n{}\n",
            cookie("s", None).line()
        );
        std::fs::write(scratch.0.join(FILE), sealed_text(&text)).expect("written");
        let jar = SignIns::kept_in(&scratch.0, Ok(k));
        assert_eq!(
            names(&jar.cookies()),
            vec!["good"],
            "a session's is never read from a disk"
        );
    }

    #[test]
    fn this_persons_are_beside_the_chats_under_application_support() {
        let chats = crate::chat_store::for_this_person();
        let signins = for_this_person();
        if let (Ok(chats), Ok(signins)) = (chats, signins) {
            assert_eq!(signins.parent(), chats.parent());
            assert!(signins.ends_with(DIRECTORY));
            assert!(!signins.starts_with(std::env::temp_dir()));
        }
    }
}
