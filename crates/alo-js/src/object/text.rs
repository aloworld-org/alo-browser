/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A string, which is a heap object and is never changed once it is made.
//!
//! ADR 0014 § 11: *a string is a heap object, immutable once made, in the
//! UTF-16 code units item 70 already decided, and its length is bounded by
//! § 9. Ropes and slices are a later representation change behind the same
//! interface, on the same terms as hidden classes.*
//!
//! **Immutable** is the load-bearing word and it is enforced by the shape:
//! there is no method here that changes a [`Text`], so a string cell handed out
//! by [`Heap::get`](crate::heap::Heap::get) cannot be edited by anything and a
//! reference to one is a reference to that text for as long as it exists. Two
//! things depend on it — the interning in [`intern`](super::intern), which
//! would hand out a key whose text had since changed, and every future
//! representation of a string, since a rope is only sound when nothing under it
//! moves.
//!
//! # UTF-16, and why not [`String`]
//!
//! [`crate::escape`] settled this for the lexer and the answer is the same
//! here: a JavaScript string is a sequence of 16-bit code units, `'\uD800'` is
//! a legal one-unit string standing for no character, and a page can build one
//! by slicing an emoji in half. Rust's [`String`] cannot hold that, so a text
//! that survived a round trip through one would have been quietly repaired —
//! and a page comparing it with what it sent would find them different.

use crate::bounds;

/// A string in the heap.
#[derive(Debug)]
pub struct Text {
    units: Vec<u16>,
}

impl Text {
    /// The string these code units spell, or [`None`] if there are more of them
    /// than [`bounds::LONGEST_STRING`] allows.
    ///
    /// The refusal is [`None`] rather than a [`Full`](crate::heap::Full)
    /// because the length is not the heap's business: a string one code unit
    /// past the limit is refused on a heap that is empty, and the caller turns
    /// that into the `RangeError` the language specifies (queue item 72, which
    /// is where there is a script to throw it in).
    pub fn of(units: Vec<u16>) -> Option<Self> {
        if too_long(units.len()) {
            return None;
        }
        Some(Self { units })
    }

    /// The code units, which are what a page compares and indexes.
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    /// How many code units — which is `String.prototype.length`, and is not a
    /// count of characters.
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// Whether this is the empty string.
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// What it owns beyond its slot: two bytes a code unit.
    pub fn footprint(&self) -> usize {
        self.units.len().saturating_mul(size_of::<u16>())
    }
}

/// Whether a string of this many code units is longer than we will make.
///
/// The deciding is a function so that it is asserted at its boundary rather
/// than by building the string — a test that made a string of
/// [`bounds::LONGEST_STRING`] code units would ask for two gibibytes of memory
/// to find out what a comparison already knows. That is the shape
/// [`next_life`](crate::heap::Ref) uses for a generation nobody can reach four
/// billion of, and the shape `alo-net` uses for every rule about bytes: a rule
/// about a limit is asserted honestly when nothing else is moving.
pub const fn too_long(units: usize) -> bool {
    units > bounds::LONGEST_STRING
}

#[cfg(test)]
mod tests {
    use super::{Text, bounds, too_long};

    #[test]
    fn a_string_keeps_the_code_units_it_was_given() {
        // A lone surrogate: legal, meaningless as a character, and the reason
        // this is not a Rust `String`.
        let text = Text::of(vec![0xD83D, 0x0041, 0xDE00]);
        assert_eq!(
            text.as_ref().map(Text::units),
            Some([0xD83D, 0x0041, 0xDE00].as_slice())
        );
        assert_eq!(text.map(|text| text.len()), Some(3));
    }

    #[test]
    fn the_length_bound_is_decided_rather_than_reached() {
        assert!(!too_long(0));
        assert!(!too_long(bounds::LONGEST_STRING));
        assert!(too_long(bounds::LONGEST_STRING + 1));
        assert!(too_long(usize::MAX));
    }
}
