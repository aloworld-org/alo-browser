/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 853 and 854 (ADR 0084): the four faces the interface now
//! carries besides Instrument Sans and Newsreader — Inter, its words'
//! face, and Geist, DM Sans and Plus Jakarta Sans, which a person may
//! choose in *Settings* — set as Chrome sets them, at every weight and size
//! the interface uses and around them.
//!
//! Two of them vary by optical size: Inter along `opsz` 14 to 32, its
//! default 14, and DM Sans along 9 to 40, its default 9. Nothing was
//! assumed of either: Chrome was measured, and with `font-optical-sizing:
//! auto` — the initial value — it sets each to the CSS pixels the text is
//! set at, clamped to the axis, as it sets Newsreader's (queue item 752),
//! and with `none` leaves each at its default; this engine does the same
//! (`alo_text::Font::optical_size`).
//!
//! Every Chrome number here was measured on this Mac on 7 November 2026 in
//! Chrome 157.0.8103.38, headless, from the interface's own font files as
//! web fonts, after `document.fonts.load` resolved: a glyph's advance as a
//! canvas's `measureText`, and a run's width as a `white-space: pre` span's
//! `getBoundingClientRect().width` — Chrome's layout unit, a 64th of a pixel
//! — frozen in `references/chosen-faces-runs.chrome.txt`. Chrome's source
//! was not read.

use alo_renderer::typefaces::{self, DM_SANS, INTER};
use alo_text::{Direction, FontDatabase, FontRequest, OpticalSizing, Weight};
use std::path::Path;

/// The interface's typefaces, as a renderer is handed them.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for font in typefaces::fonts() {
        database.add(font);
    }
    database
}

/// How wide `text` is in `family` at `weight` and `size`, as this engine
/// shapes it — a run measured whole, as a line measures its text.
fn shaped(
    database: &FontDatabase,
    (family, weight, size): (&str, u16, f32),
    sizing: OpticalSizing,
    text: &str,
) -> Option<f32> {
    let request = FontRequest {
        families: vec![family.to_owned()],
        weight: Weight::new(weight),
        optical_sizing: sizing,
        ..FontRequest::default()
    };
    let font = database.chain(&request).into_iter().next()?;
    Some(alo_text::shape::shape(text, &font, size, Direction::LeftToRight).width)
}

/// Chrome 157's canvas advance of each glyph at 400, by size: below
/// Inter's axis (9, 12, 13.5), at its ends (14, 32), between (20, 24), and
/// past it (40).
const INTER_AT: [(f32, [(char, f32); 5]); 8] = [
    (
        9.0,
        [
            ('o', 5.396_484),
            ('a', 5.053_711),
            ('G', 6.714_844),
            ('n', 5.317_383),
            (' ', 2.531_25),
        ],
    ),
    (
        12.0,
        [
            ('o', 7.195_313),
            ('a', 6.738_281),
            ('G', 8.953_125),
            ('n', 7.089_844),
            (' ', 3.375),
        ],
    ),
    (
        13.5,
        [
            ('o', 8.094_727),
            ('a', 7.580_566),
            ('G', 10.072_266),
            ('n', 7.976_074),
            (' ', 3.796_875),
        ],
    ),
    (
        14.0,
        [
            ('o', 8.394_531),
            ('a', 7.861_328),
            ('G', 10.445_313),
            ('n', 8.271_484),
            (' ', 3.937_5),
        ],
    ),
    (
        20.0,
        [
            ('o', 11.653_656),
            ('a', 10.937_515),
            ('G', 14.817_703),
            ('n', 11.523_453),
            (' ', 5.416_672),
        ],
    ),
    (
        24.0,
        [
            ('o', 13.713_547),
            ('a', 12.890_625),
            ('G', 17.697_922),
            ('n', 13.593_75),
            (' ', 6.333_344),
        ],
    ),
    (
        32.0,
        [
            ('o', 17.562_5),
            ('a', 16.562_5),
            ('G', 23.375),
            ('n', 17.5),
            (' ', 8.0),
        ],
    ),
    (
        40.0,
        [
            ('o', 21.953_125),
            ('a', 20.703_125),
            ('G', 29.218_75),
            ('n', 21.875),
            (' ', 10.0),
        ],
    ),
];

