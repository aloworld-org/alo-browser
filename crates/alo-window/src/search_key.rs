/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The Brave Search API's key: carried inside a release of the application,
//! or read from this Mac's login keychain at run time (ADR 0083 § 1, ADR
//! 0091; queue items 708 and 748).
//!
//! **In a release** the key is carried inside the application, in its
//! search slot ([`crate::carried_keys`]), put there by `alo-bundle` from
//! outside the repository as the application is made, and read by the
//! browser process as the conversation opens
//! ([`for_this_application`]). **Anywhere else** — a program built and run
//! from a terminal, a test, an application made with no key — it is a
//! generic password in this Mac's keychain: service [`SERVICE`], account
//! the person's login name, read through [`crate::keychain`]. The key is
//! never in the repository, and never compiled into a program.
//!
//! # Never said
//!
//! A [`Key`] is held in memory and handed to the one request header that
//! needs it. Its `Debug` says only that a key is held; there is no
//! `Display`; and nothing here writes it, or what the tool said about it,
//! anywhere — the tool's own complaints go nowhere, and a refusal says only
//! which item was missing.

use crate::bundle::Bundle;
use crate::carried_keys::{self, Slot};
use crate::keychain::{self, SECURITY, Unread};
use core::fmt;

/// The keychain item's service.
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
    /// that carries it inside the application ([`carried_keys::masked`]).
    pub(crate) fn header_value(&self) -> &str {
        &self.written
    }
}

/// Read the key from this Mac's login keychain, for the account `account`.
///
/// # Errors
///
/// Why there is none, in words a person reads under the box's offer to
/// search — never the key, and never what the tool printed.
pub fn from_the_keychain(account: &str) -> Result<Key, String> {
    let missing = || "no search key in this Mac's keychain".to_owned();
    match keychain::generic_password(SERVICE, account) {
        Ok(written) => Key::of(&written).ok_or_else(missing),
        Err(Unread::NoTool) => Err(format!(
            "this Mac's keychain could not be read ({SECURITY})"
        )),
        Err(Unread::Missing) => Err(missing()),
    }
}

/// Read the key for the person running this: their login name, as `USER`
/// says it.
///
/// # Errors
///
/// As [`from_the_keychain`].
pub fn for_this_person() -> Result<Key, String> {
    from_the_keychain(&keychain::this_person())
}

/// The key for the browser running inside `bundle`, or outside any
/// application when [`None`]: the one the application carries in its
/// search slot (ADR 0091), and otherwise this Mac's keychain's, as
/// [`for_this_person`] reads it.
///
/// # Errors
///
/// A key carried that cannot be read is said, and the keychain is not then
/// asked — an application's own key is never quietly swapped for another;
/// with none carried, as [`from_the_keychain`].
pub fn for_this_application(bundle: Option<&Bundle>) -> Result<Key, String> {
    if let Some(bundle) = bundle
        && let Some(carried) = carried_keys::read(bundle.resources(), Slot::Search)?
    {
        return Ok(carried);
    }
    for_this_person()
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

    #[cfg(target_os = "macos")]
    #[test]
    fn an_item_nobody_made_is_missing_and_says_which() {
        // An account no keychain holds an item for: the tool refuses, and
        // what is said names what is missing, not anything the tool printed.
        let refused = from_the_keychain("alo-no-such-account-for-a-test").err();
        assert_eq!(
            refused.as_deref(),
            Some("no search key in this Mac's keychain")
        );
        assert_eq!(
            from_the_keychain("").err().as_deref(),
            Some("no search key in this Mac's keychain")
        );
    }
}
