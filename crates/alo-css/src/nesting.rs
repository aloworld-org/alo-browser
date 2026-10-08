/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How deep a piece of CSS text nests its blocks, measured before the
//! rented parser is given it.
//!
//! `cssparser` and `selectors` read a block inside a block by calling
//! themselves, once per level. A selector a page's script passes to
//! `querySelectorAll` (queue item 329) is text a stranger chose, and
//! `":is(".repeat(1000)` overflowed a test thread's two megabytes in a
//! debug build. A crash is not a refusal (`docs/autonomy/LOOP.md`, stage 2
//! § 2), so the depth is counted here first, by a scan that recurses into
//! nothing, and text past [`LIMIT`] is refused.
//!
//! The scan sees blocks as CSS Syntax does: `(`, `[` and `{` open one (a
//! function's name and its `(` are one token, and still one level), the
//! closing three end one, and nothing inside a string, a comment or after a
//! backslash counts. It is a bound, not a parser: a mismatched bracket is
//! the parser's to refuse, and is only counted here.

/// The deepest a block may nest: past this, the text is refused.
///
/// Selectors written by people nest two or three deep — `:is(:not(.a))` is
/// two. Thirty-two is ten times that, and a debug build survived four
/// hundred on a test thread's two megabytes.
pub const LIMIT: usize = 32;

/// Whether no block in `text` nests deeper than [`LIMIT`].
pub fn within_limit(text: &str) -> bool {
    let mut depth = 0_usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' | '[' | '{' => {
                depth = depth.saturating_add(1);
                if depth > LIMIT {
                    return false;
                }
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '\\' => {
                // An escape: the next code point is part of a name, whatever
                // it is.
                chars.next();
            }
            '"' | '\'' => {
                // A string ends at its quote, or at a newline, which CSS
                // Syntax makes a bad string; either way nothing in it nests.
                while let Some(inside) = chars.next() {
                    match inside {
                        '\\' => {
                            chars.next();
                        }
                        '\n' => break,
                        _ if inside == c => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut star = false;
                for inside in chars.by_ref() {
                    if star && inside == '/' {
                        break;
                    }
                    star = inside == '*';
                }
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_people_write_is_within_the_limit() {
        for text in [
            "",
            ".btn[href]",
            ":is(:not(.a), [b='c'])",
            "li:nth-child(2n + 1 of .row)",
        ] {
            assert!(within_limit(text), "{text}");
        }
    }

    #[test]
    fn every_kind_of_block_counts() {
        for open in ["(", "[", "{", ":is("] {
            assert!(within_limit(&open.repeat(LIMIT)), "{open}");
            assert!(!within_limit(&open.repeat(LIMIT + 1)), "{open}");
        }
        assert!(!within_limit(&"([{".repeat(LIMIT)));
    }

    #[test]
    fn closing_a_block_gives_its_level_back() {
        assert!(within_limit(&"(a)".repeat(10_000)));
        // More closes than opens is the parser's to refuse, not a negative
        // depth that buys room for later.
        let text = format!("{}{}", ")".repeat(100), "(".repeat(LIMIT));
        assert!(within_limit(&text));
    }

    #[test]
    fn nothing_in_a_string_a_comment_or_an_escape_nests() {
        let deep = "(".repeat(1_000);
        assert!(within_limit(&format!("[title=\"{deep}\"]")));
        assert!(within_limit(&format!("[title='{deep}']")));
        assert!(within_limit(&format!("a/*{deep}*/b")));
        assert!(within_limit(&"\\(".repeat(1_000)));
        // An escaped quote does not end the string.
        assert!(within_limit(&format!("[t=\"\\\"{deep}\"]")));
        // A string that never ends hides the rest, as it would from the
        // parser.
        assert!(within_limit(&format!("[t=\"{deep}")));
        // A newline ends a string, and what follows counts again.
        assert!(!within_limit(&format!("[t=\"\n{deep}")));
    }
}
