/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The key the kept chats are sealed under, kept in the person's keychain
//! (queue item 790, ADR 0098 § 2).
//!
//! A generic password of the service [`SERVICE`] for the person's account,
//! 64 hexadecimal digits: made by alo the first time chats are kept and
//! there is none ([`ChatKey::in_keychain`]), read every launch after.
//!
//! It is read and made by asking the Security framework's own tool,
//! `/usr/bin/security`, as every key alo reads is ([`crate::keychain`]) —
//! no `unsafe` and no new crate. **Never on a command line**: a new key is
//! handed to the tool on its standard input, in its interactive mode,
//! because another program can read any process's arguments. What the tool
//! prints is the key and nothing else is done with it; its complaints go
//! nowhere, and a refusal says only which item and why in this browser's
//! words.
//!
//! **A key is made only when the keychain says there is none** (the tool's
//! *item not found*, 44). A keychain that is locked, refuses, or holds
//! something that is not a key is said and no key is made, so a key that
//! is there is never replaced by another: replaced, every chat sealed under
//! it would never open again (ADR 0098 § 3). An item that is there is never
//! written over (no `-U`).
//!
//! Tests name a keychain file of their own ([`Keychain::File`]), made and
//! deleted under the temporary directory, never the person's.

use crate::chat_seal::{KEY_BYTES, Key};
use crate::keychain::SECURITY;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The keychain item's service: what the person sees it called in
/// *Keychain Access*.
pub const SERVICE: &str = "alo-chats";

/// What the item is labelled.
pub const LABEL: &str = "alo: the key the chats on this Mac are sealed under";

/// What the tool exits with when there is no such item.
const NOT_FOUND: i32 = 44;

/// Which keychain the key is kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keychain {
    /// The person's own: their default, the login keychain.
    Theirs,
    /// A keychain file of a test's own.
    File(PathBuf),
}

/// The key the chats are sealed under, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatKey {
    /// The key.
    pub key: Key,
    /// Whether it was made just now, because there was none.
    pub made: bool,
}

/// What became of asking the keychain.
enum Found {
    Key(Key),
    None,
}

impl ChatKey {
    /// The key in this person's own keychain, made if there is none.
    ///
    /// # Errors
    ///
    /// As [`Self::in_keychain`].
    pub fn for_this_person() -> Result<Self, String> {
        Self::in_keychain(&Keychain::Theirs, &crate::keychain::this_person())
    }

    /// The key of `account` in `keychain`, made — fresh from the system's
    /// random source — only if the keychain says there is none.
    ///
    /// # Errors
    ///
    /// Why there is no key, in words that never hold one: no account, the
    /// tool missing, the keychain refusing or locked, an item that is not a
    /// key, or a key that could not be made.
    pub fn in_keychain(keychain: &Keychain, account: &str) -> Result<Self, String> {
        let plain = |byte: u8| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\';
        if account.is_empty() || !account.bytes().all(plain) {
            return Err(format!(
                "the keychain item {SERVICE} is not read for an account named {account:?}"
            ));
        }
        if let Found::Key(key) = find(keychain, account)? {
            return Ok(Self { key, made: false });
        }
        let key = Key::fresh()?;
        add(keychain, account, &key)?;
        // Read back: what is kept is what is used, even if another alo made
        // one between the two.
        match find(keychain, account)? {
            Found::Key(kept) => Ok(Self {
                made: kept == key,
                key: kept,
            }),
            Found::None => Err(format!(
                "the keychain did not keep a new {SERVICE} item, so chats are not sealed"
            )),
        }
    }
}

/// The tool, asked of `keychain`.
fn tool() -> Command {
    let mut command = Command::new(SECURITY);
    command.stdin(Stdio::null()).stderr(Stdio::null());
    command
}

/// The key of `account` in `keychain`, or that there is none.
fn find(keychain: &Keychain, account: &str) -> Result<Found, String> {
    let mut command = tool();
    command.args(["find-generic-password", "-a", account, "-s", SERVICE, "-w"]);
    if let Keychain::File(file) = keychain {
        command.arg(file);
    }
    let read = command
        .output()
        .map_err(|why| format!("{SECURITY} could not be started to read {SERVICE} ({why})"))?;
    if read.status.code() == Some(NOT_FOUND) {
        return Ok(Found::None);
    }
    if !read.status.success() {
        return Err(format!(
            "the keychain did not give the {SERVICE} item (it may be locked, or access was \
             refused), so chats are not kept"
        ));
    }
    let text = String::from_utf8(read.stdout).unwrap_or_default();
    from_hex(text.trim_end_matches('\n'))
        .map(Found::Key)
        .ok_or_else(|| {
            format!(
                "the keychain's {SERVICE} item is not a key alo made, so chats are not kept and \
             it is left as it is"
            )
        })
}

