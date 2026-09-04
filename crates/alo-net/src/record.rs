/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One cache entry, as bytes on a disk, and reading those bytes back from a
//! stranger.
//!
//! ADR 0011 section 4: *"`LOOP.md`'s stage 2 rule — anything that reads bytes
//! from outside gets a malformed, truncated and adversarial input test, and
//! returns an error rather than panicking — applies to a cache file as fully as
//! to a socket."* This file is the whole of that surface. Everything else about
//! the disk cache is a directory and a policy; this is the only place where
//! bytes somebody else may have written turn into a [`Response`] we would hand
//! to a page under that page's own origin.
//!
//! So it is written the way [`crate::http`] and the process boundary's encoding
//! are written: **every length is a number a stranger chose**, checked against
//! what is actually there before anything is reserved, and every arithmetic
//! step that a hostile number could push past the end is `checked_`. That part
//! is [`crate::bytes`], which is shared with the other thing this engine keeps
//! on a person's disk ([`crate::deed`]) — one hostile-input reader rather than
//! two, because the copy that is subtly weaker is the one nobody is looking at.
//!
//! # What the checksum is for, said precisely
//!
//! It catches a file that was **half written** — the power cut in the middle of
//! a rename, a disk that flipped a bit, a file another program left behind with
//! our name on it. Those are the failures that actually happen, and against them
//! it is exact: the entry is discarded and the load becomes a miss.
//!
//! It is **not** a defence against a program running as the person themselves,
//! and no unkeyed checksum is: anything that can write the file can compute the
//! number that goes with it. ADR 0011 section 3 draws that boundary and this
//! file does not move it — *"against another user on the machine, the cache is
//! protected; against a program running as the person, it is not."* A key that
//! would have to live next to the data is not a key.
//!
//! # Why the format is written out rather than derived
//!
//! The same reason the process boundary's is: a serialisation crate reaching
//! into this crate for the benefit of one file is a dependency and a set of
//! derives bought for nothing, and a format a person can read is worth more on
//! the one surface where being wrong is a page written into somebody's bank
//! origin.

use crate::bytes::{Reader, Unreadable, Writer, fingerprint, unreadable};
use crate::freshness::Stored;
use crate::headers::Headers;
use crate::response::{Response, Status};

/// What every entry begins with, so a file that is not one is refused before
/// anything else is read.
pub const MAGIC: [u8; 8] = *b"alocache";

/// The format this engine writes.
///
/// ADR 0011: *"the format carries a version, and a version we do not recognise
/// is discarded wholesale rather than interpreted hopefully."* An older entry
/// is not upgraded and not guessed at — it is a miss, and the response is
/// fetched again.
pub const VERSION: u16 = 1;

/// The bytes before the checksummed part: magic, version, sequence, checksum.
pub const PREFIX: usize = 8 + 2 + 8 + 8;

/// A cache entry as it lives on a disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// What the entry is filed under, in full.
    ///
    /// Kept **inside** the file because the file's name is a hash of it, and a
    /// hash has collisions. A read that finds a different key here is a miss
    /// rather than one URL's response served for another's.
    pub key: String,
    /// Where this entry falls in the order things were stored.
    ///
    /// A counter rather than a timestamp, for [`crate::cache`]'s reason: the
    /// clock is not involved in a decision that has nothing to do with time,
    /// and two entries written in the same millisecond still have an order.
    pub sequence: u64,
    /// The response, and the two moments that decide how old it is.
    pub stored: Stored,
}

/// The checksum an entry carries.
///
/// Over the sequence number **and** the body, rather than the body alone: the
/// sequence decides which entry is evicted first, so a file with a flipped byte
/// in it would otherwise be a valid entry that lies about its place in the
/// order. Nothing in an entry is outside this except the magic and the version,
/// and those two are checked by equality before anything else is believed.
pub fn checksum(sequence: u64, body: &[u8]) -> u64 {
    let mut over = Vec::with_capacity(8usize.saturating_add(body.len()));
    over.extend_from_slice(&sequence.to_be_bytes());
    over.extend_from_slice(body);
    fingerprint(&over)
}

/// A record as bytes.
pub fn encode(record: &Record) -> Vec<u8> {
    let mut body = Writer::default();
    body.text(&record.key);
    body.text(&record.stored.response.url.serialised);
    body.small(record.stored.response.status.0);
    body.time(record.stored.requested_at);
    body.time(record.stored.received_at);
    body.number(record.stored.response.headers.len() as u64);
    for header in record.stored.response.headers.iter() {
        body.text(&header.name);
        body.text(&header.value);
    }
    body.number(record.stored.varied_on.len() as u64);
    for (name, was) in &record.stored.varied_on {
        body.text(name);
        match was {
            Some(value) => {
                body.flag(true);
                body.text(value);
            }
            None => body.flag(false),
        }
    }
    body.bytes(&record.stored.response.body);

    let mut out = Vec::with_capacity(PREFIX.saturating_add(body.out.len()));
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&record.sequence.to_be_bytes());
    out.extend_from_slice(&checksum(record.sequence, &body.out).to_be_bytes());
    out.extend_from_slice(&body.out);
    out
}

