/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Source text into tokens, one token at a time, at the caller's request.
//!
//! # The lexer does not guess, and that is what [`Goal`] is
//!
//! Queue item 70 names two ambiguities and says both are *settled at parse time
//! rather than later*. The first is here: **`/` is a division sign or the start
//! of a regular expression, and nothing in the characters says which.** `a /b/
//! g` is three divisions if `a` is a number and one regular expression if `a`
//! is a keyword. Every editor and every syntax highlighter guesses, using the
//! token before; a guess is wrong on `return /re/` against `x++ /y/z`, and the
//! way it is wrong is that a program means something else.
//!
//! So there is no guess. [`Lexer::next`] **takes** a [`Goal`], every call, and
//! the parser — which knows whether it is expecting an operator or an operand —
//! is the only thing that decides. It is a required argument rather than a mode
//! that is set, because a mode is a thing a caller forgets to change.
//!
//! The second ambiguity is item 204's: an arrow function against a
//! parenthesised expression is decided by what comes *after* the closing
//! parenthesis, which is a question about a token stream and not about
//! characters.
//!
//! # What is skipped, and the one thing that is remembered about it
//!
//! Whitespace, line terminators, and both kinds of comment are trivia. The
//! lexer throws them away except for a single bit — whether a line ended —
//! which [`crate::token::Token::newline_before`] carries because automatic
//! semicolon insertion is the one rule in the language that can see it. A block
//! comment with a line ending inside it counts, which is why they are looked
//! into rather than jumped over.
//!
//! **`<!--` and `-->` are not comments.** They are Annex B, they exist so that
//! a 1996 page could hide its script from a browser that had none, and ADR
//! 0013 § 3 sends that whole appendix to the legacy tail. They are not refused
//! either, and refusing them would be a bug rather than a stance: `a <!--b` is
//! ordinary modern code meaning `a < !(--b)`, and a page like it must not stop
//! working because of a decision about 1996. So they lex as the punctuation
//! they are, and a page that meant them as comments fails in the parser.
//!
//! # A hashbang, once
//!
//! `#!/usr/bin/env node` is a comment when it is the very first thing in the
//! source and nothing anywhere else. [`Lexer::new`] is where that is true, and
//! is why it is not in the trivia loop: a rule that only holds at offset zero
//! belongs somewhere that only runs at offset zero.

use crate::error::{Reason, SyntaxError};
use crate::punctuator::{self, Punctuator};
use crate::token::{Kind, Token};
use crate::{bounds, number, read, regexp, string, template, unicode, word};

/// What the caller is expecting, where the characters cannot say.
///
/// The specification calls these goal symbols and has the same three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// An operator may come next, so `/` divides and `}` closes a block.
    Division,
    /// An operand may come next, so `/` begins a regular expression.
    RegularExpression,
    /// The parser has just finished the expression inside a `${`, so the `}`
    /// here continues the template rather than closing anything.
    TemplateContinuation,
}

/// A lexer over one piece of source text.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    source: &'a str,
    at: usize,
}

impl<'a> Lexer<'a> {
    /// A lexer over `source`.
    ///
    /// Refuses source longer than [`bounds::LONGEST_SOURCE`] — the one ceiling
    /// this crate has, and the one that bounds every allocation below it — and
    /// steps over a hashbang if there is one.
    ///
    /// # Errors
    ///
    /// [`Reason::SourceTooLong`], and nothing else — a lexer over any text
    /// short enough to read can always be made.
    pub fn new(source: &'a str) -> Result<Self, SyntaxError> {
        if source.len() > bounds::LONGEST_SOURCE {
            return Err(SyntaxError::new(
                Reason::SourceTooLong {
                    bytes: source.len(),
                    most: bounds::LONGEST_SOURCE,
                },
                0,
            ));
        }
        let mut at = 0;
        if read::starts_with(source, 0, "#!") {
            at = to_end_of_line(source, 2);
        }
        Ok(Self { source, at })
    }

