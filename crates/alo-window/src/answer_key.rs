/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `OpenAI`'s API key, with which alo asks for a search's answer: carried
//! inside a release of the application, or read from this Mac's login
//! keychain at run time (ADR 0081, ADR 0091; queue item 773).
//!
//! As the search key is ([`crate::search_key`]): **in a release** it is
//! carried in the application's answer slot ([`crate::carried_keys`]), put
//! there by `alo-bundle` from outside the repository as the application is
//! made, and read by the browser process alone as the conversation opens
//! ([`for_this_application`]); **anywhere else** it is a generic password in
//! this Mac's keychain — service [`SERVICE`], account the person's login
//! name. It is held as a [`Key`], whose `Debug` never says it, and handed
//! to the one request header that carries it. It is never in the
//! repository, a program, a log or a sentence.

use crate::bundle::Bundle;
use crate::carried_keys::{self, Slot};
use crate::keychain::{self, SECURITY, Unread};
use crate::search_key::Key;

/// The keychain item's service.
pub const SERVICE: &str = "alo-openai";

/// Read the key from this Mac's login keychain, for the account `account`.
///
/// # Errors
///
/// Why there is none, in words a person reads where the answer would be —
/// never the key, and never what the tool printed.
pub fn from_the_keychain(account: &str) -> Result<Key, String> {
    let missing = || format!("no OpenAI key in this Mac's keychain ({SERVICE})");
    match keychain::generic_password(SERVICE, account) {
        Ok(written) => Key::of(&written).ok_or_else(missing),
        Err(Unread::NoTool) => Err(format!(
            "this Mac's keychain could not be read ({SECURITY})"
        )),
        Err(Unread::Missing) => Err(missing()),
    }
}

/// The key for the browser running inside `bundle`, or outside any
/// application when [`None`]: the one the application carries in its
/// answer slot (ADR 0091), and otherwise this Mac's keychain's, for the
/// person running it.
///
/// # Errors
///
/// A key carried that cannot be read is said, and the keychain is not then
/// asked — an application's own key is never quietly swapped for another;
/// with none carried, as [`from_the_keychain`].
pub fn for_this_application(bundle: Option<&Bundle>) -> Result<Key, String> {
    if let Some(bundle) = bundle
        && let Some(carried) = carried_keys::read(bundle.resources(), Slot::Answer)?
    {
        return Ok(carried);
    }
    from_the_keychain(&keychain::this_person())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn an_item_nobody_made_is_missing_and_says_which() {
        // An account no keychain holds an item for: the tool refuses, and
        // what is said names the item, not anything the tool printed.
        assert_eq!(
            from_the_keychain("alo-no-such-account-for-a-test")
                .err()
                .as_deref(),
            Some("no OpenAI key in this Mac's keychain (alo-openai)")
        );
        assert_eq!(
            from_the_keychain("").err().as_deref(),
            Some("no OpenAI key in this Mac's keychain (alo-openai)")
        );
    }
}
