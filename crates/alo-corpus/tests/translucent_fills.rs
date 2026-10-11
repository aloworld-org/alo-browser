/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A translucent colour over an opaque one, in pixels (queue item 436).
//!
//! `cases/translucent-fills` draws twelve colours with an alpha over three
//! grounds. Every pixel is the colour blended source-over, as CSS
//! Compositing 1 writes it, and rounded to a byte **once**, when the picture
//! is written out — which is what `alo-paint`'s canvas holds floats for.
//!
//! Item 436 asked why `rgb(23 33 43 / 0.14)` over white was `223` here and
//! `222` in a Chrome 154 screenshot. Measured on these 36 cells, Chrome 154
//! draws two different pictures. Its default raster path is the exact blend:
//! it agrees with this one on every cell whose blend is not a half, and
//! rounds a half either way. Its software path, `--disable-gpu`, which is
//! where the `222` came from, blends in eight bits — the alpha rounded to a
//! byte, the colour premultiplied and rounded, the ground scaled by
//! `(256 − alpha) / 256` and truncated — which accounts for all 36 of its
//! cells and is up to one unit darker than the blend. That is a fast
//! approximation, not a different answer to copy.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_paint::Canvas;
use alo_value::Rgba;

/// The twelve colours, left to right, as the case's sheet writes them.
const COLOURS: [(u8, u8, u8, f64); 12] = [
    (23, 33, 43, 0.14),
    (255, 0, 0, 0.5),
    (0, 0, 255, 0.3),
    (100, 150, 200, 0.07),
    (0, 0, 0, 0.33),
    (200, 50, 120, 0.61),
    (10, 200, 30, 0.9),
    (0, 0, 0, 0.14),
    (255, 255, 255, 0.25),
    (17, 34, 51, 0.01),
    (128, 64, 32, 0.77),
    (250, 128, 114, 0.42),
];

/// The three grounds, top to bottom.
const GROUNDS: [(u8, u8, u8); 3] = [(255, 255, 255), (40, 120, 200), (240, 200, 20)];

/// How big a cell is.
const CELL: u32 = 20;

/// The case's picture.
fn picture() -> Option<Canvas> {
    let case = Case::read(&cases_directory().join("translucent-fills"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some(rendering.drawing()?.canvas.clone())
}

/// One channel blended source-over an opaque ground, exactly.
fn blend(colour: u8, ground: u8, alpha: f64) -> f64 {
    f64::from(colour) * alpha + f64::from(ground) * (1.0 - alpha)
}

/// The bytes an exact channel may be written as: itself rounded, or either
/// neighbour when it is a half, since a half is where two arithmetics that
/// are both exact enough part company.
fn allowed(exact: f64) -> Vec<f64> {
    let below = exact.floor();
    if (exact - below - 0.5).abs() < 1e-9 {
        vec![below, below + 1.0]
    } else {
        vec![exact.round()]
    }
}

#[test]
fn every_pixel_is_the_exact_blend_rounded_once() {
    let Some(canvas) = picture() else {
        panic!("the case renders");
    };
    assert_eq!((canvas.width(), canvas.height()), (240, 60));
    for (row, (ground_red, ground_green, ground_blue)) in (0u32..).zip(GROUNDS) {
        for (column, (red, green, blue, alpha)) in (0u32..).zip(COLOURS) {
            let expected = [
                allowed(blend(red, ground_red, alpha)),
                allowed(blend(green, ground_green, alpha)),
                allowed(blend(blue, ground_blue, alpha)),
            ];
            for y in row * CELL..(row + 1) * CELL {
                for x in column * CELL..(column + 1) * CELL {
                    let Some((r, g, b, a)) = canvas.at(x, y).map(Rgba::to_rgba8) else {
                        panic!("({x}, {y}) is on the canvas");
                    };
                    assert_eq!(a, 255, "({x}, {y}) is opaque");
                    for (channel, allowed) in [r, g, b].into_iter().zip(&expected) {
                        assert!(
                            allowed
                                .iter()
                                .any(|byte| (byte - f64::from(channel)).abs() < 1e-9),
                            "({x}, {y}): rgb({red} {green} {blue} / {alpha}) over \
                             rgb({ground_red} {ground_green} {ground_blue}) is \
                             ({r}, {g}, {b}), and a channel should be one of {allowed:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn alo_sites_card_ring_over_white_is_the_blend_not_eight_bit_arithmetic() {
    // 255 × 0.86 + 23 × 0.14 is 222.48. Chrome's software path draws 222,
    // its alpha first rounded to 36 / 255; its default path draws 223.
    let Some(canvas) = picture() else {
        panic!("the case renders");
    };
    assert_eq!(
        canvas.at(10, 10).map(Rgba::to_rgba8),
        Some((223, 224, 225, 255))
    );
    // And one away from white, where the software path's truncation shows:
    // rgb(23 33 43 / 0.14) over rgb(40 120 200) is 37.62, 107.82, 178.02.
    assert_eq!(
        canvas.at(10, 30).map(Rgba::to_rgba8),
        Some((38, 108, 178, 255))
    );
}
