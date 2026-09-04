/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Numbers, text and moments on a disk — and reading them back from a stranger.
//!
//! Two files in this crate keep something of their own on a person's disk: a
//! cache entry ([`crate::record`], ADR 0011) and what an agent did
//! ([`crate::deed`], ADR 0012 § 6). They hold entirely different things and they
//! read their bytes the same way, because the way is not about what is in them.
//!
//! # Why this is one file rather than two copies
//!
//! `LOOP.md`'s stage 2 rule — *anything that reads bytes from outside gets a
//! malformed, truncated and adversarial input test, and returns an error rather
//! than panicking* — applies to a file on a disk as fully as to a socket. A
//! second copy of a hostile-input reader is a second place for a length check to
//! be subtly weaker, and the copy that is weaker is the one nobody is looking
//! at. It is the same argument the queue made for having **one** value layer
//! rather than a colour parser grown inside paint.
//!
//! **[`Reader::length`] is the line both formats are built around.** A length is
//! a number somebody else chose, and reserving before checking is how eight
//! bytes in a file become a gigabyte of allocation.
//!
//! # What this file does not do
//!
//! It does not decide what a file *means*: no magic, no version, no checksum
//! policy. Those belong to each format, because *what an unknown version costs*
//! is a different answer for a cache entry (fetch it again) and for a record of
//! what an agent did (a gap somebody should be told about).

use core::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Why something read off a disk could not be read.
///
/// Never reaches a page. Each format turns one of these into whatever it can
/// honestly do without: [`crate::disk`] makes it a cache miss, and
/// [`crate::kept`] makes it a gap in a record it says the size of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// In words, for somebody looking at why something is not being read.
    pub why: String,
}

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.why)
    }
}

impl std::error::Error for Unreadable {}

/// One of the above, from anything that can be said.
pub fn unreadable(why: impl Into<String>) -> Unreadable {
    Unreadable { why: why.into() }
}

/// FNV-1a, 64 bits.
///
/// Written out rather than rented because it is nine lines and because what it
/// has to do is narrow: catch a truncated or corrupted file, and give a URL a
/// name a filesystem will accept. It is **not** a defence against a program
/// running as the person themselves, and no unkeyed checksum is — anything that
/// can write the file can compute the number that goes with it. ADR 0011
/// section 3 draws that boundary and nothing here moves it.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    const START: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = START;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// --- Writing -----------------------------------------------------------------

/// Bytes being built.
#[derive(Debug, Default)]
pub(crate) struct Writer {
    /// What has been written so far.
    pub(crate) out: Vec<u8>,
}

impl Writer {
    pub(crate) fn flag(&mut self, yes: bool) {
        self.out.push(u8::from(yes));
    }
    pub(crate) fn tag(&mut self, which: u8) {
        self.out.push(which);
    }
    pub(crate) fn small(&mut self, value: u16) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }
    pub(crate) fn number(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.number(value.len() as u64);
        self.out.extend_from_slice(value);
    }
    pub(crate) fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    /// A moment, as seconds and nanoseconds since the epoch.
    ///
    /// A moment before 1970 is written as the epoch itself. That is a machine
    /// whose clock is set to before the epoch, and clamping makes what is
    /// written read as **older** than it is — which is the safe direction for
    /// both a cache entry and a record of when something happened.
    pub(crate) fn time(&mut self, at: SystemTime) {
        let since = at.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        self.number(since.as_secs());
        self.out
            .extend_from_slice(&since.subsec_nanos().to_be_bytes());
    }
}

// --- Reading, from a stranger --------------------------------------------------

