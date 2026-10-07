/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What every bucket counts for, when each was last used, and which are open —
//! and from those, which buckets go when the profile needs room.
//!
//! ADR 0025 § 4, as arithmetic with no disk in it:
//!
//! 1. **Buckets are evicted whole.** Nothing here can name less than a bucket.
//! 2. **The least recently used goes first**, by a counter the ledger keeps
//!    rather than by a clock — ADR 0011 § 6's reason: the clock is not involved
//!    in a decision that has nothing to do with time, and two uses in the same
//!    millisecond still have an order.
//! 3. **Never the bucket that is writing, never one an open document is
//!    using.** (Never one a person has marked to keep, too; that mark is queue
//!    item 93's grant, and until it exists no bucket carries one.)
//! 4. **If that does not free the room, the write fails, and nothing else is
//!    evicted to make the attempt.** [`Ledger::to_free`] answers the whole list
//!    or nothing, so a write that is going to be refused never costs another
//!    site its data on the way.

use crate::key::StorageKey;
use std::collections::BTreeMap;

/// One bucket, as the ledger counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counted {
    /// Its counted bytes, every part of it together.
    pub bytes: u64,
    /// The use counter's value at its last use.
    pub last_use: u64,
}

/// Every bucket the store holds, counted.
#[derive(Debug, Default)]
pub struct Ledger {
    held: BTreeMap<StorageKey, Counted>,
    /// How many documents have each key open. Kept apart from [`Self::held`]
    /// because a document may have a key open whose bucket does not exist yet,
    /// and must still protect it the moment it does.
    open: BTreeMap<StorageKey, u64>,
    /// The total of every [`Counted::bytes`], kept rather than recomputed.
    total: u64,
    /// The value the next use gets.
    next_use: u64,
}

impl Ledger {
    /// A ledger holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// A bucket found at start, with the counter it was last used at.
    pub fn found(&mut self, key: StorageKey, bytes: u64, last_use: u64) {
        self.next_use = self.next_use.max(last_use.saturating_add(1));
        self.put(key, Counted { bytes, last_use });
    }

    /// The bucket's counted bytes and last use, if it is held.
    pub fn get(&self, key: &StorageKey) -> Option<Counted> {
        self.held.get(key).copied()
    }

    /// How many buckets are held.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether no bucket is held.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Every bucket's counted bytes together.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Every key held.
    pub fn keys(&self) -> impl Iterator<Item = &StorageKey> {
        self.held.keys()
    }

    /// The counter value for a use happening now. Each call is later than the
    /// one before.
    pub fn next_use(&mut self) -> u64 {
        let now = self.next_use;
        self.next_use = self.next_use.saturating_add(1);
        now
    }

    /// Record a bucket's counted bytes and when it was last used, replacing
    /// whatever was recorded. A bucket counting nothing is not held.
    pub fn put(&mut self, key: StorageKey, counted: Counted) {
        self.forget(&key);
        if counted.bytes == 0 {
            return;
        }
        self.total = self.total.saturating_add(counted.bytes);
        self.held.insert(key, counted);
    }

    /// Stop counting a bucket.
    pub fn forget(&mut self, key: &StorageKey) {
        if let Some(gone) = self.held.remove(key) {
            self.total = self.total.saturating_sub(gone.bytes);
        }
    }

    /// A document has this key open.
    pub fn opened(&mut self, key: &StorageKey) {
        let count = self.open.entry(key.clone()).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// A document that had this key open has closed. A close with no open
    /// before it is ignored rather than allowed to protect nothing twice.
    pub fn closed(&mut self, key: &StorageKey) {
        if let Some(count) = self.open.get_mut(key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.open.remove(key);
            }
        }
    }

    /// Whether a document has this key open.
    pub fn is_open(&self, key: &StorageKey) -> bool {
        self.open.contains_key(key)
    }

