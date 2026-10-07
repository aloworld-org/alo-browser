/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a bucket is as bytes on a disk, and reading those bytes back from a
//! stranger.
//!
//! ADR 0025 § 6 takes ADR 0011 § 4 for every bucket file: **a checksum per
//! record, a format version, every length checked before anything is reserved,
//! and no arithmetic a hostile number can overflow**. `LOOP.md`'s stage 2 rule
//! applies as well: malformed, truncated and adversarial input returns an error
//! and never panics. This file is the whole of that surface for storage. The
//! reading itself is `alo_net::bytes`, the one hostile-input reader this
//! engine has, shared rather than copied.
//!
//! # Three records
//!
//! - **The bucket**: its storage key and its `localStorage` area. The key is
//!   inside because the directory's name is a digest of it, and a digest says
//!   nothing a reader can check without the thing it is a digest of.
//! - **When it was used**: one counter, in a file of its own so that handing an
//!   area to a renderer — which is a use (§ 4) — rewrites a few dozen bytes
//!   rather than up to five mebibytes.
//! - **The site**: the name of the site a directory of buckets belongs to, so
//!   that a bucket set aside at start can still be reported by the site it was,
//!   when its own record is the thing that did not read.
//!
//! # What an unreadable record costs
//!
//! The opposite of the cache's answer, and the ADR says why: *"for the cache,
//! an unreadable entry is a miss. For storage, an unreadable bucket is data a
//! person or a site wanted kept."* Nothing here decides that. [`decode_bucket`]
//! and the others say *unreadable*, and [`crate::directory`] sets the bucket
//! aside whole.
//!
//! # What the checksum is for
//!
//! What `alo-net`'s cache record says of its own: it catches a file that was
//! half written, or that had a byte flipped, or that another program left with
//! our name on it. It is not a defence against a program running as the person,
//! which can compute the number as easily as we can. ADR 0011 § 3 draws that
//! boundary and ADR 0025 § 8 keeps it.

use crate::area::Area;
use crate::key::StorageKey;
use alo_net::bytes::{Reader, Unreadable, Writer, fingerprint, unreadable};

/// What a bucket's record begins with.
pub const BUCKET_MAGIC: [u8; 8] = *b"alostore";

/// What a bucket's use counter begins with.
pub const USED_MAGIC: [u8; 8] = *b"alo-used";

/// What a site's name begins with.
pub const SITE_MAGIC: [u8; 8] = *b"alo-site";

/// The format this engine writes, for all three.
///
/// A version this engine does not know is unreadable: not upgraded, not
/// guessed at. For storage that means set aside, which keeps the bytes for a
/// later engine that can read them rather than throwing them away.
pub const VERSION: u16 = 1;

/// The bytes before the checksummed body: magic, version, checksum.
pub const PREFIX: usize = 8 + 2 + 8;

/// A bucket as it was read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    /// Whose it is.
    pub key: StorageKey,
    /// Its `localStorage`.
    pub local: Area,
}

/// A bucket's record.
pub fn encode_bucket(key: &StorageKey, local: &Area) -> Vec<u8> {
    let mut body = Writer::default();
    body.text(key.origin());
    body.text(key.site());
    body.text(key.partition());
    body.number(local.len() as u64);
    for (name, value) in local.pairs() {
        body.text(name);
        body.text(value);
    }
    sealed(BUCKET_MAGIC, &body.out)
}

/// A bucket from its record, or why it is unreadable.
///
/// # Errors
///
/// [`Unreadable`], for anything at all: the wrong magic, an unknown version, a
/// checksum that does not match, a length longer than what is there, text that
/// is not UTF-8, a key no document could have, the same name stored twice, or
/// bytes after the end.
pub fn decode_bucket(bytes: &[u8]) -> Result<Bucket, Unreadable> {
    let mut reader = Reader::new(unsealed(BUCKET_MAGIC, bytes)?);
    let origin = reader.text()?;
    let site = reader.text()?;
    let partition = reader.text()?;
    let key = StorageKey::from_parts(origin, site, partition)
        .ok_or_else(|| unreadable("a bucket under a key no document could have"))?;
    let mut local = Area::new();
    // A count a stranger wrote, refused before the loop when there are not
    // that many bytes left to hold them.
    let how_many = reader.how_many()?;
    for _ in 0..how_many {
        let name = reader.text()?;
        let value = reader.text()?;
        if local.get(&name).is_some() {
            return Err(unreadable("a bucket that stores one name twice"));
        }
        local.set(name, value);
    }
    if !reader.is_done() {
        return Err(unreadable("a bucket with bytes after the end of it"));
    }
    Ok(Bucket { key, local })
}

