/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Our own JavaScript engine (ADR 0013). Today it is the lexer.
//!
//! # What this is, and what it is not yet
//!
//! ADR 0013 decided a **parser, a bytecode compiler and an interpreter**, in
//! safe Rust, correct before fast, with no JIT. Queue item 70 is the first of
//! them and was cut on starting: this crate holds the **lexer**, and the parser
//! is item 204. The cut is at the seam the language itself has — a lexer turns
//! characters into tokens and a parser turns tokens into a tree — and it is the
//! half where being wrong is being wrong about *what a character is*, which is
//! the half a stranger's bytes reach first.
//!
//! Nothing here evaluates anything. There is no value, no object and no heap;
//! [`token::Kind::BigInt`] keeps digits rather than a number for exactly that
//! reason, and inventing an integer type to hold one would be building item
//! 71's object model in the wrong crate.
//!
//! # The rule that shapes every file: a script is a stranger's bytes
//!
//! ADR 0013 § 4. **It never panics, on any source text.** Not on a truncated
//! escape, not on twenty thousand open brackets, not on a `\u{` that runs off
//! the end of the file — a renderer that stops is a denial of service, and a
//! refusal is a result. Three things hold it up:
//!
//! - The workspace lints deny `unwrap`, `expect`, `panic` and slice indexing,
//!   and [`read`] is the only way source text is looked at, so an offset that
//!   landed inside a character answers [`None`] rather than aborting.
//! - Every arithmetic on an offset is saturating. A lexer is arithmetic on
//!   indices from beginning to end, and that is the part the lints do not catch.
//! - [`bounds`] holds the ceilings, each with its reason beside it.
//!
//! # And the rule that shapes the interface: the lexer does not guess
//!
//! [`Lexer::next`] takes a [`Goal`] every time it is called, because whether
//! `/` divides or opens a regular expression is not a question the characters
//! can answer. See [`lexer`] — it is the first of the two ambiguities queue
//! item 70 names, and this is what "settled at parse time rather than later"
//! means in code.
//!
//! # Reaching nothing
//!
//! ADR 0013 § 5: this crate depends on no I/O crate at all. No network, no
//! filesystem, no clock, no entropy. The one dependency is a table of Unicode
//! properties ([`unicode`]), which reaches nothing either. Every capability a
//! script has will arrive from the embedder, which is what makes *the browser
//! process never runs page script* structural rather than remembered.

pub mod bounds;
pub mod error;
pub mod escape;
pub mod lexer;
pub mod number;
pub mod punctuator;
pub mod read;
pub mod regexp;
pub mod string;
pub mod template;
pub mod token;
pub mod unicode;
pub mod word;

pub use error::{Position, Reason, SyntaxError};
pub use lexer::{Goal, Lexer};
pub use punctuator::Punctuator;
pub use token::{Kind, Token};
pub use word::{Keyword, Status, Word};
