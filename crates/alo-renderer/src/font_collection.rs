/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One face read out of a font collection (queue item 767).
//!
//! A collection (`.ttc`) is several faces in one file sharing tables: the
//! file begins `ttcf`, then the place of each face's table directory, and
//! each directory names its tables by where they are in the **whole file**.
//! macOS keeps its Chinese, Japanese and Korean faces so — `Hiragino Sans
//! GB.ttc` is four faces in 23 MB — and a renderer, which loads a font's
//! first face ([`alo_text::Font::load`]) and is handed at most
//! [`LARGEST_FONT`] of one, could be handed none of them.
//!
//! So the browser process reads the one face it wants and writes it as a
//! font of its own: the face's directory, written anew with each table's
//! place in the new file, then each of its tables, copied byte for byte and
//! padded to four bytes as OpenType lays tables out. Nothing in a table is
//! read or changed — not even the checksums, which name each table's bytes
//! and so still hold — so the face is the collection's face, and the
//! renderer reads it as any font. The face is 7–12 MB where its file is
//! 8–55 MB, and a file is read only where the face is: its header, its
//! directory, its tables.
//!
//! Every number is a stranger's until it is checked: a collection naming
//! more than [`MOST_FACES`] faces, a face with more than [`MOST_TABLES`]
//! tables, or tables that come to more than [`LARGEST_FONT`] are refused,
//! by name, before anything that size is made.

use crate::face::LARGEST_FONT;
use std::io::{Read, Seek, SeekFrom};

/// The most faces a collection is believed to hold: macOS's largest holds
/// eighteen.
pub const MOST_FACES: u32 = 256;

/// The most tables a face is believed to have: a font has a few dozen at
/// most, and OpenType names fewer than a hundred.
pub const MOST_TABLES: u16 = 128;

/// Why a face could not be read out of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// The file could not be read where it had to be.
    Io(String),
    /// It does not begin `ttcf`: not a collection.
    NotACollection,
    /// It holds fewer faces than the one asked for.
    NoSuchFace {
        /// The face asked for.
        index: u32,
        /// How many it says it holds.
        holds: u32,
    },
    /// It says it holds more faces than any collection does.
    TooManyFaces(u32),
    /// The face says it has more tables than any face does.
    TooManyTables(u16),
    /// The face's tables come to more than a renderer is handed.
    TooLarge(u64),
}

impl core::fmt::Display for Unread {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Unread::Io(why) => write!(f, "the collection could not be read: {why}"),
            Unread::NotACollection => f.write_str("the file is not a font collection"),
            Unread::NoSuchFace { index, holds } => {
                write!(f, "face {index} was asked of a collection of {holds}")
            }
            Unread::TooManyFaces(said) => {
                write!(
                    f,
                    "a collection said it holds {said} faces, past {MOST_FACES}"
                )
            }
            Unread::TooManyTables(said) => {
                write!(f, "a face said it has {said} tables, past {MOST_TABLES}")
            }
            Unread::TooLarge(bytes) => write!(
                f,
                "a face of {bytes} bytes was not read; a renderer is handed at most {LARGEST_FONT}"
            ),
        }
    }
}

/// The face at `index` of the collection `file` holds, as a font of its own.
///
/// # Errors
///
/// [`Unread`], naming why: not a collection, no such face, or a face too
/// large to hand a renderer or not readable where it says it is.
pub fn face_of(file: &mut (impl Read + Seek), index: u32) -> Result<Vec<u8>, Unread> {
    let io = |why: std::io::Error| Unread::Io(why.to_string());
    let mut header = [0u8; 12];
    file.read_exact(&mut header).map_err(io)?;
    if header.get(..4) != Some(b"ttcf".as_slice()) {
        return Err(Unread::NotACollection);
    }
    let holds = u32_at(&header, 8);
    if holds > MOST_FACES {
        return Err(Unread::TooManyFaces(holds));
    }
    if index >= holds {
        return Err(Unread::NoSuchFace { index, holds });
    }
    file.seek(SeekFrom::Start(12 + 4 * u64::from(index)))
        .map_err(io)?;
    let mut place = [0u8; 4];
    file.read_exact(&mut place).map_err(io)?;
    file.seek(SeekFrom::Start(u64::from(u32_at(&place, 0))))
        .map_err(io)?;
    let mut directory = [0u8; 12];
    file.read_exact(&mut directory).map_err(io)?;
    let tables = u16_at(&directory, 4);
    if tables > MOST_TABLES {
        return Err(Unread::TooManyTables(tables));
    }
    let mut records = vec![0u8; 16 * usize::from(tables)];
    file.read_exact(&mut records).map_err(io)?;
    let records: Vec<Record> = records.chunks_exact(16).map(Record::read).collect();
    let head = 12 + 16 * u64::from(tables);
    let size = records
        .iter()
        .fold(head, |size, record| size + padded(record.length));
    if size > LARGEST_FONT as u64 {
        return Err(Unread::TooLarge(size));
    }
    let mut font = Vec::with_capacity(usize::try_from(size).unwrap_or(LARGEST_FONT));
    font.extend_from_slice(directory.get(..4).unwrap_or_default());
    font.extend_from_slice(&tables.to_be_bytes());
    let (search, selector, shift) = search_numbers(tables);
    for number in [search, selector, shift] {
        font.extend_from_slice(&number.to_be_bytes());
    }
    let mut at = head;
    for record in &records {
        font.extend_from_slice(&record.tag);
        font.extend_from_slice(&record.checksum.to_be_bytes());
        // Within the bound above, so within a `u32`.
        font.extend_from_slice(&u32::try_from(at).unwrap_or(u32::MAX).to_be_bytes());
        font.extend_from_slice(&record.length.to_be_bytes());
        at += padded(record.length);
    }
    for record in &records {
        file.seek(SeekFrom::Start(u64::from(record.offset)))
            .map_err(io)?;
        let start = font.len();
        font.resize(start + usize::try_from(record.length).unwrap_or(0), 0);
        file.read_exact(font.get_mut(start..).unwrap_or_default())
            .map_err(io)?;
        font.resize(
            start + usize::try_from(padded(record.length)).unwrap_or(0),
            0,
        );
    }
    Ok(font)
}

