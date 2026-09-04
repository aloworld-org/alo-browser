/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a token is.
//!
//! # Every token knows where it came from
//!
//! [`Token::start`] and [`Token::end`] are byte offsets into the source the
//! lexer was given. They are not decoration: an error somebody is shown points
//! at one, a stack trace (queue item 78) is built from them, and the
//! source text of a function is what `Function.prototype.toString` has to
//! answer with. A token that had only its value would make all three impossible
//! to add later without re-lexing.
//!
//! # And whether a line ended before it
//!
//! [`Token::newline_before`] is the whole of what automatic semicolon insertion
//! needs from a lexer, and it is the reason trivia cannot simply be thrown
//! away. `return\n1` is `return; 1` and `return 1` is not, and the only
//! difference between them is a character the token stream does not otherwise
//! contain. A comment holding a line ending counts, which is why a block
//! comment is examined rather than skipped.

use crate::punctuator::Punctuator;
use crate::{number, regexp, template, word};

/// One token, and where it was.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What it is.
    pub kind: Kind,
    /// The byte offset it starts at.
    pub start: usize,
    /// The byte offset just past it.
    pub end: usize,
    /// Whether a line ending — in whitespace or inside a comment — came between
    /// this token and the one before it.
    pub newline_before: bool,
}

/// What a token is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Kind {
    /// A name, which may or may not spell a keyword — see [`word`].
    Name(word::Word),
    /// `#count` — the name of a private class member, without its `#`.
    PrivateName(word::Word),
    /// A number, already rounded.
    Number(f64),
    /// A `BigInt`, kept as digits until there is something to hold one.
    BigInt {
        /// The digits, with any `_` removed.
        digits: String,
        /// What base they are in.
        radix: number::Radix,
    },
    /// A string, as UTF-16 code units.
    String(Vec<u16>),
    /// One piece of a template literal.
    Template(template::Piece),
    /// A regular expression literal, unread.
    RegularExpression(regexp::Literal),
    /// Punctuation.
    Punctuator(Punctuator),
    /// The end of the source.
    ///
    /// A token rather than a [`None`], because the parser has to be able to say
    /// *where* the source ended when it wanted something else, and because
    /// automatic semicolon insertion treats the end of input as a place a
    /// semicolon may go.
    End,
}
