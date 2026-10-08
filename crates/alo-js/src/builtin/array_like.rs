/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an array-like is to a builtin that walks one: a length no longer
//! than 2⁵³ − 1, and the key each index is.
//!
//! An array iterator's `next` (queue item 230), `RegExp.prototype.exec`'s
//! `lastIndex` (item 74) and `Array.prototype.forEach` (item 331) each read
//! a length and then a property per index. They share these two rules
//! rather than each spelling them: a length that one of them clamped
//! differently from another, or an index past 2³² − 2 that one of them
//! made an array index, would be the same page answered two ways.

use crate::abrupt::Escape;
use crate::numeric;
use crate::object::Key;
use crate::object::native::Call;

/// The largest length an array-like may have: 2⁵³ − 1, `ToLength`'s ceiling,
/// which an `f64` holds exactly along with every whole number below it.
pub(super) const LONGEST: f64 = 9_007_199_254_740_991.0;

/// `ToLength`: a whole number from zero to 2⁵³ − 1, with `NaN` as zero.
pub(super) fn to_length(number: f64) -> f64 {
    if number.is_nan() || number <= 0.0 {
        return 0.0;
    }
    number.trunc().min(LONGEST)
}

/// `ToString(index)` as a key: an array index below 2³² − 1, and the
/// canonical spelling of the number above it, which is an ordinary string
/// key.
///
/// **May allocate**, interning the spelling; whatever the caller means to
/// keep must be on the stack or in a slot first.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling.
pub(super) fn key_of(call: &mut Call<'_>, index: f64) -> Result<Key, Escape> {
    if let Some(key) = crate::object::array::exact_length(index).and_then(Key::index) {
        return Ok(key);
    }
    let at = call.at();
    let units: Vec<u16> = numeric::text_of(index).encode_utf16().collect();
    call.objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))
}

#[cfg(test)]
mod tests {
    use super::{LONGEST, to_length};

    #[test]
    fn a_length_is_whole_and_between_zero_and_two_to_the_fifty_three_less_one() {
        for (number, length) in [
            (f64::NAN, 0.0),
            (-0.0, 0.0),
            (-5.0, 0.0),
            (f64::NEG_INFINITY, 0.0),
            (2.9, 2.0),
            (LONGEST, LONGEST),
            (LONGEST + 2.0, LONGEST),
            (f64::INFINITY, LONGEST),
        ] {
            assert_eq!(to_length(number).to_bits(), length.to_bits(), "{number}");
        }
    }
}
