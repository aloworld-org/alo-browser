/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What names a property, and the reading that decides where it comes in the
//! order.
//!
//! ADR 0014 § 11. A property key is a string or a symbol, and a key comparison
//! is the single most frequent thing an engine does — so keys are **interned**:
//! one string cell per distinct text, and comparing two keys is comparing two
//! [`Ref`]s. [`Objects::key`](super::Objects::key) is the only thing that makes
//! a text key, which is what makes that an invariant rather than a hope.
//!
//! # The array index is a different key, not a faster one
//!
//! `a[0]` and `a["0"]` are the same property, and the specification orders
//! *integer-like* keys ahead of every other one, ascending. So a key whose text
//! is the **canonical** decimal of an array index is that index and is never a
//! string: `Key::Index(1)` rather than a reference to a string cell holding
//! `"1"`. Two things fall out and both are wanted — such a key costs no
//! allocation at all, so `a[i]` in a loop interns nothing, and the ordering rule
//! is a property of the type rather than a comparison done at enumeration time.
//!
//! **Canonical** is the whole subtlety, and [`array_index`] is where it lives.
//! `"01"`, `"+1"`, `"-0"`, `" 1"` and `"1.0"` are *not* array indices: they are
//! ordinary string keys that happen to look numeric, they sort in insertion
//! order with the other strings, and a page can tell. The upper limit is
//! 2³²−2, so `"4294967295"` is a string key too — the specification reserves
//! 2³²−1 because an array's `length` could not otherwise hold one past it.

use crate::heap::Ref;

/// The largest array index there is: 2³²−2.
///
/// One below [`u32::MAX`], because an array holding an element at 2³²−1 would
/// need a `length` of 2³², which the specification's `length` cannot hold. A
/// property named `"4294967295"` is therefore an ordinary string key, and this
/// constant is what says so in one place rather than in three comparisons.
pub const LARGEST_INDEX: u32 = u32::MAX - 1;

/// What names a property.
///
/// A struct rather than a bare enum so the variants cannot be built from
/// outside: a `Key` naming a string that was never interned would compare
/// unequal to the same text interned properly, and the table would then hold
/// one property under two names. The constructors are [`Key::index`] and
/// [`Objects::key`](super::Objects::key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(Kind);

/// Which of the three kinds of name this is.
///
/// The order of the variants is the specification's enumeration order, which is
/// why deriving [`Ord`] gives it — though nothing depends on that derive, since
/// [`Properties::keys`](super::Properties::keys) orders by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Kind {
    /// An array index, held as the number it is.
    Index(u32),
    /// An interned string.
    Text(Ref),
    /// A symbol, which is its own identity.
    Symbol(Ref),
}

impl Key {
    /// The key for an array index.
    ///
    /// [`None`] above [`LARGEST_INDEX`], where the caller has a number that is
    /// not an index and whose key is the string of its digits.
    pub const fn index(at: u32) -> Option<Self> {
        if at > LARGEST_INDEX {
            return None;
        }
        Some(Self(Kind::Index(at)))
    }

    /// The key for an interned string.
    pub(super) const fn text(interned: Ref) -> Self {
        Self(Kind::Text(interned))
    }

    /// The key for a symbol.
    pub(super) const fn symbol(held: Ref) -> Self {
        Self(Kind::Symbol(held))
    }

    /// The index this names, if it names one.
    pub const fn as_index(self) -> Option<u32> {
        match self.0 {
            Kind::Index(at) => Some(at),
            Kind::Text(_) | Kind::Symbol(_) => None,
        }
    }

    /// The interned string this names, if it names one.
    pub const fn as_text(self) -> Option<Ref> {
        match self.0 {
            Kind::Text(held) => Some(held),
            Kind::Index(_) | Kind::Symbol(_) => None,
        }
    }

    /// The symbol this names, if it names one.
    pub const fn as_symbol(self) -> Option<Ref> {
        match self.0 {
            Kind::Symbol(held) => Some(held),
            Kind::Index(_) | Kind::Text(_) => None,
        }
    }

