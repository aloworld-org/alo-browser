/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One escape sequence, and what it stands for.
//!
//! Shared by string literals, template literals and identifiers, because all
//! three read the same `\u` and getting it right three times is getting it
//! right twice and wrong once.
//!
//! # Code units, not characters
//!
//! [`read`] appends **UTF-16 code units**, which is what a JavaScript string
//! is. `'\uD800'` is a legal program: it is a string of one code unit that is
//! half of a surrogate pair and stands for no character at all. A cooked value
//! held as a Rust `String` could not represent it, so nothing here goes through
//! `char` on the way out.
//!
//! # Three escapes are refused rather than read
//!
//! `\1` to `\7`, `\0` followed by a digit, and `\8` and `\9`. ADR 0013 § 3
//! refuses the legacy tail by name and these are the string literal's share of
//! it: all four are sloppy-mode only, and `'\101'` meaning `'A'` is a reading
//! nobody expects from code written this decade. A page that needs one is a
//! page that opens queue item 142.

use crate::error::{Reason, SyntaxError};
use crate::read;

/// Read the escape sequence beginning at `at`, which must be a `\`.
///
/// Appends what it stands for to `out` — nothing at all for a line
/// continuation — and answers the offset just past it.
///
/// # Errors
///
/// A malformed `\x` or `\u`, one of the three legacy escapes, or a source
/// that ended inside the escape.
pub fn read(source: &str, at: usize, out: &mut Vec<u16>) -> Result<usize, SyntaxError> {
    let after_backslash = at.saturating_add(1);
    let Some((c, after)) = read::next_char(source, after_backslash) else {
        return Err(SyntaxError::new(Reason::UnterminatedString, at));
    };
    match c {
        'n' => Ok(push_one(out, 0x000A, after)),
        't' => Ok(push_one(out, 0x0009, after)),
        'r' => Ok(push_one(out, 0x000D, after)),
        'b' => Ok(push_one(out, 0x0008, after)),
        'f' => Ok(push_one(out, 0x000C, after)),
        'v' => Ok(push_one(out, 0x000B, after)),
        'x' => {
            let (value, end) = two_hex_digits(source, after)?;
            push_code_point(out, value);
            Ok(end)
        }
        'u' => {
            let (value, end) = code_point(source, after)?;
            push_code_point(out, value);
            Ok(end)
        }
        '0' => {
            // `\0` is NUL, but only when no digit follows it: `\01` is the
            // legacy octal escape, and reading it as NUL then `1` would turn a
            // refused program into a wrong one.
            if read::char_at(source, after).is_some_and(|d| d.is_ascii_digit()) {
                Err(SyntaxError::new(Reason::LegacyOctalEscape, at))
            } else {
                Ok(push_one(out, 0x0000, after))
            }
        }
        '1'..='7' => Err(SyntaxError::new(Reason::LegacyOctalEscape, at)),
        '8' | '9' => Err(SyntaxError::new(Reason::NonOctalDecimalEscape, at)),
        _ if crate::unicode::is_line_terminator(c) => Ok(line_continuation(source, c, after)),
        // A `NonEscapeCharacter`: the character itself. `\a` is `a`, and — the
        // one that matters — `\'` is a quote that does not end the string.
        _ => {
            let mut buffer = [0u16; 2];
            out.extend_from_slice(c.encode_utf16(&mut buffer));
            Ok(after)
        }
    }
}

/// Skip the `\` before a line ending, and the line ending with it.
///
/// A `\r\n` is one line ending rather than two, which is why this is a function
/// rather than an offset.
pub fn line_continuation(source: &str, c: char, after: usize) -> usize {
    if c == '\r' && read::char_at(source, after) == Some('\n') {
        after.saturating_add(1)
    } else {
        after
    }
}

/// The `\uXXXX` or `\u{X…}` beginning at `at`, which is just past the `u`.
///
/// Answers the code point and the offset after it. Used by string literals for
/// their code units and by identifiers, which then ask whether the character it
/// stands for may be in a name.
///
/// # Errors
///
/// [`Reason::BadUnicodeEscape`] for anything that is not four hexadecimal
/// digits or a `{`…`}` with digits in it, and [`Reason::CodePointOutOfRange`]
/// for a braced value above U+10FFFF.
pub fn code_point(source: &str, at: usize) -> Result<(u32, usize), SyntaxError> {
    if read::char_at(source, at) == Some('{') {
        return braced_code_point(source, at);
    }
    let mut value = 0u32;
    let mut end = at;
    for _ in 0..4 {
        let Some(digit) = read::char_at(source, end).and_then(hex_value) else {
            return Err(SyntaxError::new(Reason::BadUnicodeEscape, at));
        };
        value = value.saturating_mul(16).saturating_add(digit);
        end = end.saturating_add(1);
    }
    Ok((value, end))
}

/// The `{X…}` half of a `\u{…}`, beginning at the `{`.
fn braced_code_point(source: &str, at: usize) -> Result<(u32, usize), SyntaxError> {
    let mut end = at.saturating_add(1);
    let mut value = 0u32;
    let mut digits = 0usize;
    let mut too_large = false;
    while let Some(digit) = read::char_at(source, end).and_then(hex_value) {
        digits = digits.saturating_add(1);
        // Leading zeros are legal and unbounded, so growth stops rather than
        // overflowing: once the value is past the last code point it can only
        // stay there, and the refusal is the same one either way.
        if !too_large {
            value = value.saturating_mul(16).saturating_add(digit);
            too_large = value > 0x0010_FFFF;
        }
        end = end.saturating_add(1);
    }
    if digits == 0 || read::char_at(source, end) != Some('}') {
        return Err(SyntaxError::new(Reason::BadUnicodeEscape, at));
    }
    if too_large {
        return Err(SyntaxError::new(Reason::CodePointOutOfRange, at));
    }
    Ok((value, end.saturating_add(1)))
}

/// The two hexadecimal digits of a `\xHH`.
fn two_hex_digits(source: &str, at: usize) -> Result<(u32, usize), SyntaxError> {
    let high = read::char_at(source, at).and_then(hex_value);
    let low = read::char_at(source, at.saturating_add(1)).and_then(hex_value);
    match (high, low) {
        (Some(high), Some(low)) => Ok((
            high.saturating_mul(16).saturating_add(low),
            at.saturating_add(2),
        )),
        _ => Err(SyntaxError::new(Reason::BadHexEscape, at)),
    }
}

/// What a hexadecimal digit is worth, or [`None`] for anything else.
pub fn hex_value(c: char) -> Option<u32> {
    c.to_digit(16)
}

/// Append one code unit and answer the offset given.
fn push_one(out: &mut Vec<u16>, unit: u16, end: usize) -> usize {
    out.push(unit);
    end
}

/// Append a code point as one code unit, or as a surrogate pair above U+FFFF.
///
/// A value in the surrogate range goes in **as it is**, which is the whole
/// reason this is not `char::from_u32`: `\uD800` is a lone surrogate and a
/// legal string.
fn push_code_point(out: &mut Vec<u16>, value: u32) {
    if let Ok(unit) = u16::try_from(value) {
        out.push(unit);
        return;
    }
    // Above U+FFFF, so it is not a surrogate and `char` can hold it — which is
    // what does the pairing, rather than shifts of our own. A value this crate
    // could not have produced is dropped rather than made into something: the
    // scanner refuses anything above U+10FFFF before it gets here.
    if let Some(c) = char::from_u32(value) {
        let mut buffer = [0u16; 2];
        out.extend_from_slice(c.encode_utf16(&mut buffer));
    }
}
