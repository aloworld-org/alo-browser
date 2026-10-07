/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The browser process's store: every bucket, one quota over each, and the
//! profile's bound over all of them.
//!
//! ADR 0025 is the decision. [`crate::directory`] is where the bytes are,
//! [`crate::ledger`] is the arithmetic, and this is the order things happen in
//! — which is the part that makes a refusal mean *nothing changed*:
//!
//! 1. The area a write would leave is computed, and nothing is touched.
//! 2. It is held against the area's cap, then the bucket's quota, then the
//!    profile's bound. A refusal at any of the three returns here.
//! 3. Only a write that will happen evicts, and it evicts exactly what the
//!    ledger chose, all of which it needed.
//! 4. The bucket is written, then counted.
//!
//! A write that takes nothing away — a removal, a clear, a value replaced by a
//! shorter one — skips step 2. A profile already over its bound, because the
//! bound was lowered since, must still let a site delete what it stored.
//!
//! # A session-scoped profile opens nothing
//!
//! [`Store::for_the_session`] holds every bucket in memory and has no
//! directory to write to. ADR 0025 § 5: *"a session-scoped profile writes
//! nothing at all. Every bucket there is memory only, for every API."* As with
//! `alo-net`'s cache, that is not a store emptied at the end: it is a store that
//! was never given anywhere to put a file.
//!
//! # No page reaches this yet
//!
//! Queue item 302 is the page's half: a renderer's copy of an area, the writes
//! it posts, and the browser process refusing a key it did not load into that
//! renderer. This is the half those writes are posted to.

use crate::area::Area;
use crate::directory::{Directory, SetAside};
use crate::key::StorageKey;
use crate::ledger::{Counted, Ledger};
use crate::limits::{LOCAL_AREA, Limits};
use core::fmt;
use std::collections::BTreeMap;
use std::path::Path;

