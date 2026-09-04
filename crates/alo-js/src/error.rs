/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the lexer says instead of stopping.
//!
//! ADR 0013 § 4: a script is a stranger's bytes that we then execute, and the
//! engine never panics on any source text. **A refusal is a result.** So every
//! scanner in this crate returns one of these, every one of them names what was
//! wrong rather than saying "syntax error", and each carries the offset it
//! happened at so that a message can point at the character.
//!
//! # Why each refusal is a variant rather than a string
//!
//! A test asserts on a variant. A message somebody reworded in a hurry then
//! fails no test at all, and a refusal nothing asserts on is a refusal that
//! quietly turns into an acceptance — which for the three legacy forms below is
//! ADR 0013 § 3 being reversed by accident.

use crate::read;

/// A refusal, and where in the source it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// What was wrong.
    pub reason: Reason,
    /// The byte offset it was found at.
    ///
    /// A byte offset rather than a line and column, because that is what the
    /// scanner has and because turning one into the other is a scan of the
    /// source that is only worth doing for an error somebody is shown.
    /// [`Position::of`] does it.
    pub at: usize,
}

impl SyntaxError {
    /// A refusal at an offset.
    pub fn new(reason: Reason, at: usize) -> Self {
        Self { reason, at }
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for SyntaxError {}

/// What was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The source is longer than [`crate::bounds::LONGEST_SOURCE`].
    SourceTooLong {
        /// How many bytes arrived.
        bytes: usize,
        /// How many are allowed.
        most: usize,
    },
    /// A `/*` with no `*/` after it.
    UnterminatedComment,
    /// A string literal with no closing quote.
    UnterminatedString,
    /// A line terminator inside a string literal.
    ///
    /// U+2028 and U+2029 are allowed there and are not this; the two that are
    /// refused are line feed and carriage return.
    LineTerminatorInString,
    /// A template literal that the source ended inside.
    UnterminatedTemplate,
    /// A template continuation was asked for where there is no `}`.
    NotATemplateContinuation,
    /// A regular expression literal with no closing `/`.
    UnterminatedRegularExpression,
    /// A line terminator inside a regular expression literal.
    LineTerminatorInRegularExpression,
    /// A flag on a regular expression that means nothing.
    UnknownRegularExpressionFlag(char),
    /// The same regular expression flag twice.
    RepeatedRegularExpressionFlag(char),
    /// Both `u` and `v` on one regular expression, which are two spellings of
    /// how a pattern reads its characters and cannot both apply.
    BothUnicodeModes,
    /// `0755` or `08` — a legacy octal or non-octal decimal literal.
    ///
    /// Refused by ADR 0013 § 3: both exist only in sloppy mode, both are in
    /// queue item 142's list, and `0755` meaning 493 is the kind of thing a
    /// reader gets wrong rather than a browser.
    LegacyOctalLiteral,
    /// `\1` to `\7` in a string — a legacy octal escape.
    LegacyOctalEscape,
    /// `\8` or `\9` in a string, which never meant anything.
    NonOctalDecimalEscape,
    /// A `_` in a number where there are not digits on both sides of it.
    MisplacedNumericSeparator,
    /// `0x`, `0o`, `0b` or an exponent with no digits after it.
    MissingDigits,
    /// A number with a name or a digit immediately after it, such as `3in`.
    ///
    /// The grammar refuses it rather than reading two tokens, because `3in x`
    /// and `3 in x` would otherwise be the same program written two ways.
    NumberFollowedByName,
    /// `1.5n` or `1e3n` — a `BigInt` suffix on something that is not an integer.
    BigIntIsNotAnInteger,
    /// A `\x` escape without two hexadecimal digits after it.
    BadHexEscape,
    /// A `\u` escape that is neither four hexadecimal digits nor `{`…`}`.
    BadUnicodeEscape,
    /// A `\u{…}` naming a number above U+10FFFF.
    CodePointOutOfRange,
    /// A `\u` escape in an identifier standing for a character that may not be
    /// in one.
    EscapeIsNotANameCharacter,
    /// A `#` with no name after it.
    PrivateNameWithoutAName,
    /// A character that begins no token at all.
    UnexpectedCharacter(char),
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceTooLong { bytes, most } => {
                write!(
                    f,
                    "the script is {bytes} bytes and the most we read is {most}"
                )
            }
            Self::UnterminatedComment => f.write_str("a comment was opened and never closed"),
            Self::UnterminatedString => f.write_str("a string was opened and never closed"),
            Self::LineTerminatorInString => {
                f.write_str("a string cannot run past the end of its line")
            }
            Self::UnterminatedTemplate => f.write_str("a template was opened and never closed"),
            Self::NotATemplateContinuation => {
                f.write_str("a template was continued where there is no `}`")
            }
            Self::UnterminatedRegularExpression => {
                f.write_str("a regular expression was opened and never closed")
            }
            Self::LineTerminatorInRegularExpression => {
                f.write_str("a regular expression cannot run past the end of its line")
            }
            Self::UnknownRegularExpressionFlag(c) => {
                write!(f, "`{c}` is not a regular expression flag")
            }
            Self::RepeatedRegularExpressionFlag(c) => {
                write!(f, "the regular expression flag `{c}` is given twice")
            }
            Self::BothUnicodeModes => {
                f.write_str("a regular expression cannot be both `u` and `v`")
            }
            Self::LegacyOctalLiteral => f.write_str(
                "a number beginning with `0` and more digits is not read here; \
                             write `0o755` for octal, or drop the leading zero",
            ),
            Self::LegacyOctalEscape => {
                f.write_str("`\\1` to `\\7` are not escapes here; write `\\x01`")
            }
            Self::NonOctalDecimalEscape => f.write_str("`\\8` and `\\9` are not escapes"),
            Self::MisplacedNumericSeparator => {
                f.write_str("a `_` in a number needs a digit on each side of it")
            }
            Self::MissingDigits => f.write_str("there are no digits here"),
            Self::NumberFollowedByName => {
                f.write_str("a number cannot be followed straight away by a name or a digit")
            }
            Self::BigIntIsNotAnInteger => {
                f.write_str("`n` makes a whole number, so there can be no point and no exponent")
            }
            Self::BadHexEscape => f.write_str("`\\x` needs two hexadecimal digits"),
            Self::BadUnicodeEscape => {
                f.write_str("`\\u` needs four hexadecimal digits, or `{` and at least one `}`")
            }
            Self::CodePointOutOfRange => f.write_str("there is no character above U+10FFFF"),
            Self::EscapeIsNotANameCharacter => {
                f.write_str("this escape stands for a character that cannot be in a name")
            }
            Self::PrivateNameWithoutAName => f.write_str("`#` needs a name after it"),
            Self::UnexpectedCharacter(c) => write!(f, "`{c}` begins nothing"),
        }
    }
}

