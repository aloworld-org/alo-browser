/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A content hash: computing one, and reading the one an author wrote.
//!
//! A Content Security Policy may allow inline content by naming its digest —
//! `style-src 'sha256-2S4VvT2k3rjav5i3inerHe7oJsm2Yzx7kSGPgKT82yo='` is a page
//! saying *this exact stylesheet, and no other*. It is the only way to allow
//! inline content that an injection cannot also use, since an attacker who can
//! write into the page cannot change what the header said about it.
//!
//! `sha2` is **rented** (ADR 0001): a hash function is physics. It is named in
//! this file and nowhere else, and `scripts/gate.sh` checks that.
//!
//! # Why the author's value is decoded rather than ours encoded
//!
//! Both directions answer the same question, and decoding answers it once. A
//! digest may be written in either base64 alphabet — the standard one and the
//! URL-safe one — and with or without its padding, so comparing text would mean
//! producing every spelling of our own digest and comparing against each. What
//! is compared here is **bytes**, which has one spelling.
//!
//! A value that is not the right length for its algorithm therefore matches
//! nothing, rather than being an error: `'sha256-YWJj'` is a policy naming a
//! three-byte digest, which is a mistake its author should see as content that
//! does not run, not as a policy this engine refused to read.
//!
//! # And the reading is strict, in the direction of matching less
//!
//! A hash source is a *permission*, so every laxness here is a policy quietly
//! wider than its author wrote:
//!
//! - **The two alphabets are not mixed.** A value using both `+` and `-` is one
//!   no encoder produces, and the specification's own rule compares against the
//!   two spellings separately rather than against a blend of them.
//! - **The last group's spare bits must be zero.** Base64 that is not canonical
//!   has more than one spelling of the same bytes, and a permission with two
//!   spellings is a permission that can be written in a way its author would not
//!   recognise in their own header.
//! - **Nothing is trimmed, and no whitespace is skipped.** The value is what the
//!   header said between the quotes.
//!
//! Nothing here is constant-time, and that is deliberate rather than forgotten:
//! both sides of the comparison are public. The digest is the page's own
//! content and the expected value is in a header anybody can read.

use sha2::{Digest as _, Sha256, Sha384, Sha512};

/// Which digest a hash source names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digest {
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}

impl Digest {
    /// The name a policy writes it as.
    pub const fn name(self) -> &'static str {
        match self {
            Digest::Sha256 => "sha256",
            Digest::Sha384 => "sha384",
            Digest::Sha512 => "sha512",
        }
    }

    /// How many bytes long one of these is.
    pub const fn length(self) -> usize {
        match self {
            Digest::Sha256 => 32,
            Digest::Sha384 => 48,
            Digest::Sha512 => 64,
        }
    }

    /// The digest of some content.
    ///
    /// The bytes are the content's own, exactly as the document holds them: a
    /// digest is over what is there, so a stylesheet that gained a newline is a
    /// different stylesheet and a policy naming the old one does not allow it.
    pub fn of(self, content: &[u8]) -> Vec<u8> {
        match self {
            Digest::Sha256 => Sha256::digest(content).to_vec(),
            Digest::Sha384 => Sha384::digest(content).to_vec(),
            Digest::Sha512 => Sha512::digest(content).to_vec(),
        }
    }

    /// Whether `expected`, as an author wrote it in a policy, is this digest of
    /// this content.
    ///
    /// False for anything that is not — a value in neither alphabet, one of the
    /// wrong length for the algorithm, one whose bytes are somebody else's
    /// content. This module's header says why each of those is a non-match
    /// rather than an error.
    pub fn names(self, expected: &str, content: &[u8]) -> bool {
        let Some(wanted) = decoded(expected) else {
            return false;
        };
        wanted.len() == self.length() && wanted == self.of(content)
    }
}