/// Bytes being read, none of which are believed until they are checked.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Whether every byte has been read.
    ///
    /// Each format checks this at the end: bytes after what decoded were
    /// written by something else, or by us and half overwritten.
    pub(crate) fn is_done(&self) -> bool {
        self.at == self.bytes.len()
    }

    /// How many bytes are left, for a count that must not be believed either.
    ///
    /// Nothing on a disk has an item smaller than one byte, so a count larger
    /// than this cannot be honest — and refusing it here is what stops a loop
    /// running four thousand million times before the first missing field.
    pub(crate) fn left(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    /// A count of items, refused when there could not be that many.
    pub(crate) fn how_many(&mut self) -> Result<u64, Unreadable> {
        let said = self.number()?;
        if said > self.left() as u64 {
            return Err(unreadable(format!(
                "a record claiming {said} of something with {} bytes left",
                self.left()
            )));
        }
        Ok(said)
    }

    /// The next `how_many` bytes, or an error — never a panic and never a wrap.
    fn take(&mut self, how_many: usize) -> Result<&'a [u8], Unreadable> {
        let end = self
            .at
            .checked_add(how_many)
            .ok_or_else(|| unreadable("a length larger than this machine"))?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| unreadable("bytes that stop before they say they do"))?;
        self.at = end;
        Ok(slice)
    }

    pub(crate) fn flag(&mut self) -> Result<bool, Unreadable> {
        Ok(self.tag()? != 0)
    }

    pub(crate) fn tag(&mut self) -> Result<u8, Unreadable> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| unreadable("bytes that stop where a tag should be"))
    }

    pub(crate) fn small(&mut self) -> Result<u16, Unreadable> {
        let mut two = [0u8; 2];
        two.copy_from_slice(self.take(2)?);
        Ok(u16::from_be_bytes(two))
    }

    pub(crate) fn number(&mut self) -> Result<u64, Unreadable> {
        let mut eight = [0u8; 8];
        eight.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(eight))
    }

    /// A length, checked against what is actually left.
    ///
    /// The line this file exists for. Nothing is reserved on the strength of a
    /// number a stranger wrote.
    fn length(&mut self) -> Result<usize, Unreadable> {
        let said = self.number()?;
        let said =
            usize::try_from(said).map_err(|_| unreadable("a length larger than this machine"))?;
        let left = self.left();
        if said > left {
            return Err(unreadable(format!(
                "a length claiming {said} bytes with {left} left"
            )));
        }
        Ok(said)
    }

    pub(crate) fn bytes(&mut self) -> Result<Vec<u8>, Unreadable> {
        let how_many = self.length()?;
        Ok(self.take(how_many)?.to_vec())
    }

    pub(crate) fn text(&mut self) -> Result<String, Unreadable> {
        let taken = self.bytes()?;
        String::from_utf8(taken).map_err(|_| unreadable("text that is not UTF-8"))
    }

    /// A moment. A nanosecond field that is not a nanosecond is corrupt rather
    /// than something to fold into the seconds — and a moment further ahead than
    /// this machine's clock can name is refused rather than saturated.
    pub(crate) fn time(&mut self) -> Result<SystemTime, Unreadable> {
        let seconds = self.number()?;
        let mut four = [0u8; 4];
        four.copy_from_slice(self.take(4)?);
        let nanos = u32::from_be_bytes(four);
        if nanos >= 1_000_000_000 {
            return Err(unreadable("more nanoseconds than there are in a second"));
        }
        UNIX_EPOCH
            .checked_add(Duration::new(seconds, nanos))
            .ok_or_else(|| unreadable("a moment further ahead than time goes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_went_in_is_what_comes_out() {
        let mut writing = Writer::default();
        writing.tag(3);
        writing.flag(true);
        writing.small(206);
        writing.number(u64::MAX);
        writing.text("a page a person read");
        writing.bytes(&[0, 1, 2, 255]);
        writing.time(UNIX_EPOCH + Duration::new(1_700_000_000, 500));

        let mut reading = Reader::new(&writing.out);
        assert_eq!(reading.tag(), Ok(3));
        assert_eq!(reading.flag(), Ok(true));
        assert_eq!(reading.small(), Ok(206));
        assert_eq!(reading.number(), Ok(u64::MAX));
        assert_eq!(reading.text().as_deref(), Ok("a page a person read"));
        assert_eq!(reading.bytes(), Ok(vec![0, 1, 2, 255]));
        assert_eq!(
            reading.time(),
            Ok(UNIX_EPOCH + Duration::new(1_700_000_000, 500))
        );
        assert!(reading.is_done(), "something was left over");
    }

    /// The line the whole file is built around, asserted directly rather than
    /// only through the two formats that depend on it.
    #[test]
    fn a_length_a_stranger_chose_is_never_believed() {
        for claimed in [u64::MAX, 1 << 40, u64::from(u32::MAX), 17] {
            let mut written = Vec::new();
            written.extend_from_slice(&claimed.to_be_bytes());
            written.extend_from_slice(b"four");
            let refused = Reader::new(&written)
                .bytes()
                .expect_err("a length nothing backs");
            assert!(
                refused.why.contains("larger than this machine") || refused.why.contains("left"),
                "a length of {claimed} was refused for the wrong reason: {refused}"
            );
        }
    }

    /// A count is a number a stranger chose too, and the cost of believing one
    /// is a loop rather than an allocation.
    #[test]
    fn a_count_larger_than_the_bytes_left_is_refused_before_the_loop() {
        let mut written = Vec::new();
        written.extend_from_slice(&u64::MAX.to_be_bytes());
        written.extend_from_slice(b"one item, at most");
        let refused = Reader::new(&written)
            .how_many()
            .expect_err("a count nothing backs");
        assert!(refused.why.contains("bytes left"), "{refused}");

        let mut honest = Vec::new();
        honest.extend_from_slice(&2u64.to_be_bytes());
        honest.extend_from_slice(b"two of something");
        assert_eq!(Reader::new(&honest).how_many(), Ok(2));
    }

    #[test]
    fn text_that_is_not_text_is_an_error_rather_than_a_replacement_character() {
        let mut writing = Writer::default();
        writing.bytes(&[0xff, 0xfe, 0xfd]);
        let refused = Reader::new(&writing.out)
            .text()
            .expect_err("bytes that are not UTF-8");
        assert!(refused.why.contains("UTF-8"), "{refused}");
    }

    #[test]
    fn a_moment_that_cannot_exist_is_refused_rather_than_saturated() {
        let mut too_many_nanos = Vec::new();
        too_many_nanos.extend_from_slice(&1u64.to_be_bytes());
        too_many_nanos.extend_from_slice(&2_000_000_000u32.to_be_bytes());
        let refused = Reader::new(&too_many_nanos)
            .time()
            .expect_err("more nanoseconds than a second");
        assert!(refused.why.contains("nanoseconds"), "{refused}");

        let mut beyond = Vec::new();
        beyond.extend_from_slice(&u64::MAX.to_be_bytes());
        beyond.extend_from_slice(&0u32.to_be_bytes());
        let refused = Reader::new(&beyond).time().expect_err("a time beyond time");
        assert!(refused.why.contains("further ahead"), "{refused}");
    }

    /// Every prefix of something that decodes is refused, which is the property
    /// both formats inherit from here.
    #[test]
    fn every_truncation_is_refused_rather_than_believed() {
        let mut writing = Writer::default();
        writing.text("a moment and a name");
        writing.time(UNIX_EPOCH + Duration::from_secs(1));
        for cut in 0..writing.out.len() {
            let short = writing.out.get(..cut).unwrap_or_default();
            let mut reading = Reader::new(short);
            let read = reading.text().and_then(|_| reading.time());
            assert!(read.is_err(), "{cut} bytes read as though whole");
        }
    }

    #[test]
    fn the_same_bytes_fingerprint_the_same_and_different_ones_mostly_do_not() {
        assert_eq!(
            fingerprint(b"https://example.com/a"),
            fingerprint(b"https://example.com/a")
        );
        assert_ne!(
            fingerprint(b"https://example.com/a"),
            fingerprint(b"https://example.com/b")
        );
        assert_ne!(fingerprint(b""), 0, "an empty input is not a zero");
    }
}
