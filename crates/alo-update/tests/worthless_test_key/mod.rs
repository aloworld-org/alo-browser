/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! **The worthless test keys, secret halves and all** (ADR 0116 § 6).
//!
//! Committed on purpose: anybody can sign anything with these, so nothing a
//! person runs may trust them, and nothing does — `alo_update::given`
//! refuses a build given one, and `alo-bundle` refuses to carry one. They
//! exist so that every check of an update can be tested with no real key
//! anywhere near this repository. The owner's own key is made and kept on
//! the owner's machine (ADR 0116, *Answered* 2).
//!
//! Each is an Ed25519 seed and minisign's eight-byte key id, made on 14
//! November 2026 from the operating system's random source. A signature is
//! minisign's prehashed one: Ed25519 over BLAKE2b-512 of the bytes, then
//! over that signature and the trusted comment — made here with
//! `ed25519-dalek` and `blake2`, so that what `minisign-verify`'s own curve
//! code accepts is checked against another implementation's signing.
//!
//! Shared by the crate's unit tests (by path) and its integration tests.

use blake2::Digest as _;
use ed25519_dalek::Signer as _;

/// A worthless key: its seed and its key id.
#[derive(Debug, Clone, Copy)]
pub struct TestKey {
    /// The Ed25519 seed, the secret half.
    pub seed: [u8; 32],
    /// minisign's key id, as it is stored.
    pub id: [u8; 8],
}

/// The worthless key *in use*; its public half is `alo_update::test_key::PUBLIC`.
pub const IN_USE: TestKey = TestKey {
    seed: [
        0x21, 0x2f, 0x12, 0x0d, 0x55, 0xd2, 0x60, 0x68, 0x51, 0x42, 0x08, 0x31, 0x54, 0x1b, 0xf2,
        0x98, 0x3a, 0x01, 0x5c, 0x11, 0x03, 0xf2, 0x7f, 0x19, 0x22, 0x5b, 0xa9, 0x9a, 0xee, 0x71,
        0x06, 0xb6,
    ],
    id: [0x76, 0xd0, 0x33, 0xe0, 0xd6, 0x84, 0x95, 0xcb],
};

/// The worthless *spare*; its public half is `alo_update::test_key::SPARE`.
pub const SPARE: TestKey = TestKey {
    seed: [
        0x01, 0xeb, 0x13, 0xcc, 0xc6, 0xfd, 0xef, 0xf3, 0xa4, 0x23, 0x19, 0xdb, 0xbf, 0x16, 0x92,
        0x4a, 0x72, 0xf9, 0x7a, 0xda, 0xa0, 0x94, 0x76, 0xd0, 0x5b, 0x48, 0x75, 0x8b, 0x37, 0x21,
        0x00, 0x7e,
    ],
    id: [0x8e, 0x42, 0xa8, 0xfb, 0x17, 0x28, 0x0a, 0x03],
};

/// A stranger's worthless key, built into nothing: what a manifest signed
/// by somebody else is signed with.
pub const STRANGER: TestKey = TestKey {
    seed: [
        0x72, 0x3a, 0xd5, 0x03, 0xf3, 0x75, 0xa7, 0x0f, 0xd4, 0x41, 0xe5, 0x0b, 0x3b, 0xcf, 0x75,
        0x5c, 0xbc, 0xf1, 0x62, 0xd1, 0xc4, 0x99, 0x94, 0xdb, 0xae, 0x90, 0x24, 0x70, 0x63, 0xf3,
        0xe2, 0x79,
    ],
    id: [0xa7, 0x84, 0x24, 0xf9, 0xe1, 0x93, 0xca, 0xe8],
};

/// The trusted comment every signature here carries.
pub const TRUSTED: &str = "timestamp:1794646800\tfile:manifest\thashed";

impl TestKey {
    /// Its public half, as minisign writes it.
    pub fn public(self) -> String {
        let signing = ed25519_dalek::SigningKey::from_bytes(&self.seed);
        let mut bytes = b"Ed".to_vec();
        bytes.extend_from_slice(&self.id);
        bytes.extend_from_slice(signing.verifying_key().as_bytes());
        base64(&bytes)
    }

    /// minisign's signature of `bytes`, with [`TRUSTED`] as its trusted
    /// comment.
    pub fn sign(self, bytes: &[u8]) -> String {
        self.sign_saying(bytes, TRUSTED)
    }

    /// The same, with `trusted` as its trusted comment.
    pub fn sign_saying(self, bytes: &[u8], trusted: &str) -> String {
        let signing = ed25519_dalek::SigningKey::from_bytes(&self.seed);
        let hashed = blake2::Blake2b512::digest(bytes);
        let signature = signing.sign(&hashed).to_bytes();
        let mut global = signature.to_vec();
        global.extend_from_slice(trusted.as_bytes());
        let global = signing.sign(&global).to_bytes();
        let mut first = b"ED".to_vec();
        first.extend_from_slice(&self.id);
        first.extend_from_slice(&signature);
        format!(
            "untrusted comment: signature from the worthless test key\n{}\ntrusted comment: {trusted}\n{}\n",
            base64(&first),
            base64(&global)
        )
    }
}

/// Standard base64, padded.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let letter = |index: u32| {
        char::from(
            ALPHABET
                .get(usize::try_from(index & 63).unwrap_or(0))
                .copied()
                .unwrap_or(b'A'),
        )
    };
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|at| u32::from(chunk.get(at).copied().unwrap_or(0)));
        let joined = a << 16 | b << 8 | c;
        out.push(letter(joined >> 18));
        out.push(letter(joined >> 12));
        out.push(if chunk.len() > 1 {
            letter(joined >> 6)
        } else {
            '='
        });
        out.push(if chunk.len() > 2 { letter(joined) } else { '=' });
    }
    out
}
