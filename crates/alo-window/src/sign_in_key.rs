/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The key the kept sign-ins are sealed under, kept in the person's
//! keychain (ADR 0111 § 3, queue item 935).
//!
//! A generic password of the service [`SERVICE`] for the person's account,
//! 64 hexadecimal digits, made by alo the first time it keeps a sign-in
//! and there is none — as the chats' key is (ADR 0098 § 2), but **a key of
//! its own**: *Forget all sign-ins* puts a new one in its place
//! ([`replaced`]), so a copy of the old file — a backup, a synced folder —
//! never opens again, which a key shared with the chats could not do
//! without losing them.
//!
//! It is read and written through `/usr/bin/security`
//! ([`crate::keychain`]): never on a command line, never printed. A
//! keychain that is locked or refuses is said, and **no key is made**, so
//! a key that is there is never replaced but by *Forget all*.
//!
//! # Not the person's keychain, in a test or a hand run
//!
//! Tests name a keychain file of their own ([`Keychain::File`]). `alo
//! --sign-ins DIR` — for a hand run — keeps the key in a keychain file in
//! that directory, made and unlocked there ([`scratch_keychain`]), so a
//! hand run never adds an item to the person's own.

use crate::chat_seal::{KEY_BYTES, Key};
use crate::keychain::{self, Keychain, SECURITY};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The keychain item's service: what the person sees it called in
/// *Keychain Access*.
pub const SERVICE: &str = "alo-sign-ins";

/// What the item is labelled.
pub const LABEL: &str = "alo: the key the sign-ins kept on this Mac are sealed under";

/// The keychain file a hand run's key is kept in, in its directory.
pub const SCRATCH_KEYCHAIN: &str = "alo-sign-ins.keychain-db";

/// The scratch keychain's password: not a secret, since it guards a
/// keychain made for a hand run alone.
const SCRATCH_PASSWORD: &str = "alo-hand-run";

/// The key of `account` in `keychain`, made — fresh from the system's
/// random source — only if the keychain says there is none.
///
/// # Errors
///
/// Why there is no key, in words that never hold one: the keychain locked
/// or refusing, an item that is not a key alo made, or a key that could
/// not be made.
pub fn in_keychain(keychain: &Keychain, account: &str) -> Result<Key, String> {
    if let Some(kept) = keychain::find(keychain, SERVICE, account)? {
        return of_hex(&kept).ok_or_else(|| {
            format!("the keychain's {SERVICE} item is not a key alo made, and is left as it is")
        });
    }
    let key = Key::fresh()?;
    keychain::keep(keychain, (SERVICE, account, LABEL), &hex(&key))?;
    // Read back: what is kept is what is used.
    keychain::find(keychain, SERVICE, account)?
        .as_deref()
        .and_then(of_hex)
        .ok_or_else(|| format!("the keychain did not keep a new {SERVICE} item"))
}

/// A new key in place of `account`'s in `keychain`, whatever it held:
/// *Forget all sign-ins* (ADR 0111 § 5).
///
/// # Errors
///
/// Why it was not replaced, in words that never hold a key.
pub fn replaced(keychain: &Keychain, account: &str) -> Result<Key, String> {
    let key = Key::fresh()?;
    keychain::keep(keychain, (SERVICE, account, LABEL), &hex(&key))?;
    match keychain::find(keychain, SERVICE, account)?
        .as_deref()
        .and_then(of_hex)
    {
        Some(kept) if kept == key => Ok(key),
        _ => Err(format!("the keychain did not keep the new {SERVICE} item")),
    }
}

/// The keychain file in `directory` a hand run's key is kept in — made
/// there if it is not, and unlocked.
///
/// # Errors
///
/// Why it could not be made or unlocked.
pub fn scratch_keychain(directory: &Path) -> Result<Keychain, String> {
    let file: PathBuf = directory.join(SCRATCH_KEYCHAIN);
    let tool = |act: &str| {
        Command::new(SECURITY)
            .args([act, "-p", SCRATCH_PASSWORD])
            .arg(&file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|why| format!("{SECURITY} could not be started ({why})"))
    };
    if !file.exists() && !tool("create-keychain")?.success() {
        return Err(format!("no keychain could be made at {}", file.display()));
    }
    if !tool("unlock-keychain")?.success() {
        return Err(format!("the keychain at {} did not unlock", file.display()));
    }
    Ok(Keychain::File(file))
}

/// `key` in lowercase hexadecimal.
fn hex(key: &Key) -> String {
    key.bytes()
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0xf])
        .filter_map(|digit| char::from_digit(u32::from(digit), 16))
        .collect()
}

/// The key `text` spells in hexadecimal, if it is one.
fn of_hex(text: &str) -> Option<Key> {
    let digits: Vec<u8> = text
        .chars()
        .map(|c| c.to_digit(16).and_then(|digit| u8::try_from(digit).ok()))
        .collect::<Option<_>>()?;
    if digits.len() != KEY_BYTES * 2 {
        return None;
    }
    let mut bytes = [0; KEY_BYTES];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks(2)) {
        let [high, low] = <[u8; 2]>::try_from(pair).ok()?;
        *byte = high << 4 | low;
    }
    Some(Key::of(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_reads_back_from_its_hexadecimal_and_nothing_else_does() {
        let key = Key::of([0xa5; KEY_BYTES]);
        assert_eq!(of_hex(&hex(&key)), Some(key));
        assert_eq!(hex(&Key::of([0x0f; KEY_BYTES])), "0f".repeat(KEY_BYTES));
        assert_eq!(of_hex(""), None);
        assert_eq!(of_hex(&"0".repeat(KEY_BYTES * 2 - 1)), None);
        assert_eq!(of_hex(&"g".repeat(KEY_BYTES * 2)), None);
        assert_eq!(of_hex(&"0".repeat(KEY_BYTES * 2 + 2)), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_key_is_made_once_read_back_and_replaced_only_by_forgetting() {
        let directory =
            std::env::temp_dir().join(format!("alo-sign-in-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("made");
        let keychain = scratch_keychain(&directory).expect("a keychain of the test's own");
        let first = in_keychain(&keychain, "alo-test-account").expect("made");
        let again = in_keychain(&keychain, "alo-test-account").expect("read");
        assert_eq!(again, first, "read, never made again");
        let new = replaced(&keychain, "alo-test-account").expect("replaced");
        assert_ne!(new, first);
        assert_eq!(in_keychain(&keychain, "alo-test-account"), Ok(new));
        // Unlocked again, the same keychain file.
        assert_eq!(scratch_keychain(&directory), Ok(keychain.clone()));
        if let Keychain::File(file) = &keychain {
            let _ = Command::new(SECURITY)
                .arg("delete-keychain")
                .arg(file)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = std::fs::remove_dir_all(&directory);
    }
}
