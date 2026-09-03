/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A variable font is one file and many weights.
//!
//! Queue item 196, and the third of the line that began with
//! `a_font_that_names_itself_in_an_old_encoding.rs`: that one took the family
//! off the font rather than off its filename, `a_face_that_states_its_own_
//! weight.rs` took the weight and the slant off `OS/2` rather than off the same
//! filename, and this one is about the file where `OS/2` states **one** weight
//! and the font holds a continuum.
//!
//! It is not a small wrong. macOS's `SFCompact.ttf` states 1000, so this engine
//! filed the whole family as the heaviest thing CSS can name and every page
//! asking for anything lighter was drawn in black; `SFNSMono.ttf` states 295,
//! so nothing could ask for its bold.
//!
//! # The font here is built rather than found
//!
//! The sibling files' argument, and it applies harder: a machine either has a
//! variable font or does not, and a test that only passes on the machine that
//! wrote it is a test nobody can trust on a Tuesday. So this takes a real font
//! and gives it the two tables a variable font is made of — `fvar`, which says
//! what axes it has, and `HVAR`, which says how a glyph's advance changes along
//! them. Both are written here byte by byte, which is also what makes the
//! hostile half possible: a real font answers the ordinary cases and cannot be
//! made to answer an axis that runs backwards.
//!
//! The tables are **appended** and two entries the font does not need —
//! `FFTM`, a build timestamp, and `MATH` — are repointed at them. Nothing else
//! moves: the outlines, the character map and the name table stay exactly where
//! they were, so what is under test is the reading of two new tables and not a
//! font this file rebuilt.

use alo_text::{
    Direction, Font, FontDatabase, FontRequest, Slant, Weight, measure_unwrapped, style_in,
};

/// The axis every case here is about, and the two entries taken over to carry
/// the tables that describe it.
const FVAR: &[u8] = b"fvar";
const HVAR: &[u8] = b"HVAR";
const SPARE_FOR_FVAR: &[u8] = b"FFTM";
const SPARE_FOR_HVAR: &[u8] = b"MATH";

/// The axis this file's font declares, in CSS's own numbers.
const LIGHTEST: u16 = 200;
const DEFAULT: u16 = 400;
const HEAVIEST: u16 = 900;

/// How much wider or narrower every glyph gets at the ends of that axis, in the
/// font's units.
///
/// Two different numbers on purpose: one delta used both ways would pass a rule
/// that read the axis as a flag rather than as a position on it.
const AT_THE_HEAVY_END: i16 = 500;
const AT_THE_LIGHT_END: i16 = -300;

/// What this font measures in, so a delta in its units can be stated in pixels.
const UNITS_PER_EM: f32 = 2048.0;
const SIZE: f32 = 16.0;

/// Two bytes of a font, read as the file writes numbers.
///
/// Saturating and defaulting rather than panicking: these helpers are not test
/// functions, and the lints hold them to what any other code is held to.
fn pair(data: &[u8], at: usize) -> u16 {
    data.get(at..at + 2)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map_or(0, u16::from_be_bytes)
}

/// Four bytes, the same way.
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

/// How many glyphs the font has, from `maxp`.
///
/// `HVAR` needs it: with no mapping table of its own, a glyph's id **is** its
/// row in the variation data, so there has to be a row per glyph.
fn glyph_count(data: &[u8]) -> usize {
    let (at, _) = table_at(data, b"maxp").unwrap_or_default();
    usize::from(pair(data, at + 4))
}

/// A font with `table` appended and the `spare` directory entry repointed at it
/// under a new tag.
///
/// Retagging rather than adding an entry, because adding one would move every
/// table in the file by sixteen bytes and this test would then be about whether
/// it rewrote a font correctly.
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

// --- The two tables a variable font is made of -------------------------------

/// Bytes being written in the order a font writes them.
#[derive(Default)]
struct Written(Vec<u8>);