    /// Which buckets to evict so that `writer` can come to hold `bytes`
    /// without the profile passing `bound`, least recently used first.
    ///
    /// `Some(vec![])` when nothing has to go. [`None`] when evicting every
    /// bucket that may be evicted would still not be enough — and then nothing
    /// is to be evicted at all.
    pub fn to_free(&self, writer: &StorageKey, bytes: u64, bound: u64) -> Option<Vec<StorageKey>> {
        let others = self
            .total
            .saturating_sub(self.held.get(writer).map_or(0, |held| held.bytes));
        let wanted = others.saturating_add(bytes);
        if wanted <= bound {
            return Some(Vec::new());
        }
        let mut need = wanted.saturating_sub(bound);

        let mut eligible: Vec<(&StorageKey, &Counted)> = self
            .held
            .iter()
            .filter(|(key, _)| *key != writer && !self.is_open(key))
            .collect();
        eligible.sort_by_key(|(_, counted)| counted.last_use);

        let mut chosen = Vec::new();
        for (key, counted) in eligible {
            if need == 0 {
                break;
            }
            chosen.push(key.clone());
            need = need.saturating_sub(counted.bytes);
        }
        (need == 0).then_some(chosen)
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

    /// Four buckets of 100 bytes, used in the order a, b, c, d.
    fn four() -> Ledger {
        let mut ledger = Ledger::new();
        for host in ["a.example", "b.example", "c.example", "d.example"] {
            let last_use = ledger.next_use();
            ledger.put(
                key(host),
                Counted {
                    bytes: 100,
                    last_use,
                },
            );
        }
        ledger
    }

    #[test]
    fn the_total_follows_every_change() {
        let mut ledger = four();
        assert_eq!((ledger.len(), ledger.total()), (4, 400));
        ledger.put(
            key("a.example"),
            Counted {
                bytes: 250,
                last_use: 9,
            },
        );
        assert_eq!((ledger.len(), ledger.total()), (4, 550));
        ledger.forget(&key("b.example"));
        assert_eq!((ledger.len(), ledger.total()), (3, 450));
        ledger.put(
            key("c.example"),
            Counted {
                bytes: 0,
                last_use: 10,
            },
        );
        assert_eq!(
            (ledger.len(), ledger.total()),
            (2, 350),
            "an empty bucket was held"
        );
    }

    #[test]
    fn nothing_goes_when_there_is_room() {
        assert_eq!(
            four().to_free(&key("e.example"), 100, 500),
            Some(Vec::new())
        );
        // The writer's own bytes are replaced rather than added to.
        assert_eq!(
            four().to_free(&key("a.example"), 200, 500),
            Some(Vec::new())
        );
    }

    #[test]
    fn the_least_recently_used_go_first_and_only_as_many_as_needed() {
        assert_eq!(
            four().to_free(&key("e.example"), 150, 400),
            Some(vec![key("a.example"), key("b.example")])
        );
        let mut ledger = four();
        // `a` used again: `b` is now the oldest.
        let now = ledger.next_use();
        ledger.put(
            key("a.example"),
            Counted {
                bytes: 100,
                last_use: now,
            },
        );
        assert_eq!(
            ledger.to_free(&key("e.example"), 100, 400),
            Some(vec![key("b.example")])
        );
    }

    #[test]
    fn the_writer_and_an_open_bucket_are_never_chosen() {
        let mut ledger = four();
        ledger.opened(&key("b.example"));
        assert_eq!(
            ledger.to_free(&key("a.example"), 200, 400),
            Some(vec![key("c.example")]),
            "the writer or an open bucket was chosen"
        );
        ledger.closed(&key("b.example"));
        assert_eq!(
            ledger.to_free(&key("a.example"), 200, 400),
            Some(vec![key("b.example")])
        );
    }

    #[test]
    fn when_evicting_everything_allowed_is_not_enough_nothing_is_chosen() {
        let mut ledger = four();
        ledger.opened(&key("d.example"));
        // 300 can be freed, the writer needs the room of 350.
        assert_eq!(ledger.to_free(&key("e.example"), 350, 400), None);
        assert_eq!(ledger.to_free(&key("e.example"), 500, 400), None);
    }

    #[test]
    fn a_key_opened_twice_is_open_until_closed_twice_and_a_stray_close_is_ignored() {
        let mut ledger = Ledger::new();
        let one = key("a.example");
        ledger.closed(&one);
        assert!(!ledger.is_open(&one));
        ledger.opened(&one);
        ledger.opened(&one);
        ledger.closed(&one);
        assert!(ledger.is_open(&one));
        ledger.closed(&one);
        assert!(!ledger.is_open(&one));
    }

    #[test]
    fn uses_found_at_start_are_followed_by_later_ones() {
        let mut ledger = Ledger::new();
        ledger.found(key("a.example"), 10, 41);
        ledger.found(key("b.example"), 10, 7);
        assert_eq!(ledger.next_use(), 42);
        assert_eq!(ledger.next_use(), 43);
    }
}