/// The same of DM Sans, whose axis runs from 9 to 40.
const DM_SANS_AT: [(f32, [(char, f32); 5]); 8] = [
    (
        9.0,
        [
            ('o', 5.372_986),
            ('a', 4.959),
            ('G', 6.857_986),
            ('n', 5.201_996),
            (' ', 2.420_99),
        ],
    ),
    (
        12.0,
        [
            ('o', 7.136_993),
            ('a', 6.563_4),
            ('G', 9.116_989),
            ('n', 6.903_595),
            (' ', 3.206_39),
        ],
    ),
    (
        13.5,
        [
            ('o', 8.013_931),
            ('a', 7.356_491),
            ('G', 10.241_44),
            ('n', 7.748_322),
            (' ', 3.595_047),
        ],
    ),
    (
        14.0,
        [
            ('o', 8.305_496),
            ('a', 7.619_507),
            ('G', 10.615_494),
            ('n', 8.029_007),
            (' ', 3.723_999),
        ],
    ),
    (
        20.0,
        [
            ('o', 11.849_991),
            ('a', 10.857_986),
            ('G', 15.149_994),
            ('n', 11.451_996),
            (' ', 5.307_983),
        ],
    ),
    (
        24.0,
        [
            ('o', 14.207_993),
            ('a', 13.007_996),
            ('G', 18.168),
            ('n', 13.727_997),
            (' ', 6.359_985),
        ],
    ),
    (
        32.0,
        [
            ('o', 18.207_993),
            ('a', 16.655_99),
            ('G', 23.807_999),
            ('n', 17.472),
            (' ', 7.439_987),
        ],
    ),
    (
        40.0,
        [
            ('o', 21.839_996),
            ('a', 19.959_991),
            ('G', 29.239_99),
            ('n', 20.799_988),
            (' ', 8.0),
        ],
    ),
];

#[test]
fn each_glyphs_advance_along_inters_and_dm_sans_optical_size_is_chromes() {
    let database = fonts();
    for (family, sizes) in [(INTER, &INTER_AT), (DM_SANS, &DM_SANS_AT)] {
        for (size, glyphs) in sizes {
            for (glyph, chrome) in glyphs {
                let face = (family, 400, *size);
                let ours = shaped(&database, face, OpticalSizing::Auto, &glyph.to_string())
                    .unwrap_or(f32::NAN);
                assert!(
                    (ours - chrome).abs() < 1e-3,
                    "{family} {glyph:?} at {size}: {ours} where Chrome's is {chrome}"
                );
            }
        }
    }
    // The axis is what moved them: Inter's `o` at 32 with `none` is its 14
    // cut's, wider by more than a pixel — so a face left at its default
    // would be drawn visibly otherwise than Chrome draws it.
    let none = shaped(&database, (INTER, 400, 32.0), OpticalSizing::None, "o").unwrap_or(0.0);
    let auto = shaped(&database, (INTER, 400, 32.0), OpticalSizing::Auto, "o").unwrap_or(0.0);
    assert!(none - auto > 1.0, "{none} and {auto}");
}

/// One frozen run: its face, sizing, text and Chrome's width in 64ths.
struct Run<'a> {
    face: (&'a str, u16, f32),
    sizing: OpticalSizing,
    text: &'a str,
    chrome: i64,
}

fn runs(frozen: &str) -> Vec<Run<'_>> {
    frozen
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.split('|');
            let family = parts.next()?;
            let weight = parts.next()?.parse().ok()?;
            let size = parts.next()?.parse().ok()?;
            let sizing = match parts.next()? {
                "none" => OpticalSizing::None,
                _ => OpticalSizing::Auto,
            };
            let text = parts.next()?;
            let chrome = parts.next()?.parse().ok()?;
            Some(Run {
                face: (family, weight, size),
                sizing,
                text,
                chrome,
            })
        })
        .collect()
}

#[test]
fn sixteen_hundred_runs_in_the_four_faces_are_chromes_to_its_layout_unit() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/references/chosen-faces-runs.chrome.txt");
    let frozen = std::fs::read_to_string(&path).unwrap_or_default();
    let runs = runs(&frozen);
    assert_eq!(runs.len(), 1600, "{}", path.display());
    let database = fonts();
    let (mut exact, mut worst) = (0, 0);
    let mut families = std::collections::BTreeMap::<&str, usize>::new();
    for run in &runs {
        let ours = shaped(&database, run.face, run.sizing, run.text).unwrap_or(f32::NAN);
        // On Chrome's grid, a run's width rounded up (ADR 0054 § 4).
        let units = alo_layout::layout_unit::rounded_up(ours, 1.0) * 64.0;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a run's width in 64ths is far inside i64, and whole on the grid"
        )]
        let units = units.round() as i64;
        let off = (units - run.chrome).abs();
        assert!(
            off <= 1,
            "{:?} {:?} {:?}: {units} where Chrome's is {}",
            run.face,
            run.sizing,
            run.text,
            run.chrome
        );
        exact += i32::from(off == 0);
        worst = worst.max(off);
        *families.entry(run.face.0).or_default() += 1;
    }
    assert_eq!(
        families.into_iter().collect::<Vec<_>>(),
        vec![
            ("DM Sans", 500),
            ("Geist", 300),
            ("Inter Variable", 500),
            ("Plus Jakarta Sans", 300)
        ]
    );
    // The 30 a 64th apart are each within 0.28 of a 64th of the grid's
    // edge: Inter's and Geist's at most 0.13 short of it, and DM Sans' at 28
    // and 32 px up to 0.28 past it — what
    // ADR 0097 left of Newsreader and Instrument Sans too, consistent with
    // the axis's coordinate as `ttf-parser` cuts it to a 16,384th and a sum
    // in `f32`.
    assert!(exact >= 1570, "{exact} of 1600 exact");
    assert!(worst <= 1);
}
