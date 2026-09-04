/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Reading characters out of source text without ever indexing into it.
//!
//! Every scanner in this crate walks a `&str` by byte offset, and a byte offset
//! into UTF-8 is the one arithmetic in a lexer that can be wrong: an offset in
//! the middle of a character panics on `&source[at..]`, and a lexer that panics
//! on a stranger's script is a denial of service in a renderer (ADR 0013 § 4).
//!
//! So there is no `&source[..]` anywhere in this crate. These four functions
//! are the only way source text is read, they all answer [`None`] rather than
//! panicking, and the workspace's `indexing_slicing` lint keeps it that way.

/// The character at `at`, or [`None`] at the end of the source.
///
/// Answers [`None`] rather than panicking for an offset inside a character or
/// past the end. Neither can happen — every offset in this crate comes from a
/// previous character's length — and the point is that it does not have to be
/// true for this to be safe.
pub fn char_at(source: &str, at: usize) -> Option<char> {
    source.get(at..).and_then(|rest| rest.chars().next())
}

/// The character at `at` and the offset just after it.
pub fn next_char(source: &str, at: usize) -> Option<(char, usize)> {
    let c = char_at(source, at)?;
    Some((c, at.saturating_add(c.len_utf8())))
}

/// Whether the source reads `text` at `at`.
pub fn starts_with(source: &str, at: usize, text: &str) -> bool {
    source.get(at..).is_some_and(|rest| rest.starts_with(text))
}

/// The text between two offsets.
///
/// Empty for a range that is not a character boundary or runs past the end,
/// which cannot happen for offsets this crate produced. Empty rather than a
/// panic for the reason at the top of this file: a wrong answer that a test
/// catches is better than a crash a page causes.
pub fn slice(source: &str, from: usize, to: usize) -> &str {
    source.get(from..to).unwrap_or_default()
}
