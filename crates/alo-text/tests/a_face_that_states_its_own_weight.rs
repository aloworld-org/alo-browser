/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font states how heavy it is and whether it leans, and it is not the
//! filename that says so.
//!
//! Queue item 194, and the sibling of
//! `a_font_that_names_itself_in_an_old_encoding.rs`: that one took the
//! **family** off the font rather than off the name of the file it was read out
//! of, and left the other two fields of a face still guessed at by looking for
//! `bold` and `italic` in a filename. `DejaVuSans-Oblique.ttf` contains neither
//! word, and it is one of the fonts this very repository tests with.
//!
//! # The fonts here are built rather than found
//!
//! The same argument as the sibling file. A real font answers the ordinary
//! cases and cannot be made to answer the awkward ones — a weight of zero, a
//! number past the end of the scale, a table cut in half — so each of those
//! takes a real font and replaces its `OS/2` table with one this file wrote.
//! The last of them is the reason the machinery is worth having: a font file
//! comes from somewhere else, and a table that lies about itself must be an
//! answer rather than a crash.

use alo_text::{Slant, Weight, style_in};

/// The tag the table this file is about is filed under.
const OS2: &[u8] = b"OS/2";

/// Where `OS/2` writes the two things read here, in bytes from its start.
const WEIGHT_CLASS: usize = 4;
const SELECTION: usize = 62;

/// The `fsSelection` bits that say a face is slanted or heavy.
const ITALIC: u16 = 1 << 0;
const BOLD: u16 = 1 << 5;

/// Two bytes of a font, read as the file writes numbers.
///
/// Saturating and defaulting rather than panicking, because these helpers are
/// not test functions and the lints hold them to what any other code is held
/// to — the same rule the sibling file works under.
fn pair(data: &[u8], at: usize) -> u16 {
    data.get(at..at + 2)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map_or(0, u16::from_be_bytes)
}

