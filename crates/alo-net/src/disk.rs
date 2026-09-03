/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The directory a cache lives in: what may go into it, and what comes back.
//!
//! ADR 0011 is the decision this implements. [`crate::record`] is the bytes one
//! entry is; this is the directory of them, the bound on it, and — the half
//! that is a decision rather than a mechanism — **what is never written**.
//!
//! # Never written, rather than written and deleted
//!
//! [`why_it_is_never_written`] is ADR 0011 section 2 in one function. A file
//! that was deleted is a file that was on the disk: recoverable, and present for
//! the whole window between the two operations, which is precisely the window a
//! crash or a power cut lands in. The only way to promise something did not
//! outlive the session is not to write it.
//!
//! Everything on that list is still cached in **memory**, where it costs
//! nothing to be careful. The cost of the list is real and is written down in
//! the ADR: the disk cache is weakest exactly where it would help most, because
//! a site somebody is signed into and uses daily is the one whose responses
//! carry `Set-Cookie` or `private`.
//!
//! # There is no cache until somebody opens one
//!
//! [`crate::cache::Cache::new`] has no disk. That is not an oversight and it is
//! not a default to be tidied away later — it is what a session-scoped profile
//! is, in ADR 0011's words: *"not a cache that is emptied at the end: a cache
//! that was never opened."*
//!
//! # This runs in the browser process
//!
//! ADR 0005 gives a renderer no filesystem, and ADR 0011 section 5 says why the
//! temptation to permit this directory in a sandbox profile has to be refused:
//! it would hand any compromised renderer every page that person has read,
//! across every site. Nothing in a renderer opens one of these.

use crate::directives::{Directives, Flag};
use crate::freshness::Stored;
use crate::record::{self, Record};
use crate::request::Request;
use crate::response::Response;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The most entries one cache directory holds.
pub const MOST_ON_DISK: usize = 500;

/// The most bytes one cache directory holds.
///
/// ADR 0011: *"the bound is modest and it is ours to choose, for the reason
/// every bound in `alo-net` exists: a limit somebody else chooses is not a
/// limit. A browser that quietly fills somebody's disk is a browser they
/// uninstall, and they are right to."*
///
/// **This is not the quota decision.** One policy across `localStorage`,
/// `IndexedDB` and the Cache API is queue item 90 and needs its own ADR; a bound
/// on our own cache must not become that policy by precedent.
pub const LARGEST_ON_DISK: u64 = 64 * 1024 * 1024;

/// The most bytes one entry may be.
///
/// Well under [`LARGEST_ON_DISK`], so that writing one entry can never be a
/// reason to evict every other. It is also the bound on what is read back: a
/// file in the directory larger than this is not opened at all, whoever put it
/// there.
pub const LARGEST_ENTRY: u64 = 8 * 1024 * 1024;

/// What the file holding one entry is called.
const ENTRY: &str = "entry";

/// What a file being written is called until the rename makes it an entry.
const WRITING: &str = "writing";

/// One entry, as the directory knows it without opening it.
#[derive(Debug, Clone)]
struct Held {
    /// The file's name within the directory.
    file: String,
    /// How many bytes it takes, for the bound.
    size: u64,
}

/// A cache directory.
///
/// Holding one of these is what makes a cache survive a restart. Not holding
/// one is what makes a session-scoped profile leave nothing behind.
#[derive(Debug)]
pub struct Disk {
    directory: PathBuf,
    /// What is held, in the order it was stored, so the oldest can go first
    /// when the bound is reached. The clock is not involved.
    held: BTreeMap<u64, Held>,
    /// The total of every [`Held::size`], kept rather than recomputed.
    bytes: u64,
    /// The sequence the next entry written gets.
    next: u64,
    /// The most entries this one holds, and the most bytes. Values rather than
    /// constants because a bound *is* a value — a browser with a small disk and
    /// a browser with a large one want different ones, and a test wants a bound
    /// it can reach without writing sixty-four megabytes to find out.
    most: usize,
    largest: u64,
}

