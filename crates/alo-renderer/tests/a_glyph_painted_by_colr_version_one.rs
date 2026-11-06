/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 785: a `COLR` version 1 glyph drawn as its paint program —
//! gradients, transforms, clips, layers and compositing modes — as Chrome
//! draws it.
//!
//! A face made for this test with `fontTools` 4.61 (1000 units to the em,
//! ascent 800, descent 200), under this repository's licence:
//! `crates/alo-paint/tests/fonts/painted.ttf`, 2,012 bytes, one palette —
//! red `#e63946`, blue `#1d4ed8`, yellow `#f4d35e` at 0.6, teal `#2a9d8f` —
//! and sixteen glyphs, each a whole em wide, each a `PaintGlyph` clip of a
//! plain outline:
//!
//! - `L` a linear gradient across, red, yellow, blue; `K` one slanted by
//!   its third point; `R` a radial one from a point; `Q` between two
//!   circles off one another; `S` a sweep about the middle, red, teal,
//!   blue, a whole turn (its angles written with the table's bias of one);
//! - `T` a square turned 30° about the middle, `U` scaled, `V` skewed, `W`
//!   an affine `PaintTransform`, `X` moved;
//! - `C` three layers (`PaintColrLayers`), the last in the text's colour;
//!   `M`, `N`, `O` and `P` the yellow (or red) put onto the blue by
//!   `multiply`, `screen`, `src_in` and `dest_out`;
//! - `D` a diamond, a gradient from the text's colour to blue at half.
//!
//! **Measured** in Chrome 157.0.8103.38 (`--version`), headless, its
//! default raster path, at one and two, the same bytes as a web font: every
//! pixel inside a shape is Chrome's to a level — gradients included, their
//! stops mixed as straight colours and premultiplied after (mixed
//! premultiplied, as the specification words it, a stop at 0.6 was 22
//! levels off), no dithering in any of them — and a shape's slanted edge
//! within what Chrome's glyph rasteriser and this engine's coverage differ
//! by (19 levels at one, 27 at two, on the turned square); text at half
//! opacity fades each glyph once (queue item 830); a painted glyph casts
//! its alpha as its shadow (queue item 786's rule, measured here on `R`).
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_glyph_painted_by_colr_version_one`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::Size;
use alo_paint::colour_paint::{Fill, Step};
use alo_paint::{Canvas, DisplayItem, from_png, to_png};
use alo_renderer::Screen;
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// Six rows of 120: four of the glyphs, then three under text at half
/// opacity, then `R` and `T` casting a hard shadow 100 to the right. Each
/// row's text starts at 20, its baseline 90 down.
const PAGE: &str = "<!DOCTYPE html><html><body>\
<p>LKRQ</p><p>STUV</p><p>WXCM</p><p>NOPD</p>\
<p style='color: rgba(42, 157, 143, 0.5)'>CRD</p>\
<p style='text-shadow: 100px 0 0 #000000'>R T</p>\
</body></html>";

const STYLE: &str = "body { margin: 0; background: #ffffff; width: 440px }
p { margin: 0; padding-left: 20px; height: 120px; font-size: 100px; line-height: 120px; color: #2a9d8f;
  font-family: 'alo Painted Test'; white-space: pre }";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "alo Painted Test",
        Weight::NORMAL,
        Slant::Normal,
        include_bytes!("../../alo-paint/tests/fonts/painted.ttf").to_vec(),
    ) {
        database.add(font);
    }
    database
}

fn drawn(scale: f32) -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(440.0, 720.0)).at_scale(scale),
        &fonts(),
        &[],
        &[],
    )
}

fn pixel(canvas: &Canvas, x: u32, y: u32) -> Option<[u8; 3]> {
    canvas.at(x, y).map(|colour| {
        let (red, green, blue, _) = colour.to_rgba8();
        [red, green, blue]
    })
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
fn each_glyph_is_its_program_and_none_is_its_own_outline() {
    let rendered = drawn(1.0);
    let items = rendered.drawing.display.items();
    let painted: Vec<(&[Step], (f32, f32), u8)> = items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::PaintedGlyph {
                steps, pen, fade, ..
            } => Some((steps.as_slice(), *pen, *fade)),
            _ => None,
        })
        .collect();
    // Sixteen, then three faded, then two casting a shadow.
    assert_eq!(painted.len(), 21);
    let fades: Vec<u8> = painted.iter().map(|(_, _, fade)| *fade).collect();
    assert_eq!(fades.get(16..19), Some(&[128, 128, 128][..]));
    // The first, `L`, at its pen: a clip of its outline filled with a
    // linear gradient of three stops.
    let Some((steps, pen, _)) = painted.first() else {
        panic!("no painted glyph");
    };
    assert_eq!(*pen, (20.0, 90.0));
    let linear = steps.iter().find_map(|step| match step {
        Step::Paint(Fill::Linear { points, line }) => Some((*points, line.stops.len())),
        _ => None,
    });
    assert_eq!(
        linear,
        Some(([(100.0, 0.0), (900.0, 0.0), (100.0, 700.0)], 3))
    );
    // Each kind of fill is met, and the compositing groups.
    let all: Vec<&Step> = painted
        .iter()
        .flat_map(|(steps, ..)| steps.iter())
        .collect();
    assert!(
        all.iter()
            .any(|step| matches!(step, Step::Paint(Fill::Radial { .. })))
    );
    assert!(
        all.iter()
            .any(|step| matches!(step, Step::Paint(Fill::Sweep { .. })))
    );
    assert!(all.iter().any(|step| matches!(step, Step::PushLayer(_))));
    assert!(
        all.iter()
            .any(|step| matches!(step, Step::PushTransform(_)))
    );
    // No glyph's own plain outline is a text item's: the letters are empty.
    assert!(
        !items
            .iter()
            .any(|item| matches!(item, DisplayItem::Fill { paint: alo_paint::Paint::Solid(colour), .. } if colour.to_rgba8() == (0x2a, 0x9d, 0x8f, 255))),
        "no fill in the text's colour of a whole glyph"
    );
}

#[test]
fn the_pixels_worked_out_by_hand_are_drawn() {
    let rendered = drawn(1.0);
    let canvas = &rendered.drawing.canvas;
    // `M`: yellow at 0.6 multiplied onto blue where they overlap —
    // 0.4 × blue + 0.6 × blue × yellow = 28, 70, 134; Chrome the same.
    assert_eq!(pixel(canvas, 370, 300), Some([28, 70, 134]));
    // `S` just above its axis on the right: the sweep's start, nearly red.
    assert_eq!(pixel(canvas, 90, 170), Some([217, 64, 75]));
    // `C` at half: its red leg faded once, as a layered glyph is.
    assert_eq!(pixel(canvas, 40, 530), Some([242, 156, 162]));
    // `R`'s shadow, 100 to its right: its alpha in black.
    assert_eq!(pixel(canvas, 170, 660), Some([88, 88, 88]));
}

/// Whether Chrome's pixel at `(x, y)` is on a shape's edge: not the same as
/// every pixel around it.
fn on_an_edge(chrome: &Canvas, x: u32, y: u32) -> bool {
    let here = pixel(chrome, x, y);
    (x.saturating_sub(1)..=x + 1).any(|across| {
        (y.saturating_sub(1)..=y + 1).any(|down| {
            let there = pixel(chrome, across, down);
            there.is_some() && there != here
        })
    })
}

#[test]
fn every_pixel_is_chromes_inside_a_shape_and_near_it_on_an_edge() {
    for (scale, name, edge) in [
        (1.0_f32, "chrome-painted-glyphs.png", 24_u8),
        (2.0, "chrome-painted-glyphs-at-2.png", 34),
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
        let mut inside = 0;
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
                let bound = if on_an_edge(&chrome, x, y) {
                    edge
                } else {
                    inside += 1;
                    1
                };
                assert!(
                    far <= bound,
                    "({x}, {y}) at {scale}: {ours:?} where Chrome {theirs:?}"
                );
            }
        }
        assert!(inside > 200_000, "{inside} at {scale}");
    }
}

#[test]
fn the_glyphs_are_drawn_as_the_reference() {
    let rendered = drawn(1.0);
    let canvas = &rendered.drawing.canvas;
    let name = "a-glyph-painted-by-colr-version-one.png";
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        let bytes = to_png(canvas).expect("the canvas encodes");
        std::fs::write(reference(name), bytes).expect("the reference is written");
        return;
    }
    let Some(committed) = read(name) else {
        panic!("no reference {name}: ALO_UPDATE_REFERENCES=1 writes it");
    };
    let differ = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(canvas, x, y) != pixel(&committed, x, y))
        .count();
    assert_eq!(differ, 0, "{name}: pixels differ from the reference");
}