    /// The heap reference this key holds, which the collector must keep alive.
    ///
    /// An index holds none, which is the other half of why an indexed property
    /// costs nothing: there is no cell to keep.
    pub const fn reference(self) -> Option<Ref> {
        match self.0 {
            Kind::Text(held) | Kind::Symbol(held) => Some(held),
            Kind::Index(_) => None,
        }
    }
}

/// The array index this text is the canonical decimal of, if it is one.
///
/// The specification's `CanonicalNumericIndexString` narrowed to the integer
/// case, and it is a *round trip*: the text is an index exactly when writing
/// that index back out gives the same text. That is why every near miss is
/// refused — `"01"`, `"+1"`, `"1.0"`, `" 1"`, `"-0"`, an empty text, and
/// anything above [`LARGEST_INDEX`] — and why the reading is written as digits
/// rather than as a parse of a possibly-huge number.
///
/// It takes UTF-16 code units because that is what a JavaScript string is
/// ([`crate::escape`]), and a digit is one unit in every case that matters
/// here: the Unicode digits of other scripts are **not** ASCII digits and are
/// correctly refused.
pub fn array_index(units: &[u16]) -> Option<u32> {
    const ZERO: u16 = b'0' as u16;
    const NINE: u16 = b'9' as u16;

    let (first, rest) = units.split_first()?;
    if *first < ZERO || *first > NINE {
        return None;
    }
    // A leading zero is canonical only when it is the whole of it: `"0"` is the
    // index nought and `"01"` is a string key that reads like one.
    if *first == ZERO && !rest.is_empty() {
        return None;
    }

    let mut at: u32 = 0;
    for unit in units {
        if *unit < ZERO || *unit > NINE {
            return None;
        }
        let digit = u32::from(*unit - ZERO);
        // Saturating would answer `LARGEST_INDEX` for a hundred-digit number,
        // which is a wrong answer that reads like a right one. A number too
        // large to be an index is a string key.
        at = at.checked_mul(10)?.checked_add(digit)?;
        if at > LARGEST_INDEX {
            return None;
        }
    }
    Some(at)
}

/// The canonical decimal of an array index, in code units.
///
/// The other direction of [`array_index`]'s round trip, and what an enumeration
/// hands back when a page asks for the keys of an object as strings.
pub fn digits(at: u32) -> Vec<u16> {
    let mut units = Vec::new();
    let mut left = at;
    loop {
        let digit = u16::try_from(left % 10).unwrap_or(0);
        units.push(digit + u16::from(b'0'));
        left /= 10;
        if left == 0 {
            break;
        }
    }
    units.reverse();
    units
}

#[cfg(test)]
mod tests {
    use super::{LARGEST_INDEX, array_index, digits};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_canonical_decimal_is_an_index() {
        assert_eq!(array_index(&units("0")), Some(0));
        assert_eq!(array_index(&units("1")), Some(1));
        assert_eq!(array_index(&units("42")), Some(42));
        assert_eq!(array_index(&units("4294967294")), Some(LARGEST_INDEX));
    }

    #[test]
    fn everything_that_only_looks_like_one_is_a_string_key() {
        // Each of these is a property a page can define and enumerate, and each
        // must come after the indices in insertion order rather than among
        // them.
        for text in ["", "01", "+1", "-1", "-0", " 1", "1 ", "1.0", "1e2", "０"] {
            assert_eq!(array_index(&units(text)), None, "{text:?}");
        }
    }

    #[test]
    fn the_reserved_index_and_everything_above_it_is_a_string_key() {
        // 2³²−1 is reserved so that an array's `length` can hold one past the
        // last index, and a hundred-digit number must not saturate into one.
        assert_eq!(array_index(&units("4294967295")), None);
        assert_eq!(array_index(&units("4294967296")), None);
        assert_eq!(array_index(&units(&"9".repeat(100))), None);
    }

    #[test]
    fn an_index_round_trips_through_its_digits() {
        for at in [0, 1, 9, 10, 99, 100, 65_536, LARGEST_INDEX] {
            assert_eq!(array_index(&digits(at)), Some(at), "{at}");
        }
    }
}
