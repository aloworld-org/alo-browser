/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Regular expression literals: where one ends, and what its flags are.
//!
//! **Not what the pattern means.** `/[a-z]+/` is a body and a flag set here,
//! and whether the body is a pattern anybody can compile is queue item 74's —
//! including the bound on the work it may do, which is where a catastrophic
//! backtrack in a renderer would be a denial of service. Splitting it that way
//! is the specification's own split: the lexical grammar only ever asks where
//! the literal *ends*.
//!
//! Which is a question with one hard part. `/` closes the literal, except
//! inside a character class, where it is an ordinary character:
//! `/[/]/` is one literal and not two. So the scan tracks whether it is inside
//! `[`…`]`, and that single flag is the whole difficulty.
//!
//! # Flags are checked here because they change what the body means
//!
//! `u` and `v` decide whether a pattern reads characters or code units, and
//! they cannot both apply. An unknown flag is refused by name rather than
//! ignored: a page that wrote `/x/z` meant something, and ignoring it would run
//! a different regular expression than the author asked for.

use crate::error::{Reason, SyntaxError};
use crate::{read, unicode};

/// A regular expression literal, unread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// The pattern, exactly as written and between the slashes.
    pub body: String,
    /// The flags after the closing slash, in the order they were written.
    pub flags: String,
}

/// Every flag the language has.
///
/// `d` indices, `g` global, `i` ignore case, `m` multiline, `s` dot matches a
/// line ending, `u` Unicode, `v` Unicode sets, `y` sticky.
const FLAGS: &str = "dgimsuvy";

/// Read the regular expression literal beginning at `at`, which must be a `/`.
///
/// The caller has already decided that a regular expression is what may appear
/// here — see [`crate::Goal`]. Answers the literal and the offset just past its
/// flags.
///
/// # Errors
///
/// A literal the source ended inside, a line ending in one, an unknown or
/// repeated flag, or both Unicode modes at once.
pub fn scan(source: &str, at: usize) -> Result<(Literal, usize), SyntaxError> {
    let body_from = at.saturating_add(1);
    let mut end = body_from;
    let mut in_a_class = false;
    let body_to = loop {
        let Some((c, after)) = read::next_char(source, end) else {
            return Err(SyntaxError::new(Reason::UnterminatedRegularExpression, at));
        };
        if unicode::is_line_terminator(c) {
            return Err(SyntaxError::new(
                Reason::LineTerminatorInRegularExpression,
                end,
            ));
        }
        match c {
            '\\' => {
                // The escaped character is whatever it is — the pattern's
                // grammar decides that — but it is not a terminator, and it is
                // not allowed to be a line ending.
                let Some((next, past)) = read::next_char(source, after) else {
                    return Err(SyntaxError::new(Reason::UnterminatedRegularExpression, at));
                };
                if unicode::is_line_terminator(next) {
                    return Err(SyntaxError::new(
                        Reason::LineTerminatorInRegularExpression,
                        after,
                    ));
                }
                end = past;
            }
            '[' => {
                in_a_class = true;
                end = after;
            }
            ']' => {
                in_a_class = false;
                end = after;
            }
            '/' if !in_a_class => break end,
            _ => end = after,
        }
    };
    let (flags, after_flags) = flags(source, body_to.saturating_add(1))?;
    Ok((
        Literal {
            body: read::slice(source, body_from, body_to).to_owned(),
            flags,
        },
        after_flags,
    ))
}

/// The flags after the closing slash, checked.
fn flags(source: &str, at: usize) -> Result<(String, usize), SyntaxError> {
    let mut flags = String::new();
    let mut end = at;
    while let Some((c, after)) = read::next_char(source, end) {
        if !unicode::continues_a_name(c) {
            break;
        }
        if !FLAGS.contains(c) {
            return Err(SyntaxError::new(
                Reason::UnknownRegularExpressionFlag(c),
                end,
            ));
        }
        if flags.contains(c) {
            return Err(SyntaxError::new(
                Reason::RepeatedRegularExpressionFlag(c),
                end,
            ));
        }
        flags.push(c);
        end = after;
    }
    if flags.contains('u') && flags.contains('v') {
        return Err(SyntaxError::new(Reason::BothUnicodeModes, at));
    }
    Ok((flags, end))
}