impl Written {
    fn short(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn signed(&mut self, value: i16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// A 16.16 fixed-point number, which is how `fvar` states an axis bound.
    fn fixed(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&(value << 16).to_be_bytes());
        self
    }

    fn word(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }
}

/// A `fvar` table declaring one axis, between two weights.
fn fvar(tag: [u8; 4], lightest: i32, default: i32, heaviest: i32) -> Vec<u8> {
    let mut out = Written::default();
    out.short(1) // major version
        .short(0) // minor
        .short(16) // where the axes start
        .short(2) // reserved, and the specification says two
        .short(1) // one axis
        .short(20) // each of which is twenty bytes
        .short(0) // no named instances
        .short(8); // which would be this size if there were
    out.0.extend_from_slice(&tag);
    out.fixed(lightest)
        .fixed(default)
        .fixed(heaviest)
        .short(0) // flags: not hidden
        .short(17); // the axis's name, in the name table
    out.0
}

/// An `HVAR` table giving every glyph the same two deltas: one for the light
/// half of the axis and one for the heavy half.
///
/// Two regions rather than one because one is not expressible. A region that
/// starts below the default and ends above it, with a peak that is not the
/// default, is what the specification calls invalid — so an axis that varies
/// both ways is two regions that meet at the default, and this writes both.
fn hvar(glyphs: usize, light: i16, heavy: i16) -> Vec<u8> {
    /// Where the item variation store starts within the table.
    const STORE: u32 = 20;
    /// The store's own header, after which the region list begins.
    const REGIONS: u32 = 12;
    /// A region list of one axis and two regions.
    const REGIONS_LENGTH: u32 = 4 + 2 * 6;
    /// The most negative and most positive a normalised coordinate goes.
    const LOWEST: i16 = -16384;
    const HIGHEST: i16 = 16384;

    let mut out = Written::default();
    out.word(0x0001_0000) // version 1.0
        .word(STORE)
        .word(0) // no advance mapping: a glyph's id is its row
        .word(0) // no left side bearings
        .word(0); // no right ones

    out.short(1) // the store's format
        .word(REGIONS)
        .short(1) // one lot of variation data
        .word(REGIONS + REGIONS_LENGTH);

    out.short(1) // one axis
        .short(2); // two regions on it
    // The light half: full effect at the lightest weight, none at the default.
    out.signed(LOWEST).signed(LOWEST).signed(0);
    // And the heavy half, the other way about.
    out.signed(0).signed(HIGHEST).signed(HIGHEST);

    out.short(u16::try_from(glyphs).unwrap_or(u16::MAX))
        .short(2) // both deltas in a row are two bytes wide
        .short(2) // and there are two of them
        .short(0) // the first names the light region
        .short(1); // the second the heavy one
    for _ in 0..glyphs {
        out.signed(light).signed(heavy);
    }
    out.0
}

/// A real font made variable: the same outlines, plus an axis and the deltas
/// along it.
fn variable() -> Vec<u8> {
    variable_with(&fvar(
        *b"wght",
        i32::from(LIGHTEST),
        i32::from(DEFAULT),
        i32::from(HEAVIEST),
    ))
}

/// The same, with an `fvar` table of the caller's own.
fn variable_with(axes: &[u8]) -> Vec<u8> {
    let base = dejavu::sans::regular().to_vec();
    let glyphs = glyph_count(&base);
    let with_deltas = carrying(
        base,
        SPARE_FOR_HVAR,
        HVAR,
        &hvar(glyphs, AT_THE_LIGHT_END, AT_THE_HEAVY_END),
    );
    carrying(with_deltas, SPARE_FOR_FVAR, FVAR, axes)
}

/// The font under test, loaded and filed at the weight its `OS/2` states —
/// which is what the browser process does with every face it finds.
///
/// [`None`] rather than a panic, because this is not a test function and the
/// lints hold it to what any other code is held to: the assertion that asked
/// for it reports what it wanted.
fn loaded(family: &str) -> Option<Font> {
    Font::load(family, Weight::new(DEFAULT), Slant::Normal, variable())
}

// --- What a font says about its range ----------------------------------------

#[test]
fn a_font_with_an_axis_reports_the_range_rather_than_the_one_instance() {
    let style = style_in(&variable()).expect("a font this engine can read");
    assert_eq!(
        style.weight,
        Weight::new(DEFAULT),
        "the default instance is still what OS/2 states, and still read",
    );
    let axis = style.axis.expect("a font that declares a weight axis");
    assert_eq!(axis.lightest(), Weight::new(LIGHTEST));
    assert_eq!(axis.heaviest(), Weight::new(HEAVIEST));
    assert!(axis.covers(Weight::new(700)));
    assert!(!axis.covers(Weight::new(100)));
    assert_eq!(axis.nearest(Weight::new(700)), Weight::new(700));
    assert_eq!(
        axis.nearest(Weight::new(1000)),
        Weight::new(HEAVIEST),
        "a weight past the end of the axis is the end of the axis",
    );
}

#[test]
fn an_ordinary_face_declares_no_axis_and_is_unchanged_by_all_of_this() {
    for data in [
        dejavu::sans::regular(),
        dejavu::sans::bold(),
        dejavu::sans::extra_light(),
        dejavu::serif::italic(),
    ] {
        let style = style_in(data).expect("a font this repository ships with");
        assert_eq!(style.axis, None, "a static face is one weight");
    }
    let font = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    )
    .expect("the font this crate is tested with");
    assert_eq!(font.weight_axis(), None);
    assert_eq!(
        font.at_weight(Weight::BOLD).weight(),
        Weight::NORMAL,
        "a face that cannot be set to bold stays what it is rather than lying",
    );
    assert_eq!(format!("{font:?}"), "Font(DejaVu Sans 400 Normal)");
}

