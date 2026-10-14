/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Spread shadows' corners, drawn as Chrome draws them: the case that
//! closes queue item 573.
//!
//! `cases/spread-shadow-corners` holds blue boxes 30 px square on whole
//! pixels, each casting a red shadow: spread 4 px at radii 0, 1, 2, 3, 4,
//! 6 and 10 and an ellipse of 2 by 8; spread 8 at 0, 1, 2, 4, 6, 8, 12 and
//! 50 %; inset shadows moved 10 px with a spread of −6, so their hole grows,
//! at 0, 1, 3 and 6; a box 40 by 6 whose 40 px radius CSS scales to 3
//! before it is spread by 10; a radius of 1 spread by 12; and a corner of
//! 5 px by nothing, which is square.
//!
//! Chrome 154 grows a radius at least as large as the spread by all of it,
//! a smaller one by the spread times `1 + (r − 1)³` (`r` the radius over
//! the spread), and a square one not at all; each of a corner's two radii
//! on its own, after the radii are fitted to the box (`Corners::spread`).
//! Growing every radius by the whole spread, as this engine did, rounds
//! the square box's shadow by 4 and 8 px.
//!
//! `chrome.png` and `chrome-at-2.png` beside it are Chrome 154's screenshots
//! of the same bytes, headless, 500 by 200, started with
//! `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`, taken
//! 2026-10-14.
//!
//! A pixel is read as one of the case's plain colours — white, the boxes'
//! blue, the shadows' red — or as a blend of them, which says nothing.
//! Wherever both pictures have a plain colour, it is the same one: so every
//! corner is square or curved where Chrome's is. A blend is a curve's edge,
//! and there the two engines' anti-aliasing differs by at most `EDGE`.
//!
//! `render-at-2.png` is this engine's picture at two, a reference beside
//! `render.png` and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// The case's plain colours.
const PLAIN: [(u8, u8, u8); 3] = [(255, 255, 255), (0x1d, 0x4e, 0xd8), (0xc0, 0x39, 0x2b)];

/// How far a curve's edge pixel may be from Chrome's, of 255 in any
/// channel, at each scale. Measured on this case: the worst at each is on a
/// box's own rounded background, not on a shadow — (318, 76) at one, 8 px
/// round, and (392, 152) at two, 4 px round — where Chrome covers an edge
/// pixel by area and this engine samples it (queue item 459).
const EDGE: [(f32, u8); 2] = [(1.0, 37), (2.0, 43)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("spread-shadow-corners"))
}

/// The picture Chrome took at `scale`, beside the case.
fn chrome(case: &Case, scale: f32) -> Option<Canvas> {
    let name = if scale > 1.0 {
        "chrome-at-2.png"
    } else {
        "chrome.png"
    };
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Which plain colour a pixel is, if it is one.
fn plain(canvas: &Canvas, x: u32, y: u32) -> Option<usize> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    PLAIN.iter().position(|&(r, g, b)| {
        red.abs_diff(r) <= 1 && green.abs_diff(g) <= 1 && blue.abs_diff(b) <= 1
    })
}

/// The largest difference in any channel between two pictures' pixels.
fn difference(ours: &Canvas, chrome: &Canvas, x: u32, y: u32) -> u8 {
    let (Some(a), Some(b)) = (ours.at(x, y), chrome.at(x, y)) else {
        return u8::MAX;
    };
    let ((ar, ag, ab, _), (br, bg, bb, _)) = (a.to_rgba8(), b.to_rgba8());
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
        .unwrap_or(u8::MAX)
}

#[test]
fn every_corner_is_square_or_curved_where_chromes_is() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, edge) in EDGE {
        let rendering = Rendering::at_scale(&case, scale).ok();
        let (Some(drawing), Some(chrome)) = (
            rendering.as_ref().and_then(Rendering::drawing),
            chrome(&case, scale),
        ) else {
            panic!("the case is drawn at {scale}, and Chrome's picture is beside it");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut worst = (0, None);
        let mut blends = 0;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    assert_eq!(a, b, "at {scale}, ({x}, {y}) is a different colour");
                    continue;
                }
                blends += 1;
                let found = difference(ours, &chrome, x, y);
                if found > worst.0 {
                    worst = (found, Some((x, y)));
                }
            }
        }
        assert!(blends > 0, "at {scale} no curve was compared");
        assert!(
            worst.0 <= edge,
            "at {scale}, an edge is {} of 255 from Chrome's at {:?}",
            worst.0,
            worst.1
        );
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(rendering) = Rendering::at_scale(&case, 2.0).ok() else {
        panic!("the case is drawn at two");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