/// The bytes a base64 value stands for, or nothing when it is not one this
/// engine will read.
///
/// Both alphabets and either padding, per this module's header — and refused
/// for anything else, including a mixture of the two alphabets, a value with
/// spare bits set in its last group, and any character at all that is not part
/// of one of them.
fn decoded(value: &str) -> Option<Vec<u8>> {
    let body = value.trim_end_matches('=');
    let padding = value.len().checked_sub(body.len())?;
    if padding > 2 || (padding > 0 && value.len() % 4 != 0) {
        return None;
    }
    // A group of one character is six bits, which stands for no whole byte.
    if body.is_empty() || body.len() % 4 == 1 {
        return None;
    }
    let mut standard = false;
    let mut safe = false;
    let mut bits: u32 = 0;
    let mut held: u32 = 0;
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 2);
    for byte in body.bytes() {
        let six: u8 = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => {
                standard = true;
                62
            }
            b'/' => {
                standard = true;
                63
            }
            b'-' => {
                safe = true;
                62
            }
            b'_' => {
                safe = true;
                63
            }
            _ => return None,
        };
        bits = (bits << 6) | u32::from(six);
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((bits >> held) & 0xff).ok()?);
        }
    }
    if standard && safe {
        return None;
    }
    // Whatever is left over is fewer than eight bits and stands for no byte, so
    // an encoder sets it to zero and any other value is a second spelling.
    if bits & ((1 << held) - 1) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digests of `abc`, from `python3 -c 'import hashlib, base64'` rather
    /// than from the crate that computes them here — a vector produced by the
    /// thing under test is a vector that agrees with itself.
    #[test]
    fn the_digests_of_abc_are_the_ones_everybody_elses_tool_produces() {
        assert!(Digest::Sha256.names("ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=", b"abc"));
        assert!(Digest::Sha384.names(
            "ywB1P0WjXou1oD1pmsZQBycsMqsO3tFjGotgWkP/W+2AhgcroefMI1i67KE0yCWn",
            b"abc",
        ));
        assert!(Digest::Sha512.names(
            "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==",
            b"abc",
        ));
    }

    #[test]
    fn a_digest_is_the_length_the_algorithm_says() {
        for digest in [Digest::Sha256, Digest::Sha384, Digest::Sha512] {
            assert_eq!(digest.of(b"abc").len(), digest.length(), "{digest:?}");
        }
    }

    #[test]
    fn one_digest_is_never_another_ones_value() {
        let expected = "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=";
        assert!(Digest::Sha256.names(expected, b"abc"));
        assert!(
            !Digest::Sha384.names(expected, b"abc"),
            "a sha256 value was read as a sha384 one, which is a permission the author never wrote",
        );
    }

    #[test]
    fn both_alphabets_are_read_and_neither_is_mixed_with_the_other() {
        let standard = "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==";
        let safe = "3a81oZNherrMQXNJriBBMRLm-k6JqX6iCp7u5ktV05ohkpkqJ0_BqDa6PCOj_uu9RU1EI2Q86A4qmslPpUyknw==";
        assert!(Digest::Sha512.names(standard, b"abc"));
        assert!(Digest::Sha512.names(safe, b"abc"), "the URL-safe alphabet");

        let mixed = "3a81oZNherrMQXNJriBBMRLm-k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==";
        assert!(
            !Digest::Sha512.names(mixed, b"abc"),
            "no encoder writes both alphabets, and reading one is a spelling nobody's author chose",
        );
    }

    #[test]
    fn padding_is_optional_and_never_wrong() {
        assert_eq!(decoded("YWJj"), Some(b"abc".to_vec()));
        assert_eq!(decoded("YWI="), Some(b"ab".to_vec()));
        assert_eq!(decoded("YWI"), Some(b"ab".to_vec()), "unpadded");
        assert_eq!(decoded("YWI=="), None, "one byte too much padding");
        assert_eq!(decoded("YWJj="), None, "padding to no multiple of four");
        assert_eq!(decoded("YW=j"), None, "padding in the middle");
        assert_eq!(decoded("===="), None);
    }

    /// Base64 whose last group has bits set that stand for no byte. `YWJjZA` is
    /// canonical for `abcd`; `YWJjZB` decodes to the same four bytes and is a
    /// second spelling of the same permission.
    #[test]
    fn a_value_that_is_not_canonical_is_not_read() {
        assert_eq!(decoded("YWJjZA"), Some(b"abcd".to_vec()));
        assert_eq!(decoded("YWJjZB"), None);
        assert_eq!(decoded("YWJjZA=="), Some(b"abcd".to_vec()));
        assert_eq!(decoded("YWJjZB=="), None);
    }

    /// Whatever a server writes between the quotes, the answer is a bool and
    /// nothing panics — a policy header is a stranger's bytes.
    #[test]
    fn nothing_a_server_can_write_is_worse_than_a_hash_that_matches_nothing() {
        for value in [
            "",
            "=",
            "==",
            "=YWJj",
            " YWJj",
            "YWJj ",
            "YW Jj",
            "YWJj\n",
            "Y",
            "YWJ$",
            "YWJj\u{0}",
            "héllo",
            "\u{feff}",
            "*",
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=",
        ] {
            for digest in [Digest::Sha256, Digest::Sha384, Digest::Sha512] {
                assert!(
                    !digest.names(value, b"abc"),
                    "{value:?} was read as a {} of `abc`",
                    digest.name(),
                );
            }
        }
    }

    /// A long value is read without trouble and matches nothing: the length
    /// check is what stops a header being a way to spend our time.
    #[test]
    fn a_value_far_longer_than_any_digest_is_read_and_matches_nothing() {
        let value = "A".repeat(8 * 1024);
        assert!(!Digest::Sha256.names(&value, b"abc"));
        assert_eq!(decoded(&value).map(|bytes| bytes.len()), Some(6 * 1024));
    }

    #[test]
    fn a_digest_is_over_the_bytes_and_not_over_a_tidied_version_of_them() {
        let spaced = "  spaced  ";
        assert!(Digest::Sha256.names(
            "HcwkoUpr+MtNkXUqmNuP/CWeNxVpA6eA+CbCTeYeKgU=",
            spaced.as_bytes(),
        ));
        assert!(
            !Digest::Sha256.names(
                "HcwkoUpr+MtNkXUqmNuP/CWeNxVpA6eA+CbCTeYeKgU=",
                spaced.trim().as_bytes(),
            ),
            "the content was trimmed, which is a different stylesheet",
        );
    }

    #[test]
    fn the_digest_of_nothing_at_all_is_a_digest() {
        assert!(Digest::Sha256.names("47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=", b""));
        assert!(!Digest::Sha256.names("47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=", b" "));
    }
}