    /// The source this lexer is reading.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// How far it has read.
    pub fn offset(&self) -> usize {
        self.at
    }

    /// The next token, read the way `goal` says.
    ///
    /// # Errors
    ///
    /// Whatever the token it was reading refuses — see [`Reason`]. The lexer is
    /// left where the refusal happened, so a caller that wanted to report more
    /// than one can, and there is nothing it can be asked that panics.
    pub fn next(&mut self, goal: Goal) -> Result<Token, SyntaxError> {
        let newline_before = self.skip_trivia(goal)?;
        let start = self.at;
        let (kind, end) = self.read_one(goal, start)?;
        self.at = end;
        Ok(Token {
            kind,
            start,
            end,
            newline_before,
        })
    }

    /// Step over whitespace and comments, answering whether a line ended.
    ///
    /// Trivia is skipped in **every** goal, including
    /// [`Goal::RegularExpression`] — which is what makes `x = // note` followed
    /// by `/re/` on the next line work, and which is also why a regular
    /// expression can never begin with `/` or `*`: those two spellings were
    /// already taken by a comment before the goal was consulted. The
    /// specification writes that as a restriction on the first character of a
    /// pattern; here it falls out of the order.
    ///
    /// [`Goal::TemplateContinuation`] skips nothing, because the `}` it is
    /// looking for is the very next character and everything after it is a
    /// template's own text — a space inside `` `} x ${ `` is part of the
    /// string, not trivia.
    fn skip_trivia(&mut self, goal: Goal) -> Result<bool, SyntaxError> {
        if goal == Goal::TemplateContinuation {
            return Ok(false);
        }
        let mut newline = false;
        loop {
            let Some((c, after)) = read::next_char(self.source, self.at) else {
                return Ok(newline);
            };
            if unicode::is_line_terminator(c) {
                newline = true;
                self.at = after;
            } else if unicode::is_whitespace(c) {
                self.at = after;
            } else if read::starts_with(self.source, self.at, "//") {
                self.at = to_end_of_line(self.source, self.at.saturating_add(2));
            } else if read::starts_with(self.source, self.at, "/*") {
                let (end, held_a_line_ending) = self.block_comment()?;
                newline |= held_a_line_ending;
                self.at = end;
            } else {
                return Ok(newline);
            }
        }
    }

    /// Step over a `/* … */`, answering where it ends and whether a line ended
    /// inside it.
    fn block_comment(&self) -> Result<(usize, bool), SyntaxError> {
        let mut at = self.at.saturating_add(2);
        let mut held_a_line_ending = false;
        loop {
            if read::starts_with(self.source, at, "*/") {
                return Ok((at.saturating_add(2), held_a_line_ending));
            }
            let Some((c, after)) = read::next_char(self.source, at) else {
                return Err(SyntaxError::new(Reason::UnterminatedComment, self.at));
            };
            held_a_line_ending |= unicode::is_line_terminator(c);
            at = after;
        }
    }

    /// The one token beginning at `start`, which is not trivia.
    fn read_one(&self, goal: Goal, start: usize) -> Result<(Kind, usize), SyntaxError> {
        // A continuation is the one goal that says what the next token *is*
        // rather than which of two readings to take, so anything but a `}` is a
        // caller that has lost its place. Reading a name there instead would be
        // this lexer quietly deciding the template ended somewhere else.
        if goal == Goal::TemplateContinuation {
            let (piece, end) = template::scan(self.source, start)?;
            return Ok((Kind::Template(piece), end));
        }
        let Some(c) = read::char_at(self.source, start) else {
            return Ok((Kind::End, start));
        };
        match c {
            '"' | '\'' => {
                let (units, end) = string::scan(self.source, start)?;
                Ok((Kind::String(units), end))
            }
            '`' => {
                let (piece, end) = template::scan(self.source, start)?;
                Ok((Kind::Template(piece), end))
            }
            '/' => self.slash(goal, start),
            '?' => Ok(question(self.source, start)),
            '#' => self.private_name(start),
            '.' if read::char_at(self.source, start.saturating_add(1))
                .is_some_and(|d| d.is_ascii_digit()) =>
            {
                self.number(start)
            }
            _ if c.is_ascii_digit() => self.number(start),
            '\\' => self.name(start),
            _ if unicode::starts_a_name(c) => self.name(start),
            _ => punctuator::longest_at(self.source, start)
                .map(|(p, end)| (Kind::Punctuator(p), end))
                .ok_or_else(|| SyntaxError::new(Reason::UnexpectedCharacter(c), start)),
        }
    }

