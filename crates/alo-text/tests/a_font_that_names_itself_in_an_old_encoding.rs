/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font states its own family, and it does not always state it in Unicode.
//!
//! Queue item 192. A `name` table holds the same name once per platform that
//! was ever expected to read it, and several of the fonts macOS ships carry
//! **only** the Macintosh records — Apple Braille among them. Those records are
//! not UTF-16, so `alo_text::family_in` used to answer [`None`] for such a
//! font: the machine had it, the engine said it did not, and a page asking for
//! it by name got a substitution nobody could explain.
//!
//! # The fonts here are built rather than found
//!
//! A test that went looking for Apple Braille would pass on one machine and
//! skip on every other, and would say nothing at all about *which* encoding was
//! read. So each case below takes a real font, replaces its `name` table with
//! one written byte by byte, and names the encoding in the bytes themselves —
//! `0xD5` is a right single quote in Mac OS Roman and `Õ` in Latin-1, so a
//! decoder that had quietly fallen back to Latin-1 fails here rather than
//! passing on ASCII.
//!
//! The last half of the file is the other reason to build the tables: a font
//! file comes from somewhere else, and a `name` table that lies about its own
//! lengths must be an answer rather than a crash.

use alo_text::{LONGEST_NAME, family_in};

/// The `name` table's own numbers, as the standard spells them.
const FAMILY: u16 = 1;
const TYPOGRAPHIC_FAMILY: u16 = 16;
const MACINTOSH: u16 = 1;
const WINDOWS: u16 = 3;
const ROMAN: u16 = 0;
const JAPANESE: u16 = 1;
const UNICODE_BMP: u16 = 1;

/// One name record, before it is written down.
struct Record {
    platform: u16,
    encoding: u16,
    id: u16,
    bytes: Vec<u8>,
}

impl Record {
    /// A record in a Macintosh encoding, whose bytes are written out as they
    /// would sit in the file.
    fn macintosh(encoding: u16, id: u16, bytes: &[u8]) -> Self {
        Self {
            platform: MACINTOSH,
            encoding,
            id,
            bytes: bytes.to_vec(),
        }
    }

    /// A record the way nearly every font written this century carries it:
    /// Windows, UTF-16 big-endian.
    fn windows(id: u16, text: &str) -> Self {
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Self {
            platform: WINDOWS,
            encoding: UNICODE_BMP,
            id,
            bytes,
        }
    }
}

/// A length as the file writes it.
///
/// Saturating rather than panicking, because these helpers are not test
/// functions and the lints hold them to what any other code is held to. A test
/// that ever built a table larger than this would fail on its own assertion,
/// which is a better report than a helper's panic anyway.
fn short(number: usize) -> u16 {
    u16::try_from(number).unwrap_or(u16::MAX)
}

/// The same, for the table directory's four-byte offsets and lengths.
fn long(number: usize) -> u32 {
    u32::try_from(number).unwrap_or(u32::MAX)
}

/// Two bytes of a font, read as the file writes numbers.
fn pair(data: &[u8], at: usize) -> u16 {
    data.get(at..at + 2)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map_or(0, u16::from_be_bytes)
}

/// A `name` table in format 0, holding exactly these records.
fn name_table(records: &[Record]) -> Vec<u8> {
    let mut table = Vec::new();
    table.extend_from_slice(&0u16.to_be_bytes()); // format 0
    table.extend_from_slice(&short(records.len()).to_be_bytes());
    table.extend_from_slice(&short(6 + records.len() * 12).to_be_bytes());
    let mut storage = Vec::new();
    for record in records {
        table.extend_from_slice(&record.platform.to_be_bytes());
        table.extend_from_slice(&record.encoding.to_be_bytes());
        table.extend_from_slice(&0u16.to_be_bytes()); // language: English, or its
        // Macintosh equivalent, which is 0 in both
        table.extend_from_slice(&record.id.to_be_bytes());
        table.extend_from_slice(&short(record.bytes.len()).to_be_bytes());
        table.extend_from_slice(&short(storage.len()).to_be_bytes());
        storage.extend_from_slice(&record.bytes);
    }
    table.extend_from_slice(&storage);
    table
}

/// A real font with its `name` table replaced by these bytes.
///
/// The table is appended and the directory entry repointed at it, rather than
/// written over the original: everything else in the file — the outlines, the
/// character map, the metrics — stays where it was, so what is being tested is
/// the reading of one table and nothing else.
fn font_with_name_table(table: &[u8]) -> Vec<u8> {
    let mut data = dejavu::sans::regular().to_vec();
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let at = long(data.len());
    let length = long(table.len());
    data.extend_from_slice(table);

    let tables = usize::from(pair(&data, 4));
    let mut found = false;
    for index in 0..tables {
        let record = 12 + index * 16;
        if data.get(record..record + 4) != Some(b"name".as_slice()) {
            continue;
        }
        if let Some(slot) = data.get_mut(record + 8..record + 12) {
            slot.copy_from_slice(&at.to_be_bytes());
        }
        if let Some(slot) = data.get_mut(record + 12..record + 16) {
            slot.copy_from_slice(&length.to_be_bytes());
        }
        found = true;
    }
    assert!(found, "the font this test builds on has no name table");
    data
}

/// A font whose `name` table holds exactly these records.
fn font_named(records: &[Record]) -> Vec<u8> {
    font_with_name_table(&name_table(records))
}

// --- The family a page would ask for -----------------------------------------

#[test]
fn a_font_naming_itself_only_in_mac_os_roman_is_found_by_the_name_a_person_types() {
    let font = font_named(&[Record::macintosh(ROMAN, FAMILY, b"Apple\xD5s Braille")]);
    assert_eq!(
        family_in(&font).as_deref(),
        Some("Apple\u{2019}s Braille"),
        "a font carrying no Unicode name at all is still a font this machine has",
    );
}

