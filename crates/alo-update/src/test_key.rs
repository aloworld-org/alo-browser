/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The worthless test keys' public halves (ADR 0116 § 6), so that a build
//! can refuse them.
//!
//! # Worthless by design
//!
//! Two minisign key pairs — one *in use*, one *spare* — made for this
//! repository's tests alone, on 14 November 2026. Their **secret halves are
//! committed too**, in `tests/worthless_test_key/mod.rs`, so anybody can
//! sign anything with them: that is the point. A test can sign a manifest
//! with no real key anywhere near it.
//!
//! So nothing a person runs may trust them. [`crate::given`] refuses a
//! build given either as one of its keys, and `alo-bundle` refuses to put
//! one in an application (`alo-window`'s `bundle_update.rs`); both are
//! asserted. These constants are in the shipped program only to be
//! refused.

use crate::signature::Key;

/// The worthless test key *in use*'s public half (key id `76D033E0D68495CB`).
pub const PUBLIC: &str = "RWR20DPg1oSVy0j9Pj8XmnWds4OVK45bWSsRHcxmO3eQTjTgUtzU2kKE";

/// The worthless *spare* test key's public half (key id `8E42A8FB17280A03`).
pub const SPARE: &str = "RWSOQqj7FygKA3y1MKc4xHGFAKwRuZhxUzsHSoss1hYdRuBOsAaZJVuq";

/// Whether `key` is one of the worthless test keys, however it was written.
pub fn is_a_test_key(key: &Key) -> bool {
    [PUBLIC, SPARE]
        .into_iter()
        .filter_map(|test| Key::read(test).ok())
        .any(|test| test == *key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_test_keys_read_and_are_known_as_test_keys() {
        for test in [PUBLIC, SPARE] {
            let key = Key::read(test).expect("a key");
            assert!(is_a_test_key(&key));
        }
        let other =
            Key::read("RWSnhCT54ZPK6PIMYN2tCd7c4c3o0XBhFzBi8pI/aNszPoU7TSXh7QIr").expect("a key");
        assert!(!is_a_test_key(&other));
    }
}
