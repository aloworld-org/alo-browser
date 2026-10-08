/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Surrogate pairs: the one piece of UTF-16 both the pattern's reader and
//! the matcher need.
//!
//! With `u` or `v` a pair of code units is one character, in a pattern's
//! text and in the string it searches alike, and a half of a pair standing
//! alone is a character of its own. Without either, every code unit is a
//! character and none of this is asked.

/// Whether a code unit is the first half of a surrogate pair.
pub const fn is_lead(c: u32) -> bool {
    matches!(c, 0xD800..=0xDBFF)
}

/// Whether a code unit is the second half of one.
pub const fn is_trail(c: u32) -> bool {
    matches!(c, 0xDC00..=0xDFFF)
}

/// The code point a lead and a trail spell. Both must be what their names
/// say; anything else is answered without overflow but means nothing.
pub const fn combined(lead: u32, trail: u32) -> u32 {
    0x10000_u32
        .wrapping_add(lead.wrapping_sub(0xD800).wrapping_shl(10))
        .wrapping_add(trail.wrapping_sub(0xDC00))
}

#[cfg(test)]
mod tests {
    use super::{combined, is_lead, is_trail};

    #[test]
    fn a_pair_is_the_code_point_it_spells() {
        assert!(is_lead(0xD83D) && !is_trail(0xD83D));
        assert!(is_trail(0xDE00) && !is_lead(0xDE00));
        assert_eq!(combined(0xD83D, 0xDE00), 0x1F600);
        assert_eq!(combined(0xDBFF, 0xDFFF), 0x10_FFFF);
    }
}
