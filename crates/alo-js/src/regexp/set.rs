/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A set of characters, as sorted ranges: what a class compiles to.
//!
//! A class is a union of characters, ranges and escapes, and every one of
//! them is a range or a short list of ranges — so the set is the list, kept
//! sorted and merged, and asking whether a character is in it is a binary
//! search. A complement (`\D`, `\W`, `\S` inside a class) is taken against the
//! characters the mode has: code units, up to U+FFFF, without `u`; code
//! points, up to U+10FFFF, with it.
//!
//! The three escapes are the specification's own lists and are written out
//! here. `\s` is `WhiteSpace` and `LineTerminator`; `\w` is the ASCII letters,
//! digits and `_` — the wider `\w` that `u` with `i` makes is item 322's.

/// The characters `\d` is.
const DIGITS: [(u32, u32); 1] = [(0x30, 0x39)];

/// The characters `\w` is.
const WORD: [(u32, u32); 4] = [(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)];

/// The characters `\s` is: `WhiteSpace`, which is the three controls,
/// `ZWNBSP` and `Space_Separator`, and the four `LineTerminator`s.
const SPACE: [(u32, u32); 10] = [
    (0x09, 0x0D),
    (0x20, 0x20),
    (0xA0, 0xA0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
    (0xFEFF, 0xFEFF),
];

/// Which of the three escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// `\d`.
    Digit,
    /// `\s`.
    Space,
    /// `\w`.
    Word,
}

impl Named {
    /// Its ranges.
    const fn ranges(self) -> &'static [(u32, u32)] {
        match self {
            Named::Digit => &DIGITS,
            Named::Space => &SPACE,
            Named::Word => &WORD,
        }
    }
}

/// A set of characters, and whether a match is being in it or not being.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Set {
    /// Sorted, merged, inclusive ranges.
    ranges: Vec<(u32, u32)>,
    /// `[^…]`: a character matches when it is **not** in the ranges.
    negated: bool,
}

impl Set {
    /// An empty set.
    pub const fn new() -> Self {
        Self {
            ranges: Vec::new(),
            negated: false,
        }
    }

    /// Add one character.
    pub fn add(&mut self, c: u32) {
        self.add_range(c, c);
    }

    /// Add a range, both ends included.
    pub fn add_range(&mut self, from: u32, to: u32) {
        self.ranges.push((from, to));
    }

    /// Add one of the escapes, or everything the mode has but it.
    pub fn add_named(&mut self, named: Named, complement: bool, most: u32) {
        if complement {
            for (from, to) in complement_of(named.ranges(), most) {
                self.ranges.push((from, to));
            }
        } else {
            self.ranges.extend_from_slice(named.ranges());
        }
    }

    /// Make it match what is not in it.
    pub const fn negate(&mut self) {
        self.negated = true;
    }

    /// Whether `c` matches. The set must be [`Set::finished`].
    pub fn matches(&self, c: u32) -> bool {
        let found = self
            .ranges
            .binary_search_by(|(from, to)| {
                if *to < c {
                    std::cmp::Ordering::Less
                } else if *from > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok();
        found != self.negated
    }

    /// Sort and merge, so a lookup is a binary search: the set is finished.
    ///
    /// Once, after everything is added, rather than after each addition — a
    /// class of a million characters is a stranger's pattern, and sorting
    /// after each would make compiling it quadratic.
    #[must_use]
    pub fn finished(mut self) -> Self {
        self.normalise();
        self
    }

    /// [`Set::finished`]'s work.
    fn normalise(&mut self) {
        self.ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(self.ranges.len());
        for (from, to) in self.ranges.drain(..) {
            match merged.last_mut() {
                Some(last) if from <= last.1.saturating_add(1) => last.1 = last.1.max(to),
                _ => merged.push((from, to)),
            }
        }
        self.ranges = merged;
    }
}

/// Everything from zero to `most` that is not in `ranges`, which are sorted.
fn complement_of(ranges: &[(u32, u32)], most: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut next = 0_u32;
    for (from, to) in ranges {
        if *from > next {
            out.push((next, from.saturating_sub(1)));
        }
        next = to.saturating_add(1);
    }
    if next <= most {
        out.push((next, most));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Named, Set};

    #[test]
    fn ranges_merge_and_a_lookup_finds_their_ends() {
        let mut set = Set::new();
        set.add_range(u32::from('d'), u32::from('f'));
        set.add_range(u32::from('a'), u32::from('c'));
        set.add(u32::from('x'));
        let set = set.finished();
        for c in "abcdefx".chars() {
            assert!(set.matches(u32::from(c)), "{c}");
        }
        for c in "gw`y".chars() {
            assert!(!set.matches(u32::from(c)), "{c}");
        }
    }

    #[test]
    fn a_complement_is_taken_against_the_modes_characters() {
        let mut not_digits = Set::new();
        not_digits.add_named(Named::Digit, true, 0xFFFF);
        let not_digits = not_digits.finished();
        assert!(!not_digits.matches(u32::from('5')));
        assert!(not_digits.matches(u32::from('a')));
        assert!(not_digits.matches(0xFFFF));
        assert!(!not_digits.matches(0x1_0000), "a code unit is all there is");

        let mut negated = Set::new();
        negated.add_named(Named::Space, false, 0x10_FFFF);
        negated.negate();
        let negated = negated.finished();
        assert!(negated.matches(u32::from('a')));
        assert!(!negated.matches(0x2029));
        assert!(!negated.matches(0xFEFF));
    }
}