/// Where in the source something is, for a person to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The byte offset, which is what the scanner works in.
    pub offset: usize,
    /// The line, counting from one.
    pub line: usize,
    /// The column, counting from one, in UTF-16 code units.
    ///
    /// Code units rather than characters, because that is what every other
    /// engine's stack traces and every developer tool count in — a column we
    /// print can be compared with a column somebody else printed. It is also
    /// what queue item 78's stack traces will need.
    pub column: usize,
}

impl Position {
    /// Where an offset is, by counting the source up to it.
    ///
    /// A scan from the start, which is right for an error somebody is shown and
    /// wrong for anything in a loop. Nothing in this crate calls it while
    /// scanning.
    pub fn of(source: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;
        let mut at = 0;
        while at < offset {
            let Some((c, next)) = read::next_char(source, at) else {
                break;
            };
            if crate::unicode::is_line_terminator(c) {
                // A carriage return followed by a line feed is one line ending,
                // so the line feed of a `\r\n` does not count a second time.
                let is_second_half =
                    c == '\n' && at > 0 && read::starts_with(source, at - 1, "\r\n");
                if is_second_half {
                    column = 1;
                } else {
                    line += 1;
                    column = 1;
                }
            } else {
                column += c.len_utf16();
            }
            at = next;
        }
        Self {
            offset,
            line,
            column,
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}
