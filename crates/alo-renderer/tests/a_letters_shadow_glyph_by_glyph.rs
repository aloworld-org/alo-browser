/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 831: plain letters' `text-shadow` softened glyph by glyph, as
//! Chrome draws it.
//!
//! A face made for this test with `fontTools` 4.61 (1000 units to the em,
//! ascent 800, descent 200), under this repository's licence:
//! `crates/alo-paint/tests/fonts/shadowed.ttf`, 740 bytes — `A` a box
//! 100–900 across and 0–700 up, `W` a box −300–1300 by 0–700 that
//! reaches 300 units into the glyph on each side of it, each a whole em
//! wide, side bearings their outlines' own.
//!
//! **Measured** in Chrome 157.0.8103.38 (`--version`), headless, its
//! default raster path, at one and two: each glyph's shadow is softened on
//! its own and the shadows are drawn over each other in the shadow's
//! colour. Two `A`s 8 px apart, blurred 16, meet at 129 of 255 in black
//! (this engine, blurring the run's outlines as one shape, drew 110; now
//! 132, the rest the blur's own shape — a box blur's is not Chrome's, as
//! queue item 786 measured); and where two `W`s overlap, a hard shadow at
//! half opacity is 63 — darker than the 127 of one, as two halves over each
//! other are — where one shape was 128 throughout.
//!
//! Chrome draws translucent **letters** glyph by glyph too (two `W`s at
//! half opacity, 63 where they overlap); this engine fills a run's letters
//! as one shape, and that is queued apart (queue item 834).
//!
//! What closes it: the shadow in numbers (one shadow, a shape per glyph),
//! the pixels named above, and every pixel of the page Chrome's — the hard
//! row to a level, the blurred rows within the blur's own difference.

use alo_layout::Size;
use alo_paint::{Canvas, DisplayItem, from_png};
use alo_renderer::Screen;
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::Path;

/// Four rows of 140, each text starting at 20 with its baseline 42 down
/// the row: two `A`s blurred 16 in black, the same at half opacity, two
/// overlapping `W`s hard at half opacity, the same blurred 16.
const PAGE: &str = "<!DOCTYPE html><html><body>\
<p style='text-shadow: 0 50px 16px #000000'>AA</p>\
<p style='text-shadow: 0 50px 16px rgba(0, 0, 0, 0.5)'>AA</p>\
<p style='text-shadow: 0 50px 0 rgba(0, 0, 0, 0.5)'>WW</p>\
<p style='text-shadow: 0 50px 16px rgba(0, 0, 0, 0.5)'>WW</p>\
</body></html>";

const STYLE: &str = "body { margin: 0; background: #ffffff; width: 300px }
p { margin: 0; padding-left: 20px; height: 140px; font-size: 40px; line-height: 60px; color: #2a9d8f;
  font-family: 'alo Shadow Test'; white-space: pre }";

const ROW: u32 = 140;

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "alo Shadow Test",
        Weight::NORMAL,
        Slant::Normal,
        include_bytes!("../../alo-paint/tests/fonts/shadowed.ttf").to_vec(),
    ) {
        database.add(font);
    }
    database
}

fn drawn(scale: f32) -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(300.0, 560.0)).at_scale(scale),
        &fonts(),
        &[],
        &[],
    )
}

fn grey(canvas: &Canvas, x: u32, y: u32) -> Option<u8> {
    canvas.at(x, y).map(|colour| colour.to_rgba8().0)
}

fn read(name: &str) -> Option<Canvas> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name);
    std::fs::read(path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
}

