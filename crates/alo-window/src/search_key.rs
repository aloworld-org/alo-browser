/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A service's key, held and never said (ADR 0083 § 1, ADR 0091, ADR 0106
//! § 4; queue items 708, 748 and 904).
//!
//! Which key a service is asked with — the person's own given in
//! *Settings*, the one a release carries, or an older alo's keychain item
//! such as [`SERVICE`] — is [`crate::service_keys`]'s to say; this is the
//! [`Key`] it hands back, and where it was [`Held`], which is what a
//! refusal tells a person to look at. The key is never in the repository,
//! and never compiled into a program.
//!
//! # Never said
//!
//! A [`Key`] is held in memory and handed to the one request header that
//! needs it. Its `Debug` says only that a key is held; there is no
//! `Display`; and nothing here writes it, or what the tool said about it,
//! anywhere — the tool's own complaints go nowhere, and a refusal says only
//! which item was missing.

use core::fmt;

/// The keychain item's service an older alo read the Brave Search API's
/// key from, still read after a carried key ([`crate::services::Keyed::older`]).
pub const SERVICE: &str = "alo-brave-search";

/// The longest key taken: Brave's are about thirty characters.
const LONGEST: usize = 256;

/// Where a key was held, which is what a person is told to look at when
/// the provider refuses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// In this Mac's keychain, read at run time.
    Keychain,
    /// Inside the application, put there as it was made (ADR 0091).
    Carried,
    /// In this Mac's keychain, given by the person in *Settings*, under the
    /// service's own item (ADR 0106 § 4): it wins over the others.
    Settings,
}

/// A provider's key, held and never said.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    /// The key.
    written: String,
    /// Where it was held.
    held: Held,
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(held, never said)")
    }
}

impl Key {
    /// A key that is `written`, held in a keychain, if it is one a request
    /// header can carry: visible ASCII only, at most 256 characters, and
    /// something.
    pub fn of(written: &str) -> Option<Self> {
        Self::held_as(written, Held::Keychain)
    }

    /// A key that is `written`, carried inside the application, if it is
    /// one a request header can carry, as [`Key::of`].
    pub fn carried(written: &str) -> Option<Self> {
        Self::held_as(written, Held::Carried)
    }

    /// A key that is `written`, given by the person in *Settings* (ADR
    /// 0106 § 4), if it is one a request header can carry, as [`Key::of`].
    pub fn given(written: &str) -> Option<Self> {
        Self::held_as(written, Held::Settings)
    }

    fn held_as(written: &str, held: Held) -> Option<Self> {
        let written = written.trim();
        (!written.is_empty()
            && written.len() <= LONGEST
            && written.bytes().all(|byte| byte.is_ascii_graphic()))
        .then(|| Self {
            written: written.to_owned(),
            held,
        })
    }

    /// Where it was held.
    pub fn held(&self) -> Held {
        self.held
    }

    /// The key, for the one header that carries it, and for the one file
    /// that carries it inside the application ([`crate::carried_keys::masked`]).
    pub(crate) fn header_value(&self) -> &str {
        &self.written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_never_said() {
        let key = Key::of("  not-a-real-key-0123  ").unwrap();
        assert_eq!(key.header_value(), "not-a-real-key-0123");
        assert_eq!(key.held(), Held::Keychain);
        let carried = Key::carried("not-a-real-key-0123").unwrap();
        assert_eq!(carried.held(), Held::Carried);
        assert_ne!(carried, key, "the same characters, held elsewhere");
        assert!(!format!("{carried:?}").contains("not-a-real"));
        let said = format!("{key:?}");
        assert!(!said.contains("not-a-real"), "{said}");
    }

    #[test]
    fn a_key_a_header_cannot_carry_is_none() {
        assert_eq!(Key::of(""), None);
        assert_eq!(Key::of("   \n"), None);
        assert_eq!(Key::of("two words"), None);
        assert_eq!(Key::of("a\r\nX-Other: b"), None);
        assert_eq!(Key::of("é"), None);
        assert_eq!(Key::of(&"k".repeat(LONGEST + 1)), None);
        assert_eq!(Key::carried("two words"), None);
    }
}
