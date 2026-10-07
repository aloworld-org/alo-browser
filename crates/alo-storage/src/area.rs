/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `localStorage` area: string pairs, and how many bytes they count for.
//!
//! ADR 0025 § 3: the cap is *"counted as two bytes for each UTF-16 code unit of
//! every key and value"*. That is the unit a page measures its own strings in,
//! and the unit the specification's suggested 5 MiB is written against — so
//! the count is of what the page stored, never of how this engine happens to
//! hold it (UTF-8 here, which would make the same string cost a page more in
//! one script than another).
//!
//! An area does not know its own cap. Whether a write may happen is the
//! store's question ([`crate::store`]), because the answer depends on the
//! bucket's quota and the profile's bound too, and an area that refused writes
//! on its own would be one of three places answering it.

use std::collections::BTreeMap;

/// One origin's `localStorage`, under one top-level site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Area {
    pairs: BTreeMap<String, String>,
    /// The total of [`counted`] over every key and value, kept rather than
    /// recomputed.
    counted: u64,
}

impl Area {
    /// An area holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value stored under this key.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.pairs.get(name).map(String::as_str)
    }

    /// How many pairs are held.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// How many bytes the pairs count for: two for every UTF-16 code unit of
    /// every key and value.
    pub fn counted(&self) -> u64 {
        self.counted
    }

    /// Every pair, ordered by key.
    ///
    /// The specification leaves the order to the browser and asks only that it
    /// be stable while the area is unchanged. Ordered by key is stable without
    /// remembering anything.
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// This area with one pair set, replacing any value it had.
    #[must_use]
    pub fn with(&self, name: &str, value: &str) -> Self {
        let mut changed = self.clone();
        changed.set(name.to_owned(), value.to_owned());
        changed
    }

    /// This area without one pair.
    #[must_use]
    pub fn without(&self, name: &str) -> Self {
        let mut changed = self.clone();
        if let Some(value) = changed.pairs.remove(name) {
            changed.counted = changed
                .counted
                .saturating_sub(counted(name).saturating_add(counted(&value)));
        }
        changed
    }

    /// Set one pair in place. What decoding an area from a disk builds with.
    pub(crate) fn set(&mut self, name: String, value: String) {
        let name_counts = counted(&name);
        let adding = name_counts.saturating_add(counted(&value));
        if let Some(old) = self.pairs.insert(name, value) {
            self.counted = self
                .counted
                .saturating_sub(name_counts.saturating_add(counted(&old)));
        }
        self.counted = self.counted.saturating_add(adding);
    }
}

/// What one string counts for: two bytes for each of its UTF-16 code units.
///
/// Saturating rather than wrapping: a string longer than a `u64` can count is
/// one no machine holds, and the answer in that direction is *too large*,
/// which every cap refuses.
pub fn counted(text: &str) -> u64 {
    (text.encode_utf16().count() as u64).saturating_mul(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_counts_two_bytes_per_utf16_code_unit_not_per_utf8_byte() {
        assert_eq!(counted(""), 0);
        assert_eq!(counted("en"), 4);
        // One code unit, two bytes of UTF-8.
        assert_eq!(counted("é"), 2);
        // One code unit, three bytes of UTF-8.
        assert_eq!(counted("語"), 2);
        // A surrogate pair: two code units, four bytes of UTF-8.
        assert_eq!(counted("😀"), 4);
    }

    #[test]
    fn the_count_follows_every_change_exactly() {
        let area = Area::new().with("locale", "fr");
        assert_eq!(area.counted(), 12 + 4);
        let area = area.with("locale", "en-GB");
        assert_eq!(
            area.counted(),
            12 + 10,
            "a replaced value was counted twice"
        );
        let area = area.with("width", "320");
        assert_eq!(area.counted(), 22 + 10 + 6);
        let area = area.without("locale");
        assert_eq!(area.counted(), 16);
        let area = area.without("nothing-by-this-name");
        assert_eq!(area.counted(), 16);
        assert_eq!(area.get("width"), Some("320"));
        assert_eq!(area.get("locale"), None);
        assert_eq!(area.without("width"), Area::new());
    }

    #[test]
    fn a_change_makes_a_new_area_and_leaves_the_old_one_alone() {
        let before = Area::new().with("a", "1");
        let after = before.with("b", "2");
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 2);
    }

    #[test]
    fn pairs_come_out_ordered_by_key() {
        let area = Area::new().with("b", "2").with("a", "1").with("c", "3");
        let names: Vec<&str> = area.pairs().map(|(name, _)| name).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }
}