impl Disk {
    /// Open the cache directory at this path, creating it private to its owner.
    ///
    /// Whatever is already there is read: that is what "survives a restart"
    /// means. A file that is not one of ours is left alone; one of ours that
    /// does not read is removed, because it can never be served and leaving it
    /// would let a stranger fill the bound with rubbish.
    ///
    /// # Errors
    ///
    /// A sentence, when the directory cannot be made, cannot be made private,
    /// or cannot be listed. A caller that gets one should run without a disk
    /// rather than refuse to run: ADR 0011 says an unreadable cache is a miss,
    /// and an unopenable one is every request being a miss.
    pub fn at(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = directory.as_ref().to_path_buf();
        make_the_directory_private(&directory)?;
        let mut disk = Self {
            directory,
            held: BTreeMap::new(),
            bytes: 0,
            next: 0,
            most: MOST_ON_DISK,
            largest: LARGEST_ON_DISK,
        };
        disk.read_what_is_there()?;
        Ok(disk)
    }

    /// The same directory, holding at most this many entries and this many
    /// bytes.
    ///
    /// A bound set below the size of a single entry is honoured rather than
    /// argued with: everything is evicted, including what was just written, and
    /// the cache holds nothing. That is what asking for a cache smaller than one
    /// response means.
    #[must_use]
    pub fn bounded_to(mut self, entries: usize, bytes: u64) -> Self {
        self.most = entries;
        self.largest = bytes;
        self.stay_within_bounds();
        self
    }

    /// How many entries are held.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether anything is held.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// How many bytes the entries take.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Where this is.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// What is stored under this key, if anything readable is.
    ///
    /// Every way of failing is the same answer — nothing — because ADR 0011 is
    /// explicit that *"an unreadable entry is a miss. Never an error that
    /// reaches the page, never a failure to load."*
    ///
    /// An entry whose bytes do not decode is **removed**: it can never be
    /// served, so leaving it would mean carrying it against the bound forever.
    /// An entry that decodes but holds a different key is left exactly where it
    /// is — that is a hash collision, the file belongs to the other key, and
    /// deleting it would throw away something valid to no purpose.
    pub fn read(&mut self, key: &str) -> Option<Stored> {
        let file = file_for(key);
        let path = self.directory.join(&file);
        let bytes = read_at_most(&path, LARGEST_ENTRY)?;
        match record::decode(&bytes) {
            Ok(found) if found.key == key => Some(found.stored),
            Ok(_) => None,
            Err(_) => {
                self.remove(&file);
                None
            }
        }
    }

    /// Write this entry, replacing whatever was under the same key.
    ///
    /// Returns whether it was written. It is not, when the entry is larger than
    /// [`LARGEST_ENTRY`] or when the filesystem refuses — neither of which is a
    /// failure a page is told about.
    ///
    /// **Whether it *may* be written is a different question and is not asked
    /// here.** [`why_it_is_never_written`] answers that one, and
    /// [`crate::cache::Cache`] asks it before calling this.
    pub fn write(&mut self, key: &str, stored: &Stored) -> bool {
        let sequence = self.next;
        let bytes = record::encode(&Record {
            key: key.to_owned(),
            sequence,
            stored: stored.clone(),
        });
        let size = bytes.len() as u64;
        if size > LARGEST_ENTRY {
            return false;
        }

        let file = file_for(key);
        let being_written = self.directory.join(format!("{file}.{WRITING}"));
        // Written beside it and renamed over it, so that a power cut leaves
        // either the old entry or the new one and never half of either.
        if write_privately(&being_written, &bytes).is_err() {
            let _ = fs::remove_file(&being_written);
            return false;
        }
        if fs::rename(&being_written, self.directory.join(&file)).is_err() {
            let _ = fs::remove_file(&being_written);
            return false;
        }

        self.forget_the_file(&file);
        self.held.insert(sequence, Held { file, size });
        self.bytes = self.bytes.saturating_add(size);
        self.next = self.next.saturating_add(1);
        self.stay_within_bounds();
        true
    }

    /// Forget what is stored under this key.
    ///
    /// What a `POST` to a URL has to mean, and what a response that may no
    /// longer be written has to mean: an entry that was legitimately written
    /// and has since been superseded by one that may not be is removed, so that
    /// a restart cannot serve the older one.
    pub fn forget(&mut self, key: &str) {
        self.remove(&file_for(key));
    }

    /// Remove every entry.
    ///
    /// What "clear this browsing data" has to be able to do, and what a person
    /// who wants the record gone reaches for.
    pub fn empty(&mut self) {
        for held in std::mem::take(&mut self.held).values() {
            let _ = fs::remove_file(self.directory.join(&held.file));
        }
        self.bytes = 0;
    }

