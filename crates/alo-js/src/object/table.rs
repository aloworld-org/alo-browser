/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An object's properties, in the order a page can see.
//!
//! ADR 0014 § 11: *property order is observable, so it is the specification's
//! order from the first line — integer-like keys ascending, then string keys in
//! insertion order, then symbols in insertion order. It costs nothing now, and
//! it is invisible until the day a page enumerates and gets a different answer
//! from every other browser.*
//!
//! So the order is a property of the **storage** rather than a sort done at
//! enumeration time, which is what makes it impossible to get wrong later:
//!
//! - the indices live in a [`BTreeMap`], which is ascending by construction;
//! - the strings and the symbols live in one list in insertion order, walked
//!   twice — strings, then symbols — because a page that adds a symbol between
//!   two strings still sees the two strings adjacent.
//!
//! # Deleting, and the tombstone that is not a leak
//!
//! A key removed from the middle of the insertion list would move every key
//! after it, so a removal leaves a hole and the lookup map loses its entry. A
//! program that adds and deletes in a loop would then grow a list of holes for
//! ever, which is a leak a stranger's script controls — so the list is
//! **compacted** once the holes outnumber the entries. That is the same
//! argument every bound in this engine makes, and
//! `an_object_model_that_is_hostile.rs` is where it is asserted rather than
//! assumed.
//!
//! Re-adding a key that was deleted puts it at the **end**, which is what the
//! specification says and what a page can see.
//!
//! # Why this is not a hidden class
//!
//! ADR 0014 § 11 again: *one interface for property access, because the
//! representation behind it is the thing an engine changes when it gets fast.
//! Hidden classes and inline caches are refused **now** under law 3 and are
//! allowed **later without an ADR**, provided the semantics and the order are
//! unchanged.* This file is that representation. Nothing outside it may know
//! that there is a [`BTreeMap`] in here.

use std::collections::{BTreeMap, HashMap};

use crate::heap::Tracer;

use super::key::Key;
use super::property::Property;

/// The properties of one object.
#[derive(Debug, Default)]
pub struct Properties {
    /// The integer-like keys, ascending by construction.
    indexed: BTreeMap<u32, Property>,
    /// The string and symbol keys, in insertion order, with a hole where one
    /// was deleted.
    named: Vec<Option<(Key, Property)>>,
    /// Where each named key is in `named`.
    at: HashMap<Key, usize>,
    /// How many holes `named` has.
    holes: usize,
}

