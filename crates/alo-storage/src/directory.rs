/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The storage directory: where each bucket's files are, what is found there at
//! start, and what happens to a bucket that does not read.
//!
//! ```text
//! <profile>/storage/
//!   <site>/                 one per site, named by a digest of its name
//!     site                  the site's name, so a set-aside bucket can be reported by it
//!     <bucket>/             one per storage key, named by a digest of the key
//!       bucket              the key and its `localStorage` area
//!       used                when it was last used
//!     <bucket>.aside.<n>/   a bucket that failed its check, kept and never served
//! ```
//!
//! # A site is a directory, so clearing one is one act
//!
//! ADR 0025 § 8: clearing a site's data clears **every bucket under every
//! top-level site it appears in, including anything set aside**. Filing every
//! bucket of a site under one directory makes that a single removal, and makes
//! it impossible to clear a site and miss a bucket of it — set-aside ones
//! included, whose own records are the thing that could not be read.
//!
//! # Set aside, never deleted and never served in part
//!
//! § 6: *"a bucket that fails its check is set aside whole. It is not served at
//! all, not even in part, and it is not deleted."* So it is renamed within its
//! site's directory, where nothing reads it and where clearing the site still
//! reaches it. Every way a bucket can fail — a record that does not decode, a
//! key that does not name the directory it was found in, an area larger than
//! this store would have written — is the same answer.
//!
//! # Names are digests, and that is not privacy
//!
//! A name is SHA-256 of what it names, because an origin is not a file name.
//! ADR 0011's caution holds here too: anybody who can read the directory can
//! ask whether a site they already suspect is in it, and a digest does not stop
//! them. The directory is private to its owner (ADR 0025 § 8), which is the
//! protection, and it is in the place the operating system keeps application
//! data rather than caches, so that a system freeing space does not take a
//! person's work with it.
//!
//! # This runs in the browser process
//!
//! ADR 0025 § 5: a renderer never holds this directory. A sandbox profile
//! granting it would hand a compromised renderer every site's storage.

use crate::area::Area;
use crate::key::StorageKey;
use crate::record;
use alo_net::bytes::{Unreadable, Writer, unreadable};
use alo_net::digest::Digest;
use alo_net::private::{for_profile, make_the_directory_private, read_at_most, write_privately};
use std::fs;
use std::path::{Path, PathBuf};

/// What a site's name is kept in.
const SITE: &str = "site";

/// What a bucket's key and area are kept in.
const BUCKET: &str = "bucket";

/// What a bucket's last use is kept in.
const USED: &str = "used";

/// What a file is called until the rename makes it what it is.
const WRITING: &str = "writing";

/// What a set-aside bucket's directory carries after its name.
const ASIDE: &str = "aside";

/// The most bytes a use counter's or a site's record may be.
const LARGEST_SMALL_RECORD: u64 = 64 * 1024;

/// A bucket that failed its check, as the store records it.
///
/// ADR 0025 § 6: *"the browser process records that it happened, naming the
/// site, where a person can see it."* A security surface (queue item 127) or
/// settings (128) will show these when they exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAside {
    /// The site it belonged to, when its directory still says so.
    pub site: Option<String>,
    /// Why, in words.
    pub why: String,
}

/// A bucket found at start that passed its check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Whose it is.
    pub key: StorageKey,
    /// What its area counts for.
    pub bytes: u64,
    /// When it was last used.
    pub last_use: u64,
}

/// A storage directory.
#[derive(Debug)]
pub struct Directory {
    root: PathBuf,
    /// The most counted bytes an area may hold, which is what bounds how large
    /// a bucket's file can honestly be.
    local_area: u64,
}

impl Directory {
    /// The storage directory at this path, made private to its owner.
    ///
    /// # Errors
    ///
    /// A sentence, when the directory cannot be made or made private.
    pub fn at(root: impl AsRef<Path>, local_area: u64) -> Result<Self, String> {
        let root = root.as_ref().to_path_buf();
        make_the_directory_private(&root)?;
        Ok(Self { root, local_area })
    }

