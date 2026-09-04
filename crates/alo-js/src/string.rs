/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! String literals.
//!
//! The value is a `Vec<u16>` for the reason [`crate::escape`] gives: a
//! JavaScript string is a sequence of UTF-16 code units, and `'\uD800'` is a
//! legal one-unit string that stands for no character.
//!
//! Two line terminators are refused inside a literal and two are not. Line feed
//! and carriage return end it — a quote left open is nearly always a typo, and
//! reading on would swallow the rest of the file. U+2028 and U+2029 have been
//! allowed since ES2019, because they turn up inside data nobody looked at and
//! refusing them broke JSON that had been embedded for a decade.

use crate::error::{Reason, SyntaxError};
use crate::{escape, read};

/// Read the string literal beginning at `at`, which must be a quote.
///
/// Answers its code units and the offset just past the closing quote.
///
/// # Errors
///
/// A quote never closed, a line ending inside the literal, or whatever an
/// escape in it refuses.
pub fn scan(source: &str, at: usize) -> Result<(Vec<u16>, usize), SyntaxError> {
    let Some((quote, mut end)) = read::next_char(source, at) else {
        return Err(SyntaxError::new(Reason::UnterminatedString, at));
    };
    let mut units = Vec::new();
    loop {
        let Some((c, after)) = read::next_char(source, end) else {
            return Err(SyntaxError::new(Reason::UnterminatedString, at));
        };
        if c == quote {
            return Ok((units, after));
        }
        if c == '\\' {
            end = escape::read(source, end, &mut units)?;
            continue;
        }
        if c == '\n' || c == '\r' {
            return Err(SyntaxError::new(Reason::LineTerminatorInString, end));
        }
        let mut buffer = [0u16; 2];
        units.extend_from_slice(c.encode_utf16(&mut buffer));
        end = after;
    }
}
