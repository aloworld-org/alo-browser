/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An array: an ordinary object whose `length` keeps up with its indices
//! (queue item 225).
//!
//! The specification writes an array as an exotic object with **one** internal
//! method of its own, `[[DefineOwnProperty]]`, and this file is that method and
//! nothing else. Everything else — the prototype, the property table, the
//! order — is the [`Ordinary`] it wraps, exactly as a [`Function`](super::Function)
//! wraps one. ADR 0014 § 11 is why this is a fourth implementation of
//! [`Internal`] rather than a case in the interpreter.
//!
//! # The two rules, and where each one lives
//!
//! - **An index at or past the length grows it.** `a[5] = 1` on an empty array
//!   makes `length` six — and is refused outright, before anything is stored,
//!   when `length` is not writable (`ArrayDefineOwnProperty`).
//! - **A smaller length deletes from the end.** `a.length = 1` takes every
//!   index from one up away, highest first, and stops at the first that will
//!   not go: an element that is not configurable keeps the length one past it
//!   and the definition answers `false` (`ArraySetLength`). Only the indices
//!   that **exist** are visited — a length of four billion over three elements
//!   is three deletions, not four billion, which is the difference between an
//!   array and a denial of service.
//!
//! # `length` is a property, and it is not in the table
//!
//! It is held beside the table so that the one key the rules are about is
//! never stored somewhere they could be walked round. It still answers as an
//! own property — not enumerable, not configurable, writable — and it comes
//! after the indices and before every other name in `[[OwnPropertyKeys]]`,
//! which is where the specification's `ArrayCreate` puts it, since it is the
//! first string key an array is given.
//!
//! An array cannot read the heap it is in, so it cannot spell `"length"` for
//! itself: [`Objects::array`](super::Objects::array) interns the name and hands
//! the key over, and the array keeps the string alive by tracing it.
//!
//! # Converting a length is the caller's
//!
//! `a.length = "2"` converts the string, and `a.length = obj` would call the
//! object's `valueOf` — **twice**, since the specification asks `ToUint32` and
//! `ToNumber` separately and a page can count the calls. A cell can do neither.
//! So the value a definition of `length` carries must already be a length,
//! [`exact_length`], and anything else is refused here rather than guessed at;
//! the interpreter does the converting and raises the `RangeError` a length
//! that is not one deserves.

use crate::convert;
use crate::heap::{Barrier, Ref, Tracer};

use super::internal::Internal;
use super::key::Key;
use super::ordinary::Ordinary;
use super::property::Property;
use super::value::Value;

/// An array exotic object.
#[derive(Debug)]
pub struct Array {
    ordinary: Ordinary,
    /// The key `"length"` is, which this array cannot look up for itself.
    length_key: Key,
    /// `length`: a data property whose value is always a whole number from zero
    /// to 2³²−1, never enumerable and never configurable.
    length: Property,
}

/// The length a number is, or [`None`] if it is not one.
///
/// A length is a whole number from zero to 2³²−1. That is `ToUint32(v)` and
/// `ToNumber(v)` agreeing, which is the specification's test, written as the
/// range it comes to: `-0` is zero, and `NaN`, `1.5`, `-1` and `2**32` are not
/// lengths at all.
pub fn exact_length(number: f64) -> Option<u32> {
    let whole = number.fract() == 0.0;
    (whole && (0.0..=f64::from(u32::MAX)).contains(&number)).then(|| convert::to_uint32(number))
}