/// Why a write was refused.
///
/// The first three are decided before anything is touched, and leave the
/// store exactly as it was. [`Refused::Disk`] comes after: what was stored
/// under the key written to is unchanged, and any bucket evicted to make room
/// for the write stays evicted — it was going to have to go for this write to
/// happen, and putting it back would be a second write to a filesystem that
/// has just refused one.
///
/// The first three are what a page will see as `QuotaExceededError` (queue item
/// 302). None says which bound it was: a page is told it may not, and the
/// profile's bound is never reported to one (ADR 0025 § 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The `localStorage` area would pass its cap.
    AreaFull {
        /// What the area would have counted.
        would_be: u64,
        /// The cap.
        cap: u64,
    },
    /// The bucket would pass its quota.
    OverQuota {
        /// What the bucket would have counted.
        would_be: u64,
        /// The quota.
        quota: u64,
    },
    /// The profile would pass its bound, and evicting every bucket that may be
    /// evicted would not make the room.
    ProfileFull,
    /// The filesystem refused, and what was stored under this key before is
    /// still stored.
    Disk(String),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::AreaFull { would_be, cap } => {
                write!(f, "an area of {would_be} bytes, past its cap of {cap}")
            }
            Refused::OverQuota { would_be, quota } => {
                write!(f, "a bucket of {would_be} bytes, past its quota of {quota}")
            }
            Refused::ProfileFull => f.write_str("no room in the profile, even after eviction"),
            Refused::Disk(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Refused {}

/// Where the buckets are.
#[derive(Debug)]
enum Place {
    /// On a disk, surviving a restart.
    Directory(Directory),
    /// In memory, for a session-scoped profile.
    Memory(BTreeMap<StorageKey, Area>),
}

/// Every bucket a profile holds.
#[derive(Debug)]
pub struct Store {
    place: Place,
    limits: Limits,
    ledger: Ledger,
    set_aside: Vec<SetAside>,
    evicted: u64,
}

impl Store {
    /// The store in this directory, made private to its owner, with whatever
    /// it already holds — that is what surviving a restart means.
    ///
    /// Every bucket is read and checked. One that fails is set aside whole and
    /// recorded in [`Self::set_aside`]; the site starts as if it had stored
    /// nothing.
    ///
    /// # Errors
    ///
    /// A sentence, when the directory cannot be made or made private. A
    /// browser that gets one has no storage to offer, and pages are refused
    /// rather than written somewhere unprotected.
    pub fn at(directory: impl AsRef<Path>, limits: Limits) -> Result<Self, String> {
        let directory = Directory::at(directory, limits.local_area)?;
        Ok(Self::over(directory, limits))
    }

    /// The store in this directory, as [`Self::at`], bounded by ADR 0025's
    /// limits for the volume the directory is on — which is how the browser
    /// opens its profile's store when it starts.
    ///
    /// The directory is made first and the volume asked about the directory
    /// itself, so that the figure is the volume the buckets will be written
    /// to, even on a first run with nothing there yet. A volume that cannot be
    /// asked gives a profile bound of zero ([`Limits::for_the_volume_at`]), and
    /// every write is refused.
    ///
    /// # Errors
    ///
    /// As [`Self::at`].
    pub fn on_its_volume(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = Directory::at(directory, LOCAL_AREA)?;
        let limits = Limits::for_the_volume_at(directory.root());
        Ok(Self::over(directory, limits))
    }

    /// A store over a directory that is already made, holding whatever it
    /// already holds.
    fn over(directory: Directory, limits: Limits) -> Self {
        let (found, set_aside) = directory.survey();
        let mut ledger = Ledger::new();
        for bucket in found {
            ledger.found(bucket.key, bucket.bytes, bucket.last_use);
        }
        Self {
            place: Place::Directory(directory),
            limits,
            ledger,
            set_aside,
            evicted: 0,
        }
    }

    /// A store for a session-scoped profile, which opens no directory and
    /// writes no file.
    pub fn for_the_session(limits: Limits) -> Self {
        Self {
            place: Place::Memory(BTreeMap::new()),
            limits,
            ledger: Ledger::new(),
            set_aside: Vec::new(),
            evicted: 0,
        }
    }

    /// Where this store keeps its buckets, or [`None`] for a session-scoped
    /// one.
    pub fn directory(&self) -> Option<&Path> {
        match &self.place {
            Place::Directory(directory) => Some(directory.root()),
            Place::Memory(_) => None,
        }
    }

    /// The bounds this store enforces.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// How many buckets hold anything.
    pub fn len(&self) -> usize {
        self.ledger.len()
    }

    /// Whether no bucket holds anything.
    pub fn is_empty(&self) -> bool {
        self.ledger.is_empty()
    }

    /// A bucket's counted bytes: what `navigator.storage.estimate()` will
    /// report as `usage` (queue item 303).
    pub fn usage(&self, key: &StorageKey) -> u64 {
        self.ledger.get(key).map_or(0, |counted| counted.bytes)
    }

    /// Every bucket's counted bytes together, held against the profile's bound.
    pub fn total(&self) -> u64 {
        self.ledger.total()
    }

    /// Every bucket set aside, since start or found set aside at it.
    pub fn set_aside(&self) -> &[SetAside] {
        &self.set_aside
    }

    /// How many buckets this store has evicted. ADR 0025's measurement for
    /// whether the profile's bound is too tight.
    pub fn evicted(&self) -> u64 {
        self.evicted
    }

    /// A document with this key has opened, and its bucket may not be evicted
    /// until it closes.
    pub fn opened(&mut self, key: &StorageKey) {
        self.ledger.opened(key);
    }

    /// A document with this key has closed.
    pub fn closed(&mut self, key: &StorageKey) {
        self.ledger.closed(key);
    }

    /// The `localStorage` area for this key — what is sent with a load to a
    /// renderer that does not hold it (ADR 0025 § 5). A use of the bucket.
    ///
    /// A bucket that fails its check now is set aside and the area is empty,
    /// never a part of what was there.
    pub fn local(&mut self, key: &StorageKey) -> Area {
        let area = self.current(key);
        if !area.is_empty() {
            let last_use = self.ledger.next_use();
            self.ledger.put(
                key.clone(),
                Counted {
                    bytes: area.counted(),
                    last_use,
                },
            );
            if let Place::Directory(directory) = &self.place {
                // A use that is not written down costs the bucket only its
                // place in the order after a restart.
                let _ = directory.used(key, last_use);
            }
        }
        area
    }

    /// `localStorage.setItem`, as the browser process applies it.
    ///
    /// # Errors
    ///
    /// [`Refused`], and the store is as it was.
    pub fn set_item(&mut self, key: &StorageKey, name: &str, value: &str) -> Result<(), Refused> {
        let area = self.current(key).with(name, value);
        self.commit(key, area)
    }

    /// `localStorage.removeItem`.
    ///
    /// # Errors
    ///
    /// [`Refused::Disk`] only: taking something away is never over a bound.
    pub fn remove_item(&mut self, key: &StorageKey, name: &str) -> Result<(), Refused> {
        let area = self.current(key).without(name);
        self.commit(key, area)
    }

    /// `localStorage.clear`.
    ///
    /// # Errors
    ///
    /// [`Refused::Disk`] only.
    pub fn clear_local(&mut self, key: &StorageKey) -> Result<(), Refused> {
        self.commit(key, Area::new())
    }

    /// Clear everything a site stored: every bucket of an origin on that site,
    /// under every top-level site, and every one of them set aside.
    ///
    /// ADR 0025 § 8 makes this one act with clearing the site's cookies and
    /// cache entries; the function that does all three belongs to whatever
    /// owns all three, and calls this.
    ///
    /// # Errors
    ///
    /// A sentence, when the filesystem refuses. The ledger has already stopped
    /// counting the site, so nothing of it is served either way.
    pub fn clear_site(&mut self, site: &str) -> Result<(), String> {
        let theirs: Vec<StorageKey> = self
            .ledger
            .keys()
            .filter(|key| key.site() == site)
            .cloned()
            .collect();
        for key in &theirs {
            self.ledger.forget(key);
        }
        self.set_aside
            .retain(|aside| aside.site.as_deref() != Some(site));
        match &mut self.place {
            Place::Directory(directory) => directory.clear_site(site),
            Place::Memory(buckets) => {
                buckets.retain(|key, _| key.site() != site);
                Ok(())
            }
        }
    }

    /// What the bucket holds now, setting it aside if it fails its check.
    fn current(&mut self, key: &StorageKey) -> Area {
        let read = match &self.place {
            Place::Memory(buckets) => Ok(buckets.get(key).cloned()),
            Place::Directory(directory) => directory.read(key),
        };
        match read {
            Ok(Some(area)) => area,
            Ok(None) => {
                self.ledger.forget(key);
                Area::new()
            }
            Err(why) => {
                self.ledger.forget(key);
                if let Place::Directory(directory) = &self.place {
                    self.set_aside.push(directory.set_aside(key, &why));
                }
                Area::new()
            }
        }
    }

    /// Hold the area a write would leave against every bound, make the room,
    /// and keep it.
    fn commit(&mut self, key: &StorageKey, area: Area) -> Result<(), Refused> {
        let bytes = area.counted();
        let growing = bytes > self.usage(key);
        let mut evicting = Vec::new();
        if growing {
            if bytes > self.limits.local_area {
                return Err(Refused::AreaFull {
                    would_be: bytes,
                    cap: self.limits.local_area,
                });
            }
            if bytes > self.limits.bucket {
                return Err(Refused::OverQuota {
                    would_be: bytes,
                    quota: self.limits.bucket,
                });
            }
            evicting = self
                .ledger
                .to_free(key, bytes, self.limits.profile)
                .ok_or(Refused::ProfileFull)?;
        }

        for other in &evicting {
            self.remove(other);
            self.ledger.forget(other);
            self.evicted = self.evicted.saturating_add(1);
        }

        if area.is_empty() {
            self.remove(key);
            self.ledger.forget(key);
            return Ok(());
        }
        let last_use = self.ledger.next_use();
        match &mut self.place {
            Place::Memory(buckets) => {
                buckets.insert(key.clone(), area);
            }
            Place::Directory(directory) => {
                directory.write(key, &area).map_err(Refused::Disk)?;
                let _ = directory.used(key, last_use);
            }
        }
        self.ledger.put(key.clone(), Counted { bytes, last_use });
        Ok(())
    }

    /// Remove a bucket from wherever it is.
    fn remove(&mut self, key: &StorageKey) {
        match &mut self.place {
            Place::Memory(buckets) => {
                buckets.remove(key);
            }
            Place::Directory(directory) => directory.remove(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::Partition;
    use alo_url::Origin;

    fn key(host: &str) -> StorageKey {
        let url = alo_url::parse(&format!("https://{host}/")).expect("a URL");
        StorageKey::of(&Origin::of(&url), &Partition::of(&url)).expect("a key")
    }

    fn small() -> Limits {
        Limits {
            bucket: 1000,
            local_area: 100,
            profile: 300,
        }
    }

    #[test]
    fn a_write_past_the_area_cap_is_refused_and_changes_nothing() {
        let mut store = Store::for_the_session(small());
        let one = key("a.example");
        store.set_item(&one, "k", "v").expect("a write");
        let refused = store.set_item(&one, "big", &"x".repeat(60));
        assert_eq!(
            refused,
            Err(Refused::AreaFull {
                would_be: 4 + 6 + 120,
                cap: 100
            })
        );
        assert_eq!(store.usage(&one), 4);
        assert_eq!(store.local(&one), Area::new().with("k", "v"));
    }

    #[test]
    fn taking_away_is_never_refused_even_over_every_bound() {
        let mut store = Store::for_the_session(small());
        let one = key("a.example");
        store.set_item(&one, "k", &"x".repeat(40)).expect("a write");
        store.limits = Limits {
            bucket: 1,
            local_area: 1,
            profile: 1,
        };
        store
            .set_item(&one, "k", "shorter")
            .expect("a shorter value");
        store.remove_item(&one, "k").expect("a removal");
        assert_eq!((store.len(), store.total()), (0, 0));
        store.clear_local(&one).expect("clearing nothing");
    }

    #[test]
    fn an_emptied_area_is_no_bucket_at_all() {
        let mut store = Store::for_the_session(small());
        let one = key("a.example");
        store.set_item(&one, "k", "v").expect("a write");
        store.clear_local(&one).expect("a clear");
        assert!(store.is_empty());
        assert_eq!(store.usage(&one), 0);
    }

    #[test]
    fn a_session_store_has_nowhere_to_put_a_file() {
        let store = Store::for_the_session(small());
        assert_eq!(store.directory(), None);
    }
}