#[test]
fn each_row_casts_one_shadow_of_a_shape_per_glyph() {
    let rendered = drawn(1.0);
    // Each row one run, its shadow drawn with it: one text item, one shadow.
    let runs: Vec<(String, (f32, f32), usize)> = rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Text {
                text,
                origin,
                shadows,
                ..
            } => Some((text.clone(), *origin, shadows.len())),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 4, "{runs:?}");
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    let Some(font) = fonts()
        .chain(&alo_text::FontRequest::family("alo Shadow Test"))
        .into_iter()
        .next()
    else {
        panic!("the test face loads");
    };
    for (row, (text, origin, shadows)) in runs.iter().enumerate() {
        assert_eq!(*shadows, 1, "row {row}");
        // The shapes its shadow is cast from: one per glyph, 14–42 down the
        // row, and across 24–56 and 64–96 for `A`s, 8–72 and 48–112 for
        // the `W`s, which overlap.
        let glyphs = alo_paint::render::outlined_glyphs_at(text, &font, 40.0, 1.0, 0.0, *origin);
        let bounds: Vec<(f32, f32, f32, f32)> =
            glyphs.iter().filter_map(alo_paint::Path::bounds).collect();
        let across: &[(f32, f32)] = if row < 2 {
            &[(24.0, 56.0), (64.0, 96.0)]
        } else {
            &[(8.0, 72.0), (48.0, 112.0)]
        };
        assert_eq!(bounds.len(), 2, "row {row}");
        let top = f32::from(u16::try_from(row).unwrap_or(0)) * 140.0;
        for ((left, upper, right, lower), (from, to)) in bounds.iter().zip(across) {
            assert!(
                close(*left, *from) && close(*right, *to),
                "row {row}: {left}..{right}"
            );
            assert!(
                close(*upper, top + 14.0) && close(*lower, top + 42.0),
                "row {row}: {upper}..{lower}"
            );
        }
    }
}

#[test]
fn soft_edges_add_where_they_meet_and_halves_darken_where_they_overlap() {
    for scale in [1_u32, 2] {
        #[expect(clippy::cast_precision_loss, reason = "one or two")]
        let rendered = drawn(scale as f32);
        let canvas = &rendered.drawing.canvas;
        let at = |row: u32, x: u32, y: u32| grey(canvas, x * scale, (row * ROW + y) * scale);
        // Between the two `A`s, blurred 16: Chrome 157 draws 129 at one
        // and two; one shape blurred was 110.
        let meet = at(0, 60, 78).unwrap_or(0);
        assert!((126..=133).contains(&meet), "{meet} at {scale}");
        // Two hard halves over each other where the `W`s overlap; one where
        // they do not. Chrome: 63 and 127.
        let overlap = at(2, 60, 78).unwrap_or(0);
        let single = at(2, 30, 78).unwrap_or(0);
        assert!((63..=64).contains(&overlap), "{overlap} at {scale}");
        assert!((127..=128).contains(&single), "{single} at {scale}");
    }
}

/// Every pixel against Chrome's: the hard row to a level; the blurred rows
/// within 4 at one and 7 at two (a box blur is not Chrome's — one shape
/// blurred was 22 and 23 off at the meeting point alone).
#[test]
fn every_pixel_is_chromes_within_the_blurs_difference() {
    for (scale, name, soft) in [
        (1.0_f32, "chrome-letters-shadows.png", 4_u8),
        (2.0, "chrome-letters-shadows-at-2.png", 7),
    ] {
        let rendered = drawn(scale);
        let canvas = &rendered.drawing.canvas;
        let Some(chrome) = read(name) else {
            panic!("Chrome's picture {name} is beside the references");
        };
        assert_eq!(
            (chrome.width(), chrome.height()),
            (canvas.width(), canvas.height())
        );
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a row of 140 at one or two"
        )]
        let row_height = (140.0 * scale) as u32;
        for y in 0..canvas.height() {
            let bound = if y / row_height == 2 { 1 } else { soft };
            for x in 0..canvas.width() {
                let (Some(ours), Some(theirs)) = (canvas.at(x, y), chrome.at(x, y)) else {
                    continue;
                };
                let (ours, theirs) = (ours.to_rgba8(), theirs.to_rgba8());
                let far = [
                    ours.0.abs_diff(theirs.0),
                    ours.1.abs_diff(theirs.1),
                    ours.2.abs_diff(theirs.2),
                ]
                .into_iter()
                .max()
                .unwrap_or(0);
                assert!(
                    far <= bound,
                    "({x}, {y}) at {scale}: {ours:?} where Chrome {theirs:?}"
                );
            }
        }
    }
}