#[test]
fn a_variable_font_says_which_instance_it_is() {
    let font = loaded("Continuum").expect("a font this engine can load");
    assert_eq!(
        format!("{font:?}"),
        "Font(Continuum 400 Normal of 200..900)",
        "a weight alone does not say whether it was a face or an instance",
    );
    assert_eq!(
        format!("{:?}", font.at_weight(Weight::new(700))),
        "Font(Continuum 700 Normal of 200..900)",
    );
}

// --- What a request gets -----------------------------------------------------

#[test]
fn one_file_answers_a_request_for_every_weight_it_covers() {
    let mut database = FontDatabase::new();
    database.add(loaded("Continuum").expect("a font this engine can load"));

    for wanted in [200u16, 300, 400, 700, 900] {
        let request = FontRequest {
            families: vec!["Continuum".to_owned()],
            weight: Weight::new(wanted),
            slant: Slant::Normal,
        };
        let given = database
            .chain(&request)
            .into_iter()
            .next()
            .expect("the only font there is");
        assert_eq!(
            given.weight(),
            Weight::new(wanted),
            "a request for {wanted} was answered at another weight",
        );
    }

    // And a weight past the end of the axis is the end of it, rather than a
    // refusal: a page asking for 950 wants the heaviest thing there is.
    let request = FontRequest {
        families: vec!["Continuum".to_owned()],
        weight: Weight::new(1000),
        slant: Slant::Normal,
    };
    let given = database
        .chain(&request)
        .into_iter()
        .next()
        .expect("the only font there is");
    assert_eq!(given.weight(), Weight::new(HEAVIEST));
}

