/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which characters may start a name, continue one, or be skipped.
//!
//! `unicode-id-start` is **rented** (ADR 0001, and ADR 0013 § 8 puts Unicode
//! tables among the physics an engine does not differ by). It is named in this
//! file and nowhere else, and `scripts/gate.sh` checks that.
//!
//! # Why this crate rather than the usual one
//!
//! `unicode-ident` is the crate everything else in Rust uses, and it answers a
//! different question: **`XID_Start` and `XID_Continue`**, the normalisation-closed
//! variants. ECMAScript's grammar names **`ID_Start` and `ID_Continue`**. The two
//! differ on a handful of code points nobody writes an identifier with, and
//! taking the crate that answers the question the specification asks costs
//! nothing and means there is no list of exceptions for somebody to maintain.
//!
//! Both crates read their table with `get_unchecked`, which is `unsafe`. That
//! is the rented crate's, not ours: law 4 governs `unsafe` **we** write, and
//! ADR 0005 says in as many words that the physics' `unsafe` is not ours to
//! remove. The workspace still forbids it in this crate at the compiler.
//!
//! # What is written out here rather than rented
//!
//! **Whitespace and line terminators**, because they are two short closed lists
//! in the specification rather than a table: `WhiteSpace` is the three ASCII
//! controls, `ZWNBSP`, and the Unicode `Space_Separator` category, and
//! `LineTerminator` is four code points. A crate for either would be a
//! dependency carrying twenty numbers.

/// Whether a character may begin an identifier.
///
/// ECMAScript's `IdentifierStartChar`: `ID_Start`, `$` and `_`. The two ASCII
/// ones are named here rather than assumed of the table, because a table that
/// happened to include them would make this file's answer depend on a fork's
/// convenience rather than on the grammar.
pub fn starts_a_name(c: char) -> bool {
    c == '$' || c == '_' || unicode_id_start::is_id_start(c)
}

/// Whether a character may continue an identifier.
///
/// ECMAScript's `IdentifierPartChar`: `ID_Continue`, `$`, and the two joiners
/// — U+200C ZERO WIDTH NON-JOINER and U+200D ZERO WIDTH JOINER, which are in
/// no identifier class and are in the grammar because scripts that need them to
/// spell a word need them to spell a name.
pub fn continues_a_name(c: char) -> bool {
    c == '$' || c == '\u{200C}' || c == '\u{200D}' || unicode_id_start::is_id_continue(c)
}

/// Whether a character is `WhiteSpace` — skipped, and separating nothing.
///
/// The list is the specification's, in its order: character tabulation, line
/// tabulation, form feed, ZWNBSP, and `Space_Separator`. A line terminator is
/// **not** here: it is skipped too, but it is also what
/// [`crate::token::Token::newline_before`] records, so the two are asked
/// separately.
pub fn is_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{FEFF}') || is_space_separator(c)
}

/// The Unicode `Space_Separator` general category, written out.
///
/// Seventeen code points, and closed since Unicode 3.2 — a table for it would
/// be a dependency holding a list this length.
fn is_space_separator(c: char) -> bool {
    matches!(
        c,
        '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Whether a character is a `LineTerminator`.
///
/// Four: line feed, carriage return, and the two Unicode separators. They end a
/// line comment, they are forbidden inside a string literal and a regular
/// expression, and they are what automatic semicolon insertion is decided by —
/// which is why every token records whether one came before it.
pub fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
