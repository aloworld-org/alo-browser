/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 766: a glyph drawn in layers of colour — a `COLR` version 0
//! face, as Windows' Segoe UI Emoji is — drawn as its layers, each in the
//! palette's colour or the text's, never as its own plain outline in the
//! text's colour.
//!
//! The face is `crates/alo-paint/tests/fonts/layered.ttf`, 972 bytes,
//! drawn for these tests and under this repository's licence: made with
//! `fontTools` 4.61's `FontBuilder` (1000 units to the em, ascent 800,
//! descent 200), every glyph a rectangle a whole em wide. `A` and `C` are
//! plain, a box 100–900 across and 0–700 up. `B` has a plain outline of its
//! own, the whole 0–1000 by 0–800, and three layers in `COLR` version 0:
//! the back, 100–900 by 0–800, in palette entry 0, `#e63946`; the middle,
//! 300–700 by 200–600, in entry 1, `#1d4ed8`; and the foot, 100–900 by
//! 0–100, in `0xFFFF`, the text's colour. Its one palette is `CPAL`'s.
//!
//! Chrome 156 draws the same bytes, as a web font, the same way — the
//! middle blue, the back red, the foot in the text's colour, nothing of
//! `B`'s own outline at its edges — and this engine's picture at one is
//! Chrome's in every pixel of one colour (`chrome-layers.png`, beside the
//! reference; measured 2026-11-01, headless, `--force-device-scale-factor=1`).
//!
//! What closes it: the drawing in numbers — the fills `B` is drawn as, in
//! their colours and places, and no letter of the text where `B` is; the
//! pixels, at one and two; a reference render,
//! `references/a-glyph-in-layers-of-colour.png`.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_glyph_in_layers_of_colour`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::Size;
use alo_paint::{Canvas, DisplayItem, Paint, from_png, to_png};
use alo_renderer::Screen;
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_value::Rgba;
use std::path::{Path, PathBuf};

/// The page: three letters of the layered face, 40 px, in teal, from
/// (10, 10).
const PAGE: &str = "<!DOCTYPE html><html><body><p>ABC</p></body></html>";

/// Its style: the letters set on whole pixels.
const STYLE: &str = "body { margin: 0; background: #ffffff }
p { margin: 10px; font-family: 'alo Layered Test'; font-size: 40px; line-height: 50px;
  color: #2a9d8f }";

/// The text's colour, the palette's two, and the page's white.
const TEAL: [u8; 4] = [0x2a, 0x9d, 0x8f, 0xff];
const RED: [u8; 4] = [0xe6, 0x39, 0x46, 0xff];
const BLUE: [u8; 4] = [0x1d, 0x4e, 0xd8, 0xff];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "alo Layered Test",
        Weight::NORMAL,
        Slant::Normal,
        include_bytes!("../../alo-paint/tests/fonts/layered.ttf").to_vec(),
    ) {
        database.add(font);
    }
    database
}

fn drawn(scale: f32) -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(200.0, 70.0)).at_scale(scale),
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

/// The line's baseline at one: 10 down, then the 50 line's half-leading
/// over the face's 800 ascent of 1000 at 40 px — (50 − 40) / 2 + 32.
const BASELINE: f32 = 10.0 + 5.0 + 32.0;

