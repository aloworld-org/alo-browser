/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font states its own family once per language, and only one of them is the
//! name a page will ask for.
//!
//! Queue item 195, and item 192's failure arriving by another road. That item
//! made a font whose name was written before Unicode readable; this one is about
//! a font whose name is readable several times over. macOS's system font states
//! its family thirty-five times — `System Font`, `Police système`,
//! `システムフォント` — and `alo_text::family_in` took the **first** record of
//! each kind, whatever language it was in. On this machine the first Windows
//! record of that font is Catalan, and the engine was saved from filing it under
//! Catalan only by the accident that the font's unlocalised record comes before
//! all of them.
//!
//! It was not saved anywhere else: before this item, `Songti.ttc` was filed
//! under `宋體-簡`, `STHeiti Light.ttc` under `黑體-繁` and `Hiragino Sans GB.ttc`
//! under `冬青黑體簡體中文` — four of this machine's fonts under names no
//! stylesheet writes.
//!
//! # The fonts here are built rather than found
//!
//! The same reason as item 192's file, which this one borrows its table builder
//! from: a test that went looking for Songti would pass on one machine and skip
//! on every other, and would say nothing about *which* record was preferred. So
//! each case writes a `name` table byte by byte, with the language ids in it.

use alo_text::{FontDatabase, FontRequest, Slant, Weight, family_in, measure_unwrapped};

/// The `name` table's own numbers, as the standard spells them.
const FAMILY: u16 = 1;
const TYPOGRAPHIC_FAMILY: u16 = 16;
const UNICODE: u16 = 0;
const MACINTOSH: u16 = 1;
const WINDOWS: u16 = 3;
const ROMAN: u16 = 0;
const UNICODE_BMP: u16 = 1;

/// The Windows language ids that are English, as the OpenType specification
/// lists them: the United States, and fifteen other places that write it.
///
/// Written out rather than derived from the low ten bits of an id, so that this
/// file and the engine reach the same answer by two different roads. The low ten
/// bits are the *primary* language and 0x009 is English, which would make
/// 0x3809 English too — the specification lists no such place, and the engine
/// reads the list rather than the arithmetic.
const ENGLISH: [u16; 16] = [
    0x0409, 0x0809, 0x0C09, 0x1009, 0x1409, 0x1809, 0x1C09, 0x2009, 0x2409, 0x2809, 0x2C09, 0x3009,
    0x3409, 0x4009, 0x4409, 0x4809,
];

/// Some languages that are not, and the way each writes "system font".
const CATALAN: (u16, &str) = (0x0403, "Tipus de lletra del sistema");
const GERMAN: (u16, &str) = (0x0407, "Systemschrift");
const FRENCH: (u16, &str) = (0x040C, "Police système");
const JAPANESE: (u16, &str) = (0x0411, "システムフォント");

/// What the English records of that font say.
const IN_ENGLISH: &str = "System Font";

/// One name record, before it is written down.
struct Record {
    platform: u16,
    encoding: u16,
    language: u16,
    id: u16,
    bytes: Vec<u8>,
}

impl Record {
    /// A record the way nearly every font written this century carries it:
    /// Windows, UTF-16 big-endian, in a stated language.
    fn windows(language: u16, id: u16, text: &str) -> Self {
        Self {
            platform: WINDOWS,
            encoding: UNICODE_BMP,
            language,
            id,
            bytes: utf16(text),
        }
    }

    /// A record on the Unicode platform, which defines no languages at all.
    fn unicode(id: u16, text: &str) -> Self {
        Self {
            platform: UNICODE,
            encoding: UNICODE_BMP,
            language: 0,
            id,
            bytes: utf16(text),
        }
    }

    /// A record in Mac OS Roman, where Apple's language code 0 is English.
    fn macintosh(language: u16, id: u16, bytes: &[u8]) -> Self {
        Self {
            platform: MACINTOSH,
            encoding: ROMAN,
            language,
            id,
            bytes: bytes.to_vec(),
        }
    }
}

fn utf16(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    bytes
}

