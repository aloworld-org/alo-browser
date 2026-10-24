/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How deeply a piece of JSON nests, read before anything parses it.
//!
//! The rented reader has a limit of its own, and a limit somebody else
//! chooses is not a limit (ADR 0013). So every JSON this browser reads from
//! outside — an agent's line ([`crate::protocol`]) and a message from Chrome
//! ([`crate::chrome_protocol`]) — is scanned here first, and refused if it is
//! deeper than its reader allows.

/// How deeply `json` nests its arrays and objects, read as JSON's grammar
/// reads it: a bracket inside a string is text.
pub fn of(json: &[u8]) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    let (mut in_text, mut escaped) = (false, false);
    for &byte in json {
        if in_text {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_text = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_text = true,
            b'[' | b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_counted_across_arrays_and_objects() {
        assert_eq!(of(b"1"), 0);
        assert_eq!(of(b"{}"), 1);
        assert_eq!(of(br#"{"a":[{"b":[]}]}"#), 4);
        assert_eq!(of(br"[[],[[]]]"), 3, "siblings are not depth");
    }

    #[test]
    fn a_bracket_in_a_string_is_text() {
        assert_eq!(of(br#"{"tab":"[[[[[[[[[["}"#), 1);
        assert_eq!(
            of(br#"{"t":"\"[[[["}"#),
            1,
            "an escaped quote does not end it"
        );
        assert_eq!(of(br#"{"t":"\\"}"#), 1, "an escaped backslash does");
    }

    #[test]
    fn a_close_before_an_open_is_not_depth() {
        assert_eq!(of(b"]]]]["), 1);
    }
}
