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
//!
//! # A key a person gives (ADR 0106 § 4)
//!
//! [`find`], [`keep`] and [`delete`] read, write and remove one item of a
//! [`Keychain`] — the person's own, or a test's file — for the keys a
//! person gives in *Settings* ([`crate::service_keys`]). A secret is
//! handed to the tool **on its standard input**, in its interactive mode,
//! **as hexadecimal** (`-X`): never on a command line, which another
//! program can read, and never in a form its line could misread.

use std::io::Write as _;
use std::path::PathBuf;
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

/// What the tool exits with when there is no such item.
const NOT_FOUND: i32 = 44;

/// Which keychain an item is kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keychain {
    /// The person's own: their default, the login keychain.
    Theirs,
    /// A keychain file of a test's own.
    File(PathBuf),
}

/// Whether `account` and `service` can be said to the tool as words of its
/// line: visible ASCII, no quote or backslash, something.
fn plain(word: &str) -> bool {
    !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
}

/// The tool, asked to `act` on the item of `service` for `account` in
/// `keychain`, with nothing on its input.
fn asked(keychain: &Keychain, act: &str, service: &str, account: &str) -> Command {
    let mut command = Command::new(SECURITY);
    command
        .args([act, "-a", account, "-s", service])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    if act == "find-generic-password" {
        command.arg("-w");
    }
    if let Keychain::File(file) = keychain {
        command.arg(file);
    }
    command
}

/// The generic password of `service` for `account` in `keychain`, if there
/// is one, with its line's end taken off.
///
/// # Errors
///
/// Why it could not be read, in words naming the item and never what the
/// tool said: no tool, a keychain locked or refusing, or what it holds not
/// being text.
pub fn find(keychain: &Keychain, service: &str, account: &str) -> Result<Option<String>, String> {
    if !plain(service) || !plain(account) {
        return Ok(None);
    }
    let read = asked(keychain, "find-generic-password", service, account)
        .output()
        .map_err(|why| format!("{SECURITY} could not be started to read {service} ({why})"))?;
    if read.status.code() == Some(NOT_FOUND) {
        return Ok(None);
    }
    if !read.status.success() {
        return Err(format!(
            "the keychain did not give the {service} item (it may be locked, or access was \
             refused)"
        ));
    }
    let text = String::from_utf8(read.stdout)
        .map_err(|_| format!("the keychain's {service} item is not text"))?;
    Ok(Some(text.trim_end_matches('\n').to_owned()))
}

/// Keep `secret` as the generic password of `service` for `account` in
/// `keychain`, labelled `label`, in place of any it held: handed to the
/// tool on its standard input, as hexadecimal.
///
/// # Errors
///
/// Why it was not kept, in words that never hold the secret.
pub fn keep(
    keychain: &Keychain,
    (service, account, label): (&str, &str, &str),
    secret: &str,
) -> Result<(), String> {
    let refused = |why: &dyn core::fmt::Display| {
        format!("the {service} item could not be kept in the keychain ({why})")
    };
    if !plain(service) || !plain(account) {
        return Err(refused(&"its name cannot be said to the tool"));
    }
    if label.contains(['"', '\\']) || label.chars().any(char::is_control) {
        return Err(refused(&"its label cannot be said to the tool"));
    }
    let file = match keychain {
        Keychain::Theirs => String::new(),
        Keychain::File(file) => match file.to_str().filter(|file| plain_path(file)) {
            Some(said) => format!(" \"{said}\""),
            None => {
                return Err(refused(
                    &"a test keychain's path cannot be said to the tool",
                ));
            }
        },
    };
    let hex: String = secret
        .bytes()
        .flat_map(|byte| [byte >> 4, byte & 0xf])
        .filter_map(|digit| char::from_digit(u32::from(digit), 16))
        .collect();
    let line = format!(
        "add-generic-password -U -a {account} -s {service} -l \"{label}\" -X {hex}{file}\n"
    );
    let mut child = Command::new(SECURITY)
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| refused(&why))?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| refused(&"no way to hand it the key"))
        .and_then(|mut input| {
            input
                .write_all(line.as_bytes())
                .map_err(|why| refused(&why))
        });
    let waited = child.wait().map_err(|why| refused(&why))?;
    written?;
    if waited.success() {
        Ok(())
    } else {
        Err(refused(&"the tool refused"))
    }
}

/// Whether a test keychain's path can be said inside quotes on the tool's
/// line.
fn plain_path(path: &str) -> bool {
    !path
        .chars()
        .any(|c| c == '"' || c == '\\' || c.is_control())
}

/// Remove the generic password of `service` for `account` from `keychain`:
/// whether there was one.
///
/// # Errors
///
/// Why it could not be removed, in words naming the item.
pub fn delete(keychain: &Keychain, service: &str, account: &str) -> Result<bool, String> {
    if !plain(service) || !plain(account) {
        return Ok(false);
    }
    let removed = asked(keychain, "delete-generic-password", service, account)
        .stdout(Stdio::null())
        .status()
        .map_err(|why| format!("{SECURITY} could not be started to remove {service} ({why})"))?;
    match removed.code() {
        Some(0) => Ok(true),
        Some(NOT_FOUND) => Ok(false),
        _ => Err(format!(
            "the keychain did not remove the {service} item (it may be locked, or access was \
             refused)"
        )),
    }
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
