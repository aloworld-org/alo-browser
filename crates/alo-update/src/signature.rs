/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An update's own signature, checked: alo's, independent of Apple's (ADR
//! 0116 § 2).
//!
//! # Rented, behind this file
//!
//! The format is minisign's, prehashed — BLAKE2b-512 of the file, then
//! Ed25519 over it, and a second Ed25519 over that signature and its
//! *trusted comment* — and it is verified by `minisign-verify` 0.3.0
//! (MIT, no dependencies, no `unsafe` in its source), named in this file
//! and nowhere else. alo writes no signing code: the owner signs with an
//! off-the-shelf tool (`rsign2`, or `minisign` itself), and the secret key
//! is never in this repository.
//!
//! # Two keys built in, either accepted
//!
//! The one in use and a spare, made at the same time and kept offline
//! ([`Keys`]). A signature by either is accepted, and which one said
//! ([`Signer`]). The manifest never names a key: a release changes the
//! keys only by shipping a new application.
//!
//! # What is refused
//!
//! A signature file longer than [`LARGEST`] or not text; one that does not
//! decode; one made by neither key (minisign's key id says whose it claims
//! to be, and the curve says whether it is); one over other bytes; one
//! whose trusted comment was changed; and a *legacy* one, Ed25519 over the
//! file itself rather than its hash, which minisign has not written by
//! default since 0.8 and which nothing here needs.

use core::fmt;

/// The most bytes a signature file is read to: a minisign signature is
/// four lines, about 300 bytes.
pub const LARGEST: usize = 1024;

/// One public key, as minisign writes it: the base64 line of a
/// `minisign.pub`.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    /// The key, decoded.
    key: minisign_verify::PublicKey,
    /// Its base64, as it was given.
    text: String,
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key({})", self.text)
    }
}

impl Key {
    /// The key `text` is: the base64 line alone, or a whole `minisign.pub`
    /// (its untrusted comment, then that line).
    ///
    /// # Errors
    ///
    /// Why not, in words.
    pub fn read(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let line = match text.split_once('\n') {
            Some((comment, line)) if comment.starts_with("untrusted comment:") => line.trim(),
            Some(_) => return Err("a public key is one line, or a minisign.pub".to_owned()),
            None => text,
        };
        let key = minisign_verify::PublicKey::from_base64(line)
            .map_err(|why| format!("{line:?} is not a minisign public key ({why})"))?;
        Ok(Self {
            key,
            text: line.to_owned(),
        })
    }

    /// Its base64, as minisign writes it.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// The two public keys a build carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    /// The one releases are signed with.
    pub in_use: Key,
    /// The one kept offline, for when the first is lost or leaked.
    pub spare: Key,
}

/// Which of the two keys signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signer {
    /// The key in use.
    InUse,
    /// The spare.
    Spare,
}

/// Why a signature was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsigned {
    /// The signature file is longer than [`LARGEST`], or is not text.
    Unreadable,
    /// It does not decode as a minisign signature.
    NotASignature,
    /// It is a legacy one, over the file rather than its hash.
    Legacy,
    /// Neither key made it, or it is over other bytes, or its trusted
    /// comment was changed.
    NotByAlosKeys,
}

impl fmt::Display for Unsigned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unsigned::Unreadable => "its signature is not one alo reads",
            Unsigned::NotASignature => "its signature is not a minisign signature",
            Unsigned::Legacy => "its signature is minisign's legacy kind, which alo does not take",
            Unsigned::NotByAlosKeys => "it is not signed by alo\u{2019}s key",
        })
    }
}