    /// A `/`, which is division or a regular expression and never both.
    fn slash(&self, goal: Goal, start: usize) -> Result<(Kind, usize), SyntaxError> {
        if goal == Goal::RegularExpression {
            let (literal, end) = regexp::scan(self.source, start)?;
            return Ok((Kind::RegularExpression(literal), end));
        }
        if read::starts_with(self.source, start, "/=") {
            return Ok((
                Kind::Punctuator(Punctuator::DivideAssign),
                start.saturating_add(2),
            ));
        }
        Ok((
            Kind::Punctuator(Punctuator::Divide),
            start.saturating_add(1),
        ))
    }

    /// A number.
    fn number(&self, start: usize) -> Result<(Kind, usize), SyntaxError> {
        let (value, end) = number::scan(self.source, start)?;
        let kind = match value {
            number::Value::Number(n) => Kind::Number(n),
            number::Value::BigInt { digits, radix } => Kind::BigInt { digits, radix },
        };
        Ok((kind, end))
    }

    /// A name, which may or may not spell a keyword.
    fn name(&self, start: usize) -> Result<(Kind, usize), SyntaxError> {
        let (found, end) = word::scan(self.source, start)?;
        Ok((Kind::Name(found), end))
    }

    /// A `#name`.
    ///
    /// The `#` is not part of the name — `#x` and `x` are different members of
    /// a class and neither is the other — but it is not a token of its own
    /// either, because `# x` is not a private name however it is spaced.
    fn private_name(&self, start: usize) -> Result<(Kind, usize), SyntaxError> {
        let after_hash = start.saturating_add(1);
        let begins_a_name = read::char_at(self.source, after_hash)
            .is_some_and(|c| unicode::starts_a_name(c) || c == '\\');
        if !begins_a_name {
            return Err(SyntaxError::new(Reason::PrivateNameWithoutAName, start));
        }
        let (found, end) = word::scan(self.source, after_hash)?;
        Ok((Kind::PrivateName(found), end))
    }
}

/// Where a line comment ends: at the line terminator, which is left for
/// [`Lexer::skip_trivia`] to see.
fn to_end_of_line(source: &str, from: usize) -> usize {
    let mut at = from;
    while let Some((c, after)) = read::next_char(source, at) {
        if unicode::is_line_terminator(c) {
            return at;
        }
        at = after;
    }
    at
}

/// A `?`, which is four punctuators and one lookahead.
///
/// `a?.5:b` is a conditional whose consequent is `.5`, so `?.` is only the
/// optional-chaining operator when a digit does not follow it. The
/// specification writes it as a lookahead restriction in the grammar; it is one
/// line here and it is the only place `?` is read.
fn question(source: &str, start: usize) -> (Kind, usize) {
    let spelling = if read::starts_with(source, start, "??=") {
        Punctuator::CoalesceAssign
    } else if read::starts_with(source, start, "??") {
        Punctuator::Coalesce
    } else if read::starts_with(source, start, "?.")
        && !read::char_at(source, start.saturating_add(2)).is_some_and(|c| c.is_ascii_digit())
    {
        Punctuator::OptionalChain
    } else {
        Punctuator::Question
    };
    (
        Kind::Punctuator(spelling),
        start.saturating_add(spelling.as_str().len()),
    )
}
