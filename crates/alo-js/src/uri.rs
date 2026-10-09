/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ECMA-262 § 19.2.6's `Encode`, for `encodeURIComponent` (queue item 359).
//!
//! A string is UTF-16 code units, and a URI is bytes, so encoding one is two
//! steps the specification spells out whole: a code unit in the *unreserved*
//! set is written as itself, and every other code point is written as the
//! bytes of its UTF-8, each as `%` and two **uppercase** hexadecimal digits.
//! A surrogate pair is one code point and four bytes; a surrogate alone stands
//! for no character, has no UTF-8, and is the `URIError` the specification
//! gives.
//!
//! This is arithmetic on code units and nothing else — no heap, no call — so
//! the builtin that answers it ([`crate::builtin`]'s `encodeURIComponent`) is
//! the conversion of its argument and the error, and this is the encoding.
//!
//! # Bounded by the longest string
//!
//! One code unit can become nine (`€` is `%E2%82%AC`), so a string the engine
//! could make may encode to one it may not. The output is checked against
//! [`LONGEST_STRING`] before every write, and the encoding stops the moment it
//! would pass it — work proportional to what was written, never to what a
//! hostile length would have asked for.
//!
//! # What is not here
//!
//! `encodeURI` leaves the *reserved* set alone as well, and `decodeURI` and
//! `decodeURIComponent` are the same section read backwards. Each is taken when
//! a page needs it (queue item 73), not before.

use crate::bounds::LONGEST_STRING;

/// Why a string cannot be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unencodable {
    /// A surrogate with no partner, which is no character and so has no UTF-8:
    /// the specification's `URIError`.
    LoneSurrogate {
        /// The surrogate.
        unit: u16,
        /// Where it is in the string, in code units.
        index: usize,
    },
    /// The encoding would be longer than [`LONGEST_STRING`]: the `RangeError`
    /// any string too long to make is.
    TooLong {
        /// How long it would have been when it passed the bound.
        units: usize,
    },
}

/// The digits a byte is written in, uppercase as the specification says.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Whether `unit` is in `encodeURIComponent`'s unreserved set, and so written
/// as itself: `uriAlpha`, `DecimalDigit` and `uriMark` (`-_.!~*'()`).
pub const fn is_unreserved(unit: u16) -> bool {
    matches!(
        unit,
        0x41..=0x5A // A-Z
            | 0x61..=0x7A // a-z
            | 0x30..=0x39 // 0-9
            | 0x2D // -
            | 0x5F // _
            | 0x2E // .
            | 0x21 // !
            | 0x7E // ~
            | 0x2A // *
            | 0x27 // '
            | 0x28 // (
            | 0x29 // )
    )
}

/// `Encode(string, "")` with `encodeURIComponent`'s unreserved set.
///
/// # Errors
///
/// [`Unencodable::LoneSurrogate`] at the first surrogate with no partner, and
/// [`Unencodable::TooLong`] the moment the output would pass
/// [`LONGEST_STRING`] — whichever the string reaches first, which is the order
/// the specification's loop would meet them in.
pub fn encode_component(string: &[u16]) -> Result<Vec<u16>, Unencodable> {
    encode_within(string, LONGEST_STRING)
}

/// [`encode_component`], with the bound named: [`LONGEST_STRING`] always,
/// except in this file's tests, which could not otherwise reach it without
/// writing half a gibibyte.
fn encode_within(string: &[u16], longest: usize) -> Result<Vec<u16>, Unencodable> {
    let mut out: Vec<u16> = Vec::with_capacity(string.len().min(longest));
    let mut index = 0;
    while let Some(&unit) = string.get(index) {
        if is_unreserved(unit) {
            grow(&out, 1, longest)?;
            out.push(unit);
            index = index.saturating_add(1);
            continue;
        }
        let (character, width) = code_point_at(string, index, unit)?;
        let mut buffer = [0u8; 4];
        let bytes = character.encode_utf8(&mut buffer).as_bytes();
        grow(&out, bytes.len().saturating_mul(3), longest)?;
        for &byte in bytes {
            out.push(u16::from(b'%'));
            out.extend(
                [byte >> 4, byte & 0x0F]
                    .iter()
                    .filter_map(|nibble| HEX.get(usize::from(*nibble)))
                    .map(|digit| u16::from(*digit)),
            );
        }
        index = index.saturating_add(width);
    }
    Ok(out)
}

/// Refuse to write `more` units if that would pass `longest`.
fn grow(out: &[u16], more: usize, longest: usize) -> Result<(), Unencodable> {
    let units = out.len().saturating_add(more);
    if units > longest {
        return Err(Unencodable::TooLong { units });
    }
    Ok(())
}

