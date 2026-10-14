/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Translucent pictures drawn larger than they are, over opaque grounds:
//! the case that closes queue item 600.
//!
//! `cases/translucent-pictures-drawn-larger` puts the same six pictures on
//! five grounds — white, black, grey, blue and yellow: a 4 × 3 picture of
//! twelve colours from clear to opaque, drawn 40 by 30 and 20 by 18;
//! iteration 358's red beside a transparent blue, 40 by 10 and 10 by 10;
//! and black at four alphas, 36 by 36 and 12 by 20. Every size was chosen
//! so that no column's or row's weight is within a fiftieth of a 256th of
//! rounding the other way, at one or at two, where Chrome's position in
//! floats would decide it (item 594).
//!
//! Chrome 154 premultiplies each texel to a level, mixes and holds the mix
//! in sixteenths of a level, and puts it onto the eight-bit pixel beneath
//! in one 32-bit fused multiply-add (`alo-paint`'s `bilinear.rs` and
//! `mix_onto.rs`). This engine mixed the texels premultiplied in exact
//! arithmetic and composited in floats, which differed from Chrome's in
//! 2,850 pixels of this case at one and 12,399 at two, each by a level.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 220 by 220 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! the sheet inline after the doctype and the pictures beside it, taken
//! 2026-10-15. `origin.txt` says how the pictures were made.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::display::DisplayItem;
use alo_renderer::Drawing;
use alo_value::Rgba;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("translucent-pictures-drawn-larger"))
}

fn drawn(case: &Case, scale: f32) -> Result<std::rc::Rc<Drawing>, String> {
    Rendering::at_scale(case, scale)?
        .drawing()
        .ok_or_else(|| format!("no drawing at {scale}"))
}

/// The six pictures on each ground: where each is drawn on the ground, its
/// size, and how many texels it has across and down.
const PICTURES: [([f32; 4], (u32, u32)); 6] = [
    ([4.0, 4.0, 40.0, 30.0], (4, 3)),
    ([50.0, 4.0, 20.0, 18.0], (4, 3)),
    ([76.0, 4.0, 40.0, 10.0], (2, 1)),
    ([122.0, 4.0, 10.0, 10.0], (2, 1)),
    ([138.0, 4.0, 36.0, 36.0], (2, 2)),
    ([180.0, 4.0, 12.0, 20.0], (2, 2)),
];

/// How tall a ground is.
const GROUND: f32 = 44.0;

/// Every picture is where the sheet puts it, on whole pixels, and larger
/// than it is on both axes: so every pixel inside it is a mix of its
/// texels put onto its ground, and none is an edge blended with the page.
#[test]
fn every_picture_is_drawn_larger_on_whole_pixels() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = match drawn(&case, 1.0) {
        Ok(drawing) => drawing,
        Err(why) => panic!("{why}"),
    };
    let found: Vec<([f32; 4], (u32, u32))> = drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Picture { rect, picture, .. } => Some((
                [
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ],
                (picture.width(), picture.height()),
            )),
            _ => None,
        })
        .collect();
    let expected: Vec<([f32; 4], (u32, u32))> = (0_u8..5)
        .flat_map(|ground| {
            PICTURES.map(|([left, top, width, height], texels)| {
                (
                    [left, top + f32::from(ground) * GROUND, width, height],
                    texels,
                )
            })
        })
        .collect();
    assert_eq!(found.len(), 30);
    for (index, ((at, texels), (want, want_texels))) in found.iter().zip(&expected).enumerate() {
        assert_eq!(texels, want_texels, "picture {index}");
        assert!(
            at.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-4),
            "picture {index}: {at:?}, expected {want:?}"
        );
        assert!(
            at[2] > 2.0 * f32::from(u16::try_from(texels.0).unwrap_or(u16::MAX))
                && at[3] > 2.0 * f32::from(u16::try_from(texels.1).unwrap_or(u16::MAX)),
            "picture {index} is drawn larger"
        );
    }
}

/// Every device pixel of the page is Chrome's exactly, at one and at two.
#[test]
fn every_pixel_is_chromes_at_one_and_at_two() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, name) in [(1_u8, "chrome.png"), (2, "chrome-at-2.png")] {
        let drawing = match drawn(&case, f32::from(scale)) {
            Ok(drawing) => drawing,
            Err(why) => panic!("at {scale}: {why}"),
        };
        let Some(chrome) = std::fs::read(case.expectation(name))
            .ok()
            .and_then(|bytes| alo_paint::from_png(&bytes).ok())
        else {
            panic!("Chrome's picture at {scale} is beside the case");
        };
        let ours = &drawing.canvas;
        let side = 220 * u32::from(scale);
        assert_eq!((ours.width(), ours.height()), (side, side), "at {scale}");
        assert_eq!(
            (chrome.width(), chrome.height()),
            (side, side),
            "at {scale}"
        );
        let mut wrong = Vec::new();
        for y in 0..side {
            for x in 0..side {
                let (mine, theirs) = (
                    ours.at(x, y).map(Rgba::to_rgba8),
                    chrome.at(x, y).map(Rgba::to_rgba8),
                );
                if mine != theirs {
                    wrong.push(format!("({x}, {y}): {mine:?}, Chrome {theirs:?}"));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "at {scale}, {} pixels differ; the first: {}",
            wrong.len(),
            wrong.iter().take(8).cloned().collect::<Vec<_>>().join("; ")
        );
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = match drawn(&case, 2.0) {
        Ok(drawing) => drawing,
        Err(why) => panic!("{why}"),
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