/// One table's record in a face's directory.
struct Record {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}

impl Record {
    /// A record's sixteen bytes, read.
    fn read(bytes: &[u8]) -> Self {
        let mut tag = [0u8; 4];
        tag.copy_from_slice(bytes.get(..4).unwrap_or(&[0; 4]));
        Self {
            tag,
            checksum: u32_at(bytes, 4),
            offset: u32_at(bytes, 8),
            length: u32_at(bytes, 12),
        }
    }
}

/// `length` rounded up to four bytes, where OpenType starts each table.
fn padded(length: u32) -> u64 {
    u64::from(length).div_ceil(4) * 4
}

/// A directory's `searchRange`, `entrySelector` and `rangeShift` for
/// `tables` tables, as OpenType defines them.
fn search_numbers(tables: u16) -> (u16, u16, u16) {
    if tables == 0 {
        return (0, 0, 0);
    }
    let selector = 15u16.saturating_sub(u16::try_from(tables.leading_zeros()).unwrap_or(15));
    let search = (1u16 << selector).saturating_mul(16);
    (
        search,
        selector,
        tables.saturating_mul(16).saturating_sub(search),
    )
}

/// The big-endian `u32` at `at` of `bytes`, or 0 past their end.
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .and_then(|four| four.try_into().ok())
        .map_or(0, u32::from_be_bytes)
}

/// The big-endian `u16` at `at` of `bytes`, or 0 past their end.
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    bytes
        .get(at..at + 2)
        .and_then(|two| two.try_into().ok())
        .map_or(0, u16::from_be_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn the_search_numbers_are_opentypes() {
        // OpenType's own example: 9 tables search 128 (8 × 16), select 3,
        // shift 16.
        assert_eq!(search_numbers(9), (128, 3, 16));
        assert_eq!(search_numbers(16), (256, 4, 0));
        assert_eq!(search_numbers(1), (16, 0, 0));
    }

    #[test]
    fn what_is_not_a_collection_or_says_too_much_is_refused_by_name() {
        let face = |bytes: &[u8], index| face_of(&mut Cursor::new(bytes.to_vec()), index);
        assert_eq!(
            face(dejavu::sans::regular(), 0),
            Err(Unread::NotACollection)
        );
        assert!(matches!(face(b"ttcf", 0), Err(Unread::Io(_))));
        let mut many = b"ttcf\x00\x01\x00\x00".to_vec();
        many.extend_from_slice(&1000u32.to_be_bytes());
        assert_eq!(face(&many, 0), Err(Unread::TooManyFaces(1000)));
        let mut two = b"ttcf\x00\x01\x00\x00".to_vec();
        two.extend_from_slice(&2u32.to_be_bytes());
        assert_eq!(
            face(&two, 2),
            Err(Unread::NoSuchFace { index: 2, holds: 2 })
        );
        // A face whose one table says it is 4 GB long.
        let mut huge = two.clone();
        huge.extend_from_slice(&20u32.to_be_bytes());
        huge.extend_from_slice(&20u32.to_be_bytes());
        huge.extend_from_slice(&[0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0]);
        huge.extend_from_slice(b"glyf\0\0\0\0\0\0\0\0\xff\xff\xff\xff");
        assert!(matches!(face(&huge, 0), Err(Unread::TooLarge(_))));
        assert!(
            Unread::TooLarge(5).to_string().contains("16777216"),
            "{}",
            Unread::TooLarge(5)
        );
    }
}