#[test]
fn the_layered_letter_is_drawn_as_its_layers_in_their_colours() {
    let rendered = drawn(1.0);
    let fills: Vec<(Rgba, (f32, f32, f32, f32))> = rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill {
                path,
                paint: Paint::Solid(colour),
                ..
            } if *colour != Rgba::from_rgba8(0xff, 0xff, 0xff, 0xff) => {
                path.bounds().map(|bounds| (*colour, bounds))
            }
            _ => None,
        })
        .collect();
    // `B` starts one em along, at 50: its back 54–86 across and 32 up from
    // the baseline; its middle 62–78 by 8–24 up; its foot in the text's
    // colour, 54–86 by 0–4 up.
    let expected = [
        (RED, (54.0, BASELINE - 32.0, 86.0, BASELINE)),
        (BLUE, (62.0, BASELINE - 24.0, 78.0, BASELINE - 8.0)),
        (TEAL, (54.0, BASELINE - 4.0, 86.0, BASELINE)),
    ];
    assert_eq!(fills.len(), expected.len(), "{fills:?}");
    for ((colour, bounds), (wanted, at)) in fills.iter().zip(expected) {
        let [red, green, blue, alpha] = wanted;
        assert_eq!(*colour, Rgba::from_rgba8(red, green, blue, alpha));
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(
            close(bounds.0, at.0)
                && close(bounds.1, at.1)
                && close(bounds.2, at.2)
                && close(bounds.3, at.3),
            "{bounds:?} where {at:?}"
        );
    }
}

#[test]
fn its_pixels_are_the_layers_and_none_of_its_own_outline() {
    for scale in [1.0_f32, 2.0] {
        let rendered = drawn(scale);
        let canvas = &rendered.drawing.canvas;
        let at = |x: f32, y: f32| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a point of a 200 × 70 page, times one or two"
            )]
            let device = ((x * scale) as u32, (y * scale) as u32);
            pixel(canvas, device.0, device.1)
        };
        // `A`, plain, in the text's colour.
        assert_eq!(at(30.0, BASELINE - 10.0), Some(TEAL), "at {scale}");
        // `B`: the back, the middle, the foot.
        assert_eq!(at(56.0, BASELINE - 20.0), Some(RED), "at {scale}");
        assert_eq!(at(70.0, BASELINE - 16.0), Some(BLUE), "at {scale}");
        assert_eq!(at(70.0, BASELINE - 2.0), Some(TEAL), "at {scale}");
        // Where only `B`'s own outline reaches — its first and last 4 px
        // across, and 32 to 32 up — nothing is drawn.
        assert_eq!(at(51.0, BASELINE - 20.0), Some(WHITE), "at {scale}");
        assert_eq!(at(88.0, BASELINE - 20.0), Some(WHITE), "at {scale}");
    }
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

#[test]
fn the_layered_letters_are_drawn_as_the_reference() {
    let rendered = drawn(1.0);
    let canvas = &rendered.drawing.canvas;
    let path = reference("a-glyph-in-layers-of-colour.png");
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        let bytes = to_png(canvas).expect("the canvas encodes");
        std::fs::write(&path, bytes).expect("the reference is written");
        return;
    }
    let Some(committed) = std::fs::read(&path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
    else {
        panic!(
            "no reference at {}: ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        );
    };
    assert_eq!(
        (committed.width(), committed.height()),
        (canvas.width(), canvas.height())
    );
    let differ = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(canvas, x, y) != pixel(&committed, x, y))
        .count();
    assert_eq!(differ, 0, "pixels differ from the reference");
}

/// Chrome's picture of the same bytes at one, beside the reference: every
/// pixel that is one of the page's four colours in both is the same colour
/// in both.
#[test]
fn every_plain_pixel_is_chromes() {
    let rendered = drawn(1.0);
    let canvas = &rendered.drawing.canvas;
    let Some(chrome) = std::fs::read(reference("chrome-layers.png"))
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
    else {
        panic!("Chrome's picture is beside the reference");
    };
    assert_eq!(
        (chrome.width(), chrome.height()),
        (canvas.width(), canvas.height())
    );
    let plain =
        |colour: Option<[u8; 4]>| colour.filter(|colour| [TEAL, RED, BLUE, WHITE].contains(colour));
    let mut compared = 0;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if let (Some(ours), Some(theirs)) =
                (plain(pixel(canvas, x, y)), plain(pixel(&chrome, x, y)))
            {
                assert_eq!(ours, theirs, "({x}, {y})");
                compared += 1;
            }
        }
    }
    assert!(compared > 10_000, "{compared}");
}