impl Array {
    /// An array of this length with no elements: `ArrayCreate`.
    ///
    /// `length_key` must be the interned key of `"length"`, which is what
    /// [`Objects::array`](super::Objects::array) passes.
    pub fn new(prototype: Option<Ref>, length_key: Key, length: u32) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            length_key,
            length: Property::data(Value::Number(f64::from(length)), true, false, false),
        }
    }

    /// How long it is.
    pub fn length(&self) -> u32 {
        self.length
            .value()
            .and_then(|value| match value {
                Value::Number(number) => exact_length(number),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// Whether `length` may be written — which decides whether an assignment
    /// to it converts the value at all, since `OrdinarySet` refuses a property
    /// that is not writable before `ArraySetLength` is ever reached.
    pub fn length_is_writable(&self) -> bool {
        self.length.is_writable()
    }

    /// Whether this key is the array's `length`.
    pub fn is_length(&self, key: Key) -> bool {
        key == self.length_key
    }

    /// Report every edge: what the ordinary object holds, and the name of
    /// `length`, which this array may be the only thing keeping alive.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        if let Some(held) = self.length_key.reference() {
            tracer.edge(held);
        }
    }

    /// What it owns beyond its slot, which is its table.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }

    /// Store a length, with the writability it is to have.
    fn store_length(&mut self, length: u32, writable: bool) {
        self.length = Property::data(Value::Number(f64::from(length)), writable, false, false);
    }

    /// `ArraySetLength`, after the caller has checked the definition against
    /// the one it replaces.
    fn define_length(&mut self, property: &Property) -> bool {
        let Some(new) = property.value().and_then(|value| match value {
            Value::Number(number) => exact_length(number),
            _ => None,
        }) else {
            // A length nobody converted, or an accessor. The second is refused
            // before this is reached — `length` is not configurable — and the
            // first is a caller's mistake that must not become a number.
            return false;
        };
        let old = self.length();
        let writable = property.is_writable();
        if new >= old {
            self.store_length(new, writable);
            return true;
        }
        if !self.length.is_writable() {
            return false;
        }
        // Highest first, so that a refusal half way leaves every index below
        // it in place and the length one past the one that stayed.
        for at in self.ordinary.indices_from(new).into_iter().rev() {
            let Some(key) = Key::index(at) else {
                continue;
            };
            let configurable = self
                .ordinary
                .own_property(key)
                .is_none_or(Property::is_configurable);
            if !configurable {
                self.store_length(at.saturating_add(1), writable);
                return false;
            }
            self.ordinary.delete_own(key);
        }
        self.store_length(new, writable);
        true
    }
}

impl Internal for Array {
    fn own_property(&self, key: Key) -> Option<&Property> {
        if self.is_length(key) {
            return Some(&self.length);
        }
        self.ordinary.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        if self.is_length(key) {
            return self.define_length(&property);
        }
        let Some(at) = key.as_index() else {
            return self.ordinary.define_own(barrier, key, property);
        };
        let length = self.length();
        if at >= length && !self.length.is_writable() {
            return false;
        }
        if !self.ordinary.define_own(barrier, key, property) {
            return false;
        }
        if at >= length {
            // The largest index is 2³²−2, so one past it is still a length.
            self.store_length(at.saturating_add(1), true);
        }
        true
    }

    fn delete_own(&mut self, key: Key) -> bool {
        // `length` is not configurable, so the caller never asks; if it did,
        // the answer is the one a non-configurable property gives.
        if self.is_length(key) {
            return false;
        }
        self.ordinary.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        let mut keys = self.ordinary.own_keys();
        let indices = keys
            .iter()
            .take_while(|key| key.as_index().is_some())
            .count();
        keys.insert(indices, self.length_key);
        keys
    }

    fn prototype(&self) -> Option<Ref> {
        self.ordinary.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.ordinary.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.ordinary.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.ordinary.prevent_extensions()
    }
}

#[cfg(test)]
mod tests {
    use super::exact_length;

    #[test]
    fn a_length_is_a_whole_number_a_u32_can_hold() {
        assert_eq!(exact_length(0.0), Some(0));
        assert_eq!(exact_length(-0.0), Some(0));
        assert_eq!(exact_length(3.0), Some(3));
        assert_eq!(exact_length(4_294_967_295.0), Some(u32::MAX));
        for number in [
            -1.0,
            1.5,
            4_294_967_296.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(exact_length(number), None, "{number}");
        }
    }
}