impl Properties {
    /// An object with no properties.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many properties there are.
    pub fn len(&self) -> usize {
        self.indexed.len().saturating_add(self.at.len())
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The property this key names, if there is one.
    pub fn get(&self, key: Key) -> Option<&Property> {
        if let Some(at) = key.as_index() {
            return self.indexed.get(&at);
        }
        let at = *self.at.get(&key)?;
        self.named.get(at)?.as_ref().map(|(_, property)| property)
    }

    /// The property this key names, to be written to.
    pub fn get_mut(&mut self, key: Key) -> Option<&mut Property> {
        if let Some(at) = key.as_index() {
            return self.indexed.get_mut(&at);
        }
        let at = *self.at.get(&key)?;
        self.named
            .get_mut(at)?
            .as_mut()
            .map(|(_, property)| property)
    }

    /// Put a property under this key, replacing whatever was there.
    ///
    /// Replacing keeps the key's **place** in the order, which is the half of
    /// the rule that is easy to lose: redefining `a` on `{a: 1, b: 2}` must not
    /// move `a` behind `b`. Only a key that is genuinely new goes at the end.
    ///
    /// Whether a replacement is *allowed* is
    /// [`Property::may_replace`](super::Property::may_replace) and is the
    /// caller's question — this is the storage.
    pub fn put(&mut self, key: Key, property: Property) {
        if let Some(at) = key.as_index() {
            self.indexed.insert(at, property);
            return;
        }
        if let Some(at) = self.at.get(&key).copied() {
            if let Some(slot) = self.named.get_mut(at) {
                *slot = Some((key, property));
                return;
            }
        }
        self.named.push(Some((key, property)));
        self.at.insert(key, self.named.len().saturating_sub(1));
    }

    /// Take a property away, answering whether there was one.
    pub fn remove(&mut self, key: Key) -> bool {
        if let Some(at) = key.as_index() {
            return self.indexed.remove(&at).is_some();
        }
        let Some(at) = self.at.remove(&key) else {
            return false;
        };
        if let Some(slot) = self.named.get_mut(at) {
            *slot = None;
        }
        self.holes = self.holes.saturating_add(1);
        self.compact_if_mostly_holes();
        true
    }

    /// Every key, in the order the language says.
    ///
    /// Indices ascending, then strings in insertion order, then symbols in
    /// insertion order. This is `OrdinaryOwnPropertyKeys`, and it is where the
    /// clause in ADR 0014 § 11 is either kept or quietly broken.
    pub fn keys(&self) -> Vec<Key> {
        let mut keys = Vec::with_capacity(self.len());
        for at in self.indexed.keys() {
            if let Some(key) = Key::index(*at) {
                keys.push(key);
            }
        }
        for (key, _) in self.named.iter().flatten() {
            if key.as_text().is_some() {
                keys.push(*key);
            }
        }
        for (key, _) in self.named.iter().flatten() {
            if key.as_symbol().is_some() {
                keys.push(*key);
            }
        }
        keys
    }

    /// Report every edge the table holds: the keys as well as the values.
    ///
    /// **The keys are edges too.** A property named by a string is a property
    /// whose name a page can read back out of `Object.keys`, so the string cell
    /// is alive because the object is — and the intern table is weak precisely
    /// so that this is the thing keeping it, rather than the interning.
    pub fn trace(&self, tracer: &mut Tracer) {
        for (at, property) in &self.indexed {
            let _ = at;
            property.trace(tracer);
        }
        for (key, property) in self.named.iter().flatten() {
            if let Some(held) = key.reference() {
                tracer.edge(held);
            }
            property.trace(tracer);
        }
    }

    /// What this table owns, in bytes, for the heap's ceiling.
    ///
    /// An estimate rather than a measurement, and honest about being one: it
    /// counts the entries and the room the lookup map takes for them, which is
    /// what grows when a page mints keys. What it deliberately does **not**
    /// count is the cells the keys and values name — the heap counts those
    /// where they are, and counting them here would count them twice.
    pub fn footprint(&self) -> usize {
        let entry = size_of::<Option<(Key, Property)>>();
        let lookup = size_of::<(Key, usize)>().saturating_mul(2);
        let indexed = size_of::<(u32, Property)>().saturating_mul(2);

        self.named
            .len()
            .saturating_mul(entry)
            .saturating_add(self.at.len().saturating_mul(lookup))
            .saturating_add(self.indexed.len().saturating_mul(indexed))
    }

    /// Close the holes up once there are more of them than there are
    /// properties.
    ///
    /// The threshold is what makes a loop of add-then-delete cost a bounded
    /// amount of memory *and* a bounded amount of work: compacting on every
    /// delete would be quadratic, and never compacting is the leak.
    fn compact_if_mostly_holes(&mut self) {
        if self.holes <= self.at.len() {
            return;
        }
        self.named.retain(Option::is_some);
        self.at.clear();
        for (at, entry) in self.named.iter().enumerate() {
            if let Some((key, _)) = entry {
                self.at.insert(*key, at);
            }
        }
        self.holes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, Properties, Property};
    use crate::heap::Ref;
    use crate::object::value::Value;

    /// A key naming a string cell that nothing interned, which is fine here:
    /// this file's business is the order, and what makes two keys equal is the
    /// reference rather than the text behind it.
    fn text(slot: u32) -> Key {
        Key::text(Ref::for_a_test(slot))
    }

    fn symbol(slot: u32) -> Key {
        Key::symbol(Ref::for_a_test(slot))
    }

    fn index(at: u32) -> Key {
        match Key::index(at) {
            Some(key) => key,
            None => panic!("{at} is an array index"),
        }
    }

    fn put(table: &mut Properties, key: Key) {
        table.put(key, Property::plain(Value::Undefined));
    }

    #[test]
    fn the_order_is_indices_then_strings_then_symbols() {
        // Put in deliberately by the worst order for the rule: a symbol first,
        // the indices descending, the strings in the middle.
        let mut table = Properties::new();
        put(&mut table, symbol(100));
        put(&mut table, index(2));
        put(&mut table, text(10));
        put(&mut table, index(0));
        put(&mut table, symbol(101));
        put(&mut table, text(11));
        put(&mut table, index(1));

        assert_eq!(
            table.keys(),
            vec![
                index(0),
                index(1),
                index(2),
                text(10),
                text(11),
                symbol(100),
                symbol(101)
            ]
        );
    }

    #[test]
    fn redefining_a_key_keeps_its_place_and_deleting_it_loses_it() {
        let mut table = Properties::new();
        put(&mut table, text(1));
        put(&mut table, text(2));

        table.put(text(1), Property::plain(Value::Number(9.0)));
        assert_eq!(table.keys(), vec![text(1), text(2)], "a redefinition stays");
        assert_eq!(
            table.get(text(1)).and_then(Property::value),
            Some(Value::Number(9.0))
        );

        assert!(table.remove(text(1)));
        put(&mut table, text(1));
        assert_eq!(
            table.keys(),
            vec![text(2), text(1)],
            "and a key added again is a new key, at the end"
        );
    }

    #[test]
    fn a_hole_is_closed_up_rather_than_kept_for_ever() {
        // The bound this file exists to hold: add and delete in a loop and the
        // list of holes must not grow without end.
        let mut table = Properties::new();
        put(&mut table, text(0));
        for slot in 1..2000 {
            put(&mut table, text(slot));
            assert!(table.remove(text(slot)));
        }
        assert_eq!(table.len(), 1);
        assert!(
            table.named.len() <= 4,
            "the list was compacted rather than grown: {}",
            table.named.len()
        );
        assert_eq!(table.keys(), vec![text(0)]);
    }

    #[test]
    fn removing_what_is_not_there_says_so() {
        let mut table = Properties::new();
        assert!(!table.remove(text(1)));
        assert!(!table.remove(index(1)));
        assert!(table.is_empty());
    }
}