/// Keep `key` as `account`'s in `keychain`, handed to the tool on its
/// standard input.
fn add(keychain: &Keychain, account: &str, key: &Key) -> Result<(), String> {
    let file = match keychain {
        Keychain::Theirs => String::new(),
        Keychain::File(file) => {
            let said = file.to_str().filter(|file| {
                !file
                    .chars()
                    .any(|c| c == '"' || c == '\\' || c.is_control())
            });
            let Some(said) = said else {
                return Err("a test keychain's path cannot be said to the tool".to_owned());
            };
            format!(" \"{said}\"")
        }
    };
    let line = format!(
        "add-generic-password -a {account} -s {SERVICE} -l \"{LABEL}\" -w {}{file}\n",
        to_hex(key)
    );
    let refused = |why: &dyn core::fmt::Display| {
        format!("a {SERVICE} item could not be made in the keychain ({why})")
    };
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

/// `key` in lowercase hexadecimal.
fn to_hex(key: &Key) -> String {
    key.bytes()
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0xf])
        .filter_map(|digit| char::from_digit(u32::from(digit), 16))
        .collect()
}

/// A key from exactly 64 hexadecimal digits.
fn from_hex(text: &str) -> Option<Key> {
    if text.len() != KEY_BYTES * 2 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0; KEY_BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
        let pair = core::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(Key::of(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_kept_as_sixty_four_hexadecimal_digits() {
        let mut bytes = [0; KEY_BYTES];
        for (at, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::try_from(at * 7).unwrap_or(0);
        }
        let key = Key::of(bytes);
        let hex = to_hex(&key);
        assert_eq!(hex.len(), 64);
        assert!(hex.starts_with("00070e15"), "{hex}");
        assert_eq!(from_hex(&hex), Some(key));
        for not in ["", "00", &"g".repeat(64), &"0".repeat(63), &"0".repeat(66)] {
            assert_eq!(from_hex(not), None, "{not:?}");
        }
    }

    #[test]
    fn no_account_or_an_odd_one_is_never_asked() {
        for account in ["", "two words", "a\"b"] {
            let refused = ChatKey::in_keychain(&Keychain::Theirs, account)
                .err()
                .unwrap_or_default();
            assert!(refused.contains("is not read for an account"), "{refused}");
        }
    }

    /// A keychain file of this test's own, made unlocked under the
    /// temporary directory, and deleted when dropped.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = Command::new(SECURITY)
                .arg("delete-keychain")
                .arg(&self.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_key_is_made_once_in_a_keychain_with_none_and_read_back_after() {
        let file =
            std::env::temp_dir().join(format!("alo-chat-key-{}.keychain-db", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let made = Command::new(SECURITY)
            .args(["create-keychain", "-p", "alo-test"])
            .arg(&file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("the tool");
        assert!(made.success());
        let scratch = Scratch(file.clone());
        let keychain = Keychain::File(file);
        let first = ChatKey::in_keychain(&keychain, "alo-test-account").expect("made");
        assert!(first.made);
        let again = ChatKey::in_keychain(&keychain, "alo-test-account").expect("read");
        assert!(!again.made, "read, never made again");
        assert_eq!(again.key, first.key);
        // Another account has a key of its own.
        let other = ChatKey::in_keychain(&keychain, "alo-other-account").expect("made");
        assert!(other.made);
        assert_ne!(other.key, first.key);
        // An item that is not a key is said, and left: no key is made over it.
        let line = format!(
            "add-generic-password -a alo-odd -s {SERVICE} -w not-a-key \"{}\"\n",
            scratch.0.display()
        );
        let mut child = Command::new(SECURITY)
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the tool");
        child
            .stdin
            .take()
            .expect("its input")
            .write_all(line.as_bytes())
            .expect("written");
        assert!(child.wait().expect("waited").success());
        let refused = ChatKey::in_keychain(&keychain, "alo-odd")
            .err()
            .unwrap_or_default();
        assert!(refused.contains("is not a key alo made"), "{refused}");
        drop(scratch);
    }
}