    /// Where this is.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every bucket that reads, and every one that does not — the second set
    /// aside as it is found.
    ///
    /// Each bucket is read whole and checked, rather than believed from a
    /// prefix: a counted size the ledger trusted before the checksum did would
    /// let one flipped byte make the profile look full, and every other site
    /// would be evicted for it.
    pub fn survey(&self) -> (Vec<Found>, Vec<SetAside>) {
        let mut found = Vec::new();
        let mut set_aside = Vec::new();
        for site_directory in directories_in(&self.root) {
            let Some(site_name) = name_of(&site_directory) else {
                continue;
            };
            // Anything not named as one of ours belongs to somebody else and is
            // left exactly as it is.
            if !is_a_digest(&site_name) {
                continue;
            }
            let site = read_at_most(&site_directory.join(SITE), LARGEST_SMALL_RECORD)
                .and_then(|bytes| record::decode_site(&bytes).ok())
                .filter(|site| digest_of_site(site) == site_name);
            remove_unfinished(&site_directory);
            for bucket in directories_in(&site_directory) {
                let Some(name) = name_of(&bucket) else {
                    continue;
                };
                if is_set_aside(&name) {
                    set_aside.push(SetAside {
                        site: site.clone(),
                        why: "a bucket set aside before this start".to_owned(),
                    });
                    continue;
                }
                if !is_a_digest(&name) {
                    continue;
                }
                remove_unfinished(&bucket);
                match self.check(&bucket, &name, &site_name) {
                    Ok(Some(one)) => found.push(one),
                    // Made and never written: a crash between the two. It was
                    // never a bucket, and nothing will ever finish it.
                    Ok(None) => {
                        let _ = fs::remove_file(bucket.join(USED));
                        let _ = fs::remove_dir(&bucket);
                    }
                    Err(why) => set_aside.push(Self::put_aside(&bucket, site.clone(), &why)),
                }
            }
        }
        (found, set_aside)
    }

    /// One bucket's area, when it has one.
    ///
    /// # Errors
    ///
    /// [`Unreadable`], when there is a bucket and it fails its check. The
    /// caller sets it aside; nothing of it is served.
    pub fn read(&self, key: &StorageKey) -> Result<Option<Area>, Unreadable> {
        let place = self.bucket_of(key);
        if !place.join(BUCKET).exists() {
            return Ok(None);
        }
        let bucket = self.read_bucket(&place)?;
        if bucket.key != *key {
            return Err(unreadable("a bucket found under another key's name"));
        }
        Ok(Some(bucket.local))
    }

    /// Write a bucket's area, replacing what it held.
    ///
    /// # Errors
    ///
    /// A sentence, when the filesystem refuses. What was there before is still
    /// there: the new file is written beside the old one and renamed over it.
    pub fn write(&self, key: &StorageKey, local: &Area) -> Result<(), String> {
        let site = self.site_of(key.site());
        // Written when missing and written again when it does not read, so
        // that one damaged name does not leave every later set-aside bucket of
        // the site unattributed.
        let named = read_at_most(&site.join(SITE), LARGEST_SMALL_RECORD)
            .and_then(|bytes| record::decode_site(&bytes).ok())
            .is_some_and(|named| named == key.site());
        if !named {
            make_the_directory_private(&site)?;
            replace(&site.join(SITE), &record::encode_site(key.site()))?;
        }
        let place = self.bucket_of(key);
        make_the_directory_private(&place)?;
        replace(&place.join(BUCKET), &record::encode_bucket(key, local))
    }

    /// Write when a bucket was last used.
    ///
    /// # Errors
    ///
    /// A sentence, when the filesystem refuses.
    pub fn used(&self, key: &StorageKey, counter: u64) -> Result<(), String> {
        let place = self.bucket_of(key);
        if !place.is_dir() {
            return Ok(());
        }
        replace(&place.join(USED), &record::encode_used(counter))
    }

    /// Remove a bucket and every file of it.
    pub fn remove(&self, key: &StorageKey) {
        let _ = fs::remove_dir_all(self.bucket_of(key));
    }

    /// Set a bucket aside that failed its check after start.
    pub fn set_aside(&self, key: &StorageKey, why: &Unreadable) -> SetAside {
        Self::put_aside(&self.bucket_of(key), Some(key.site().to_owned()), why)
    }

    /// Remove every bucket of a site, under every top-level site, set-aside
    /// ones included.
    ///
    /// # Errors
    ///
    /// A sentence, when the filesystem refuses to remove what is there.
    pub fn clear_site(&self, site: &str) -> Result<(), String> {
        match fs::remove_dir_all(self.site_of(site)) {
            Ok(()) => Ok(()),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(why) => Err(format!("a site's storage could not be removed: {why}")),
        }
    }