/// The sequence number an entry carries, without reading the rest of it.
///
/// What [`crate::disk`] needs at start-up to put the entries it found back in
/// the order they were stored. The magic and the version are checked here too,
/// so a file that is not one of ours is refused before its length is believed.
///
/// # Errors
///
/// [`Unreadable`], for a file that is too short, is not ours, or is a version
/// this engine does not know.
pub fn sequence_of(bytes: &[u8]) -> Result<u64, Unreadable> {
    let magic = bytes
        .get(..8)
        .ok_or_else(|| unreadable("an entry too short to be one"))?;
    if magic != MAGIC {
        return Err(unreadable("a file in the cache that is not a cache entry"));
    }
    let version = bytes
        .get(8..10)
        .ok_or_else(|| unreadable("an entry that stops where its version should be"))?;
    let version = u16::from_be_bytes([
        *version
            .first()
            .ok_or_else(|| unreadable("an entry with no version"))?,
        *version
            .get(1)
            .ok_or_else(|| unreadable("an entry with half a version"))?,
    ]);
    if version != VERSION {
        return Err(unreadable(format!(
            "an entry written in format {version}, and this engine reads {VERSION}"
        )));
    }
    let sequence = bytes
        .get(10..18)
        .ok_or_else(|| unreadable("an entry that stops where its order should be"))?;
    let mut eight = [0u8; 8];
    eight.copy_from_slice(sequence);
    Ok(u64::from_be_bytes(eight))
}

