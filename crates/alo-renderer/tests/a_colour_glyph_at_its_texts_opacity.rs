/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 830: a colour glyph drawn at its text colour's own opacity,
//! as Chrome 156 draws it.
//!
//! Two faces made for these tests with `fontTools` 4.61 (1000 units to the
//! em, ascent 800, descent 200), under this repository's licence, each with
//! `B` a colour glyph a whole em wide:
//!
//! - `crates/alo-paint/tests/fonts/faded.ttf`, 7,368 bytes: `B` a `CBDT`
//!   picture, one strike of 40 to the em, 40 × 40 texels of random colours
//!   at random alphas (seeded, every alpha from 0 to 255 among them), its
//!   foot 10 under the baseline.
//! - `crates/alo-paint/tests/fonts/layered-deep.ttf` (queue item 786): `B`
//!   a `COLR` version 0 glyph — a leg in palette red, a side in palette
//!   blue at 128 of 255, a foot in the text's colour over both.
//!
//! **Measured** in Chrome 156.0.8078.76 (checked with `--version`),
//! headless, its default raster path, at one and two: the page below, each
//! row six `B`s under text of `rgb(42, 157, 143)` at 0, 0.05, 0.3, 0.5, 0.7
//! and 0.123, over white, black and grey. A picture is faded texel by texel
//! — each texel premultiplied to a level, times the alpha's byte over 255,
//! over the ground — and a layered glyph as one group: its layers put over
//! each other (the foot at the text colour's own opacity), held
//! premultiplied in levels, and faded once. Under a transparent colour,
//! nothing. The scratchpad fit took 86,400 picture samples under eighteen
//! alphas, every one Chrome's; this page is six of them.
//!
//! The picture's rows write it as U+1F642, which `faded.ttf` maps to `B`
//! too, so that this engine takes the picture face for it beside the
//! layered one, as Chrome does (queue item 829).
//!
//! What closes it: the faded items in numbers, a pixel fitted by hand
//! (247, 195, 199), and every pixel of the page Chrome's at one, and at two
//! within a level — the layered rows there every pixel too, but a picture
//! drawn larger than its strike is smoothed a level from Chrome's in about
//! half its pixels, under opaque text as much as faint (queue item 832).

use alo_layout::Size;
use alo_paint::{Canvas, DisplayItem, from_png};
use alo_renderer::Screen;
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::Path;

/// The text alphas of each row's six glyphs, left to right.
const ALPHAS: [&str; 6] = ["0", "0.05", "0.3", "0.5", "0.7", "0.123"];

/// The page's grounds, each a pair of rows: the picture, then the layers.
const GROUNDS: [&str; 3] = ["#ffffff", "#000000", "#808080"];

/// Its style: each row 50 tall, its baseline 37 down (half-leading 5,
/// ascent 32), its first glyph at 10 and each 40 wide.
const STYLE: &str = "body { margin: 0; background: #ffffff; width: 300px }
p { margin: 0; padding-left: 10px; height: 50px; font-size: 40px; line-height: 50px;
  white-space: nowrap }
.fa { font-family: 'alo Faded Test' }
.dp { font-family: 'alo Layered Deep Test' }";

fn page() -> String {
    let mut rows = Vec::new();
    for ground in GROUNDS {
        for (family, glyph) in [("fa", '\u{1f642}'), ("dp", 'B')] {
            let glyphs = ALPHAS
                .iter()
                .map(|alpha| {
                    format!("<span style='color: rgba(42, 157, 143, {alpha})'>{glyph}</span>")
                })
                .collect::<Vec<_>>()
                .concat();
            rows.push(format!(
                "<p class='{family}' style='background: {ground}'>{glyphs}</p>"
            ));
        }
    }
    format!("<!DOCTYPE html><html><body>{}</body></html>", rows.concat())
}

fn face(family: &str, bytes: &[u8]) -> Option<Font> {
    Font::load(family, Weight::NORMAL, Slant::Normal, bytes.to_vec())
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for font in [
        face(
            "alo Faded Test",
            include_bytes!("../../alo-paint/tests/fonts/faded.ttf"),
        ),
        face(
            "alo Layered Deep Test",
            include_bytes!("../../alo-paint/tests/fonts/layered-deep.ttf"),
        ),
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
        &page(),
        STYLE,
        Screen::new(Size::new(300.0, 300.0)).at_scale(scale),
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

fn read(name: &str) -> Option<Canvas> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name);
    std::fs::read(path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
}

#[test]
fn each_colour_glyph_is_faded_by_its_texts_alpha_and_a_clear_one_is_not_drawn() {
    let rendered = drawn(1.0);
    let items = rendered.drawing.display.items();
    // Per ground, the picture row's five glyphs that are drawn at all.
    let mut fades: Vec<u8> = items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Picture { fade, .. } => Some(*fade),
            _ => None,
        })
        .collect();
    assert_eq!(fades, [13, 77, 128, 179, 31].repeat(3));
    // And the layered row's five, each one group of its three layers, the
    // foot in the text's own colour.
    fades.clear();
    for item in items {
        if let DisplayItem::LayeredGlyph { layers, fade, .. } = item {
            assert_eq!(layers.len(), 3);
            let foot = layers.last().map(|(_, colour)| colour.to_rgba8());
            assert_eq!(foot, Some((42, 157, 143, *fade)));
            fades.push(*fade);
        }
    }
    assert_eq!(fades, [13, 77, 128, 179, 31].repeat(3));
}

#[test]
fn a_palette_red_under_text_at_three_tenths_is_247_195_199_over_white() {
    let rendered = drawn(1.0);
    let canvas = &rendered.drawing.canvas;
    // The third glyph of the layered row over white: its leg 4–20 across
    // from 90, 7–45 down row 1.
    assert_eq!(pixel(canvas, 98, 70), Some([247, 195, 199, 255]));
    // The foot over the leg at 0.5: the leg's red moved towards the faint
    // teal, then faded.
    assert_eq!(pixel(canvas, 138, 85), Some([195, 181, 181, 255]));
    // Under a transparent colour, the ground.
    assert_eq!(pixel(canvas, 18, 70), Some([255, 255, 255, 255]));
}

/// Every pixel of the page against Chrome's: all of them at one; at two,
/// where the picture is drawn larger than its strike and smoothed, within
/// `slack` levels (0 at one): 27,752 of its 360,000 pixels are a level
/// apart there, every one in a picture, as they are under opaque text
/// (queue item 832).
#[test]
fn every_pixel_is_chromes() {
    for (scale, name, slack) in [
        (1.0_f32, "chrome-faded-colour-glyphs.png", 0_u8),
        (2.0, "chrome-faded-colour-glyphs-at-2.png", 1),
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
        let mut apart = 0;
        let mut worst = 0;
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let (Some(ours), Some(theirs)) = (pixel(canvas, x, y), pixel(&chrome, x, y)) else {
                    continue;
                };
                let far = ours
                    .iter()
                    .zip(theirs)
                    .map(|(one, other)| one.abs_diff(other))
                    .max()
                    .unwrap_or(0);
                worst = worst.max(far);
                if far > 0 {
                    apart += 1;
                }
            }
        }
        assert!(
            worst <= slack,
            "{name}: {apart} pixels apart, the worst by {worst}"
        );
    }
}
