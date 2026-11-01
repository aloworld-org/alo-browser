/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The keys a release of alo carries inside the application, each in a
//! named slot (ADR 0091, queue item 748).
//!
//! The owner decided on 31 October 2026 that the search and AI providers'
//! keys are shipped inside the application. A key is put in as the
//! application is made, by `alo-bundle` from outside the repository
//! ([`crate::bundle_keys`]), as one file per slot at
//! `Contents/Resources/keys/SLOT`, sealed with the rest of the application
//! by `codesign`. The browser process alone reads it, when the conversation
//! opens ([`crate::search_key::for_this_application`]); no renderer is ever
//! sent it.
//!
//! # A slot
//!
//! Two: [`Slot::Search`], the Brave Search API's key (ADR 0082 § 0, ADR
//! 0083); and [`Slot::Answer`], `OpenAI`'s API key, which writes a search's
//! answer (ADR 0081, decided 1 November 2026; queue item 773). A slot names
//! where its key is taken from, which is its provider's keychain item.
//!
//! # Masked, and not protected
//!
//! A carried key is written XOR a fixed stream of bytes, so it is not a
//! plain `strings` hit in the application. **That is not protection**:
//! the stream is in this file and in every copy of `alo`, and anyone who
//! has the application can read the key out of it (ADR 0091 § 2). It keeps
//! the key from being found by accident, nothing more.

use crate::search_key::{self, Key};
use std::path::{Path, PathBuf};

/// The directory under `Contents/Resources` that holds the carried keys.
pub const DIRECTORY: &str = "keys";

/// What a carried key's file begins with: what it is, and its form's
/// version.
const MAGIC: &[u8] = b"alo carried key 1\n";

/// The longest file read: a key is at most 256 characters.
const LONGEST: usize = 1024;

/// A key's place in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The search provider's key: the Brave Search API's (ADR 0082 § 0).
    Search,
    /// The model provider's key: `OpenAI`'s, which writes a search's answer
    /// (ADR 0081, queue items 773 and 707).
    Answer,
}

impl Slot {
    /// Every slot, in the order they are taken.
    pub const ALL: [Self; 2] = [Self::Search, Self::Answer];

    /// Its file's name under [`DIRECTORY`].
    pub fn name(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Answer => "answer",
        }
    }

    /// The keychain item's service a release's key is taken from as the
    /// application is made, and a development build's read at run time.
    pub fn service(self) -> &'static str {
        match self {
            Self::Search => search_key::SERVICE,
            Self::Answer => crate::answer_key::SERVICE,
        }
    }

    /// What its key is, in words a person reads.
    pub fn what(self) -> &'static str {
        match self {
            Self::Search => "Brave Search key",
            Self::Answer => "OpenAI key",
        }
    }
}

/// Where `slot`'s key is carried, in the application whose
/// `Contents/Resources` is `resources`.
pub fn path(resources: &Path, slot: Slot) -> PathBuf {
    resources.join(DIRECTORY).join(slot.name())
}

/// The bytes `slot`'s file holds for `key`: [`MAGIC`], then the key masked.
pub fn masked(slot: Slot, key: &Key) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend(
        key.header_value()
            .bytes()
            .zip(Stream::for_slot(slot))
            .map(|(byte, mask)| byte ^ mask),
    );
    bytes
}

/// The key `slot`'s file of `bytes` carries, if it is one.
pub fn unmasked(slot: Slot, bytes: &[u8]) -> Option<Key> {
    let body = bytes.strip_prefix(MAGIC)?;
    let plain: Vec<u8> = body
        .iter()
        .zip(Stream::for_slot(slot))
        .map(|(byte, mask)| byte ^ mask)
        .collect();
    let written = String::from_utf8(plain).ok()?;
    // Exactly as written: a key carried is never trimmed into another.
    (written.trim() == written)
        .then(|| Key::carried(&written))
        .flatten()
}

/// Write `key` into `slot`'s file in the application whose
/// `Contents/Resources` is `resources`.
///
/// # Errors
///
/// What the disk said, with which file.
pub fn write(resources: &Path, slot: Slot, key: &Key) -> Result<(), String> {
    let file = path(resources, slot);
    let directory = resources.join(DIRECTORY);
    std::fs::create_dir_all(&directory)
        .map_err(|why| format!("{} could not be made: {why}", directory.display()))?;
    std::fs::write(&file, masked(slot, key))
        .map_err(|why| format!("{} could not be written: {why}", file.display()))
}

/// The key the application whose `Contents/Resources` is `resources`
/// carries in `slot`: [`None`] when it carries none there.
///
/// # Errors
///
/// When a file is there and is not a key carried: never the key, and never
/// what the file held.
pub fn read(resources: &Path, slot: Slot) -> Result<Option<Key>, String> {
    let file = path(resources, slot);
    let damaged = || {
        format!(
            "the {} this application carries could not be read ({}/{})",
            slot.what(),
            DIRECTORY,
            slot.name()
        )
    };
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(damaged()),
    };
    if bytes.len() > LONGEST {
        return Err(damaged());
    }
    unmasked(slot, &bytes).map(Some).ok_or_else(damaged)
}

/// The fixed stream a slot's key is masked with: xorshift64, from a seed
/// of the slot's name.
struct Stream(u64);