/// A length as the file writes it.
///
/// Saturating rather than panicking, because these helpers are not test
/// functions and the lints hold them to what any other code is held to.
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

/// A `name` table in format 0, holding exactly these records in this order.
fn name_table(records: &[Record]) -> Vec<u8> {
    let mut table = Vec::new();
    table.extend_from_slice(&0u16.to_be_bytes()); // format 0
    table.extend_from_slice(&short(records.len()).to_be_bytes());
    table.extend_from_slice(&short(6 + records.len() * 12).to_be_bytes());
    let mut storage = Vec::new();
    // Both are fields of two bytes, and a fixture that overflowed one would be a
    // table of nonsense that this file then asserted things about. It has
    // happened: four thousand records of fourteen characters is more storage
    // than an offset can point into.
    assert!(
        u16::try_from(records.len()).is_ok(),
        "more records than the count field can hold",
    );
    assert!(
        u16::try_from(
            records
                .iter()
                .map(|record| record.bytes.len())
                .sum::<usize>()
        )
        .is_ok(),
        "more name bytes than a record's offset can reach",
    );
    for record in records {
        table.extend_from_slice(&record.platform.to_be_bytes());
        table.extend_from_slice(&record.encoding.to_be_bytes());
        table.extend_from_slice(&record.language.to_be_bytes());
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
/// The table is appended and the directory entry repointed at it, so everything
/// else in the file stays where it was and what is under test is the reading of
/// one table.
fn font_named_with(bytes: &[u8], records: &[Record]) -> Vec<u8> {
    let table = name_table(records);
    let mut data = bytes.to_vec();
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let at = long(data.len());
    let length = long(table.len());
    data.extend_from_slice(&table);

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

/// The same, on the font this crate is tested with.
fn font_named(records: &[Record]) -> Vec<u8> {
    font_named_with(dejavu::sans::regular(), records)
}

/// The records macOS's system font carries, in the order it carries them and
/// without the unlocalised one — which is the shape of every font this item is
/// about, and the shape the old rule got wrong.
fn localised_first() -> Vec<Record> {
    vec![
        Record::windows(CATALAN.0, FAMILY, CATALAN.1),
        Record::windows(GERMAN.0, FAMILY, GERMAN.1),
        Record::windows(ENGLISH[0], FAMILY, IN_ENGLISH),
        Record::windows(FRENCH.0, FAMILY, FRENCH.1),
        Record::windows(JAPANESE.0, FAMILY, JAPANESE.1),
    ]
}

// --- Which of a font's names is the one a page asks for ----------------------

#[test]
fn a_localised_record_first_does_not_decide_the_family() {
    let font = font_named(&localised_first());
    assert_eq!(
        family_in(&font).as_deref(),
        Some(IN_ENGLISH),
        "the font was filed under whichever language its table happened to list first",
    );
}

#[test]
fn the_english_record_wins_from_wherever_it_sits() {
    // Last of thirty, which is roughly where it sits in a real one: the Windows
    // records are ordered by language id and English is 0x0409, so a font
    // carrying Catalan, Chinese, Czech, Danish, German and Greek has six ahead
    // of it before anybody has done anything wrong.
    let mut records: Vec<Record> = (0..29)
        .map(|index| 0x0401 + index)
        .filter(|id| !ENGLISH.contains(id))
        .map(|id| Record::windows(id, FAMILY, "Not The Name"))
        .collect();
    records.push(Record::windows(ENGLISH[0], FAMILY, IN_ENGLISH));
    assert_eq!(
        family_in(&font_named(&records)).as_deref(),
        Some(IN_ENGLISH)
    );
}

#[test]
fn every_english_a_font_may_state_is_read_as_english() {
    // Eighteen places write English and each has its own id. A font that stated
    // the family in English for Ireland and in Catalan for Catalonia is an
    // English-named font, and a rule that knew only 0x0409 would file it under
    // the Catalan.
    for id in ENGLISH {
        let font = font_named(&[
            Record::windows(CATALAN.0, FAMILY, CATALAN.1),
            Record::windows(id, FAMILY, IN_ENGLISH),
        ]);
        assert_eq!(
            family_in(&font).as_deref(),
            Some(IN_ENGLISH),
            "language {id:#06x} is English and was read as a translation",
        );
    }
}

#[test]
fn an_english_macintosh_record_beats_a_localised_one() {
    // Apple's language code 0 is English, and it is what nearly every Macintosh
    // record carries — so the older half of a `name` table is ordered by the
    // same rule as the newer half.
    let font = font_named(&[
        Record::macintosh(1, FAMILY, b"Police syst\x8fme"),
        Record::macintosh(0, FAMILY, b"System Font"),
    ]);
    assert_eq!(family_in(&font).as_deref(), Some(IN_ENGLISH));
}

#[test]
fn a_name_that_states_no_language_is_the_font_s_own_name() {
    // The Unicode platform defines no language ids, so a record there is not
    // written *in* anything: it is what the font calls itself, and the Windows
    // records beside it — English included — are its translations.
    //
    // macOS is the evidence rather than the specification. `SFNS.ttf` is exactly
    // this shape, and CoreText answers `.SF NS` for the family while keeping
    // `System Font` as the name to show a person. Reading it the other way round
    // would file the system font of this machine under a name the machine itself
    // does not use, and this engine's `sans-serif` names both.
    let mut records = vec![Record::unicode(FAMILY, ".SF NS")];
    records.extend(localised_first());
    assert_eq!(family_in(&font_named(&records)).as_deref(), Some(".SF NS"));
}

#[test]
fn a_font_that_states_no_english_name_at_all_is_still_a_font_this_machine_has() {
    // A font in one language is a font somebody may still have, and its own
    // name is better than no name: filing it under nothing would mean a page
    // could not ask for it and a person could not be told it was there.
    let font = font_named(&[
        Record::windows(JAPANESE.0, FAMILY, JAPANESE.1),
        Record::windows(FRENCH.0, FAMILY, FRENCH.1),
    ]);
    assert_eq!(
        family_in(&font).as_deref(),
        Some(JAPANESE.1),
        "the first record is all there is to go on, and it was not taken",
    );
}

#[test]
fn the_language_decides_inside_a_kind_of_name_and_never_between_two_kinds() {
    // The older name splits a large family into one name per handful of faces
    // and the typographic name is the family throughout — which is the family
    // CSS means, whatever language either of them is in. A language that
    // outranked the kind of name would file this font under a name for four of
    // its faces.
    let font = font_named(&[
        Record::windows(ENGLISH[0], FAMILY, "Inter Semibold"),
        Record::windows(FRENCH.0, TYPOGRAPHIC_FAMILY, "Inter"),
    ]);
    assert_eq!(family_in(&font).as_deref(), Some("Inter"));
}

#[test]
fn the_fonts_this_repository_is_tested_with_still_answer_what_they_always_did() {
    assert_eq!(
        family_in(dejavu::sans::regular()).as_deref(),
        Some("DejaVu Sans"),
    );
    assert_eq!(
        family_in(dejavu::serif::regular()).as_deref(),
        Some("DejaVu Serif"),
    );
    assert_eq!(
        family_in(dejavu::sans::bold()).as_deref(),
        Some("DejaVu Sans"),
    );
}

// --- The numbers -------------------------------------------------------------

#[test]
fn text_asking_for_the_english_name_is_measured_in_the_font_that_carries_it() {
    // Which family a font is filed under decides whether a page asking for it by
    // name is given it, and so how wide its text is and where every line of it
    // breaks. The face here is the bold one, because it is a different width
    // from the regular and the difference is what makes the assertion mean
    // something.
    let font = font_named_with(dejavu::sans::bold(), &localised_first());
    let family = family_in(&font).expect("a font that states a family");
    assert_eq!(family, IN_ENGLISH);

    let regular = alo_text::Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
    .expect("the regular face this crate is tested with");
    let filed_as = |name: &str| {
        let mut database = FontDatabase::new();
        // The other face first, so that a request this database cannot answer
        // falls through to it rather than to the font under test.
        database.add(regular.clone());
        database.add(
            alo_text::Font::load(name, Weight::NORMAL, Slant::Normal, font.clone())
                .expect("a face this engine can load"),
        );
        database
    };
    let asking = FontRequest::family(IN_ENGLISH);
    let width = |database: &FontDatabase| {
        measure_unwrapped("Invoices, twelve rows", database, &asking, 16.0).width()
    };

    // Filed under the English name: the page gets the font it asked for.
    let given = width(&filed_as(&family));
    // Filed under the German one, which is what the first record of such a font
    // used to decide: the same page asks for a family nobody has and is drawn in
    // whatever else was to hand.
    let missed = width(&filed_as(GERMAN.1));
    assert!(
        (given - missed).abs() > 0.5,
        "the request was answered the same way whether or not the font was findable: \
         {given} against {missed}",
    );

    // And it is that font that answered rather than merely a different one: the
    // same text through a database holding only those bytes measures the same,
    // to the pixel.
    let mut only = FontDatabase::new();
    only.add(
        alo_text::Font::load(IN_ENGLISH, Weight::NORMAL, Slant::Normal, font.clone())
            .expect("a face this engine can load"),
    );
    assert!(
        (given - width(&only)).abs() < 0.01,
        "a request for {IN_ENGLISH} was answered with something else: {given} against {}",
        width(&only),
    );
}

// --- A language id is two bytes somebody else wrote --------------------------

#[test]
fn every_language_id_a_font_could_carry_is_an_answer_rather_than_a_crash() {
    // The id is read out of a file, so all sixty-five thousand of them are
    // possible and the engine must answer something about each. The reason to
    // walk the whole range rather than a handful is the rented table underneath:
    // it maps an id to a language by looking it up in a list, and a list is a
    // thing with an end.
    //
    // In chunks, because one font per id would be sixty-five thousand copies of
    // a megabyte. Each chunk also asserts what a chunk is for: the English
    // record wins from among a thousand competitors, and the chunks that hold an
    // English id of their own keep the first English name in the file.
    const CHUNK: u32 = 1024;
    for start in (0..=u32::from(u16::MAX)).step_by(CHUNK as usize) {
        let ids: Vec<u16> = (start..(start + CHUNK).min(u32::from(u16::MAX) + 1))
            .filter_map(|id| u16::try_from(id).ok())
            .collect();
        let mut records: Vec<Record> = ids
            .iter()
            .map(|id| Record::windows(*id, FAMILY, &format!("L{id}")))
            .collect();
        records.push(Record::windows(ENGLISH[0], FAMILY, IN_ENGLISH));
        let expected = ids
            .iter()
            .find(|id| ENGLISH.contains(id))
            .map_or_else(|| IN_ENGLISH.to_owned(), |id| format!("L{id}"));
        assert_eq!(
            family_in(&font_named(&records)).as_deref(),
            Some(expected.as_str()),
            "the chunk starting at {start:#06x} was read wrongly",
        );
    }
}

#[test]
fn a_language_tag_this_engine_cannot_read_never_outranks_one_it_can() {
    // A `name` table in format 1 may put a language *tag* — `en`, `fr`, `ja` —
    // behind an id of 0x8000 or more, and this engine does not read those. A
    // record naming a language we cannot check is treated as a translation,
    // which is the answer that cannot promote a localised name over an English
    // one. The Unicode platform is where such ids appear, and it is the same
    // platform whose id 0 means "no language at all" — so the two readings sit
    // one byte apart and only one of them may win.
    let font = font_named(&[
        Record {
            platform: UNICODE,
            encoding: UNICODE_BMP,
            language: 0x8000,
            id: FAMILY,
            bytes: utf16(JAPANESE.1),
        },
        Record::windows(ENGLISH[0], FAMILY, IN_ENGLISH),
    ]);
    assert_eq!(family_in(&font).as_deref(), Some(IN_ENGLISH));
}