/// A use counter's record.
pub fn encode_used(counter: u64) -> Vec<u8> {
    let mut body = Writer::default();
    body.number(counter);
    sealed(USED_MAGIC, &body.out)
}

/// A use counter from its record.
///
/// # Errors
///
/// [`Unreadable`], as for [`decode_bucket`].
pub fn decode_used(bytes: &[u8]) -> Result<u64, Unreadable> {
    let mut reader = Reader::new(unsealed(USED_MAGIC, bytes)?);
    let counter = reader.number()?;
    if !reader.is_done() {
        return Err(unreadable("a use counter with bytes after the end of it"));
    }
    Ok(counter)
}

/// A site's name, as a record.
pub fn encode_site(site: &str) -> Vec<u8> {
    let mut body = Writer::default();
    body.text(site);
    sealed(SITE_MAGIC, &body.out)
}

/// A site's name from its record.
///
/// # Errors
///
/// [`Unreadable`], as for [`decode_bucket`].
pub fn decode_site(bytes: &[u8]) -> Result<String, Unreadable> {
    let mut reader = Reader::new(unsealed(SITE_MAGIC, bytes)?);
    let site = reader.text()?;
    if !reader.is_done() || site.is_empty() {
        return Err(unreadable("a site's name that is not one"));
    }
    Ok(site)
}

/// The checksum a record carries over its body.
fn checksum(body: &[u8]) -> u64 {
    fingerprint(body)
}

/// A body behind its magic, version and checksum.
fn sealed(magic: [u8; 8], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(PREFIX.saturating_add(body.len()));
    out.extend_from_slice(&magic);
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&checksum(body).to_be_bytes());
    out.extend_from_slice(body);
    out
}