impl Stream {
    fn for_slot(slot: Slot) -> Self {
        // FNV-1a of the name, never zero (xorshift's one fixed point).
        let seed = slot
            .name()
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
        Self(seed | 1)
    }
}

impl Iterator for Stream {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        Some(self.0.to_le_bytes()[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOT_A_KEY: &str = "not-a-real-key-carried-0123";

    fn key() -> Key {
        Key::of(NOT_A_KEY).unwrap()
    }

    fn directory(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("alo-carried-keys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_key_carried_reads_back_as_carried_and_is_not_plain_in_its_file() {
        let bytes = masked(Slot::Search, &key());
        assert!(bytes.starts_with(MAGIC));
        assert_eq!(bytes.len(), MAGIC.len() + NOT_A_KEY.len());
        assert!(
            !bytes
                .windows(8)
                .any(|window| NOT_A_KEY.as_bytes().windows(8).any(|part| part == window)),
            "eight characters of the key are plain in its file"
        );
        let read = unmasked(Slot::Search, &bytes).unwrap();
        assert_eq!(read.header_value(), NOT_A_KEY);
        assert_eq!(read.held(), crate::search_key::Held::Carried);
    }

    #[test]
    fn a_file_that_is_not_a_key_carried_is_none() {
        assert_eq!(unmasked(Slot::Search, b""), None);
        assert_eq!(unmasked(Slot::Search, NOT_A_KEY.as_bytes()), None);
        assert_eq!(unmasked(Slot::Search, MAGIC), None);
        let mut bytes = masked(Slot::Search, &key());
        // A space where a character was: not a key a header carries.
        let last = bytes.len() - 1;
        let mask = bytes[last] ^ NOT_A_KEY.as_bytes()[NOT_A_KEY.len() - 1];
        bytes[last] = b' ' ^ mask;
        assert_eq!(unmasked(Slot::Search, &bytes), None);
    }

    #[test]
    fn the_stream_is_fixed_and_mixes_every_bit() {
        let first: Vec<u8> = Stream::for_slot(Slot::Search).take(64).collect();
        let again: Vec<u8> = Stream::for_slot(Slot::Search).take(64).collect();
        assert_eq!(first, again);
        let mut distinct = first.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(distinct.len() > 32, "{first:?}");
    }

    #[test]
    fn a_slot_is_written_and_read_where_the_application_carries_it() {
        let resources = directory("written");
        assert_eq!(read(&resources, Slot::Search), Ok(None), "none carried");
        write(&resources, Slot::Search, &key()).unwrap();
        assert_eq!(
            path(&resources, Slot::Search),
            resources.join("keys").join("search")
        );
        let read_back = read(&resources, Slot::Search).unwrap().unwrap();
        assert_eq!(read_back.header_value(), NOT_A_KEY);

        // Damaged: said, without the key or what the file held.
        std::fs::write(path(&resources, Slot::Search), b"damaged").unwrap();
        let said = read(&resources, Slot::Search).unwrap_err();
        assert_eq!(
            said,
            "the Brave Search key this application carries could not be read (keys/search)"
        );
        std::fs::write(path(&resources, Slot::Search), vec![b'k'; LONGEST + 1]).unwrap();
        assert!(read(&resources, Slot::Search).is_err());
        let _ = std::fs::remove_dir_all(&resources);
    }

    #[test]
    fn each_slot_is_taken_from_its_providers_item() {
        assert_eq!(Slot::ALL, [Slot::Search, Slot::Answer]);
        assert_eq!(Slot::Search.service(), "alo-brave-search");
        assert_eq!(Slot::Search.name(), "search");
        assert_eq!(Slot::Answer.service(), "alo-openai");
        assert_eq!(Slot::Answer.name(), "answer");
        assert_eq!(Slot::Answer.what(), "OpenAI key");
    }

    #[test]
    fn two_slots_are_masked_apart_and_neither_reads_the_others_file() {
        let search = masked(Slot::Search, &key());
        let answer = masked(Slot::Answer, &key());
        assert_ne!(search, answer, "the same key masked alike in two slots");
        assert_eq!(
            unmasked(Slot::Answer, &answer).map(|key| key.header_value().to_owned()),
            Some(NOT_A_KEY.to_owned())
        );
        // Unmasked by the other slot's stream, it is not this key.
        assert_ne!(
            unmasked(Slot::Search, &answer).map(|key| key.header_value().to_owned()),
            Some(NOT_A_KEY.to_owned())
        );
        let resources = directory("two");
        write(&resources, Slot::Answer, &key()).unwrap();
        assert_eq!(
            read(&resources, Slot::Search),
            Ok(None),
            "only answer carried"
        );
        assert_eq!(
            read(&resources, Slot::Answer)
                .unwrap()
                .map(|key| key.header_value().to_owned()),
            Some(NOT_A_KEY.to_owned())
        );
        std::fs::write(path(&resources, Slot::Answer), b"damaged").unwrap();
        assert_eq!(
            read(&resources, Slot::Answer),
            Err("the OpenAI key this application carries could not be read (keys/answer)".into())
        );
        let _ = std::fs::remove_dir_all(&resources);
    }
}