#[test]
fn a_variable_face_is_preferred_to_a_static_one_at_the_wrong_weight() {
    // The rule this is really testing is the distance: a face's distance from a
    // request is to what it can *be*, not to what it is. Reading it the old way
    // put this family's one static face — filed at 400, whatever was asked —
    // against a variable one filed at 400 as well, and the two tied.
    let mut database = FontDatabase::new();
    database.add(
        Font::load(
            "Continuum",
            Weight::new(DEFAULT),
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("a static face"),
    );
    database.add(loaded("Continuum").expect("a font this engine can load"));

    let request = FontRequest {
        families: vec!["Continuum".to_owned()],
        weight: Weight::new(HEAVIEST),
        slant: Slant::Normal,
    };
    let given = database
        .chain(&request)
        .into_iter()
        .next()
        .expect("a font for the request");
    assert_eq!(
        given.weight_axis().map(alo_text::WeightAxis::heaviest),
        Some(Weight::new(HEAVIEST)),
        "the static face was five hundred away and the variable one was here",
    );
    assert_eq!(given.weight(), Weight::new(HEAVIEST));
}

// --- The numbers -------------------------------------------------------------

/// How wide this text is when the page asks for one weight of the family.
fn width_at(font: &Font, weight: u16) -> f32 {
    let mut database = FontDatabase::new();
    database.add(font.clone());
    let request = FontRequest {
        families: vec![font.family().to_owned()],
        weight: Weight::new(weight),
        slant: Slant::Normal,
    };
    measure_unwrapped(TEXT, &database, &request, SIZE).width()
}

/// Text with one glyph per character in this font, so the delta per glyph and
/// the delta for the line are the same arithmetic.
const TEXT: &str = "Invoices";

/// How many glyphs that is, asked of the shaper rather than assumed from the
/// characters — the rule the whole crate is built on.
fn glyphs_of_the_text(font: &Font) -> f32 {
    let run = alo_text::shape(TEXT, font, SIZE, Direction::LeftToRight);
    let count = u16::try_from(run.glyphs.len()).unwrap_or(u16::MAX);
    f32::from(count)
}

#[test]
fn two_weights_of_one_variable_family_are_two_widths_of_text() {
    // The layout assertion. Which weight a page is given decides how wide its
    // text is and so where every line of it breaks, and before this item one
    // file could only ever be drawn at one width.
    let font = loaded("Continuum").expect("a font this engine can load");
    let lightest = width_at(&font, LIGHTEST);
    let default = width_at(&font, DEFAULT);
    let heavier = width_at(&font, 700);
    let heaviest = width_at(&font, HEAVIEST);

    assert!(
        lightest < default && default < heavier && heavier < heaviest,
        "the axis was not read as a position on it: \
         {lightest} {default} {heavier} {heaviest}",
    );

    // At the ends of the axis the delta is the whole of what the font states,
    // so the numbers are exact rather than interpolated — which is what makes
    // them worth asserting to the pixel. Every advance is an integer number of
    // font units and the scale is a power of two, so nothing here rounds.
    let glyphs = glyphs_of_the_text(&font);
    let per_unit = SIZE / UNITS_PER_EM;
    assert!(
        (heaviest - default - glyphs * f32::from(AT_THE_HEAVY_END) * per_unit).abs() < 0.001,
        "at the heavy end the line should be {} wider, and it is {}",
        glyphs * f32::from(AT_THE_HEAVY_END) * per_unit,
        heaviest - default,
    );
    assert!(
        (lightest - default - glyphs * f32::from(AT_THE_LIGHT_END) * per_unit).abs() < 0.001,
        "at the light end the line should be {} narrower, and it is {}",
        glyphs * f32::from(AT_THE_LIGHT_END) * per_unit,
        default - lightest,
    );

    // And the weight between them is between them rather than snapped to one:
    // 700 is three fifths of the way from the default to the heaviest.
    let expected = default + 0.6 * (heaviest - default);
    assert!(
        (heavier - expected).abs() < 0.05,
        "700 was drawn at {heavier} where the axis puts it at {expected}",
    );
}

#[test]
fn a_page_that_asks_for_a_weight_this_font_cannot_reach_still_gets_the_font() {
    // Past either end the axis stops, and so does the width. A page asking for
    // 1000 of a family that goes to 900 is drawn at 900 — not refused, and not
    // extrapolated into a face the designer never drew.
    let font = loaded("Continuum").expect("a font this engine can load");
    assert!((width_at(&font, 1000) - width_at(&font, HEAVIEST)).abs() < 0.001);
    assert!((width_at(&font, 1) - width_at(&font, LIGHTEST)).abs() < 0.001);
    // Stopping at the end is not the same as never having moved, which is what
    // this assertion is here to tell apart.
    assert!((width_at(&font, 1000) - width_at(&font, DEFAULT)).abs() > 0.5);
    assert!((width_at(&font, 1) - width_at(&font, DEFAULT)).abs() > 0.5);
}

// --- The bytes are somebody else's -------------------------------------------

#[test]
fn an_axis_that_is_not_the_weight_one_is_read_past_rather_than_guessed_at() {
    // `wdth`, `slnt` and `opsz` are separate CSS properties with grammars of
    // their own. A font is not narrower because this engine assumed an axis it
    // had not looked at.
    for tag in [b"wdth", b"slnt", b"opsz", b"ital"] {
        let font = variable_with(&fvar(*tag, 50, 100, 200));
        assert_eq!(
            style_in(&font).and_then(|style| style.axis),
            None,
            "{} was read as a weight axis",
            String::from_utf8_lossy(tag),
        );
    }
}

#[test]
fn an_axis_of_no_width_is_a_static_face_spelt_at_greater_length() {
    // Both ends on one weight. Calling it variable would make it a candidate
    // for every request while it can only ever draw the one thing.
    let font = variable_with(&fvar(*b"wght", 400, 400, 400));
    assert_eq!(style_in(&font).and_then(|style| style.axis), None);
}

#[test]
fn an_axis_older_than_the_scale_it_is_supposed_to_be_in_is_not_read_as_weight() {
    // This machine's `Skia.ttf` runs from 1 to 3 — an Apple axis from before
    // `wght` had a shared meaning — and its `OS/2` states 5. Read as CSS
    // numbers the whole axis is hairline, so every request would land on its
    // heaviest end and a page of ordinary text would come out black.
    let font = variable_with(&fvar(*b"wght", 1, 2, 3));
    assert_eq!(
        style_in(&font).and_then(|style| style.axis),
        None,
        "an axis nothing on CSS's scale could mean was read as if it did",
    );

    // And the line is where CSS stops having a word for a weight, not anywhere
    // near where a real axis starts: `thin` to `black` is read.
    let real = variable_with(&fvar(*b"wght", 100, 400, 900));
    assert_eq!(
        style_in(&real).and_then(|style| style.axis.map(alo_text::WeightAxis::lightest)),
        Some(Weight::new(100)),
    );
}

#[test]
fn an_axis_written_backwards_is_not_a_reason_to_lose_the_font() {
    // The bounds come out of somebody else's file and nothing stops them being
    // the wrong way round. Whatever is made of them, the font is still a font
    // and its default instance is still readable.
    let font = variable_with(&fvar(*b"wght", 900, 400, 200));
    let style = style_in(&font).expect("a font this engine can read");
    assert_eq!(style.weight, Weight::new(DEFAULT));
    assert!(
        style
            .axis
            .is_none_or(|axis| axis.lightest() < axis.heaviest()),
        "an axis was kept with its ends the wrong way round",
    );
}

#[test]
fn a_variation_table_that_lies_about_itself_is_an_answer_rather_than_a_crash() {
    // Each of these is a file somebody could really hand a browser: a font
    // truncated by a download that stopped, one written to be awkward, one with
    // a single bit flipped. The engine must answer something about every one of
    // them, because the alternative in a renderer is a tab that disappears.
    let axes = fvar(
        *b"wght",
        i32::from(LIGHTEST),
        i32::from(DEFAULT),
        i32::from(HEAVIEST),
    );

    for cut in 0..axes.len() {
        let font = variable_with(axes.get(..cut).unwrap_or_default());
        let _ = style_in(&font);
        let _ = alo_text::Font::load("Cut", Weight::NORMAL, Slant::Normal, font);
    }

    let mut flipped = axes.clone();
    for byte in 0..flipped.len() {
        for bit in 0..8u8 {
            if let Some(slot) = flipped.get_mut(byte) {
                *slot ^= 1 << bit;
            }
            let font = variable_with(&flipped);
            let _ = style_in(&font);
            if let Some(loaded) = Font::load("Bent", Weight::NORMAL, Slant::Normal, font) {
                // Whatever the table said, drawing with it must still terminate
                // and still produce a number.
                let run = alo_text::shape("Ab", &loaded, SIZE, Direction::LeftToRight);
                assert!(run.width.is_finite());
            }
            if let Some(slot) = flipped.get_mut(byte) {
                *slot ^= 1 << bit;
            }
        }
    }

    // Nothing above may be believed, and this is the assertion the whole test
    // hangs on: the sound table still reads, so what came back from the damaged
    // ones was the engine refusing rather than the engine failing to look.
    assert_eq!(
        style_in(&variable_with(&axes))
            .and_then(|style| style.axis.map(alo_text::WeightAxis::heaviest)),
        Some(Weight::new(HEAVIEST)),
    );
}

#[test]
fn deltas_that_lie_are_an_answer_rather_than_a_crash() {
    // The other table: `HVAR` says how far a glyph's advance moves, and its
    // rows are indexed by glyph id. A table claiming fewer rows than the font
    // has glyphs is the ordinary way for this to be wrong.
    let base = dejavu::sans::regular().to_vec();
    for rows in [0usize, 1, 8, glyph_count(&base) / 2] {
        let with_deltas = carrying(
            base.clone(),
            SPARE_FOR_HVAR,
            HVAR,
            &hvar(rows, AT_THE_LIGHT_END, AT_THE_HEAVY_END),
        );
        let font = carrying(
            with_deltas,
            SPARE_FOR_FVAR,
            FVAR,
            &fvar(
                *b"wght",
                i32::from(LIGHTEST),
                i32::from(DEFAULT),
                i32::from(HEAVIEST),
            ),
        );
        let loaded = Font::load("Short", Weight::new(DEFAULT), Slant::Normal, font)
            .expect("a font this engine can load");
        let run = alo_text::shape(
            TEXT,
            &loaded.at_weight(Weight::new(HEAVIEST)),
            SIZE,
            Direction::LeftToRight,
        );
        assert!(
            run.width.is_finite() && run.width > 0.0,
            "a table of {rows} rows produced a line of {}",
            run.width,
        );
    }
}
