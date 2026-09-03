/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A variable font's letters are a different **shape** at a different weight,
//! not only a different width.
//!
//! Queue item 196's other half. `alo-text`'s
//! `a_font_that_is_many_weights.rs` asserts what a variable font *measures* at
//! two weights, because that is what decides where a line breaks. This asserts
//! what it *draws*, and the two halves are not the same line of code: advances
//! come out of a font's `HVAR` table and outlines out of its `gvar`, and each
//! is applied by the parser only once the face has been told which instance it
//! is.
//!
//! Leaving this one out would be the worse of the two failures. A font measured
//! at 700 and drawn at 400 puts light letters at heavy spacing — every word
//! visibly loose, and nothing in a width assertion would have noticed.
//!
//! # The font is built here, and does not vary the same thing
//!
//! The sibling file's font varies advances. This one varies a **point of an
//! outline**, which is the only thing that proves the outline path was told
//! about the axis. Building it here rather than sharing one is deliberate: the
//! two crates are testing two different tables, and a fixture that carried both
//! would let either half pass on the other's evidence.

use alo_paint::{Segment, outline};
use alo_text::{Direction, Font, Slant, Weight, shape};

/// The axis this font declares, and how far it moves a point along it.
const DEFAULT: i32 = 400;
const HEAVIEST: i32 = 900;
/// In the font's own units, at the heavy end.
const MOVED: i16 = 300;
/// What this font measures in, and the size everything here is drawn at.
const UNITS_PER_EM: f32 = 2048.0;
const SIZE: f32 = 100.0;

/// The entries taken over to carry the two tables, neither of which this font
/// needs: a build timestamp and a mathematical typesetting table.
const SPARE_FOR_FVAR: &[u8] = b"FFTM";
const SPARE_FOR_GVAR: &[u8] = b"MATH";

/// Two bytes of a font, read as the file writes numbers.
///
/// Saturating rather than panicking: this is not a test function, and the lints
/// hold it to what any other code is held to.
fn pair(data: &[u8], at: usize) -> u16 {
    data.get(at..at + 2)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map_or(0, u16::from_be_bytes)
}

/// A length as the table directory writes it.
fn long(number: usize) -> u32 {
    u32::try_from(number).unwrap_or(u32::MAX)
}

/// How many glyphs the font has, from `maxp` — `gvar` carries one entry each.
fn glyph_count(data: &[u8]) -> usize {
    let at = (0..usize::from(pair(data, 4)))
        .find(|index| data.get(12 + index * 16..12 + index * 16 + 4) == Some(b"maxp"))
        .map_or(0, |index| {
            data.get(12 + index * 16 + 8..12 + index * 16 + 12)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map_or(0, |bytes| u32::from_be_bytes(bytes) as usize)
        });
    usize::from(pair(data, at + 4))
}

/// A font with `table` appended and the `spare` directory entry repointed at it
/// under a new tag.
///
/// Retagging rather than adding an entry: adding one would move every table in
/// the file by sixteen bytes, and this test would then be about whether it
/// rewrote a font correctly.
fn carrying(mut data: Vec<u8>, spare: &[u8], tag: &[u8], table: &[u8]) -> Vec<u8> {
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let at = long(data.len());
    let length = long(table.len());
    data.extend_from_slice(table);

    let mut found = false;
    for index in 0..usize::from(pair(&data, 4)) {
        let record = 12 + index * 16;
        if data.get(record..record + 4) != Some(spare) {
            continue;
        }
        if let Some(slot) = data.get_mut(record..record + 4) {
            slot.copy_from_slice(tag);
        }
        if let Some(slot) = data.get_mut(record + 8..record + 12) {
            slot.copy_from_slice(&at.to_be_bytes());
        }
        if let Some(slot) = data.get_mut(record + 12..record + 16) {
            slot.copy_from_slice(&length.to_be_bytes());
        }
        found = true;
    }
    assert!(
        found,
        "the font this test builds on has no {} table to take over",
        String::from_utf8_lossy(spare),
    );
    data
}

/// Bytes being written in the order a font writes them.
#[derive(Default)]
struct Written(Vec<u8>);

impl Written {
    fn byte(&mut self, value: u8) -> &mut Self {
        self.0.push(value);
        self
    }