/// Four bytes of a font, the same way.
fn quad(data: &[u8], at: usize) -> usize {
    data.get(at..at + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map_or(0, |bytes| u32::from_be_bytes(bytes) as usize)
}

/// A length as the table directory writes it.
fn long(number: usize) -> u32 {
    u32::try_from(number).unwrap_or(u32::MAX)
}

/// Where a tagged table sits in a font: its offset and its length.
fn table_at(data: &[u8], tag: &[u8]) -> Option<(usize, usize)> {
    (0..usize::from(pair(data, 4))).find_map(|index| {
        let record = 12 + index * 16;
        let named = data.get(record..record + 4)? == tag;
        named.then(|| (quad(data, record + 8), quad(data, record + 12)))
    })
}

/// The font every case here is built on, and its `OS/2` table as it ships.
fn sound() -> Vec<u8> {
    let data = dejavu::sans::regular();
    let (at, length) = table_at(data, OS2).unwrap_or_default();
    data.get(at..at + length)
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}

/// A real font with its `OS/2` table replaced by these bytes.
///
/// Appended and the directory entry repointed at it, rather than written over
/// the original: everything else — the outlines, the character map, the name
/// table — stays where it was, so what is being tested is the reading of one
/// table and nothing else.
fn font_with_os2(table: &[u8]) -> Vec<u8> {
    let mut data = dejavu::sans::regular().to_vec();
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let at = long(data.len());
    let length = long(table.len());
    data.extend_from_slice(table);

    let mut found = false;
    for index in 0..usize::from(pair(&data, 4)) {
        let record = 12 + index * 16;
        if data.get(record..record + 4) != Some(OS2) {
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
    assert!(found, "the font this test builds on has no OS/2 table");
    data
}

/// A real font with no `OS/2` table at all, which is what an older Macintosh
/// font really looks like.
///
/// The entry is retagged rather than removed, because removing it would mean
/// rewriting every offset after it. Nothing reads `NONE`.
fn font_without_os2() -> Vec<u8> {
    let mut data = dejavu::sans::regular().to_vec();
    for index in 0..usize::from(pair(&data, 4)) {
        let record = 12 + index * 16;
        if data.get(record..record + 4) != Some(OS2) {
            continue;
        }
        if let Some(slot) = data.get_mut(record..record + 4) {
            slot.copy_from_slice(b"NONE");
        }
    }
    data
}

// --- What a real font says ---------------------------------------------------

#[test]
fn a_real_font_states_what_its_filename_only_hints_at() {
    for (data, weight, slant, called) in [
        (
            dejavu::sans::regular(),
            Weight::NORMAL,
            Slant::Normal,
            "DejaVuSans.ttf",
        ),
        (
            dejavu::sans::bold(),
            Weight::BOLD,
            Slant::Normal,
            "DejaVuSans-Bold.ttf",
        ),
        (
            dejavu::sans::extra_light(),
            Weight::new(200),
            Slant::Normal,
            "DejaVuSans-ExtraLight.ttf",
        ),
        (
            dejavu::sans::oblique(),
            Weight::NORMAL,
            Slant::Italic,
            "DejaVuSans-Oblique.ttf",
        ),
        (
            dejavu::sans::bold_oblique(),
            Weight::BOLD,
            Slant::Italic,
            "DejaVuSans-BoldOblique.ttf",
        ),
        (
            dejavu::serif::italic(),
            Weight::NORMAL,
            Slant::Italic,
            "DejaVuSerif-Italic.ttf",
        ),
    ] {
        assert_eq!(
            style_in(data),
            Some(alo_text::Style {
                weight,
                slant,
                axis: None,
            }),
            "{called} states something else about itself",
        );
    }
}

#[test]
fn a_weight_a_filename_has_no_word_for_is_read_all_the_same() {
    // The two faces the old rule could name are 400 and 700; every real family
    // has more than two, and this is one of them shipped with this repository.
    let light = style_in(dejavu::sans::extra_light()).map(|style| style.weight);
    assert_eq!(light, Some(Weight::new(200)));
    assert!(
        light < Some(Weight::NORMAL),
        "an extra light face read as normal is a face nothing can ask for",
    );
}

#[test]
fn a_face_that_leans_without_the_word_italic_anywhere_is_still_slanted() {
    // `Oblique` is what half the world calls it, and the rule this replaces
    // looked for `italic`. So the file this repository has tested with since
    // stage 1 was filed upright.
    assert_eq!(
        style_in(dejavu::sans::oblique()).map(|style| style.slant),
        Some(Slant::Italic),
    );
}

#[test]
fn bytes_that_are_not_a_font_state_no_style() {
    assert_eq!(style_in(&[]), None);
    assert_eq!(style_in(&[0; 64]), None);
    let cut = dejavu::sans::regular().get(..1024).unwrap_or_default();
    assert_eq!(style_in(cut), None, "a font that stops half way");
}

// --- What a font written by somebody else says -------------------------------

/// The sound table with two of its bytes written over.
///
/// The one place a case says what a font states, so that every case below reads
/// as the sentence it is checking rather than as an offset.
fn stating(table: &[u8], at: usize, number: u16) -> Vec<u8> {
    let mut table = table.to_vec();
    let written = table
        .get_mut(at..at + 2)
        .map(|slot| slot.copy_from_slice(&number.to_be_bytes()));
    assert!(written.is_some(), "an OS/2 table too short to state {at}");
    table
}

/// The sound table with `usWeightClass` set to something else.
fn stating_weight(weight: u16) -> Vec<u8> {
    stating(&sound(), WEIGHT_CLASS, weight)
}

/// The sound table with `fsSelection` set to exactly these bits.
fn stating_bits(bits: u16) -> Vec<u8> {
    stating(&sound(), SELECTION, bits)
}

#[test]
fn a_weight_between_the_two_a_filename_could_say_is_kept_as_written() {
    for stated in [1u16, 9, 250, 350, 999, 1000] {
        assert_eq!(
            style_in(&font_with_os2(&stating_weight(stated))).map(|style| style.weight),
            Some(Weight::new(stated)),
            "a font stating {stated} was filed under something else",
        );
    }
}

#[test]
fn a_weight_of_nothing_is_not_a_statement_that_a_face_is_the_lightest_there_is() {
    // Zero is what a font writes when it did not say. Brought into the range
    // CSS allows it would become 1 — a hairline, and a face a page asking for
    // anything light would be given in preference to the one it wanted.
    let saying_nothing = stating(&stating_weight(0), SELECTION, 0);
    assert_eq!(
        style_in(&font_with_os2(&saying_nothing)).map(|style| style.weight),
        Some(Weight::NORMAL),
    );

    // And the one other thing the table says about heaviness is read rather
    // than ignored: a font that set the bold bit and left the number empty said
    // bold, in the spelling the software of its day went by.
    let saying_bold = stating(&stating_weight(0), SELECTION, BOLD);
    assert_eq!(
        style_in(&font_with_os2(&saying_bold)).map(|style| style.weight),
        Some(Weight::BOLD),
    );
}

#[test]
fn a_number_is_taken_at_its_word_where_the_bold_bit_disagrees_with_it() {
    // A number is the finer answer and CSS asks its question as a number, so a
    // face stating 300 and setting the bold bit is filed at 300. Reading it the
    // other way would put a light face where a page asked for a heavy one and
    // hide the light one entirely.
    let table = stating(&stating_weight(300), SELECTION, BOLD);
    assert_eq!(
        style_in(&font_with_os2(&table)).map(|style| style.weight),
        Some(Weight::new(300)),
    );
}

#[test]
fn a_weight_past_the_end_of_the_scale_is_brought_into_it() {
    assert_eq!(
        style_in(&font_with_os2(&stating_weight(u16::MAX))).map(|style| style.weight),
        Some(Weight::new(1000)),
    );
}

#[test]
fn the_italic_bit_is_read_and_the_others_are_not_mistaken_for_it() {
    assert_eq!(
        style_in(&font_with_os2(&stating_bits(ITALIC))).map(|style| style.slant),
        Some(Slant::Italic),
    );
    // Every bit on its own, so that nothing else in `fsSelection` — the bold
    // bit, the regular bit, the typographic-metrics bit — is read as a lean.
    // The font underneath is upright, and its `post` table says so too.
    for bit in 0..16u16 {
        let bits = 1u16 << bit;
        if bits == ITALIC || bits == 1 << 9 {
            continue;
        }
        assert_eq!(
            style_in(&font_with_os2(&stating_bits(bits))).map(|style| style.slant),
            Some(Slant::Normal),
            "bit {bit} of fsSelection was read as a lean",
        );
    }
}

#[test]
fn a_font_that_states_nothing_is_a_face_rather_than_no_face() {
    // Most of a machine's fonts are one unlabelled face of a family, and the
    // older ones may carry no `OS/2` table at all. Losing them over a table
    // nobody needed would be losing the font.
    let without = font_without_os2();
    assert_eq!(
        style_in(&without),
        Some(alo_text::Style {
            weight: Weight::NORMAL,
            slant: Slant::Normal,
            axis: None,
        }),
    );
    assert_eq!(
        alo_text::family_in(&without).as_deref(),
        Some("DejaVu Sans"),
        "and it is still a font a page can ask for by name",
    );
}

#[test]
fn a_face_that_leans_and_states_no_table_still_leans() {
    // `post` states an italic angle as well, and a face with no `OS/2` has not
    // said nothing. This is the one thing a missing table does not cost.
    let mut data = dejavu::sans::oblique().to_vec();
    for index in 0..usize::from(pair(&data, 4)) {
        let record = 12 + index * 16;
        if data.get(record..record + 4) == Some(OS2) {
            data[record..record + 4].copy_from_slice(b"NONE");
        }
    }
    assert_eq!(
        style_in(&data).map(|style| style.slant),
        Some(Slant::Italic),
    );
}

#[test]
fn an_os2_table_that_lies_about_itself_is_an_answer_rather_than_a_crash() {
    // Each of these is a table a file could really hold: a half-written font, a
    // truncated download, one written to be awkward. The engine must answer
    // something about every one of them, because the alternative in a renderer
    // is a tab that disappears.
    let table = sound();

    let mut future = table.clone();
    future[0..2].copy_from_slice(&u16::MAX.to_be_bytes());
    let _ = style_in(&font_with_os2(&future));

    for cut in 0..table.len() {
        let _ = style_in(&font_with_os2(table.get(..cut).unwrap_or_default()));
    }

    let mut single_bit = table.clone();
    for byte in 0..single_bit.len() {
        for bit in 0..8u8 {
            single_bit[byte] ^= 1 << bit;
            let _ = style_in(&font_with_os2(&single_bit));
            single_bit[byte] ^= 1 << bit;
        }
    }

    // Nothing above may be believed, and this is the assertion the whole test
    // hangs on: the sound table still reads, so what came back from the damaged
    // ones was the engine refusing rather than the engine failing to look.
    assert_eq!(
        style_in(&font_with_os2(&table)).map(|style| style.weight),
        Some(Weight::NORMAL),
    );
}
