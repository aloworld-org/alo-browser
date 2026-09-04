/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The punctuation the language has, as one table.
//!
//! A lexer's punctuation is usually a nest of hand-written branches — read one
//! `>`, then look for a second, then for a third, then for an `=` — and the bug
//! it produces is always the same one: the longest spelling is missing, so
//! `a >>>= b` reads as `a >>>` then `= b`, and the error points somewhere else.
//! Here it is a **table, sorted longest first**, and matching is the first
//! spelling the source starts with. A new operator is one line, and it cannot
//! be shadowed by a shorter one because the sort is asserted by a test.
//!
//! Two are not in the table, and both are decisions rather than spellings:
//!
//! - **`/` and `/=`** are only punctuation when the caller asked for
//!   [`crate::Goal::Division`]. That is the ambiguity queue item 70 names, and
//!   [`crate::Lexer`] settles it before it gets here.
//! - **`?.`** is not punctuation when a digit follows it, because `a?.5:b` is a
//!   conditional whose second half is `.5`. The specification writes that rule
//!   into the grammar as a lookahead restriction, and [`crate::Lexer`] applies
//!   it in the one place `?` is read.

/// A punctuator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Punctuator {
    /// `{`
    LeftBrace,
    /// `}`
    RightBrace,
    /// `(`
    LeftParenthesis,
    /// `)`
    RightParenthesis,
    /// `[`
    LeftBracket,
    /// `]`
    RightBracket,
    /// `.`
    Dot,
    /// `...`
    Spread,
    /// `;`
    Semicolon,
    /// `,`
    Comma,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessOrEqual,
    /// `>=`
    GreaterOrEqual,
    /// `==`
    Equal,
    /// `!=`
    NotEqual,
    /// `===`
    StrictlyEqual,
    /// `!==`
    StrictlyNotEqual,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Times,
    /// `/`
    Divide,
    /// `%`
    Remainder,
    /// `**`
    Power,
    /// `++`
    Increment,
    /// `--`
    Decrement,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
    /// `>>>`
    ShiftRightUnsigned,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `!`
    Not,
    /// `~`
    BitNot,
    /// `&&`
    And,
    /// `||`
    Or,
    /// `??`
    Coalesce,
    /// `?`
    Question,
    /// `?.`
    OptionalChain,
    /// `:`
    Colon,
    /// `=`
    Assign,
    /// `=>`
    Arrow,
    /// `+=`
    AddAssign,
    /// `-=`
    SubtractAssign,
    /// `*=`
    TimesAssign,
    /// `/=`
    DivideAssign,
    /// `%=`
    RemainderAssign,
    /// `**=`
    PowerAssign,
    /// `<<=`
    ShiftLeftAssign,
    /// `>>=`
    ShiftRightAssign,
    /// `>>>=`
    ShiftRightUnsignedAssign,
    /// `&=`
    BitAndAssign,
    /// `|=`
    BitOrAssign,
    /// `^=`
    BitXorAssign,
    /// `&&=`
    AndAssign,
    /// `||=`
    OrAssign,
    /// `??=`
    CoalesceAssign,
}

impl Punctuator {
    /// How it is spelled.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeftBrace => "{",
            Self::RightBrace => "}",
            Self::LeftParenthesis => "(",
            Self::RightParenthesis => ")",
            Self::LeftBracket => "[",
            Self::RightBracket => "]",
            Self::Dot => ".",
            Self::Spread => "...",
            Self::Semicolon => ";",
            Self::Comma => ",",
            Self::Less => "<",
            Self::Greater => ">",
            Self::LessOrEqual => "<=",
            Self::GreaterOrEqual => ">=",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::StrictlyEqual => "===",
            Self::StrictlyNotEqual => "!==",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Times => "*",
            Self::Divide => "/",
            Self::Remainder => "%",
            Self::Power => "**",
            Self::Increment => "++",
            Self::Decrement => "--",
            Self::ShiftLeft => "<<",
            Self::ShiftRight => ">>",
            Self::ShiftRightUnsigned => ">>>",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Not => "!",
            Self::BitNot => "~",
            Self::And => "&&",
            Self::Or => "||",
            Self::Coalesce => "??",
            Self::Question => "?",
            Self::OptionalChain => "?.",
            Self::Colon => ":",
            Self::Assign => "=",
            Self::Arrow => "=>",
            Self::AddAssign => "+=",
            Self::SubtractAssign => "-=",
            Self::TimesAssign => "*=",
            Self::DivideAssign => "/=",
            Self::RemainderAssign => "%=",
            Self::PowerAssign => "**=",
            Self::ShiftLeftAssign => "<<=",
            Self::ShiftRightAssign => ">>=",
            Self::ShiftRightUnsignedAssign => ">>>=",
            Self::BitAndAssign => "&=",
            Self::BitOrAssign => "|=",
            Self::BitXorAssign => "^=",
            Self::AndAssign => "&&=",
            Self::OrAssign => "||=",
            Self::CoalesceAssign => "??=",
        }
    }
}

/// Every punctuator that is punctuation whatever the caller asked for, longest
/// spelling first.
///
/// `/`, `/=`, `?` and `?.` are absent for the two reasons at the top of this
/// file. Everything else is here, and [`longest_at`] takes the first match.
pub const IN_ANY_GOAL: &[Punctuator] = &[
    Punctuator::ShiftRightUnsignedAssign,
    Punctuator::StrictlyEqual,
    Punctuator::StrictlyNotEqual,
    Punctuator::Spread,
    Punctuator::ShiftRightUnsigned,
    Punctuator::PowerAssign,
    Punctuator::ShiftLeftAssign,
    Punctuator::ShiftRightAssign,
    Punctuator::AndAssign,
    Punctuator::OrAssign,
    Punctuator::CoalesceAssign,
    Punctuator::Equal,
    Punctuator::NotEqual,
    Punctuator::LessOrEqual,
    Punctuator::GreaterOrEqual,
    Punctuator::Power,
    Punctuator::Increment,
    Punctuator::Decrement,
    Punctuator::ShiftLeft,
    Punctuator::ShiftRight,
    Punctuator::And,
    Punctuator::Or,
    Punctuator::Coalesce,
    Punctuator::Arrow,
    Punctuator::AddAssign,
    Punctuator::SubtractAssign,
    Punctuator::TimesAssign,
    Punctuator::RemainderAssign,
    Punctuator::BitAndAssign,
    Punctuator::BitOrAssign,
    Punctuator::BitXorAssign,
    Punctuator::LeftBrace,
    Punctuator::RightBrace,
    Punctuator::LeftParenthesis,
    Punctuator::RightParenthesis,
    Punctuator::LeftBracket,
    Punctuator::RightBracket,
    Punctuator::Dot,
    Punctuator::Semicolon,
    Punctuator::Comma,
    Punctuator::Less,
    Punctuator::Greater,
    Punctuator::Plus,
    Punctuator::Minus,
    Punctuator::Times,
    Punctuator::Remainder,
    Punctuator::BitAnd,
    Punctuator::BitOr,
    Punctuator::BitXor,
    Punctuator::Not,
    Punctuator::BitNot,
    Punctuator::Colon,
    Punctuator::Assign,
];

/// The longest punctuator the source spells at `at`, and where it ends.
pub fn longest_at(source: &str, at: usize) -> Option<(Punctuator, usize)> {
    IN_ANY_GOAL
        .iter()
        .find(|p| crate::read::starts_with(source, at, p.as_str()))
        .map(|p| (*p, at.saturating_add(p.as_str().len())))
}