    /// Remove one file, by name, and stop counting it.
    fn remove(&mut self, file: &str) {
        let _ = fs::remove_file(self.directory.join(file));
        self.forget_the_file(file);
    }

    /// Stop counting one file, without touching the filesystem.
    fn forget_the_file(&mut self, file: &str) {
        let Some(sequence) = self
            .held
            .iter()
            .find(|(_, held)| held.file == file)
            .map(|(sequence, _)| *sequence)
        else {
            return;
        };
        if let Some(held) = self.held.remove(&sequence) {
            self.bytes = self.bytes.saturating_sub(held.size);
        }
    }

    /// Oldest first, until both bounds are met.
    fn stay_within_bounds(&mut self) {
        while self.held.len() > self.most || self.bytes > self.largest {
            let Some(oldest) = self.held.keys().next().copied() else {
                break;
            };
            let Some(held) = self.held.remove(&oldest) else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(held.size);
            let _ = fs::remove_file(self.directory.join(&held.file));
        }
    }

    /// What is already in the directory, put back in the order it was written.
    fn read_what_is_there(&mut self) -> Result<(), String> {
        let listing = fs::read_dir(&self.directory)
            .map_err(|why| format!("the cache directory cannot be listed: {why}"))?;
        let mut found: Vec<(u64, Held)> = Vec::new();
        for entry in listing {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // A file left behind by a crash between writing and renaming. It
            // was never an entry, and nothing will ever finish it.
            if path.extension().is_some_and(|kind| kind == WRITING) {
                let _ = fs::remove_file(&path);
                continue;
            }
            // Anything else in the directory belongs to somebody else and is
            // left exactly as it is.
            if path.extension().is_none_or(|kind| kind != ENTRY) {
                continue;
            }
            let Ok(about) = entry.metadata() else {
                continue;
            };
            if !about.is_file() || about.len() > LARGEST_ENTRY {
                let _ = fs::remove_file(&path);
                continue;
            }
            match prefix_of(&path) {
                Some(sequence) => found.push((
                    sequence,
                    Held {
                        file: name.to_owned(),
                        size: about.len(),
                    },
                )),
                // One of ours that is not readable as one of ours: a version we
                // do not know, a truncated write, or a name collision with
                // something else. It can never be served.
                None => {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        // Two entries claiming the same place in the order can only be a file
        // somebody else wrote. The later-named one keeps the slot and the other
        // is dropped rather than allowed to make the bound uncountable.
        for (sequence, held) in found {
            self.bytes = self.bytes.saturating_add(held.size);
            if let Some(displaced) = self.held.insert(sequence, held) {
                self.bytes = self.bytes.saturating_sub(displaced.size);
                let _ = fs::remove_file(self.directory.join(&displaced.file));
            }
            self.next = self.next.max(sequence.saturating_add(1));
        }
        self.stay_within_bounds();
        Ok(())
    }
}

/// Why this exchange is never written to a disk, or [`None`] when it may be.
///
/// ADR 0011 section 2, as a function. Every entry on the list is something
/// whose leak is a person's account, a person's session, or a person's session
/// being remembered past the moment they ended it.
///
/// The one thing on the ADR's list that is not here is the session-scoped
/// profile, and that is because it cannot be a check: a cache that is never
/// opened has no disk to refuse, which is [`crate::cache::Cache::new`].
pub fn why_it_is_never_written(request: &Request, response: &Response) -> Option<&'static str> {
    // `file:` is already on the disk and copying it achieves nothing but a
    // second copy; `data:` is part of the page, and a page may put a secret in
    // one. Both ends, because a redirect can change the scheme.
    if !is_http(&request.url.scheme) || !is_http(&response.url.scheme) {
        return Some("a response that did not come over HTTP");
    }
    // The page being behind a password, said in the request itself.
    if request.headers.get("Authorization").is_some() {
        return Some("a request that carried Authorization");
    }
    // A session token, and a session token in a file is a login somebody can
    // pick up later.
    if response.headers.get("Set-Cookie").is_some() {
        return Some("a response that carried Set-Cookie");
    }
    let said = Directives::of(response.headers.all("Cache-Control"));
    if said.says(Flag::NoStore) {
        return Some("a response marked no-store");
    }
    // `private` means *for one person*, and a disk other programs can read is
    // not one person. Reusable from memory for as long as the process lives.
    if said.says(Flag::Private) {
        return Some("a response marked private");
    }
    if Directives::of(request.headers.all("Cache-Control")).says(Flag::NoStore) {
        return Some("a request that asked for nothing to be stored");
    }
    // A body that stopped early is a fact worth keeping for the length of a
    // download and is not a page. One that stopped without saying how long it
    // was meant to be never reaches here, because `Pool::fetch` refuses a short
    // body before anything is kept at all.
    if let Some(said) = response
        .headers
        .get("Content-Length")
        .and_then(|value| value.trim().parse::<u64>().ok())
        && said != response.body.len() as u64
    {
        return Some("a body that is not the length it was said to be");
    }
    None
}

/// Where this operating system keeps caches, and our directory inside it.
///
/// ADR 0011: *"one directory per profile, in the place the operating system
/// keeps caches, so that it is somewhere a person can find and delete without
/// breaking the browser."*
///
/// [`None`] when there is nowhere to put it — no home directory, or a profile
/// name that is not a single plain path component. A profile name is not a
/// place to be lenient: one containing a separator would put the cache
/// somewhere nobody chose.
pub fn where_the_system_keeps_caches(profile: &str) -> Option<PathBuf> {
    if profile.is_empty()
        || !profile
            .chars()
            .all(|glyph| glyph.is_ascii_alphanumeric() || glyph == '-' || glyph == '_')
    {
        return None;
    }
    let home = std::env::var_os("HOME").filter(|home| !home.is_empty())?;
    let caches = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library").join("Caches")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|set| !set.is_empty())
            .map_or_else(|| PathBuf::from(home).join(".cache"), PathBuf::from)
    };
    Some(caches.join("alo-browser").join(profile).join("http"))
}

/// Whether a scheme is one whose responses may be written down.
fn is_http(scheme: &str) -> bool {
    scheme == "http" || scheme == "https"
}

/// What the file holding this key is called.
///
/// ADR 0011: *"a file name is a hash of the cache key because a URL is not a
/// file name, and for no other reason. It is not claimed as privacy: anybody
/// who can read the directory can ask whether a URL they already suspect is in
/// it, and a hash does not stop them."* The key is written inside the file, so
/// a collision is a miss rather than one URL's response served for another's.
fn file_for(key: &str) -> String {
    format!("{:016x}.{ENTRY}", record::fingerprint(key.as_bytes()))
}

/// The sequence number in a file's prefix, without reading the rest of it.
fn prefix_of(path: &Path) -> Option<u64> {
    let mut file = fs::File::open(path).ok()?;
    let mut prefix = [0u8; record::PREFIX];
    file.read_exact(&mut prefix).ok()?;
    record::sequence_of(&prefix).ok()
}

/// A file's contents, when it is not larger than this.
///
/// The length is asked of the filesystem before anything is allocated, for the
/// reason every length in [`crate::record`] is checked: a number somebody else
/// chose is not a size to reserve.
fn read_at_most(path: &Path, most: u64) -> Option<Vec<u8>> {
    let about = fs::metadata(path).ok()?;
    if !about.is_file() || about.len() > most {
        return None;
    }
    fs::read(path).ok()
}

#[cfg(unix)]
fn make_the_directory_private(directory: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !directory.is_dir() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|why| format!("the cache directory cannot be made: {why}"))?;
    }
    // Set rather than assume: a directory that already existed may have been
    // made by something else, and ADR 0011 promises this one is private to its
    // owner rather than promising we would have made it that way.
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
        .map_err(|why| format!("the cache directory cannot be made private: {why}"))
}

