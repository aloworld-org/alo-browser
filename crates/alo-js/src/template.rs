/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Template literals: the pieces between the substitutions.
//!
//! # A template is scanned in pieces, and the parser decides where each begins
//!
//! `` `a${b}c${d}e` `` is four tokens with two expressions between them, and
//! the `}` that ends a substitution is the same character as the `}` that ends
//! a block. Nothing in a token stream can tell them apart — only the parser
//! knows it is inside a substitution — so this is the second thing
//! [`crate::Goal`] exists for, and a caller that asks for
//! [`crate::Goal::TemplateContinuation`] anywhere else gets
//! [`crate::Reason::NotATemplateContinuation`] rather than a `}` read as a
//! brace.
//!
//! # Two values, because a tag can see what the author wrote
//!
//! An ordinary template is its **cooked** value: escapes resolved. A tagged
//! template is handed the **raw** text as well, which is why `` String.raw`\n`
//! `` is a backslash and an `n`. Both are kept, and both normalise `\r\n` and a
//! lone `\r` to a single `\n` — the specification does that in the raw value
//! too, so that a file saved on Windows and one saved anywhere else are the
//! same program.
//!
//! # An escape nobody can read is not an error here
//!
//! `` tag`\unicode` `` is legal: a tagged template may hold anything, and its
//! cooked value is `undefined` for the piece that could not be read. So a bad
//! escape sets [`Piece::cooked`] to [`None`] rather than refusing, and it is
//! the **parser** that refuses when such a template has no tag. Refusing here
//! would be this file deciding a thing it cannot see.

use crate::error::{Reason, SyntaxError};
use crate::{escape, read};

/// Which piece of a template this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// `` `no substitutions` `` — the whole of it.
    Whole,
    /// `` `up to the first ${ `` .
    Head,
    /// `} between two ${`
    Middle,
    /// `` } to the end` ``
    Tail,
}

/// One piece of a template.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// Which piece it is.
    pub part: Part,
    /// The text as it was written, with line endings normalised.
    pub raw: String,
    /// The text with its escapes resolved, or [`None`] if one could not be
    /// read — which only a tagged template may have.
    pub cooked: Option<Vec<u16>>,
}

/// Read the template piece beginning at `at`, which must be a `` ` `` or a `}`.
///
/// Answers the piece and the offset just past the `` ` `` or `${` that ended
/// it.
///
/// # Errors
///
/// A template the source ended inside, or a caller that asked for a
/// continuation where there is no `}`. **Not** a bad escape, which is the whole
/// of the last section above.
pub fn scan(source: &str, at: usize) -> Result<(Piece, usize), SyntaxError> {
    let opener = read::char_at(source, at);
    let opens = match opener {
        Some('`') => true,
        Some('}') => false,
        _ => return Err(SyntaxError::new(Reason::NotATemplateContinuation, at)),
    };
    let content = at.saturating_add(1);
    let mut end = content;
    let mut cooked = Some(Vec::new());
    loop {
        let Some((c, after)) = read::next_char(source, end) else {
            return Err(SyntaxError::new(Reason::UnterminatedTemplate, at));
        };
        match c {
            '`' => {
                let part = if opens { Part::Whole } else { Part::Tail };
                return Ok((finish(source, part, content, end, cooked), after));
            }
            '$' if read::char_at(source, after) == Some('{') => {
                let part = if opens { Part::Head } else { Part::Middle };
                return Ok((
                    finish(source, part, content, end, cooked),
                    after.saturating_add(1),
                ));
            }
            '\\' => end = past_escape(source, end, &mut cooked),
            '\r' => {
                // A `\r\n` and a lone `\r` are both one line feed, in the
                // cooked value and in the raw one. The raw value is normalised
                // in `finish`, which is why only the cooked one is written
                // here.
                if let Some(units) = cooked.as_mut() {
                    units.push(0x000A);
                }
                end = if read::char_at(source, after) == Some('\n') {
                    after.saturating_add(1)
                } else {
                    after
                };
            }
            _ => {
                if let Some(units) = cooked.as_mut() {
                    let mut buffer = [0u16; 2];
                    units.extend_from_slice(c.encode_utf16(&mut buffer));
                }
                end = after;
            }
        }
    }
}

/// Read an escape into the cooked value, or give up on cooking and step over it.
///
/// Stepping over exactly one character after the backslash is what keeps the
/// raw scan right: `` `\` `` is a template whose one character is a backtick
/// that does not end it, and a scanner that stopped at a bad escape would end
/// the template in the wrong place.
fn past_escape(source: &str, at: usize, cooked: &mut Option<Vec<u16>>) -> usize {
    if let Some(units) = cooked.as_mut() {
        match escape::read(source, at, units) {
            Ok(after) => return after,
            Err(_) => *cooked = None,
        }
    }
    let after_backslash = at.saturating_add(1);
    match read::next_char(source, after_backslash) {
        Some((c, after)) if crate::unicode::is_line_terminator(c) => {
            escape::line_continuation(source, c, after)
        }
        Some((_, after)) => after,
        None => after_backslash,
    }
}

/// Cut the raw text out of the source and normalise its line endings.
fn finish(source: &str, part: Part, from: usize, to: usize, cooked: Option<Vec<u16>>) -> Piece {
    let raw = read::slice(source, from, to)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    Piece { part, raw, cooked }
}