#[test]
fn a_unicode_name_wins_wherever_a_font_has_one() {
    // Both records name the same family and only one of them can spell it.
    // Macintosh comes first in a well-formed table, so reading in file order
    // would take the older one — which is the regression this guards: no font
    // that already had a readable name may change its answer.
    let font = font_named(&[
        Record::macintosh(ROMAN, FAMILY, b"Hiragino Kaku Gothic"),
        Record::windows(FAMILY, "\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}"),
    ]);
    assert_eq!(
        family_in(&font).as_deref(),
        Some("\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}"),
    );
}

#[test]
fn the_typographic_name_is_still_the_one_css_means() {
    // The older name splits a large family into one name per handful of faces;
    // the typographic name is the family throughout, and that is what a
    // `font-family` in a stylesheet is naming. True whichever encoding either
    // of them is in.
    let font = font_named(&[
        Record::macintosh(ROMAN, FAMILY, b"Inter Semibold"),
        Record::macintosh(ROMAN, TYPOGRAPHIC_FAMILY, b"Inter"),
    ]);
    assert_eq!(family_in(&font).as_deref(), Some("Inter"));
}

#[test]
fn an_encoding_this_engine_will_not_guess_at_leaves_a_font_unnameable() {
    // Mac OS Japanese is close to Shift JIS and is not Shift JIS. Reading it as
    // one would put a character Apple never wrote into the name of somebody's
    // font, and a family read wrongly is worse than a family not read: it is a
    // name a page can match by accident.
    let font = font_named(&[Record::macintosh(JAPANESE, FAMILY, b"\x82\xA0\x82\xA2")]);
    assert_eq!(
        family_in(&font),
        None,
        "an encoding nobody here holds a table for was decoded anyway",
    );
}

#[test]
fn a_real_font_still_answers_what_it_always_did() {
    assert_eq!(
        family_in(dejavu::sans::regular()).as_deref(),
        Some("DejaVu Sans")
    );
    assert_eq!(
        family_in(dejavu::serif::regular()).as_deref(),
        Some("DejaVu Serif")
    );
}

// --- A name table written by somebody else -----------------------------------

#[test]
fn a_name_longer_than_a_family_name_could_be_is_not_a_family_name() {
    let long = vec![b'a'; LONGEST_NAME + 1];
    let font = font_named(&[Record::macintosh(ROMAN, FAMILY, &long)]);
    assert_eq!(family_in(&font), None);

    // And the bound is a bound rather than a refusal of anything long: one byte
    // under it is read.
    let allowed = vec![b'a'; LONGEST_NAME];
    let font = font_named(&[Record::macintosh(ROMAN, FAMILY, &allowed)]);
    assert_eq!(family_in(&font).map(|name| name.len()), Some(LONGEST_NAME));
}

#[test]
fn a_name_carrying_a_control_character_is_skipped_rather_than_cleaned() {
    for bytes in [
        b"Inter\x00polated".as_slice(),
        b"Inter\x07".as_slice(),
        b"\x1b[31mInter".as_slice(),
        b"Inter\nSans".as_slice(),
    ] {
        let font = font_named(&[Record::macintosh(ROMAN, FAMILY, bytes)]);
        assert_eq!(
            family_in(&font),
            None,
            "{bytes:?} was read as a family name",
        );
    }
}

#[test]
fn a_name_of_nothing_is_no_name() {
    assert_eq!(family_in(&font_named(&[])), None, "a table with no records");
    assert_eq!(
        family_in(&font_named(&[Record::macintosh(ROMAN, FAMILY, b"")])),
        None,
        "a record with no bytes",
    );
    assert_eq!(
        family_in(&font_named(&[Record::macintosh(ROMAN, FAMILY, b"   ")])),
        None,
        "a record of spaces",
    );
}

#[test]
fn a_name_table_that_lies_about_its_own_lengths_is_an_answer_rather_than_a_crash() {
    // Each of these is a `name` table a file could really hold: they are what a
    // half-written font, a truncated download and a hostile one look like. The
    // engine must answer something about every one of them, because the
    // alternative in a renderer is a tab that disappears.
    let sound = name_table(&[Record::macintosh(ROMAN, FAMILY, b"Inter")]);

    let mut lying_length = sound.clone();
    lying_length[6 + 8..6 + 10].copy_from_slice(&u16::MAX.to_be_bytes());
    let _ = family_in(&font_with_name_table(&lying_length));

    let mut lying_offset = sound.clone();
    lying_offset[6 + 10..6 + 12].copy_from_slice(&u16::MAX.to_be_bytes());
    let _ = family_in(&font_with_name_table(&lying_offset));

    let mut too_many = sound.clone();
    too_many[2..4].copy_from_slice(&999u16.to_be_bytes());
    let _ = family_in(&font_with_name_table(&too_many));

    for cut in 0..sound.len() {
        let _ = family_in(&font_with_name_table(&sound[..cut]));
    }

    let mut single_bit = sound.clone();
    for byte in 0..single_bit.len() {
        for bit in 0..8u8 {
            single_bit[byte] ^= 1 << bit;
            let _ = family_in(&font_with_name_table(&single_bit));
            single_bit[byte] ^= 1 << bit;
        }
    }

    // Nothing above may be believed, and this is the assertion the whole test
    // hangs on: a sound table still reads, so what came back from the damaged
    // ones was the engine refusing rather than the engine failing to look.
    assert_eq!(
        family_in(&font_with_name_table(&sound)).as_deref(),
        Some("Inter"),
    );
}