/// A record from bytes, or the reason it is a miss.
///
/// # Errors
///
/// [`Unreadable`], for anything at all: the wrong magic, an unknown version, a
/// checksum that does not match, a length longer than what is there, text that
/// is not UTF-8, a time that cannot exist, or bytes left over at the end.
pub fn decode(bytes: &[u8]) -> Result<Record, Unreadable> {
    let sequence = sequence_of(bytes)?;

    let claimed = bytes
        .get(18..PREFIX)
        .ok_or_else(|| unreadable("an entry that stops where its checksum should be"))?;
    let mut eight = [0u8; 8];
    eight.copy_from_slice(claimed);
    let claimed = u64::from_be_bytes(eight);

    let body = bytes
        .get(PREFIX..)
        .ok_or_else(|| unreadable("an entry with a header and nothing else"))?;
    if checksum(sequence, body) != claimed {
        return Err(unreadable(
            "an entry whose checksum does not match what is in it",
        ));
    }

    let mut reader = Reader::new(body);
    let key = reader.text()?;
    let url = reader.text()?;
    let url =
        alo_url::parse(&url).map_err(|_| unreadable("an entry with a URL that is not one"))?;
    let status = Status(reader.small()?);
    let requested_at = reader.time()?;
    let received_at = reader.time()?;

    let mut headers = Headers::new();
    // A count, refused before the loop when there are not that many bytes left
    // to hold them. The fields inside would each refuse in turn, so this is the
    // difference between an error and four thousand million turns of a loop.
    let how_many = reader.how_many()?;
    for _ in 0..how_many {
        let name = reader.text()?;
        let value = reader.text()?;
        headers.add(name, value);
    }

    let mut varied_on = Vec::new();
    let how_many = reader.how_many()?;
    for _ in 0..how_many {
        let name = reader.text()?;
        let was = if reader.flag()? {
            Some(reader.text()?)
        } else {
            None
        };
        varied_on.push((name, was));
    }

    let payload = reader.bytes()?;
    if !reader.is_done() {
        // A file that decoded and then kept going was written by something
        // else, or by us and half overwritten by something else. Either way it
        // is not an entry this engine wrote.
        return Err(unreadable("an entry with bytes after the end of it"));
    }

    Ok(Record {
        key,
        sequence,
        stored: Stored {
            response: Response {
                url,
                status,
                headers,
                body: payload,
            },
            requested_at,
            received_at,
            varied_on,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn an_entry() -> Record {
        let url = alo_url::parse("https://example.com/a").expect("a URL");
        let mut headers = Headers::new();
        headers.add("Cache-Control", "max-age=3600");
        headers.add("ETag", "\"v1\"");
        Record {
            key: "example.com GET https://example.com/a".to_owned(),
            sequence: 7,
            stored: Stored {
                response: Response {
                    url,
                    status: Status(200),
                    headers,
                    body: b"the stored body".to_vec(),
                },
                requested_at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                received_at: UNIX_EPOCH + Duration::new(1_700_000_002, 500),
                varied_on: vec![
                    ("accept-language".to_owned(), Some("fr".to_owned())),
                    ("accept-encoding".to_owned(), None),
                ],
            },
        }
    }

    #[test]
    fn what_went_in_is_what_comes_out() {
        let entry = an_entry();
        let read = decode(&encode(&entry)).expect("a record this engine just wrote");
        assert_eq!(read, entry, "a round trip changed something");
        assert_eq!(
            sequence_of(&encode(&entry)).expect("a prefix"),
            7,
            "the order is readable without decoding the rest"
        );
    }

    #[test]
    fn an_absent_header_and_an_empty_one_survive_the_disk_as_different_things() {
        // The `Vary` contract turns on exactly this distinction, so a format
        // that flattened them would serve a French page to a German reader
        // after a restart and never before one.
        let mut entry = an_entry();
        entry.stored.varied_on = vec![
            ("accept-language".to_owned(), None),
            ("accept-encoding".to_owned(), Some(String::new())),
        ];
        let read = decode(&encode(&entry)).expect("a record");
        assert_eq!(read.stored.varied_on, entry.stored.varied_on);
    }

    #[test]
    fn every_truncation_of_an_entry_is_refused_rather_than_believed() {
        let whole = encode(&an_entry());
        for cut in 0..whole.len() {
            let short = whole.get(..cut).expect("a prefix of what we wrote");
            assert!(
                decode(short).is_err(),
                "an entry cut to {cut} bytes was read as though it were whole"
            );
        }
        assert!(decode(&whole).is_ok(), "the whole of it still reads");
    }

    #[test]
    fn a_single_flipped_byte_anywhere_is_a_miss() {
        let whole = encode(&an_entry());
        for at in 0..whole.len() {
            let mut damaged = whole.clone();
            if let Some(byte) = damaged.get_mut(at) {
                *byte ^= 0xff;
            }
            assert!(
                decode(&damaged).is_err(),
                "a byte flipped at {at} was read as though nothing had happened"
            );
        }
    }

    #[test]
    fn a_version_this_engine_does_not_know_is_discarded_rather_than_guessed_at() {
        let mut written = encode(&an_entry());
        // Two bytes at offset 8 are the version. A future format's entry must
        // not be interpreted by today's reader.
        if let Some(slot) = written.get_mut(8..10) {
            slot.copy_from_slice(&99u16.to_be_bytes());
        }
        let refused = decode(&written).expect_err("a format from the future");
        assert!(refused.why.contains("format 99"), "{refused}");
    }

    /// The same bytes with the checksum made to agree with them again.
    ///
    /// Without this, every tampering test would be caught by the checksum and
    /// the check it is actually aiming at would never run.
    fn resealed(mut bytes: Vec<u8>) -> Vec<u8> {
        let sequence = sequence_of(&bytes).expect("a prefix");
        let body = bytes[PREFIX..].to_vec();
        bytes[18..PREFIX].copy_from_slice(&checksum(sequence, &body).to_be_bytes());
        bytes
    }

    #[test]
    fn a_length_a_stranger_chose_is_never_believed() {
        // The first field of the body is the key's length. Nothing may be
        // reserved on the strength of it, and both of these are checksummed
        // back into agreement so the length check is what refuses them.
        for claimed in [u64::MAX, 1 << 40, u64::from(u32::MAX)] {
            let mut written = encode(&an_entry());
            written[PREFIX..PREFIX + 8].copy_from_slice(&claimed.to_be_bytes());
            let refused = decode(&resealed(written)).expect_err("a length of {claimed}");
            assert!(
                refused.why.contains("larger than this machine") || refused.why.contains("left"),
                "a length of {claimed} was refused for the wrong reason: {refused}"
            );
        }
    }

    #[test]
    fn bytes_appended_after_an_entry_make_it_a_miss() {
        let mut written = encode(&an_entry());
        written.extend_from_slice(b"and then something else");
        assert!(
            decode(&written).is_err(),
            "a file half overwritten by another program was read as an entry"
        );
    }

    #[test]
    fn a_file_that_is_not_a_cache_entry_at_all_is_refused_before_anything_is_read() {
        assert!(decode(b"").is_err());
        assert!(decode(b"\x89PNG\r\n\x1a\n and then a picture").is_err());
        assert!(decode(&[0xff; 64]).is_err());
        // Our magic, and then nothing that follows it.
        assert!(decode(&MAGIC).is_err());
    }

    #[test]
    fn a_time_that_cannot_exist_is_a_miss_rather_than_a_panic() {
        let entry = an_entry();
        let written = encode(&entry);
        // Find where `requested_at` starts: after the key and the URL, both
        // length-prefixed, and the two-byte status.
        let key = 8 + entry.key.len();
        let url = 8 + entry.stored.response.url.serialised.len();
        let at = PREFIX + key + url + 2;
        let mut nanos_too_big = written.clone();
        nanos_too_big[at + 8..at + 12].copy_from_slice(&2_000_000_000u32.to_be_bytes());
        let refused = decode(&resealed(nanos_too_big)).expect_err("more nanoseconds than a second");
        assert!(refused.why.contains("nanoseconds"), "{refused}");

        // And a moment past the end of what a `SystemTime` can name.
        let mut too_far_ahead = written;
        too_far_ahead[at..at + 8].copy_from_slice(&u64::MAX.to_be_bytes());
        let refused = decode(&resealed(too_far_ahead)).expect_err("a time beyond time");
        assert!(refused.why.contains("further ahead"), "{refused}");
    }
}