    fn short(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn signed(&mut self, value: i16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn word(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// A 16.16 fixed-point number, which is how `fvar` states an axis bound.
    fn fixed(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&(value << 16).to_be_bytes());
        self
    }
}

/// A `fvar` table declaring one weight axis.
fn fvar() -> Vec<u8> {
    let mut out = Written::default();
    out.short(1) // major version
        .short(0) // minor
        .short(16) // where the axes start
        .short(2) // reserved, and the specification says two
        .short(1) // one axis
        .short(20) // twenty bytes of it
        .short(0) // no named instances
        .short(8); // which would be this size if there were
    out.0.extend_from_slice(b"wght");
    out.fixed(DEFAULT)
        .fixed(DEFAULT)
        .fixed(HEAVIEST)
        .short(0) // flags
        .short(17); // the axis's name, in the name table
    out.0
}

/// One glyph's variation data: move its first point sideways at the heavy end
/// of the axis, and say nothing about any other point.
///
/// The first point rather than all of them because a delta has to be written
/// per point, and one is enough to prove the outline was read at an instance —
/// the parser interpolates the rest, which is what a real font relies on too.
fn one_glyphs_variation() -> Vec<u8> {
    /// A peak on the axis, embedded here rather than shared, with the point
    /// numbers written in this tuple rather than beside it.
    const EMBEDDED_PEAK_AND_PRIVATE_POINTS: u16 = 0x8000 | 0x2000;
    /// The far end of a normalised axis.
    const HIGHEST: i16 = 16384;

    let mut serialized = Written::default();
    // The points this tuple has deltas for: one of them, numbered zero.
    serialized
        .byte(1) // one point
        .byte(0) // a run of one, written as a byte
        .byte(0); // point zero
    // Its delta along x: one delta, written as a word.
    serialized.byte(0x40).signed(MOVED);
    // And along y: one delta, and it is zero.
    serialized.byte(0x80);

    let mut out = Written::default();
    out.short(1) // one tuple
        .short(4 + 6); // where its data starts: past this and one header
    out.short(u16::try_from(serialized.0.len()).unwrap_or(u16::MAX))
        .short(EMBEDDED_PEAK_AND_PRIVATE_POINTS)
        .signed(HIGHEST);
    out.0.extend_from_slice(&serialized.0);
    out.0
}

/// A `gvar` table giving every glyph the same variation.
fn gvar(glyphs: usize) -> Vec<u8> {
    let one = one_glyphs_variation();
    let offsets = glyphs + 1;
    let array = 20 + 4 * offsets;

    let mut out = Written::default();
    out.word(0x0001_0000) // version 1.0
        .short(1) // one axis, as fvar says
        .short(0) // no shared tuples
        .word(0) // and so no offset to them
        .short(u16::try_from(glyphs).unwrap_or(u16::MAX))
        .short(1) // long offsets
        .word(long(array));
    for index in 0..offsets {
        out.word(long(index * one.len()));
    }
    for _ in 0..glyphs {
        out.0.extend_from_slice(&one);
    }
    out.0
}

/// A real font made variable, with an axis that moves its outlines.
fn variable() -> Vec<u8> {
    let base = dejavu::sans::regular().to_vec();
    let glyphs = glyph_count(&base);
    let with_outlines = carrying(base, SPARE_FOR_GVAR, b"gvar", &gvar(glyphs));
    carrying(with_outlines, SPARE_FOR_FVAR, b"fvar", &fvar())
}

/// The font, filed at the weight its `OS/2` states — which is what the browser
/// process does with every face it finds.
///
/// [`None`] rather than a panic, because this is not a test function and the
/// lints hold it to what any other code is held to.
fn loaded() -> Option<Font> {
    Font::load("Continuum", Weight::new(400), Slant::Normal, variable())
}

/// Where the pen is first put down when this letter is drawn at a weight.
///
/// The first point of the outline is the one this font states a delta for, so
/// it is the one whose movement is arithmetic rather than interpolation.
fn first_point_of(character: char, weight: u16) -> Option<(f32, f32)> {
    let font = loaded()?.at_weight(Weight::new(weight));
    let run = shape(&character.to_string(), &font, SIZE, Direction::LeftToRight);
    let id = run.glyphs.first().map(|glyph| glyph.glyph_id)?;
    let glyph = outline(&font, id, SIZE)?;
    glyph.path.segments().first().and_then(|segment| {
        if let Segment::MoveTo(point) = segment {
            Some((point.x, point.y))
        } else {
            None
        }
    })
}

#[test]
fn a_letter_drawn_at_two_weights_is_two_shapes() {
    let light = first_point_of('H', 400).expect("an H at the default instance");
    let heavy = first_point_of('H', 900).expect("an H at the heavy end");

    // Exactly the delta the font states, in pixels: the axis is at its far end,
    // so nothing here is interpolated and the number is arithmetic.
    let expected = f32::from(MOVED) * SIZE / UNITS_PER_EM;
    assert!(
        (heavy.0 - light.0 - expected).abs() < 0.01,
        "the outline should have moved {expected} and moved {}",
        heavy.0 - light.0,
    );
    assert!(
        (heavy.1 - light.1).abs() < 0.01,
        "and it should not have moved up or down: {} against {}",
        light.1,
        heavy.1,
    );
}

#[test]
fn a_letter_of_a_font_with_no_axis_is_the_same_shape_whatever_is_asked_for() {
    // The other direction, and the one that says the rule above is about the
    // font rather than about the request: an ordinary face cannot be set to
    // anything, so asking for black gets the face that is there, unchanged.
    let font = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
    .expect("the font this repository is tested with");
    let at = |weight: u16| {
        let font = font.at_weight(Weight::new(weight));
        let run = shape("H", &font, SIZE, Direction::LeftToRight);
        run.glyphs
            .first()
            .map(|glyph| glyph.glyph_id)
            .and_then(|id| outline(&font, id, SIZE))
    };
    assert_eq!(at(400), at(900));
    assert!(at(400).is_some_and(|glyph| !glyph.is_blank()));
}
