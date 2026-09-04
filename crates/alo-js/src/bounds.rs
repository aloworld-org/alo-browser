/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The ceilings a script cannot walk past, and why each is the number it is.
//!
//! ADR 0013 § 4: *every allocation a script can cause has a ceiling we chose*,
//! for the reason `alo-net` gives in every file — **a limit somebody else
//! chooses is not a limit**. A bound with no reason beside it is a number
//! nobody can argue with later, so each one here says what it protects and what
//! a page hitting it would look like.
//!
//! The lexer needs one and the parser needs the other. A lexer allocates in
//! proportion to its source and nothing else — a string's characters, a name's
//! bytes and a token's span are all cut out of the text that was already read,
//! so bounding the text bounds all of them — and it does not recurse, so a
//! million open brackets is a million tokens and no stack at all. The parser
//! is the thing that recurses, which is why the second bound arrived with it.

/// The most source text we will read, in bytes.
///
/// Sixty-four mebibytes. The largest bundles anybody ships are a few megabytes
/// and a page that concatenated all of its script into one file would still be
/// far under this; a script above it is a page trying to decide how much memory
/// this process uses. A page that hits it gets a refusal naming the size, which
/// is a bug report somebody can act on — unlike a renderer that stopped.
pub const LONGEST_SOURCE: usize = 64 * 1024 * 1024;

/// How many brackets deep a program may nest.
///
/// Two hundred and fifty-six. The parser is recursive descent, so a nesting
/// level is a handful of stack frames, and a script chooses its own nesting —
/// twenty thousand `[` is four bytes of somebody else's file and a stack
/// overflow in ours, which is an abort rather than a refusal and which
/// ADR 0013 § 4 forbids outright.
///
/// Hand-written code does not reach a tenth of it: the deepest thing a person
/// writes is a nested conditional, and a bundler's output nests wide rather
/// than deep because its input was written by people too. What does reach it is
/// a file written to reach it, which is the case this exists for.
///
/// It is paired with [`STACK_FOR_A_PARSE`], and neither number means anything
/// without the other.
pub const DEEPEST_NESTING: usize = 256;

/// The stack a parse is given, which is why [`DEEPEST_NESTING`] is a number we
/// chose rather than a number somebody else did.
///
/// Thirty-two mebibytes, and the reasoning is the one every bound in this
/// crate has: *a limit somebody else chooses is not a limit*. A recursive
/// descent parser's real ceiling is how much stack it was called on, and that
/// is a property of the caller — a `cargo test` thread has two mebibytes, a
/// process's first thread has eight, and a renderer will have whatever
/// somebody set. So the parser does not use the caller's stack at all: it runs
/// on a scoped thread of its own with this much, and [`DEEPEST_NESTING`] then
/// means the same thing everywhere, in a debug build and a release one alike.
///
/// The number is measured rather than guessed at. The most expensive nesting
/// this grammar has is a bracket — thirteen frames a level, since an array
/// element is a whole assignment expression — and **256 of them needs under
/// twelve mebibytes in a debug build**, which is the worst case because a
/// release build reuses stack slots and needs roughly a fifth of it. Thirty-two
/// is that with the margin a measurement taken on one machine deserves.
///
/// Reserved rather than used: an operating system commits the pages a thread
/// touches, so a script that nests four deep costs four levels of stack and
/// not this.
pub const STACK_FOR_A_PARSE: usize = 32 * 1024 * 1024;
