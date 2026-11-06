/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A kept chat sealed under the person's key, and opened again (queue item
//! 790, ADR 0098): the one file that names the rented cipher,
//! `chacha20poly1305` — `RustCrypto`'s `XChaCha20-Poly1305`.
//!
//! # What a sealed file is
//!
//! [`MAGIC`], then a nonce of 24 bytes read fresh from the system's random
//! source for every write, then the chat's words as
//! [`crate::chat_record::write`] wrote them, encrypted and followed by the
//! cipher's 16-byte tag. The magic and **the chat's number** are the
//! associated data: a file copied over another chat's, or a sealed file of
//! another format, does not open, as a changed byte does not.
//!
//! # The key
//!
//! [`Key`] is 32 bytes, made by [`Key::fresh`] and kept in the person's
//! keychain by [`crate::chat_key`]; it is never printed — its `Debug` says
//! only that it is one — and never written anywhere but there.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use core::fmt;
use std::fs::File;
use std::io::Read;

/// What every sealed chat begins with: its format, and its version.
pub const MAGIC: &[u8] = b"alo sealed chat 1\n";

/// How long a key is, in bytes.
pub const KEY_BYTES: usize = 32;

/// How long a nonce is, in bytes.
pub const NONCE_BYTES: usize = 24;

/// How long the cipher's tag is, in bytes.
pub const TAG_BYTES: usize = 16;

/// Where fresh random bytes are read from.
pub const RANDOM: &str = "/dev/urandom";

/// The key the chats are sealed under.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; KEY_BYTES]);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never its bytes.
        f.write_str("Key(…)")
    }
}

impl Key {
    /// A key of these bytes: what the keychain held, or a test's own.
    pub fn of(bytes: [u8; KEY_BYTES]) -> Self {
        Self(bytes)
    }

    /// A new key, from the system's random source.
    ///
    /// # Errors
    ///
    /// Why no random bytes could be read, in words.
    pub fn fresh() -> Result<Self, String> {
        let mut bytes = [0; KEY_BYTES];
        random(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Its bytes, for the keychain alone.
    pub(crate) fn bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

/// Fill `bytes` from the system's random source.
fn random(bytes: &mut [u8]) -> Result<(), String> {
    File::open(RANDOM)
        .and_then(|mut source| source.read_exact(bytes))
        .map_err(|why| format!("no random bytes could be read from {RANDOM} ({why})"))
}

/// Why a sealed file did not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsealed {
    /// It does not begin as a sealed chat does, or is too short to be one.
    NotSealed,
    /// It did not open under this key: sealed under another, changed since,
    /// or another chat's.
    NotOpened,
}

impl fmt::Display for Unsealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unsealed::NotSealed => "it is not a sealed chat",
            Unsealed::NotOpened => {
                "it does not open with the key in this keychain: it was sealed under another \
                 key, or changed since"
            }
        })
    }
}

/// The associated data for the chat numbered `number`.
fn associated(number: u64) -> Vec<u8> {
    let mut data = MAGIC.to_vec();
    data.extend_from_slice(number.to_string().as_bytes());
    data
}

/// `plain`, the chat numbered `number`, sealed under `key` with a fresh
/// nonce.
///
/// # Errors
///
/// Why it could not be sealed: no random bytes, or the cipher refusing,
/// which only a chat past its largest length could make it.
pub fn seal(key: &Key, number: u64, plain: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0; NONCE_BYTES];
    random(&mut nonce)?;
    seal_with(key, number, plain, nonce)
}

/// `plain` sealed with `nonce`: [`seal`] past its random bytes.
fn seal_with(
    key: &Key,
    number: u64,
    plain: &[u8],
    nonce: [u8; NONCE_BYTES],
) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new(key.0.as_slice().into());
    let aad = associated(number);
    let sealed = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: &aad,
            },
        )
        .map_err(|_| "the chat could not be sealed".to_owned())?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_BYTES + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// The chat numbered `number`, opened from `sealed` under `key`.
///
/// # Errors
///
/// [`Unsealed`]: not a sealed chat, or not opened under this key.
pub fn open(key: &Key, number: u64, sealed: &[u8]) -> Result<Vec<u8>, Unsealed> {
    let rest = sealed.strip_prefix(MAGIC).ok_or(Unsealed::NotSealed)?;
    if rest.len() < NONCE_BYTES + TAG_BYTES {
        return Err(Unsealed::NotSealed);
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_BYTES);
    let cipher = XChaCha20Poly1305::new(key.0.as_slice().into());
    let aad = associated(number);
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| Unsealed::NotOpened)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: &[u8] = b"{\"format\":\"alo chat\",\"version\":1,\"name\":\"Kampala\"}";

    #[test]
    fn a_chat_sealed_opens_under_its_key_and_number_alone() {
        let key = Key::of([7; KEY_BYTES]);
        let sealed = seal(&key, 3, WORDS).expect("sealed");
        assert!(sealed.starts_with(MAGIC));
        assert_eq!(
            sealed.len(),
            MAGIC.len() + NONCE_BYTES + WORDS.len() + TAG_BYTES
        );
        assert!(
            !sealed.windows(7).any(|window| window == b"Kampala"),
            "no word of it in the clear"
        );
        assert_eq!(open(&key, 3, &sealed).as_deref(), Ok(WORDS));
        // Another key, another chat's number: neither opens it.
        assert_eq!(
            open(&Key::of([8; KEY_BYTES]), 3, &sealed),
            Err(Unsealed::NotOpened)
        );
        assert_eq!(open(&key, 4, &sealed), Err(Unsealed::NotOpened));
    }

    #[test]
    fn every_seal_has_a_fresh_nonce() {
        let key = Key::of([7; KEY_BYTES]);
        let first = seal(&key, 1, WORDS).expect("sealed");
        let second = seal(&key, 1, WORDS).expect("sealed");
        assert_ne!(first, second);
        assert_eq!(open(&key, 1, &second).as_deref(), Ok(WORDS));
    }

    #[test]
    fn a_changed_byte_anywhere_does_not_open() {
        let key = Key::of([9; KEY_BYTES]);
        let sealed = seal_with(&key, 2, WORDS, [1; NONCE_BYTES]).expect("sealed");
        for at in MAGIC.len()..sealed.len() {
            let mut changed = sealed.clone();
            changed[at] ^= 1;
            assert_eq!(open(&key, 2, &changed), Err(Unsealed::NotOpened), "{at}");
        }
        let mut magic = sealed.clone();
        magic[0] ^= 1;
        assert_eq!(open(&key, 2, &magic), Err(Unsealed::NotSealed));
        assert_eq!(
            open(&key, 2, &sealed[..MAGIC.len() + 30]),
            Err(Unsealed::NotSealed)
        );
        assert_eq!(open(&key, 2, WORDS), Err(Unsealed::NotSealed));
    }

    #[test]
    fn a_key_is_never_printed_and_a_fresh_one_is_random() {
        let key = Key::of([0xab; KEY_BYTES]);
        assert_eq!(format!("{key:?}"), "Key(…)");
        let a = Key::fresh().expect("fresh");
        let b = Key::fresh().expect("fresh");
        assert_ne!(a, b);
        assert_ne!(a.bytes(), &[0; KEY_BYTES]);
    }
}