    /// Whether a bucket is set aside under this site, for a test to ask.
    pub fn set_aside_under(&self, site: &str) -> usize {
        directories_in(&self.site_of(site))
            .iter()
            .filter_map(|bucket| name_of(bucket))
            .filter(|name| is_set_aside(name))
            .count()
    }

    /// A bucket found at start: what it holds, nothing when it was never
    /// written, or why it fails its check.
    fn check(
        &self,
        place: &Path,
        name: &str,
        site_name: &str,
    ) -> Result<Option<Found>, Unreadable> {
        if !place.join(BUCKET).exists() {
            return Ok(None);
        }
        let bucket = self.read_bucket(place)?;
        if digest_of_key(&bucket.key) != name || digest_of_site(bucket.key.site()) != site_name {
            return Err(unreadable("a bucket found under another key's name"));
        }
        let used = place.join(USED);
        // A missing counter is a crash between the two writes that make a
        // bucket, and costs only its place in the order. A damaged one is a
        // record that failed its check, like any other.
        let last_use = if used.exists() {
            read_at_most(&used, LARGEST_SMALL_RECORD)
                .ok_or_else(|| unreadable("a use counter that cannot be read"))
                .and_then(|bytes| record::decode_used(&bytes))?
        } else {
            0
        };
        Ok(Some(Found {
            bytes: bucket.local.counted(),
            key: bucket.key,
            last_use,
        }))
    }

    /// A bucket's record, read and checked.
    fn read_bucket(&self, place: &Path) -> Result<record::Bucket, Unreadable> {
        let bytes = read_at_most(&place.join(BUCKET), self.largest_bucket())
            .ok_or_else(|| unreadable("a bucket larger than this store writes, or unreadable"))?;
        let bucket = record::decode_bucket(&bytes)?;
        if bucket.local.counted() > self.local_area {
            return Err(unreadable("an area larger than this store writes"));
        }
        Ok(bucket)
    }

    /// The most bytes a bucket's file can honestly be.
    ///
    /// An area of `local_area` counted bytes holds at most half that many
    /// pairs, plus the one whose name and value are both empty; each costs
    /// sixteen bytes of lengths, and its text at most one and a half times its
    /// counted size as UTF-8. Ten times the cap, and a mebibyte for the key,
    /// is above all of that.
    fn largest_bucket(&self) -> u64 {
        self.local_area.saturating_mul(10).saturating_add(1 << 20)
    }

    /// Rename a bucket out of the way, and say so.
    fn put_aside(place: &Path, site: Option<String>, why: &Unreadable) -> SetAside {
        let mut n: u64 = 0;
        let moved = loop {
            let Some(name) = name_of(place) else {
                break false;
            };
            let aside = place.with_file_name(format!("{name}.{ASIDE}.{n}"));
            if !aside.exists() {
                break fs::rename(place, aside).is_ok();
            }
            match n.checked_add(1) {
                Some(next) => n = next,
                None => break false,
            }
        };
        SetAside {
            site,
            why: if moved {
                why.why.clone()
            } else {
                format!("{why}, and it could not be moved out of the way")
            },
        }
    }

    fn site_of(&self, site: &str) -> PathBuf {
        self.root.join(digest_of_site(site))
    }

    fn bucket_of(&self, key: &StorageKey) -> PathBuf {
        self.site_of(key.site()).join(digest_of_key(key))
    }
}

/// Where this operating system keeps application data, and this profile's
/// storage inside it.
///
/// [`None`] when there is nowhere to put it: no home directory, or a profile
/// name that is not a single plain path component.
pub fn where_the_system_keeps_storage(profile: &str) -> Option<PathBuf> {
    for_profile(&alo_net::private::application_data()?, profile, "storage")
}

/// Write beside, then rename over, so that a power cut leaves either the old
/// file or the new one and never half of either.
fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut beside = path.as_os_str().to_owned();
    beside.push(".");
    beside.push(WRITING);
    let beside = PathBuf::from(beside);
    let done = write_privately(&beside, bytes).and_then(|()| fs::rename(&beside, path));
    done.map_err(|why| {
        let _ = fs::remove_file(&beside);
        format!("storage could not be written: {why}")
    })
}