/// The specification's `CodePointAt(string, index)`, where `unit` is the code
/// unit there: the character, and how many code units it took.
///
/// # Errors
///
/// [`Unencodable::LoneSurrogate`] for a leading surrogate not followed by a
/// trailing one, or a trailing surrogate on its own.
fn code_point_at(string: &[u16], index: usize, unit: u16) -> Result<(char, usize), Unencodable> {
    let lone = Unencodable::LoneSurrogate { unit, index };
    match unit {
        0xD800..=0xDBFF => {
            let Some(&low @ 0xDC00..=0xDFFF) = string.get(index.saturating_add(1)) else {
                return Err(lone);
            };
            let high = u32::from(unit - 0xD800);
            let low = u32::from(low - 0xDC00);
            let code_point = 0x1_0000 + (high << 10) + low;
            char::from_u32(code_point)
                .map(|character| (character, 2))
                .ok_or(lone)
        }
        0xDC00..=0xDFFF => Err(lone),
        _ => char::from_u32(u32::from(unit))
            .map(|character| (character, 1))
            .ok_or(lone),
    }
}

#[cfg(test)]
mod tests {
    use super::{Unencodable, encode_component, encode_within, is_unreserved};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn encoded(text: &[u16]) -> Result<String, Unencodable> {
        encode_component(text).map(|out| String::from_utf16_lossy(&out))
    }

    #[test]
    fn the_unreserved_set_is_written_as_itself() {
        let unreserved = "ABCXYZabcxyz0189-_.!~*'()";
        assert_eq!(encoded(&units(unreserved)).as_deref(), Ok(unreserved));
        // Exactly the 71 the specification lists, and nothing else below 128.
        let count = (0..128u16).filter(|unit| is_unreserved(*unit)).count();
        assert_eq!(count, 26 + 26 + 10 + 9);
    }

    #[test]
    fn the_reserved_set_and_the_rest_of_ascii_are_escaped() {
        assert_eq!(
            encoded(&units(";/?:@&=+$,#")).as_deref(),
            Ok("%3B%2F%3F%3A%40%26%3D%2B%24%2C%23")
        );
        assert_eq!(
            encoded(&units(" \"%<>[]\\^`{|}")).as_deref(),
            Ok("%20%22%25%3C%3E%5B%5D%5C%5E%60%7B%7C%7D")
        );
        assert_eq!(encoded(&[0, 0x7F]).as_deref(), Ok("%00%7F"));
    }

    #[test]
    fn each_width_of_utf8_is_written_as_its_bytes() {
        // Two bytes, three, and the last of the basic plane.
        assert_eq!(encoded(&units("é")).as_deref(), Ok("%C3%A9"));
        assert_eq!(encoded(&units("€")).as_deref(), Ok("%E2%82%AC"));
        assert_eq!(encoded(&[0xFFFF]).as_deref(), Ok("%EF%BF%BF"));
        // A pair is one code point, four bytes.
        assert_eq!(encoded(&units("😀")).as_deref(), Ok("%F0%9F%98%80"));
        assert_eq!(encoded(&[0xDBFF, 0xDFFF]).as_deref(), Ok("%F4%8F%BF%BF"));
        assert_eq!(
            encoded(&units("/a b/é")).as_deref(),
            Ok("%2Fa%20b%2F%C3%A9")
        );
        assert_eq!(encoded(&[]).as_deref(), Ok(""));
    }

    #[test]
    fn a_lone_surrogate_is_refused_where_it_is() {
        let lone = |unit, index| Err(Unencodable::LoneSurrogate { unit, index });
        assert_eq!(encoded(&[0x61, 0xD800]), lone(0xD800, 1));
        assert_eq!(encoded(&[0xD800, 0x61]), lone(0xD800, 0));
        assert_eq!(encoded(&[0xDC00, 0xD800]), lone(0xDC00, 0));
        // A leading surrogate followed by another leading one.
        assert_eq!(encoded(&[0xD83D, 0xD83D, 0xDE00]), lone(0xD83D, 0));
        // A pair, then a trailing surrogate alone.
        assert_eq!(encoded(&[0xD83D, 0xDE00, 0xDE00]), lone(0xDE00, 2));
    }

    #[test]
    fn an_encoding_longer_than_any_string_is_refused_as_it_grows() {
        // Each `€` is nine units, so three fit in 27 and the fourth passes it.
        let euros = [0x20AC; 4];
        assert_eq!(
            encode_within(&euros, 27),
            Err(Unencodable::TooLong { units: 36 })
        );
        assert_eq!(encode_within(&euros[..3], 27).map(|out| out.len()), Ok(27));
        // One unreserved unit past the bound is refused as it is written…
        assert_eq!(
            encode_within(&units("abcd"), 3),
            Err(Unencodable::TooLong { units: 4 })
        );
        // …and a lone surrogate the string reaches first is what it says.
        assert_eq!(
            encode_within(&[0x61, 0xD800, 0x20AC], 3),
            Err(Unencodable::LoneSurrogate {
                unit: 0xD800,
                index: 1
            })
        );
        // A string a million units long is answered in one pass.
        let long = vec![0x20AC; 1 << 20];
        assert_eq!(encode_component(&long).map(|out| out.len()), Ok(9 << 20));
    }
}