/// Whether one of `keys` signed `bytes` with `signature`, and which.
///
/// # Errors
///
/// [`Unsigned`], saying why not.
pub fn verify(bytes: &[u8], signature: &[u8], keys: &Keys) -> Result<Signer, Unsigned> {
    if signature.len() > LARGEST {
        return Err(Unsigned::Unreadable);
    }
    let text = core::str::from_utf8(signature).map_err(|_| Unsigned::Unreadable)?;
    let decoded = minisign_verify::Signature::decode(text).map_err(|_| Unsigned::NotASignature)?;
    let mut legacy = false;
    for (key, signer) in [(&keys.in_use, Signer::InUse), (&keys.spare, Signer::Spare)] {
        match key.key.verify(bytes, &decoded, false) {
            Ok(()) => return Ok(signer),
            Err(minisign_verify::Error::UnexpectedAlgorithm) => legacy = true,
            Err(_) => {}
        }
    }
    Err(if legacy {
        Unsigned::Legacy
    } else {
        Unsigned::NotByAlosKeys
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_key;

    fn keys() -> Keys {
        Keys {
            in_use: Key::read(test_key::PUBLIC).expect("the test key"),
            spare: Key::read(test_key::SPARE).expect("the spare test key"),
        }
    }

    /// The manifest the fixed vectors below sign: ADR 0116's own example.
    const VECTOR_MANIFEST: &str = "alo-update 1\nplatform macos-arm64\nversion 0.4.2\nserial 118\n\
published 2026-11-14T09:00:00Z\nexpires 2026-11-28T09:00:00Z\nminimum 0.3.0\n\
chromium 156.0.8078.96\npackage https://updates.example.org/alo-browser-0.4.2-macos-arm64.zip\n\
size 158204416\nsha256 3f6c00112233445566778899aabbccddeeff00112233445566778899aabbccdd\n\
security yes\nbehind no\n";

    /// Fixed vectors: minisign signatures of [`VECTOR_MANIFEST`], made on
    /// 14 November 2026 by a third implementation — Python's
    /// `cryptography` 46.0.5 and `hashlib`'s `BLAKE2b`, following minisign's
    /// format — with the worthless test keys. Ed25519 is deterministic, so
    /// the in-use key's must also be exactly what `ed25519-dalek` makes.
    /// If a new `minisign-verify` answers any of these otherwise, this
    /// fails.
    const GOOD: &str = "untrusted comment: signature from the worthless test key\nRUR20DPg1oSVy5yyvMVi68pwzYHgYalz19/cyEHEKp2+d2/+eaSlPmAdFyV02qAjAIAnetOvmwFXQAELk0dlIJht7RAt5/ca9AI=\ntrusted comment: timestamp:1794646800\tfile:manifest\thashed\nUvP2CFU9uEFta58Qe4C8VyeBmI8eE7DD9sDbVRSJ34gahhRirssZB9JHljJoe/XAazuROpXjRtEkYwts2dKBBA==\n";
    /// The spare's.
    const BY_SPARE: &str = "untrusted comment: signature from the worthless test key\nRUSOQqj7FygKA0aXvKGqJoSLirnU7xzuf2B0X0JC0hrm9ygcMnedT8GzhEhEJRNCTvvh5I86+kab0TBECLTczg+sz80gtrgFbwg=\ntrusted comment: timestamp:1794646800\tfile:manifest\thashed\nJAF7XyrAu+MEY/HufeL0d7gjrkwFL04NoKYgMplu79T+gSbKCVaE3QPQvh+MhSwmtFjdpqDEnE/kw9FD3Z7aDg==\n";
    /// The in-use key's, over the file itself: minisign's legacy kind.
    const LEGACY: &str = "untrusted comment: signature from the worthless test key\nRWR20DPg1oSVy73aVG89/koc+gU5rkGN4734UlR2pTNoODEAOgV8q7271BR4DIbvBeWb63OPIoR79TQnq+Eq+BSSelOTzXaPzgw=\ntrusted comment: timestamp:1794646800\tfile:manifest\n82IU9/48ZaE6SZIEcNgk49/zonIcLpZw0E3eVajRtpyugaL8MShg34MjM3YejXET45PbCQ6iVaZvL767HKsYDg==\n";
    /// A stranger's key's, under its own key id.
    const BY_STRANGER: &str = "untrusted comment: signature from the worthless test key\nRUSnhCT54ZPK6Lliu4wFvn7veyLC0CxOQ3HHbmy9GAvE2Q10GUXIdJit6sDv3CfSuoMsPJ2GWrRFQfTjeH2Hk64zJfvfKWumFAc=\ntrusted comment: timestamp:1794646800\tfile:manifest\thashed\nOKODOu/N+reP63V215gSc5R3IwnYJqKPtkqjGGp3/KA/39W1k8MO9J3ZvfnOnJ9O5SREOYKLeZlIdEnqDpauDw==\n";
    /// A stranger's key's, claiming the in-use key's id.
    const STRANGER_AS_IN_USE: &str = "untrusted comment: signature from the worthless test key\nRUR20DPg1oSVy7liu4wFvn7veyLC0CxOQ3HHbmy9GAvE2Q10GUXIdJit6sDv3CfSuoMsPJ2GWrRFQfTjeH2Hk64zJfvfKWumFAc=\ntrusted comment: timestamp:1794646800\tfile:manifest\thashed\nOKODOu/N+reP63V215gSc5R3IwnYJqKPtkqjGGp3/KA/39W1k8MO9J3ZvfnOnJ9O5SREOYKLeZlIdEnqDpauDw==\n";
    /// The in-use key's, claiming a stranger's key id.
    const WRONG_ID: &str = "untrusted comment: signature from the worthless test key\nRUSnhCT54ZPK6JyyvMVi68pwzYHgYalz19/cyEHEKp2+d2/+eaSlPmAdFyV02qAjAIAnetOvmwFXQAELk0dlIJht7RAt5/ca9AI=\ntrusted comment: timestamp:1794646800\tfile:manifest\thashed\nUvP2CFU9uEFta58Qe4C8VyeBmI8eE7DD9sDbVRSJ34gahhRirssZB9JHljJoe/XAazuROpXjRtEkYwts2dKBBA==\n";

    #[test]
    fn the_fixed_vectors_are_answered_as_they_were_made() {
        let keys = keys();
        let manifest = VECTOR_MANIFEST.as_bytes();
        assert_eq!(verify(manifest, GOOD.as_bytes(), &keys), Ok(Signer::InUse));
        assert_eq!(
            verify(manifest, BY_SPARE.as_bytes(), &keys),
            Ok(Signer::Spare)
        );
        assert_eq!(
            verify(manifest, LEGACY.as_bytes(), &keys),
            Err(Unsigned::Legacy)
        );
        for refused in [BY_STRANGER, STRANGER_AS_IN_USE, WRONG_ID] {
            assert_eq!(
                verify(manifest, refused.as_bytes(), &keys),
                Err(Unsigned::NotByAlosKeys),
                "{refused}"
            );
        }
        // One byte of the manifest changed.
        let changed = VECTOR_MANIFEST.replacen("serial 118", "serial 119", 1);
        assert_eq!(
            verify(changed.as_bytes(), GOOD.as_bytes(), &keys),
            Err(Unsigned::NotByAlosKeys)
        );
        // Its trusted comment changed: the second signature fails.
        let comment = GOOD.replace("file:manifest\thashed", "file:manifest\thashed\tlater");
        assert_eq!(
            verify(manifest, comment.as_bytes(), &keys),
            Err(Unsigned::NotByAlosKeys)
        );
        // One character of the signature itself changed.
        let flipped = GOOD.replacen("RUR20DPg1oSVy5yy", "RUR20DPg1oSVy5yz", 1);
        assert_eq!(
            verify(manifest, flipped.as_bytes(), &keys),
            Err(Unsigned::NotByAlosKeys)
        );
        // The same signatures, made by `ed25519-dalek` with the same keys.
        assert_eq!(crate::worthless_test_key::IN_USE.sign(manifest), GOOD);
        assert_eq!(crate::worthless_test_key::SPARE.sign(manifest), BY_SPARE);
    }

    #[test]
    fn a_key_reads_as_its_line_or_its_whole_file_and_nothing_else() {
        let line = Key::read(test_key::PUBLIC).expect("read");
        let file = Key::read(&format!(
            "untrusted comment: minisign public key 76D033E0D68495CB\n{}\n",
            test_key::PUBLIC
        ))
        .expect("read");
        assert_eq!(line, file);
        assert_eq!(line.as_str(), test_key::PUBLIC);
        for odd in ["", "RWR", "hello\nworld", &test_key::PUBLIC[1..]] {
            assert!(Key::read(odd).is_err(), "{odd:?}");
        }
    }

    #[test]
    fn a_signature_too_long_or_not_text_is_not_read() {
        let keys = keys();
        assert_eq!(
            verify(b"x", &vec![b'a'; LARGEST + 1], &keys),
            Err(Unsigned::Unreadable)
        );
        assert_eq!(verify(b"x", b"\xff\xfe", &keys), Err(Unsigned::Unreadable));
        assert_eq!(
            verify(b"x", b"untrusted comment: x\nnot base64\n", &keys),
            Err(Unsigned::NotASignature)
        );
    }
}