/// The body of a record, once its magic, version and checksum are believed.
fn unsealed(magic: [u8; 8], bytes: &[u8]) -> Result<&[u8], Unreadable> {
    let mut reader = Reader::new(bytes);
    let mut found = [0u8; 8];
    for slot in &mut found {
        *slot = reader.tag()?;
    }
    if found != magic {
        return Err(unreadable(
            "a file in storage that is not the record it is named as",
        ));
    }
    let version = reader.small()?;
    if version != VERSION {
        return Err(unreadable(format!(
            "a record written in format {version}, and this engine reads {VERSION}"
        )));
    }
    let claimed = reader.number()?;
    let body = bytes
        .get(PREFIX..)
        .ok_or_else(|| unreadable("a record with a header and nothing else"))?;
    if checksum(body) != claimed {
        return Err(unreadable(
            "a record whose checksum does not match what is in it",
        ));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::Partition;
    use alo_url::Origin;

    fn a_key() -> StorageKey {
        let document = alo_url::parse("https://mail.example.com/").expect("a URL");
        StorageKey::of(&Origin::of(&document), &Partition::of(&document)).expect("a key")
    }

    fn an_area() -> Area {
        Area::new()
            .with("alo.locale", "fr")
            .with("alo.panel-width", "320")
            .with("", "an empty name is a name")
    }

    fn every_record() -> Vec<Vec<u8>> {
        vec![
            encode_bucket(&a_key(), &an_area()),
            encode_used(41),
            encode_site("example.com"),
        ]
    }

    /// Each record decoded by its own reader, as `Ok(())` or why not.
    fn decoded(bytes: &[u8], which: usize) -> Result<(), Unreadable> {
        match which {
            0 => decode_bucket(bytes).map(|_| ()),
            1 => decode_used(bytes).map(|_| ()),
            _ => decode_site(bytes).map(|_| ()),
        }
    }

    #[test]
    fn what_went_in_is_what_comes_out() {
        let read = decode_bucket(&encode_bucket(&a_key(), &an_area())).expect("a bucket");
        assert_eq!(read.key, a_key());
        assert_eq!(read.local, an_area());
        assert_eq!(read.local.counted(), an_area().counted());
        assert_eq!(decode_used(&encode_used(41)), Ok(41));
        assert_eq!(
            decode_site(&encode_site("example.com")).as_deref(),
            Ok("example.com")
        );
    }

    #[test]
    fn every_truncation_of_every_record_is_refused() {
        for (which, whole) in every_record().iter().enumerate() {
            for cut in 0..whole.len() {
                let short = whole.get(..cut).expect("a prefix");
                assert!(
                    decoded(short, which).is_err(),
                    "record {which} cut to {cut} bytes was read as whole"
                );
            }
            assert!(decoded(whole, which).is_ok());
        }
    }

    #[test]
    fn a_single_flipped_byte_anywhere_in_any_record_is_refused() {
        for (which, whole) in every_record().iter().enumerate() {
            for at in 0..whole.len() {
                let mut damaged = whole.clone();
                if let Some(byte) = damaged.get_mut(at) {
                    *byte ^= 0xff;
                }
                assert!(
                    decoded(&damaged, which).is_err(),
                    "record {which} with byte {at} flipped was read as undamaged"
                );
            }
        }
    }

    #[test]
    fn one_kind_of_record_is_never_read_as_another() {
        let records = every_record();
        for (written, bytes) in records.iter().enumerate() {
            for read_as in 0..records.len() {
                if read_as != written {
                    assert!(
                        decoded(bytes, read_as).is_err(),
                        "{written} read as {read_as}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_version_this_engine_does_not_know_is_refused_rather_than_guessed_at() {
        let mut written = encode_used(1);
        written[8..10].copy_from_slice(&2u16.to_be_bytes());
        let refused = decode_used(&written).expect_err("a future format");
        assert!(refused.why.contains("format 2"), "{refused}");
    }

    /// The same bytes with the checksum made to agree with them again, so that
    /// the check a test aims at is the one that refuses.
    fn resealed(mut bytes: Vec<u8>) -> Vec<u8> {
        let sum = checksum(&bytes[PREFIX..]);
        bytes[10..PREFIX].copy_from_slice(&sum.to_be_bytes());
        bytes
    }

    #[test]
    fn a_length_or_a_count_a_stranger_chose_is_never_believed() {
        // The origin's length, then — after the three key texts — the count of
        // pairs, each set to numbers nothing backs and resealed.
        let key = a_key();
        let count_at =
            PREFIX + 8 + key.origin().len() + 8 + key.site().len() + 8 + key.partition().len();
        for claimed in [u64::MAX, 1 << 40, u64::from(u32::MAX)] {
            for at in [PREFIX, count_at] {
                let mut written = encode_bucket(&key, &an_area());
                written[at..at + 8].copy_from_slice(&claimed.to_be_bytes());
                assert!(
                    decode_bucket(&resealed(written)).is_err(),
                    "{claimed} at {at} was believed"
                );
            }
        }
    }

    #[test]
    fn a_name_stored_twice_is_a_bucket_this_engine_never_wrote() {
        let mut body = Writer::default();
        let key = a_key();
        body.text(key.origin());
        body.text(key.site());
        body.text(key.partition());
        body.number(2);
        for _ in 0..2 {
            body.text("alo.locale");
            body.text("fr");
        }
        let refused = decode_bucket(&sealed(BUCKET_MAGIC, &body.out)).expect_err("a duplicate");
        assert!(refused.why.contains("twice"), "{refused}");
    }

    #[test]
    fn a_key_for_an_opaque_origin_is_refused_even_when_well_formed() {
        let mut body = Writer::default();
        body.text("null");
        body.text("example.com");
        body.text("example.com");
        body.number(0);
        assert!(decode_bucket(&sealed(BUCKET_MAGIC, &body.out)).is_err());
    }

    #[test]
    fn bytes_appended_after_a_record_make_it_unreadable() {
        for (which, whole) in every_record().into_iter().enumerate() {
            let mut longer = whole;
            longer.extend_from_slice(b"and then something else");
            assert!(decoded(&resealed(longer), which).is_err());
        }
    }

    #[test]
    fn a_file_that_is_not_a_record_at_all_is_refused() {
        for which in 0..3 {
            assert!(decoded(b"", which).is_err());
            assert!(decoded(b"\x89PNG\r\n\x1a\n and then a picture", which).is_err());
            assert!(decoded(&[0xff; 64], which).is_err());
        }
    }
}