/// Remove files a crash left between writing and renaming. They were never
/// records, and nothing will ever finish them.
fn remove_unfinished(directory: &Path) {
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    for entry in listing.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|kind| kind == WRITING) && path.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
}

/// The directories directly inside this one, in no particular order.
fn directories_in(directory: &Path) -> Vec<PathBuf> {
    let Ok(listing) = fs::read_dir(directory) else {
        return Vec::new();
    };
    listing
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn name_of(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}

/// Whether a name is one this directory gives: thirty-two lowercase
/// hexadecimal digits.
fn is_a_digest(name: &str) -> bool {
    name.len() == 32
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether a name is a set-aside bucket's.
fn is_set_aside(name: &str) -> bool {
    let mut parts = name.splitn(3, '.');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(digest), Some(ASIDE), Some(n)) if is_a_digest(digest) && n.parse::<u64>().is_ok()
    )
}

fn digest_of_site(site: &str) -> String {
    let mut named = Writer::default();
    named.text(SITE);
    named.text(site);
    digest_of(&named.out)
}

fn digest_of_key(key: &StorageKey) -> String {
    // Length-prefixed, so no two keys are ever the same bytes. The site is not
    // in it because the origin decides the site.
    let mut named = Writer::default();
    named.text(BUCKET);
    named.text(key.origin());
    named.text(key.partition());
    digest_of(&named.out)
}

