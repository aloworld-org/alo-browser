/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 786: a colour glyph's `text-shadow`, and a decoration line
//! under it — as Chrome 156 draws them.
//!
//! Two faces made for these tests with `fontTools` 4.61 (1000 units to the
//! em, ascent 800, descent 200, an underline 100 under the baseline and 50
//! thick), under this repository's licence, each with `A` a plain box
//! 100–900 across and 0–700 up and `B` a colour glyph, which U+1F642 is
//! too (and U+E000 in the layered face):
//!
//! - `crates/alo-paint/tests/fonts/pictured.ttf`, 1,060 bytes: `B` is a
//!   `CBDT` picture, one strike of 40 to the em, 40 × 40 pixels, its foot
//!   10 under the baseline — red and opaque across 8–32 down to 30, then
//!   under that teal and opaque across 5–15 and blue at half opacity
//!   across 25–35, clear everywhere else.
//! - `crates/alo-paint/tests/fonts/layered-deep.ttf`, 1,004 bytes: `B` is
//!   `COLR` version 0 — a leg 100–500 across, 200 under the baseline to
//!   750 over it, in palette red; a side 500–900, 0–750, in palette blue at
//!   half opacity; a foot 100–900, 0–100, in the text's colour — over a
//!   plain outline of its own, the whole em 200 under to 800 over.
//!
//! **Measured** in Chrome 156.0.8078.76 (checked with `--version`),
//! headless, at one, two and eight, the same page with the same faces as
//! web fonts and `DejaVu Sans`'s own file (its source not read):
//!
//! - A picture glyph's shadow is its picture's alpha in the shadow's
//!   colour — half where the picture is half, nothing where it is clear —
//!   and a layered glyph's is its layers as they are drawn over each other,
//!   never its own plain outline. A layer in the text's colour casts whole
//!   though the text is faint, as a letter does. Blurred, a glyph is one
//!   shape, softened apart from the letters beside it.
//! - An underline does **not** step aside for either: under a picture's
//!   clear pixels, and under the layered glyph's leg and own outline, it is
//!   drawn as with `text-decoration-skip-ink: none`; the glyph is drawn
//!   over it, and the line's shadow is drawn under the glyph's.
//!
//! What closes it: the shadows in numbers (what each casts), the pixels at
//! one and two, a reference render (`references/colour-glyph-shadows.png`),
//! and every plain pixel Chrome's at one and two
//! (`chrome-colour-glyph-shadows.png`, `…-at-2.png`).
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_colour_glyphs_shadow`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::Size;
use alo_paint::glyph_shadow::Cast;
use alo_paint::{Canvas, DisplayItem, from_png, to_png};
use alo_renderer::Screen;
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The page: twelve rows of 110, then two of 160 blurred far, each one
/// test. The picture glyph is
/// written as the emoji it is mapped to, and the layered one beside
/// `DejaVu Sans` as U+E000, which `DejaVu Sans` lacks: each so that the face
/// this engine takes for it is the face Chrome takes (a character meant as
/// a letter is not taken from a face of pictures while another face has it,
/// and an emoji is taken from a face of pictures first — queue item 829).
const PAGE: &str = "<!DOCTYPE html><html><body>\
<p class='cb hard'>A\u{1f642}A</p>\
<p class='cb soft'>A\u{1f642}A</p>\
<p class='cb u'>A\u{1f642}A</p>\
<p class='cb u n'>A\u{1f642}A</p>\
<p class='dp hard'>ABA</p>\
<p class='dp soft'>ABA</p>\
<p class='dp u'>ABA</p>\
<p class='dp u n'>ABA</p>\
<p class='cb u hard'>A\u{1f642}A</p>\
<p class='dp hard faint'>ABA</p>\
<p class='mcb soft'>a\u{1f642}a</p>\
<p class='mdp soft'>a\u{e000}a</p>\
<p class='cb wide'>A\u{1f642}</p>\
<p class='dp wide'>B</p>\
</body></html>";

/// Its style. Every row's text starts at 10 across and its baseline is 42
/// down the row: 60's half-leading over the faces' 40 is 10, then 32 of
/// ascent.
const STYLE: &str = "body { margin: 0; background: #ffffff; width: 300px }
p { margin: 0; padding-left: 10px; height: 110px; font-size: 40px; line-height: 60px;
  color: #2a9d8f }
.cb { font-family: 'alo Pictured Test' }
.dp { font-family: 'alo Layered Deep Test' }
.mcb { font-family: 'DejaVu Sans', 'alo Pictured Test' }
.mdp { font-family: 'DejaVu Sans', 'alo Layered Deep Test' }
.hard { text-shadow: 0 50px 0 #000000 }
.soft { text-shadow: 0 50px 4px #000000 }
.u { text-decoration-line: underline; text-decoration-color: #8338ec }
.n { text-decoration-skip-ink: none }
.faint { color: rgba(42, 157, 143, 0.3) }
.wide { height: 160px; text-shadow: 0 50px 16px #000000 }";

/// The page's size: a column of fourteen rows.
const WIDTH: f32 = 300.0;
const HEIGHT: f32 = 1640.0;

/// Where the two rows of 160 start.
const WIDE: f32 = 1320.0;

/// How far this engine's shadows blurred 16 are from Chrome's, at one and
/// two (a box blur is not Chrome's), measured.
const FAR_AT_ONE: u8 = 5;
const FAR_AT_TWO: u8 = 6;

/// A row's height, and its baseline's depth in it.
const ROW: f32 = 110.0;
const BASELINE: f32 = 42.0;

/// `B` starts one em along from the text's start at 10.
const B: f32 = 50.0;

fn face(family: &str, bytes: &[u8]) -> Option<Font> {
    Font::load(family, Weight::NORMAL, Slant::Normal, bytes.to_vec())
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for font in [
        face(
            "alo Pictured Test",
            include_bytes!("../../alo-paint/tests/fonts/pictured.ttf"),
        ),
        face(
            "alo Layered Deep Test",
            include_bytes!("../../alo-paint/tests/fonts/layered-deep.ttf"),
        ),
        face("DejaVu Sans", dejavu::sans::regular()),
    ]
    .into_iter()
    .flatten()
    {
        database.add(font);
    }
    database
}

fn drawn(scale: f32) -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(WIDTH, HEIGHT)).at_scale(scale),
        &fonts(),
        &[],
        &[],
    )
}

fn pixel(canvas: &Canvas, x: u32, y: u32) -> Option<[u8; 4]> {
    canvas.at(x, y).map(|colour| {
        let (red, green, blue, alpha) = colour.to_rgba8();
        [red, green, blue, alpha]
    })
}

/// The pixel at the page's point `(x, y)` of row `row` (from nought), at
/// `scale`.
fn at(canvas: &Canvas, scale: f32, row: u8, x: f32, y: f32) -> Option<[u8; 4]> {
    let device = |value: f32| {
        let value = (value * scale).floor();
        (0.0..65_536.0)
            .contains(&value)
            .then(|| format!("{value}").parse::<u32>().ok())
            .flatten()
    };
    pixel(canvas, device(x)?, device(top_of(row) + y)?)
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const TEAL: [u8; 4] = [0x2a, 0x9d, 0x8f, 255];
const RED: [u8; 4] = [0xe6, 0x39, 0x46, 255];
const LINE: [u8; 4] = [0x83, 0x38, 0xec, 255];

/// Each glyph shadow of the page, as its top and its layers' opacities —
/// none for a picture.
fn glyph_shadows(rendered: &alo_renderer::Rendered) -> Vec<(f32, Option<Vec<f32>>)> {
    rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::GlyphShadow { cast, .. } => cast.bounds().map(|bounds| {
                let layers = match cast {
                    Cast::Picture(..) | Cast::Painted(..) => None,
                    Cast::Layers(layers) => {
                        Some(layers.iter().map(|(_, opacity)| *opacity).collect())
                    }
                };
                (bounds.1, layers)
            }),
            _ => None,
        })
        .collect()
}

#[test]
fn a_colour_glyph_casts_what_it_draws() {
    let rendered = drawn(1.0);
    let shadows = glyph_shadows(&rendered);
    // Rows 0, 1, 8 and 12 (a picture), 4, 5, 9 and 13 (layers), 10 and 11
    // (a fallback run beside letters): ten glyphs, each casting a shadow
    // of its own, every one 50 down.
    assert_eq!(shadows.len(), 10, "{shadows:?}");
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    for (row, (top, layers)) in [0_u8, 1, 4, 5, 8, 9, 10, 11, 12, 13]
        .into_iter()
        .zip(&shadows)
    {
        let base = top_of(row) + BASELINE + 50.0;
        // Beside DejaVu Sans, in rows 10 and 11, the line's baseline is
        // its own; elsewhere the glyph's top is 30 over it.
        if row != 10 && row != 11 {
            assert!(close(*top, base - 30.0), "row {row}: {top}");
        }
        match (row, layers) {
            // The picture.
            (0 | 1 | 8 | 10 | 12, None) => {}
            // The layers: the leg and the foot whole, the side at half —
            // the foot whole though the text is faint in row 9.
            (4 | 5 | 9 | 11 | 13, Some(opacities)) => {
                assert_eq!(opacities.len(), 3, "row {row}");
                let wanted = [1.0, 128.0 / 255.0, 1.0];
                for (opacity, wanted) in opacities.iter().zip(wanted) {
                    assert!((opacity - wanted).abs() < 0.01, "row {row}: {opacities:?}");
                }
            }
            _ => panic!("row {row} cast {layers:?}"),
        }
    }
    // The letters beside each glyph cast their outlines apart from it: one
    // shadow per row that has letters and a shadow (row 13's `B` stands
    // alone), and row 8's line's, 2 under its baseline and 4 thick.
    let others = rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Shadow { path, .. } => path.bounds(),
            // The letters' own, glyph by glyph (queue item 831).
            DisplayItem::LettersShadow { glyphs, .. } => glyphs
                .iter()
                .filter_map(alo_paint::Path::bounds)
                .reduce(|one, other| {
                    (
                        one.0.min(other.0),
                        one.1.min(other.1),
                        one.2.max(other.2),
                        one.3.max(other.3),
                    )
                }),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(others.len(), 10, "{others:?}");
    assert!(
        others
            .iter()
            .any(|bounds| close(bounds.3 - bounds.1, 4.0) && close(bounds.1, 8.0 * ROW + 94.0)),
        "{others:?}"
    );
}

#[test]
fn a_shadow_is_the_pictures_alpha_and_the_layers_as_drawn() {
    for scale in [1.0_f32, 2.0] {
        let rendered = drawn(scale);
        let canvas = &rendered.drawing.canvas;
        let shadow = BASELINE + 50.0;
        let p = |row, x, y| at(canvas, scale, row, x, y);
        // The picture's: red's opaque rows black; under the baseline its
        // teal black, its clear middle white, its blue half grey.
        assert_eq!(p(0, B + 20.0, shadow - 15.0), Some(BLACK), "at {scale}");
        assert_eq!(p(0, B + 10.0, shadow + 5.0), Some(BLACK), "at {scale}");
        assert_eq!(p(0, B + 20.0, shadow + 5.0), Some(WHITE), "at {scale}");
        let half = p(0, B + 30.0, shadow + 5.0).unwrap_or(WHITE);
        assert!(
            (126..=129).contains(&half[0]) && half[0] == half[1] && half[1] == half[2],
            "at {scale}: {half:?}"
        );
        // Its picture's clear left columns cast nothing.
        assert_eq!(p(0, B + 2.0, shadow - 15.0), Some(WHITE), "at {scale}");
        // The layers': the leg black, under the baseline too; the side grey
        // over its foot's black; nothing where only `B`'s own outline is.
        assert_eq!(p(4, B + 8.0, shadow - 15.0), Some(BLACK), "at {scale}");
        assert_eq!(p(4, B + 8.0, shadow + 5.0), Some(BLACK), "at {scale}");
        let side = p(4, B + 30.0, shadow - 15.0).unwrap_or(WHITE);
        assert!((126..=129).contains(&side[0]), "at {scale}: {side:?}");
        assert_eq!(p(4, B + 30.0, shadow - 2.0), Some(BLACK), "at {scale}");
        assert_eq!(p(4, B + 2.0, shadow - 15.0), Some(WHITE), "at {scale}");
        assert_eq!(p(4, B + 30.0, shadow + 5.0), Some(WHITE), "at {scale}");
        // Faint text's foot casts whole.
        assert_eq!(p(9, B + 30.0, shadow - 2.0), Some(BLACK), "at {scale}");
    }
}

#[test]
fn a_line_runs_under_a_colour_glyph_as_if_it_skipped_no_ink() {
    for scale in [1.0_f32, 2.0] {
        let rendered = drawn(scale);
        let canvas = &rendered.drawing.canvas;
        // The underline is 2 under the baseline and 4 thick, as Chrome
        // draws this face's.
        let line = BASELINE + 5.0;
        let p = |row, x| at(canvas, scale, row, x, line);
        // Under the picture's clear columns and through the gap in its
        // foot, the line; over it, the glyph.
        assert_eq!(p(2, B + 2.0), Some(LINE), "at {scale}");
        assert_eq!(p(2, B + 20.0), Some(LINE), "at {scale}");
        assert_eq!(p(2, B + 10.0), Some(TEAL), "at {scale}");
        // Under the layered glyph's own outline and its leg: the line, and
        // the leg over it.
        assert_eq!(p(6, B + 2.0), Some(LINE), "at {scale}");
        assert_eq!(p(6, B + 30.0), Some(LINE), "at {scale}");
        assert_eq!(p(6, B + 8.0), Some(RED), "at {scale}");
        // Each row is drawn as the row that asks for no skipping.
        for (auto, none) in [(2_u8, 3_u8), (6, 7)] {
            for step in 0..120_u8 {
                let x = 10.0 + f32::from(step);
                for y in [line - 1.0, line, line + 1.0] {
                    assert_eq!(
                        at(canvas, scale, auto, x, y),
                        at(canvas, scale, none, x, y),
                        "rows {auto} and {none} at ({x}, {y}), at {scale}"
                    );
                }
            }
        }
        // The line's shadow under the picture's: black under its half.
        assert_eq!(p(8, B + 30.0).map(|_| ()), Some(()));
        assert_eq!(
            at(canvas, scale, 8, B + 30.0, line + 50.0),
            Some(BLACK),
            "at {scale}"
        );
    }
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

fn read(name: &str) -> Option<Canvas> {
    std::fs::read(reference(name))
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
}

#[test]
fn the_rows_are_drawn_as_the_references_at_one_and_two() {
    for (scale, name) in [
        (1.0_f32, "colour-glyph-shadows.png"),
        (2.0, "colour-glyph-shadows-at-2.png"),
    ] {
        let rendered = drawn(scale);
        let canvas = &rendered.drawing.canvas;
        if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
            let bytes = to_png(canvas).expect("the canvas encodes");
            std::fs::write(reference(name), bytes).expect("the reference is written");
            continue;
        }
        let Some(committed) = read(name) else {
            panic!("no reference {name}: ALO_UPDATE_REFERENCES=1 writes it");
        };
        assert_eq!(
            (committed.width(), committed.height()),
            (canvas.width(), canvas.height())
        );
        let differ = (0..canvas.height())
            .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(canvas, x, y) != pixel(&committed, x, y))
            .count();
        assert_eq!(differ, 0, "{name}: pixels differ from the reference");
    }
}

/// Where row `row` (from nought) starts down the page.
fn top_of(row: u8) -> f32 {
    if row < 12 {
        f32::from(row) * ROW
    } else {
        WIDE + f32::from(row - 12) * 160.0
    }
}

/// The row of the page a point `down` it is in, and how far down that row.
fn row_of(down: f32) -> (u8, f32) {
    let (first, rows, tall) = if down < WIDE {
        (0, down, ROW)
    } else {
        (12, down - WIDE, 160.0)
    };
    let mut row = 0_u8;
    while f32::from(row + 1) * tall <= rows && row < 2 * 12 {
        row += 1;
    }
    (first + row, rows - f32::from(row) * tall)
}

/// The largest difference in any channel between two pixels.
fn apart(ours: [u8; 4], theirs: [u8; 4]) -> u8 {
    ours.iter()
        .zip(theirs)
        .take(3)
        .map(|(one, other)| one.abs_diff(other))
        .max()
        .unwrap_or(0)
}

/// Chrome's pictures of the same page at one and two, beside the
/// references (shot headless, its letters' smoothing off): the rows of
/// hard shadows and lines are Chrome's to a level in every pixel, and
/// row 9's shadow; the blurred rows within what a box blur and Chrome's
/// differ by (Chrome's blur is not this engine's: 19 levels at one, 12 at
/// two, measured); and where letters of `DejaVu Sans` stand beside a colour
/// glyph (rows 10 and 11), their blurred shadow within 31 and 13 (the
/// letters' own edges are not Chrome's), and above it every pixel of the
/// page's own colours that is one in both pictures the same in both. Row
/// 9's glyph, drawn at the faint text's own opacity, is Chrome's to a level
/// too (queue item 830).
#[test]
fn every_pixel_is_chromes_within_its_rows_bound() {
    let plain = [WHITE, BLACK, TEAL, RED, LINE];
    for (scale, name, soft, beside, far) in [
        (
            1.0_f32,
            "chrome-colour-glyph-shadows.png",
            19,
            31,
            FAR_AT_ONE,
        ),
        (
            2.0,
            "chrome-colour-glyph-shadows-at-2.png",
            12,
            13,
            FAR_AT_TWO,
        ),
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
        let mut plain_compared = 0;
        for y in 0..canvas.height() {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a row of a page 2,640 device pixels tall"
            )]
            let down = y as f32 / scale;
            let (row, within) = row_of(down);
            for x in 0..canvas.width() {
                let (Some(ours), Some(theirs)) = (pixel(canvas, x, y), pixel(&chrome, x, y)) else {
                    continue;
                };
                let bound = match row {
                    1 | 5 => soft,
                    12 | 13 => far,
                    10 | 11 if within >= 60.0 => beside,
                    10 | 11 => {
                        if plain.contains(&ours) && plain.contains(&theirs) {
                            assert_eq!(ours, theirs, "({x}, {y}) at {scale}");
                            plain_compared += 1;
                        }
                        continue;
                    }
                    _ => 1,
                };
                assert!(
                    apart(ours, theirs) <= bound,
                    "({x}, {y}), row {row}, at {scale}: {ours:?} where Chrome {theirs:?}"
                );
            }
        }
        assert!(plain_compared > 25_000, "{plain_compared} at {scale}");
    }
}
