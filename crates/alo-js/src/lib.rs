/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Our own JavaScript engine (ADR 0013). Today it reads a program.
//!
//! # What this is, and what it is not yet
//!
//! ADR 0013 decided a **parser, a bytecode compiler and an interpreter**, in
//! safe Rust, correct before fast, with no JIT. The first two queue items are
//! built: [`lexer`] turns characters into tokens (item 70) and [`parser`] turns
//! tokens into the tree in [`ast`] (item 204). [`script`] and [`module`] are
//! the two ways in, and which of the two a file is, is the caller's to say —
//! the same bytes are a legal script and a legal module with different
//! meanings.
//!
//! [`heap`] is the third (ADR 0014, item 71): the arena everything a script
//! makes lives in, and the collector that owns it. A reference into it is an
//! **index carrying a generation** rather than a pointer, which is why nothing
//! here needs `unsafe` to hold one; and the collector is **precise**, so the
//! interpreter's frames and value stack will live in structures it walks rather
//! than in Rust locals.
//!
//! [`object`] is the fourth (ADR 0014 § 11, item 206): what a cell in that heap
//! **is**. A prototype, a property table whose order a page can observe,
//! internal methods that are the same trait an embedder gets, interned keys and
//! strings. [`Heap<T>`](heap::Heap) was generic in its cell so that this could
//! land inside it without changing a line of `heap.rs`, and it did.
//!
//! Nothing here evaluates anything yet. [`token::Kind::BigInt`] still keeps
//! digits rather than a number, and now for a narrower reason: arbitrary
//! precision arithmetic is a decision about renting rather than a variant to
//! add, which is queue item 207. The tree is what item 72's compiler will read.
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

pub mod ast;
pub mod bounds;
pub mod error;
pub mod escape;
pub mod heap;
pub mod lexer;
pub mod number;
pub mod object;
pub mod parser;
pub mod punctuator;
pub mod read;
pub mod regexp;
pub mod string;
pub mod template;
pub mod token;
pub mod unicode;
pub mod word;

pub use ast::{Program, Source};
pub use error::{Position, Reason, SyntaxError};
pub use heap::{Field, Full, Heap, Ref, Root, Scope, Survivors, Trace, Tracer, Weak};
pub use lexer::{Goal, Lexer};
pub use object::{Cell, Fault, Found, Key, Objects, Property, Refused, Set, Value};
pub use parser::{Parser, module, script};
pub use punctuator::Punctuator;
pub use token::{Kind, Token};
pub use word::{Keyword, Status, Word};
