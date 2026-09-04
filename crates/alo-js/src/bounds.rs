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
//! There is exactly one so far, and that is not an oversight. A lexer allocates
//! in proportion to its source and nothing else: a string's characters, a
//! name's bytes and a token's span are all cut out of the text that was already
//! read, so bounding the text bounds all of them. **Nesting depth is not
//! bounded here** because this lexer has no nesting — a template's `${` is
//! closed by the parser calling back in, which is why item 204's parser is
//! where a depth bound belongs and where a script of twenty thousand open
//! brackets is refused.

/// The most source text we will read, in bytes.
///
/// Sixty-four mebibytes. The largest bundles anybody ships are a few megabytes
/// and a page that concatenated all of its script into one file would still be
/// far under this; a script above it is a page trying to decide how much memory
/// this process uses. A page that hits it gets a refusal naming the size, which
/// is a bug report somebody can act on — unlike a renderer that stopped.
pub const LONGEST_SOURCE: usize = 64 * 1024 * 1024;