#[cfg(not(unix))]
fn make_the_directory_private(_directory: &Path) -> Result<(), String> {
    // ADR 0011 promises the directory and every file in it are private to their
    // owner. On a platform where this engine cannot say that, the honest answer
    // is no disk cache rather than a promise it does not keep.
    Err("this platform has no way to make the cache private to its owner".to_owned())
}

#[cfg(unix)]
fn write_privately(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
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
    // contents do, and what survives a power cut is an entry full of zeroes —
    // which the checksum would catch, and which there is no reason to create.
    file.sync_all()
}

#[cfg(not(unix))]
fn write_privately(_path: &Path, _bytes: &[u8]) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "this platform has no way to make a cache file private to its owner",
    ))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::headers::Headers;
    use crate::response::Status;
    use std::time::{Duration, UNIX_EPOCH};

    fn url(text: &str) -> alo_url::Url {
        alo_url::parse(text).expect("a URL")
    }

    fn answered(headers: &[(&str, &str)]) -> Response {
        let mut carried = Headers::new();
        for (name, value) in headers {
            carried.add(*name, *value);
        }
        Response {
            url: url("https://example.com/a"),
            status: Status(200),
            headers: carried,
            body: b"the stored body".to_vec(),
        }
    }

    fn stored(response: Response) -> Stored {
        Stored {
            response,
            requested_at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            received_at: UNIX_EPOCH + Duration::from_secs(1_700_000_001),
            varied_on: Vec::new(),
        }
    }

    /// A directory of this test's own, in the place the machine keeps
    /// temporary things. Named after the caller so two tests never share one.
    fn somewhere(called: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!(
            "alo-cache-{}-{called}",
            std::process::id().wrapping_mul(2_654_435_761)
        ));
        let _ = fs::remove_dir_all(&place);
        place
    }

    #[test]
    fn a_directory_is_made_private_and_so_is_every_file_in_it() {
        use std::os::unix::fs::PermissionsExt;
        let place = somewhere("private");
        let mut disk = Disk::at(&place).expect("a cache directory");
        assert!(disk.write(
            "example.com GET https://example.com/a",
            &stored(answered(&[]))
        ));

        let about = fs::metadata(&place).expect("the directory");
        assert_eq!(
            about.permissions().mode() & 0o777,
            0o700,
            "the cache directory is readable by somebody else"
        );
        let entry = fs::read_dir(&place)
            .expect("a listing")
            .flatten()
            .next()
            .expect("the entry just written");
        assert_eq!(
            entry.metadata().expect("the file").permissions().mode() & 0o777,
            0o600,
            "a cache file is readable by somebody else"
        );
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_file_that_does_not_decode_is_a_miss_and_stops_being_one() {
        let place = somewhere("rubbish");
        let key = "example.com GET https://example.com/a";
        let mut disk = Disk::at(&place).expect("a cache directory");
        assert!(disk.write(key, &stored(answered(&[]))));

        // Somebody else's bytes, under our name.
        let path = place.join(file_for(key));
        fs::write(&path, b"not an entry, and never was").expect("a write");
        assert!(disk.read(key).is_none(), "rubbish was served as a response");
        assert!(!path.exists(), "an entry that can never be served was kept");
        let _ = fs::remove_dir_all(&place);
    }

    /// A file name is a hash, and a hash collides. The key inside the file is
    /// what stops one URL's response being served for another's.
    #[test]
    fn an_entry_under_another_keys_name_is_a_miss_and_stays_where_it_is() {
        let place = somewhere("collision");
        let mut disk = Disk::at(&place).expect("a cache directory");
        let one = "example.com GET https://example.com/a";
        let two = "example.com GET https://example.com/b";
        assert!(disk.write(one, &stored(answered(&[]))));

        // The entry for one key, put exactly where the other's would live.
        let whole = fs::read(place.join(file_for(one))).expect("what was written");
        fs::write(place.join(file_for(two)), &whole).expect("a write");
        assert!(
            disk.read(two).is_none(),
            "an entry was served for a key it was not stored under"
        );
        assert!(
            place.join(file_for(two)).exists(),
            "an entry that decoded perfectly was deleted for belonging elsewhere"
        );
        assert!(disk.read(one).is_some(), "and the real one still reads");
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn the_oldest_go_first_when_there_are_too_many_bytes() {
        let place = somewhere("weighed");
        let mut disk = Disk::at(&place).expect("a cache directory").bounded_to(
            MOST_ON_DISK,
            // Room for two entries and not three.
            2 * (record::encode(&Record {
                key: "example.com GET https://example.com/0".to_owned(),
                sequence: 0,
                stored: stored(answered(&[])),
            })
            .len() as u64),
        );
        for n in 0..3 {
            let key = format!("example.com GET https://example.com/{n}");
            assert!(disk.write(&key, &stored(answered(&[]))));
        }
        assert_eq!(disk.len(), 2, "the byte bound did not evict anything");
        assert!(
            disk.read("example.com GET https://example.com/0").is_none(),
            "the oldest entry outlived the byte bound"
        );
        assert!(
            disk.read("example.com GET https://example.com/2").is_some(),
            "the newest went instead of the oldest"
        );
        assert!(disk.bytes() <= disk.largest, "the bound is not a bound");
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn what_is_written_is_still_there_when_the_directory_is_opened_again() {
        let place = somewhere("again");
        let key = "example.com GET https://example.com/a";
        {
            let mut disk = Disk::at(&place).expect("a cache directory");
            assert!(disk.write(key, &stored(answered(&[("ETag", "\"v1\"")]))));
            assert_eq!(disk.len(), 1);
        }
        let mut reopened = Disk::at(&place).expect("the same directory");
        assert_eq!(reopened.len(), 1, "what was written was not found again");
        let found = reopened.read(key).expect("the entry written before");
        assert_eq!(found.response.headers.get("ETag"), Some("\"v1\""));
        assert_eq!(
            reopened.read("example.com GET https://example.com/b"),
            None,
            "a key nobody stored answered"
        );
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_half_written_file_is_never_an_entry() {
        let place = somewhere("half");
        let mut disk = Disk::at(&place).expect("a cache directory");
        let key = "example.com GET https://example.com/a";
        assert!(disk.write(key, &stored(answered(&[]))));
        // What a crash between the write and the rename leaves behind.
        fs::write(place.join(format!("{}.{WRITING}", file_for(key))), b"half").expect("a leftover");
        let reopened = Disk::at(&place).expect("the same directory");
        assert_eq!(reopened.len(), 1, "a leftover was counted as an entry");
        assert!(
            !place.join(format!("{}.{WRITING}", file_for(key))).exists(),
            "a leftover nothing will ever finish was kept"
        );
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_file_somebody_else_put_there_is_left_alone() {
        let place = somewhere("theirs");
        Disk::at(&place).expect("a cache directory");
        let theirs = place.join("notes.txt");
        fs::write(&theirs, b"somebody else's file").expect("a write");
        let reopened = Disk::at(&place).expect("the same directory");
        assert_eq!(reopened.len(), 0);
        assert!(theirs.exists(), "a file that is not ours was deleted");
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn the_oldest_go_first_when_there_are_too_many() {
        let place = somewhere("bounded");
        let mut disk = Disk::at(&place).expect("a cache directory");
        for n in 0..(MOST_ON_DISK + 20) {
            let key = format!("example.com GET https://example.com/{n}");
            assert!(disk.write(&key, &stored(answered(&[]))));
        }
        assert_eq!(disk.len(), MOST_ON_DISK);
        assert!(
            disk.read("example.com GET https://example.com/0").is_none(),
            "the oldest entry outlived the bound"
        );
        assert!(
            disk.read(&format!(
                "example.com GET https://example.com/{}",
                MOST_ON_DISK + 19
            ))
            .is_some(),
            "the newest entry went instead of the oldest"
        );
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn writing_the_same_key_twice_holds_one_entry() {
        let place = somewhere("twice");
        let mut disk = Disk::at(&place).expect("a cache directory");
        let key = "example.com GET https://example.com/a";
        assert!(disk.write(key, &stored(answered(&[("ETag", "\"v1\"")]))));
        assert!(disk.write(key, &stored(answered(&[("ETag", "\"v2\"")]))));
        assert_eq!(disk.len(), 1, "one key became two entries");
        assert_eq!(
            disk.bytes(),
            fs::metadata(place.join(file_for(key)))
                .expect("the file")
                .len()
        );
        let found = disk.read(key).expect("the second write");
        assert_eq!(found.response.headers.get("ETag"), Some("\"v2\""));
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn an_entry_larger_than_the_bound_is_not_written() {
        let place = somewhere("large");
        let mut disk = Disk::at(&place).expect("a cache directory");
        let mut response = answered(&[]);
        let too_many = usize::try_from(LARGEST_ENTRY).expect("a bound this machine can hold") + 1;
        response.body = vec![b'x'; too_many];
        assert!(!disk.write("example.com GET https://example.com/a", &stored(response)));
        assert_eq!(disk.len(), 0);
        assert_eq!(disk.bytes(), 0);
        let _ = fs::remove_dir_all(&place);
    }
}