/// The first sixteen bytes of SHA-256, as thirty-two hexadecimal digits.
fn digest_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Digest::Sha256
        .of(bytes)
        .iter()
        .take(16)
        .fold(String::with_capacity(32), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use alo_net::Partition;
    use alo_url::Origin;

    fn key(document: &str, top_level: &str) -> StorageKey {
        let url = |text: &str| alo_url::parse(text).expect("a URL");
        StorageKey::of(&Origin::of(&url(document)), &Partition::of(&url(top_level))).expect("a key")
    }

    fn somewhere(called: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!(
            "alo-storage-directory-{}-{called}",
            std::process::id().wrapping_mul(2_654_435_761)
        ));
        let _ = fs::remove_dir_all(&place);
        place
    }

    #[test]
    fn names_are_digests_and_two_keys_never_share_one() {
        let one = digest_of_key(&key("https://a.example/", "https://a.example/"));
        let two = digest_of_key(&key("https://a.example/", "https://b.example/"));
        assert!(is_a_digest(&one) && is_a_digest(&two));
        assert_ne!(one, two);
        assert_ne!(digest_of_site("a.example"), digest_of_site("b.example"));
        assert!(is_set_aside(&format!("{one}.aside.0")));
        assert!(!is_set_aside(&format!("{one}.aside")));
        assert!(!is_set_aside("notes.aside.0"));
    }

    #[test]
    fn a_bucket_is_filed_under_its_site_and_every_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let place = somewhere("filed");
        let directory = Directory::at(&place, 1 << 20).expect("a directory");
        let widget = key("https://widget.example/", "https://news.example/");
        directory
            .write(&widget, &Area::new().with("a", "1"))
            .expect("a write");
        directory.used(&widget, 3).expect("a write");

        let bucket = place
            .join(digest_of_site("widget.example"))
            .join(digest_of_key(&widget));
        for file in [BUCKET, USED] {
            let mode = fs::metadata(bucket.join(file))
                .expect("a file")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{file} is readable by somebody else");
        }
        for dir in [place.clone(), bucket.clone()] {
            let mode = fs::metadata(&dir)
                .expect("a directory")
                .permissions()
                .mode();
            assert_eq!(
                mode & 0o777,
                0o700,
                "{} is readable by somebody else",
                dir.display()
            );
        }
        let (found, aside) = directory.survey();
        assert_eq!(
            found,
            vec![Found {
                key: widget,
                bytes: 4,
                last_use: 3
            }]
        );
        assert!(aside.is_empty());
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_bucket_found_under_another_keys_name_is_set_aside() {
        let place = somewhere("moved");
        let directory = Directory::at(&place, 1 << 20).expect("a directory");
        let one = key("https://a.example/", "https://a.example/");
        let two = key("https://a.example/", "https://b.example/");
        directory
            .write(&one, &Area::new().with("a", "1"))
            .expect("a write");
        directory
            .write(&two, &Area::new().with("b", "2"))
            .expect("a write");
        let site = place.join(digest_of_site("a.example"));
        // One bucket's record copied over the other's.
        fs::copy(
            site.join(digest_of_key(&one)).join(BUCKET),
            site.join(digest_of_key(&two)).join(BUCKET),
        )
        .expect("a copy");

        assert!(
            directory.read(&two).is_err(),
            "one key's bucket was served for another"
        );
        let (found, aside) = directory.survey();
        assert_eq!(found.len(), 1);
        assert_eq!(
            aside,
            vec![SetAside {
                site: Some("a.example".to_owned()),
                why: "a bucket found under another key's name".to_owned()
            }]
        );
        assert_eq!(directory.set_aside_under("a.example"), 1);
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn what_a_crash_leaves_half_done_is_removed_and_nothing_else_is() {
        let place = somewhere("crash");
        let directory = Directory::at(&place, 1 << 20).expect("a directory");
        let one = key("https://a.example/", "https://a.example/");
        directory
            .write(&one, &Area::new().with("a", "1"))
            .expect("a write");
        let site = place.join(digest_of_site("a.example"));
        let bucket = site.join(digest_of_key(&one));
        fs::write(bucket.join("bucket.writing"), b"half").expect("a leftover");
        // A bucket made and never written.
        let empty = site.join(digest_of_key(&key(
            "https://b.a.example/",
            "https://a.example/",
        )));
        fs::create_dir_all(&empty).expect("a directory");
        // Somebody else's things.
        fs::write(place.join("notes.txt"), b"theirs").expect("a file");
        fs::create_dir_all(place.join("theirs")).expect("a directory");

        let (found, aside) = directory.survey();
        assert_eq!((found.len(), aside.len()), (1, 0));
        assert!(!bucket.join("bucket.writing").exists());
        assert!(!empty.exists(), "a bucket that was never written was kept");
        assert!(place.join("notes.txt").exists() && place.join("theirs").exists());
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn an_area_larger_than_this_store_writes_is_set_aside() {
        let place = somewhere("large");
        let generous = Directory::at(&place, 1 << 20).expect("a directory");
        let one = key("https://a.example/", "https://a.example/");
        generous
            .write(&one, &Area::new().with("a", &"x".repeat(100)))
            .expect("a write");
        let strict = Directory::at(&place, 16).expect("the same directory");
        let (found, aside) = strict.survey();
        assert!(found.is_empty());
        assert_eq!(aside.len(), 1);
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_damaged_site_name_is_written_again_by_the_next_write() {
        let place = somewhere("renamed");
        let directory = Directory::at(&place, 1 << 20).expect("a directory");
        let one = key("https://a.example/", "https://a.example/");
        directory
            .write(&one, &Area::new().with("a", "1"))
            .expect("a write");
        let named = place.join(digest_of_site("a.example")).join(SITE);
        fs::write(&named, b"rubbish").expect("damage");
        directory
            .write(&one, &Area::new().with("a", "2"))
            .expect("a write");
        let read = fs::read(&named).expect("the name");
        assert_eq!(record::decode_site(&read).as_deref(), Ok("a.example"));
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn clearing_a_site_removes_its_directory_and_no_other() {
        let place = somewhere("clear");
        let directory = Directory::at(&place, 1 << 20).expect("a directory");
        let mine = key("https://a.example/", "https://b.example/");
        let theirs = key("https://b.example/", "https://b.example/");
        directory
            .write(&mine, &Area::new().with("a", "1"))
            .expect("a write");
        directory
            .write(&theirs, &Area::new().with("b", "2"))
            .expect("a write");
        directory.clear_site("a.example").expect("cleared");
        assert!(!place.join(digest_of_site("a.example")).exists());
        assert_eq!(
            directory
                .read(&theirs)
                .ok()
                .flatten()
                .map(|area| area.len()),
            Some(1)
        );
        directory
            .clear_site("nobody.example")
            .expect("nothing to clear is not an error");
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn storage_is_kept_with_application_data_not_with_caches() {
        let Some(storage) = where_the_system_keeps_storage("default") else {
            return;
        };
        let Some(cache) = alo_net::disk::where_the_system_keeps_caches("default") else {
            return;
        };
        assert!(storage.ends_with("alo-browser/default/storage"));
        assert_ne!(storage.parent(), cache.parent());
        assert_eq!(where_the_system_keeps_storage("../elsewhere"), None);
    }
}
