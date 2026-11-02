/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pictures rounded by their own `border-radius`, laid out at fractions of
//! a pixel: the case that closes queue item 783.
//!
//! `cases/rounded-pictures-at-fractions` places 48 pictures of one black
//! 8 × 8 PNG, each at its own sixteenths of a pixel, with its own size,
//! padding, transparent border and radius (the case's style sheet says how
//! they were drawn). Chrome 156.0.8078.76 trims each to its content edge's
//! curve — each radius less the border and the padding beside it — laid on
//! its **snapped border box less the border and the padding as laid out**,
//! not on the picture's own box, whose edges are snapped one by one: the
//! two differ by under a device pixel, and the curve's straight sides cut
//! the picture's own edge pixels where they fall inside it (a padding of
//! 2.25 leaves a picture's first column 0.75 covered). Measured, not read
//! from its source, headless, without `--disable-gpu`, at one, one and a
//! half and two, on this case and on a second seed of 48 held out: with
//! the curve on the picture's own box the worst edge pixel was 252, 254
//! and 223 of 255 from Chrome's (median per picture 177, 138 and 130); laid
//! on the snapped border box it is within [`BOUND`] at every scale (median
//! 12 to 14) — Chrome's smoothing against this engine's, no more.
//!
//! `chrome.png`, `chrome-at-1.5.png` and `chrome-at-2.png` beside the case
//! are Chrome's screenshots of the same bytes, 500 by 380 CSS pixels, with
//! `--force-device-scale-factor` 1, 1.5 and 2, taken 2026-11-02.
//! `render-at-2.png` is this engine's picture at two.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::Rect;
use alo_paint::display::DisplayItem;
use alo_paint::path::Path;
use alo_paint::{Canvas, Corners, rounded_rectangle};

/// How far any pixel may be from Chrome's, of 255, at each scale, and the
/// picture Chrome took at it. Measured on this case: 20 at one, 24 at one
/// and a half, 21 at two — every one on a curve's or a cut edge's smoothed
/// pixel.
const BOUND: [(f32, &str, u8); 3] = [
    (1.0, "chrome.png", 20),
    (1.5, "chrome-at-1.5.png", 24),
    (2.0, "chrome-at-2.png", 21),
];

/// Two pictures' curves in numbers, at one and at two: the rectangle the
/// corners round, as left, top, width and height, and the radius — worked
/// out by hand from the case's style sheet.
///
/// `p0`: laid out at (10.9375, 10) with a border of 2, a padding of 2.75
/// and a radius of 6.1875, its border box 41.625 × 43.6875. Snapped, that
/// is 11–53 by 10–54 at one and 11–52.5 by 10–53.5 at two; less 4.75 on
/// each side, and a radius of 1.4375. Its picture is drawn into 16–48 by
/// 15–49 at one, inside the curve's rectangle on every side.
///
/// `p1`: at (70, 10), no border, a padding of 1.4375 and a radius of 8, its
/// border box 39.6875 × 33.75: snapped 70–110 by 10–44 at one and 70–109.5
/// by 10–44 at two, less 1.4375, and a radius of 6.5625.
const CURVES: [(&str, [f32; 4], [f32; 4], f32); 2] = [
    (
        "p0",
        [15.75, 14.75, 32.5, 34.5],
        [15.75, 14.75, 32.0, 34.0],
        1.4375,
    ),
    (
        "p1",
        [71.4375, 11.4375, 37.125, 31.125],
        [71.4375, 11.4375, 36.625, 31.125],
        6.5625,
    ),
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("rounded-pictures-at-fractions"))
}

/// The box of the element with `class`.
fn box_of(rendering: &Rendering, class: &str) -> Option<BoxId> {
    let (document, drawing) = (rendering.document()?, rendering.drawing()?);
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("class") == Some(class))
    })?;
    let boxes = &drawing.boxes;
    boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// The clip pushed for the box with `class`'s picture.
fn clip_of(rendering: &Rendering, class: &str) -> Option<Path> {
    let id = box_of(rendering, class)?;
    let drawing = rendering.drawing()?;
    drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::PushClip { box_id, path } if *box_id == id => Some(path.clone()),
        _ => None,
    })
}

#[test]
fn each_curve_is_laid_on_the_snapped_border_box_less_what_is_inside_it() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0_f32, 0), (2.0, 1)] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two, radius) in CURVES {
            let [left, top, width, height] = if pick == 0 { at_one } else { at_two };
            let expected =
                rounded_rectangle(Rect::new(left, top, width, height), Corners::all(radius));
            let Some(clip) = clip_of(&rendering, class) else {
                panic!("{class} at {scale} is not trimmed");
            };
            let close = |a: f32, b: f32| (a - b).abs() < 0.001;
            let same = clip.segments().len() == expected.segments().len()
                && clip.bounds().zip(expected.bounds()).is_some_and(|(a, b)| {
                    close(a.0, b.0) && close(a.1, b.1) && close(a.2, b.2) && close(a.3, b.3)
                })
                && format!("{:?}", clip.segments()) == format!("{:?}", expected.segments());
            assert!(
                same,
                "{class} at {scale} is trimmed to {:?}, not {:?}",
                clip.bounds(),
                expected.bounds()
            );
        }
    }
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
fn every_pixel_is_chromes_within_its_smoothing_at_one_one_and_a_half_and_two() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, name, bound) in BOUND {
        let drawing = Rendering::at_scale(&case, scale)
            .ok()
            .and_then(|rendering| rendering.drawing());
        let chrome = std::fs::read(case.expectation(name))
            .ok()
            .and_then(|bytes| alo_paint::from_png(&bytes).ok());
        let (Some(drawing), Some(chrome)) = (drawing, chrome) else {
            panic!("the case is drawn at {scale}, and {name} is beside it");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut worst = (0, (0, 0));
        let mut edges = 0;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                let found = difference(ours, &chrome, x, y);
                if found > worst.0 {
                    worst = (found, (x, y));
                }
                if found > 0 {
                    edges += 1;
                }
            }
        }
        assert!(
            worst.0 <= bound,
            "at {scale} ({}, {}) is {} from Chrome's",
            worst.1.0,
            worst.1.1,
            worst.0
        );
        // The curves were compared: every picture's corners differ a level
        // or more somewhere.
        assert!(
            edges > 48 * 4,
            "at {scale} only {edges} pixels differ at all"
        );
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = Rendering::at_scale(&case, 2.0)
        .ok()
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
