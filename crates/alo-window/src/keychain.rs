/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A generic password read from this Mac's login keychain (ADR 0083 § 1,
//! ADR 0091).
//!
//! It is read by asking the Security framework's own tool,
//! `/usr/bin/security`, and taking what it prints — no `unsafe` and no new
//! crate. The browser reads the search key this way at run time when it is
//! not inside an application that carries one ([`crate::search_key`]), and
//! `alo-bundle` reads each key a release carries this way as it makes the
//! application ([`crate::bundle_keys`]).
//!
//! # Never said
//!
//! What the tool printed is handed back and nothing else is done with it:
//! the tool's own complaints go nowhere, and a refusal is only *none*, so
//! no caller can say more than which item was missing.

use std::process::{Command, Stdio};

/// The tool that reads the keychain.
pub const SECURITY: &str = "/usr/bin/security";

/// Why no password was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// The tool could not be started.
    NoTool,
    /// No such item for the account, the tool refusing, or what it printed
    /// not being text.
    Missing,
}

/// The generic password of `service` for `account`, as the tool printed it
/// (with its line's end still on it).
///
/// # Errors
///
/// [`Unread`]: an empty account is [`Unread::Missing`] without asking.
pub fn generic_password(service: &str, account: &str) -> Result<String, Unread> {
    if account.is_empty() || service.is_empty() {
        return Err(Unread::Missing);
    }
    let read = Command::new(SECURITY)
        .args(["find-generic-password", "-a", account, "-s", service, "-w"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| Unread::NoTool)?;
    if !read.status.success() {
        return Err(Unread::Missing);
    }
    String::from_utf8(read.stdout).map_err(|_| Unread::Missing)
}

/// The login name of the person running this, as `USER` says it: the
/// account a key is held under.
pub fn this_person() -> String {
    std::env::var("USER").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn an_item_nobody_made_is_missing() {
        assert_eq!(
            generic_password("alo-brave-search", "alo-no-such-account-for-a-test"),
            Err(Unread::Missing)
        );
        assert_eq!(
            generic_password(
                "alo-no-such-service-for-a-test",
                "alo-no-such-account-for-a-test"
            ),
            Err(Unread::Missing)
        );
    }

    #[test]
    fn nothing_is_asked_for_no_account_or_no_service() {
        assert_eq!(
            generic_password("alo-brave-search", ""),
            Err(Unread::Missing)
        );
        assert_eq!(generic_password("", "someone"), Err(Unread::Missing));
    }
}
