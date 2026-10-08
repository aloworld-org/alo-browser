/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a regular expression's flags say, read once.
//!
//! The lexer has already refused an unknown flag, a repeated one and `u` with
//! `v` ([`literal`](super::literal)). This turns the string it kept into the
//! set of flags the parser, the compiler and the matcher each ask about, and
//! keeps the string itself: `[[OriginalFlags]]` is what a page reads back, in
//! the order its author wrote it.
//!
//! A set of letters rather than eight fields, because that is what a flag
//! string is: each letter present or not, and nothing else.

/// One flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// `d`: a match carries the indices of its captures.
    HasIndices,
    /// `g`: a match starts at `lastIndex` and moves it.
    Global,
    /// `i`: letters match whatever their case.
    IgnoreCase,
    /// `m`: `^` and `$` match at every line ending.
    Multiline,
    /// `s`: `.` matches a line ending too.
    DotAll,
    /// `u`: the pattern and the input are code points rather than code units.
    Unicode,
    /// `v`: `u`, and classes that are sets with operations on them.
    UnicodeSets,
    /// `y`: a match starts exactly at `lastIndex` or not at all.
    Sticky,
}

impl Flag {
    /// The flag a letter is.
    const fn of(letter: char) -> Option<Self> {
        Some(match letter {
            'd' => Flag::HasIndices,
            'g' => Flag::Global,
            'i' => Flag::IgnoreCase,
            'm' => Flag::Multiline,
            's' => Flag::DotAll,
            'u' => Flag::Unicode,
            'v' => Flag::UnicodeSets,
            'y' => Flag::Sticky,
            _ => return None,
        })
    }

    /// Its bit in a [`Flags`].
    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A regular expression's flags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flags {
    /// One bit per [`Flag`].
    set: u8,
    /// The flags as they were written.
    original: String,
}

impl Flags {
    /// Read a flag string, or [`None`] for one that is not a regular
    /// expression's — a flag that means nothing, one written twice, or both
    /// Unicode modes. The lexer refuses each of those first, so [`None`] here
    /// is a string that did not come from a literal.
    pub fn of(written: &str) -> Option<Self> {
        let mut set = 0_u8;
        for letter in written.chars() {
            let bit = Flag::of(letter)?.bit();
            if set & bit != 0 {
                return None;
            }
            set |= bit;
        }
        let flags = Self {
            set,
            original: written.to_owned(),
        };
        if flags.has(Flag::Unicode) && flags.has(Flag::UnicodeSets) {
            return None;
        }
        Some(flags)
    }

    /// Whether `flag` is one of them.
    pub const fn has(&self, flag: Flag) -> bool {
        self.set & flag.bit() != 0
    }

    /// Whether the pattern reads code points: `u` or `v`. The specification
    /// calls this *full Unicode*, and it is what decides whether a surrogate
    /// pair is one character or two.
    pub const fn code_points(&self) -> bool {
        self.has(Flag::Unicode) || self.has(Flag::UnicodeSets)
    }

    /// The flags as they were written.
    pub fn original(&self) -> &str {
        &self.original
    }
}

#[cfg(test)]
mod tests {
    use super::{Flag, Flags};

    #[test]
    fn each_flag_is_read_and_the_written_order_is_kept() {
        let Some(flags) = Flags::of("ygm") else {
            panic!("three flags a literal may have");
        };
        assert!(flags.has(Flag::Global) && flags.has(Flag::Multiline) && flags.has(Flag::Sticky));
        assert!(!flags.has(Flag::IgnoreCase) && !flags.code_points());
        assert_eq!(flags.original(), "ygm");
        assert!(Flags::of("v").is_some_and(|flags| flags.code_points()));
        let Some(all) = Flags::of("dgimsy") else {
            panic!("six flags at once");
        };
        assert!(all.has(Flag::HasIndices) && all.has(Flag::DotAll));
    }

    #[test]
    fn what_the_lexer_refuses_is_refused_here_too() {
        for written in ["x", "gg", "uv"] {
            assert_eq!(Flags::of(written), None, "{written}");
        }
    }
}
