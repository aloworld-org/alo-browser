/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 795 and 796 (ADR 0097), with 797: the interface's variable
//! faces — Instrument Sans along `wght` 400 to 700, Newsreader along `wght`
//! 200 to 800 and `opsz` 6 to 72 — set as Chrome 156 sets them, at every
//! weight and size, not only at the faces' masters.
//!
//! The rented shaper, `rustybuzz` 0.20.1, gives a varied glyph's advance in
//! whole font units and leaves a kerning pair's variation out where the
//! pair is listed glyph by glyph (its parser, `ttf-parser` 0.25.1, reads the
//! offset of the pair's variation from the wrong place) and rounds it where
//! the pair is kerned by classes. `alo-text` gives back both (`advance.rs`,
//! `pair_variation.rs`).
//!
//! Every Chrome number here was measured on this Mac on 2 November 2026 in
//! Chrome 156.0.8078.76, headless, from the interface's own font files as web
//! fonts, after `document.fonts.load` resolved: a glyph's advance as a
//! canvas's `measureText`, and a run's width as a `white-space: pre` span's
//! `getBoundingClientRect().width` — Chrome's layout unit, a 64th of a pixel
//! — frozen in `references/varied-runs.chrome.txt`. Chrome's source was not
//! read.

use alo_renderer::typefaces;
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
/// shapes it — a run measured whole, as a line now measures its text.
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

const AT_600: [(char, f32); 27] = [
    ('G', 12.218_658),
    ('o', 9.557_327),
    ('d', 9.930_664),
    ('a', 8.933_334),
    ('f', 5.919_998),
    ('t', 6.501_343),
    ('e', 9.130_661),
    ('r', 6.245_331),
    ('n', 9.797_333),
    ('A', 11.733_322),
    ('.', 4.400_009),
    (',', 4.400_009),
    ('i', 4.160_004),
    ('v', 8.581_329),
    ('w', 12.794_678),
    ('x', 9.392_014),
    ('y', 8.581_329),
    ('z', 8.213_333),
    ('S', 10.197_342),
    ('T', 10.624_008),
    ('O', 12.703_995),
    ('P', 10.666_672),
    ('h', 9.797_333),
    ('l', 4.160_004),
    ('m', 15.050_674),
    ('q', 9.930_664),
    (' ', 3.093_323),
];
const AT_500: [(char, f32); 27] = [
    ('G', 12.229_324),
    ('o', 9.450_653),
    ('d', 9.813_324),
    ('a', 8.730_652),
    ('f', 5.791_992),
    ('t', 6.266_647),
    ('e', 9.077_316),
    ('r', 6.122_65),
    ('n', 9.690_659),
    ('A', 11.690_659),
    ('.', 4.239_99),
    (',', 4.239_99),
    ('i', 3.999_985),
    ('v', 8.474_655),
    ('w', 12.533_31),
    ('x', 9.103_973),
    ('y', 8.474_655),
    ('z', 8.074_646),
    ('S', 9.962_646),
    ('T', 10.495_987),
    ('O', 12.639_984),
    ('P', 10.581_314),
    ('h', 9.690_659),
    ('l', 3.999_985),
    ('m', 14.901_321),
    ('q', 9.813_324),
    (' ', 3.146_667),
];

#[test]
fn every_glyphs_advance_at_500_and_600_is_chromes_to_a_thousandth() {
    // Neither weight is a master of Instrument Sans: each advance is
    // `HVAR`'s fraction of a unit, which rounding to whole units put up to
    // 0.0054 px off at 16 px.
    let database = fonts();
    for (weight, glyphs) in [(600, &AT_600), (500, &AT_500)] {
        for (glyph, chrome) in glyphs {
            let face = (typefaces::INSTRUMENT_SANS, weight, 16.0);
            let ours = shaped(&database, face, OpticalSizing::Auto, &glyph.to_string());
            let ours = ours.unwrap_or(f32::NAN);
            assert!(
                (ours - chrome).abs() < 1e-3,
                "{glyph:?} at {weight}: {ours} where Chrome's is {chrome}"
            );
        }
    }
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
fn two_thousand_four_hundred_runs_are_chromes_to_its_layout_unit() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/references/varied-runs.chrome.txt");
    let frozen = std::fs::read_to_string(&path).unwrap_or_default();
    let runs = runs(&frozen);
    assert_eq!(runs.len(), 2400, "{}", path.display());
    let database = fonts();
    let (mut exact, mut worst) = (0, 0);
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
            "{:?} {:?}: {units} where Chrome's is {}",
            run.face,
            run.text,
            run.chrome
        );
        exact += i32::from(off == 0);
        worst = worst.max(off);
    }
    // With each run whole but the advances rounded and the pairs'
    // variations left out, 759 were, and the worst 354 units off (5.5 px,
    // Newsreader at 800 and 90 px). The 30 left are a 64th apart, each
    // within a quarter of a 64th of the grid's edge: the axis's coordinate
    // as `ttf-parser` cuts it, and a sum in `f32`.
    assert!(exact >= 2370, "{exact} of 2400 exact");
    assert!(worst <= 1);
}
