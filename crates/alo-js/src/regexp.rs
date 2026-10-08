/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Regular expressions: ours, and their work counted (ADR 0029, queue item
//! 74).
//!
//! # Four stages, each its own file
//!
//! - [`literal`]: where a literal ends and what its flags are — the lexer's
//!   question, and the only one the lexical grammar asks.
//! - [`parse`]: the pattern's text into a [`tree`], or the early `SyntaxError`
//!   it is. The **whole** grammar, so that whether a pattern is a pattern is
//!   decided for every pattern, and Annex B's forms refused by name.
//! - [`emit`]: the tree into a [`Program`], or the piece that is not built
//!   yet, by name: `i` and `\p{…}` (queue item 322), `v` and `d` (item 324).
//! - [`matcher`]: the program run against a string, by a loop with a stack of
//!   its own, every step counted against
//!   [`bounds::STEPS_IN_A_MATCH`](crate::bounds::STEPS_IN_A_MATCH) and the
//!   embedder's stop asked inside.
//!
//! # When each runs
//!
//! The parser checks a literal as soon as it reads one ([`check`]), so a bad
//! pattern is an early `SyntaxError` for the whole script, as the
//! specification requires. The compiler compiles it again ([`compile`]) and
//! keeps the program in the [`Unit`](crate::Unit), where every object the
//! literal makes shares it: a pattern is compiled once however often the
//! literal is evaluated (ADR 0029 § 5). Reading a pattern twice costs a few
//! microseconds; the alternative was a tree in the syntax tree that nothing
//! else needs.
//!
//! # What reaches nothing
//!
//! Nothing here is rented. ADR 0029 § 1: the bound on a match can only live
//! inside the loop that runs it, so the loop is ours, and so is everything
//! the loop depends on. The Unicode tables `i` and `\p` need are the one
//! thing that will be rented, and they are not here yet.

pub mod emit;
pub mod flags;
pub mod literal;
pub mod matcher;
pub mod parse;
pub mod program;
pub mod set;
pub mod surrogate;
pub mod tree;
pub mod wrong;

pub use emit::Unbuilt;
pub use flags::Flags;
pub use literal::{Literal, scan};
pub use matcher::{Found, Halt, search};
pub use program::Program;
pub use wrong::{Legacy, PatternError, Wrong};

/// Whether `body` is a pattern under `flags`: the parser's early error.
///
/// # Errors
///
/// [`PatternError`], with the byte offset into `body`.
pub fn check(body: &str, flags: &str) -> Result<(), PatternError> {
    let flags = Flags::of(flags).ok_or(PatternError {
        wrong: Wrong::BadFlags,
        at: 0,
    })?;
    parse::parse(body, &flags).map(|_| ())
}

/// Why a pattern did not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// It is not a pattern.
    Wrong(PatternError),
    /// It is, and a piece of it is not built yet.
    Unbuilt(Unbuilt),
}

/// Compile `body` under `flags`.
///
/// # Errors
///
/// [`Refused`]: what is wrong with it, or what in it is not built yet.
pub fn compile(body: &str, flags: &str) -> Result<Program, Refused> {
    let flags = Flags::of(flags).ok_or(Refused::Wrong(PatternError {
        wrong: Wrong::BadFlags,
        at: 0,
    }))?;
    let pattern = parse::parse(body, &flags).map_err(Refused::Wrong)?;
    emit::emit(pattern, flags, body).map_err(Refused::Unbuilt)
}
