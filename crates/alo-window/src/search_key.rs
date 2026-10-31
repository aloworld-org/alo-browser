/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The Brave Search API's key, read from this Mac's login keychain at run
//! time, for development (ADR 0083 § 1, queue item 708).
//!
//! The key is a generic password: service [`SERVICE`], account the
//! person's login name. It is read by asking the Security framework's own
//! tool, `/usr/bin/security`, and taking what it prints — no `unsafe` and
//! no new crate. It is never put in the binary or the repository, and in
//! release how alo carries a key is the owner's decision (ADR 0083 § 1).
//!
//! # Never said
//!
//! A [`Key`] is held in memory and handed to the one request header that
//! needs it. Its `Debug` says only that a key is held; there is no
//! `Display`; and nothing here writes it, or what the tool said about it,
//! anywhere — the tool's own complaints go nowhere, and a refusal says only
//! which item was missing.

use core::fmt;
use std::process::{Command, Stdio};

/// The keychain item's service.
pub const SERVICE: &str = "alo-brave-search";

/// The tool that reads the keychain.
const SECURITY: &str = "/usr/bin/security";

/// The longest key taken: Brave's are about thirty characters.
const LONGEST: usize = 256;

/// The search provider's key, held and never said.
#[derive(Clone, PartialEq, Eq)]
pub struct Key(String);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(held, never said)")
    }
}

impl Key {
    /// A key that is `written`, if it is one a request header can carry:
    /// visible ASCII only, at most 256 characters, and something.
    pub fn of(written: &str) -> Option<Self> {
        let written = written.trim();
        (!written.is_empty()
            && written.len() <= LONGEST
            && written.bytes().all(|byte| byte.is_ascii_graphic()))
        .then(|| Self(written.to_owned()))
    }

    /// The key, for the one header that carries it.
    pub(crate) fn header_value(&self) -> &str {
        &self.0
    }
}

/// Read the key from this Mac's login keychain, for the account `account`.
///
/// # Errors
///
/// Why there is none, in words a person reads under the box's offer to
/// search — never the key, and never what the tool printed.
pub fn from_the_keychain(account: &str) -> Result<Key, String> {
    let missing = || format!("no Brave Search key in this Mac's keychain ({SERVICE})");
    if account.is_empty() {
        return Err(missing());
    }
    let read = Command::new(SECURITY)
        .args(["find-generic-password", "-a", account, "-s", SERVICE, "-w"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| format!("this Mac's keychain could not be read ({SECURITY})"))?;
    if !read.status.success() {
        return Err(missing());
    }
    let written = String::from_utf8(read.stdout).map_err(|_| missing())?;
    Key::of(&written).ok_or_else(missing)
}

/// Read the key for the person running this: their login name, as `USER`
/// says it.
///
/// # Errors
///
/// As [`from_the_keychain`].
pub fn for_this_person() -> Result<Key, String> {
    from_the_keychain(&std::env::var("USER").unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_never_said() {
        let key = Key::of("  not-a-real-key-0123  ").unwrap();
        assert_eq!(key.header_value(), "not-a-real-key-0123");
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
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_item_nobody_made_is_missing_and_says_which() {
        // An account no keychain holds an item for: the tool refuses, and
        // what is said names the item, not anything the tool printed.
        let refused = from_the_keychain("alo-no-such-account-for-a-test").err();
        assert_eq!(
            refused.as_deref(),
            Some("no Brave Search key in this Mac's keychain (alo-brave-search)")
        );
        assert_eq!(
            from_the_keychain("").err().as_deref(),
            Some("no Brave Search key in this Mac's keychain (alo-brave-search)")
        );
    }
}
