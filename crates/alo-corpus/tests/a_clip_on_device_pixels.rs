/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Boxes that clip, laid out at tenths of a pixel, clipping on whole device
//! pixels: the case that closes the overflow clip of queue item 575 (ADR
//! 0057 § 2, *Measured since*).
//!
//! `cases/clips-on-device-pixels` places a box of each kind that clips — a
//! plain one, one with a background, a bordered one, one whose `2.6px`
//! border is floored, a rounded one, `overflow: clip`, `overflow-x` alone,
//! a box in the flow inside a positioned one, and an inline block on a line
//! — at fractions of a pixel, each holding a dark child that reaches 5 px
//! past it on every side. Chrome clips to the border box with each edge
//! rounded to a whole device pixel, `floor(x·s + 0.5) / s`, less the
//! border, which is where the box's own background is drawn; this engine
//! clipped to the padding box as laid out, so `backed`'s child stopped a
//! fraction short of its parent's background and left a sliver of it.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 140 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2` (never the emulated scale
//! of Chrome's developer tools), without `--disable-gpu`, taken
//! 2026-10-14. The boxes below are its `getBoundingClientRect()` in the
//! same runs.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::Rect;
use alo_paint::Canvas;

/// Chrome's border box of each box that clips, at one and at two, in
/// 1/128 px: left, top, width, height.
const CHROME: [(&str, [i32; 4], [i32; 4]); 9] = [
    ("plain", [1318, 1318, 2610, 2610], [1318, 1318, 2611, 2611]),
    ("backed", [6476, 1368, 2598, 2610], [6476, 1369, 2598, 2611]),
    (
        "bordered",
        [11608, 1304, 2598, 2598],
        [11609, 1305, 2598, 2598],
    ),
    (
        "floored",
        [16696, 1350, 2598, 2598],
        [16697, 1350, 2726, 2726],
    ),
    (
        "rounded",
        [21868, 1298, 2610, 2610],
        [21868, 1299, 2611, 2611],
    ),
    (
        "clipped",
        [26912, 1376, 2610, 2610],
        [26912, 1376, 2611, 2611],
    ),
    (
        "sideways",
        [32064, 1344, 2624, 2624],
        [32064, 1344, 2624, 2624],
    ),
    (
        "nested",
        [37170, 1356, 2610, 2610],
        [37171, 1356, 2611, 2611],
    ),
    ("piece", [1368, 7718, 3878, 2610], [1369, 7718, 3878, 2611]),
];

/// How far past a box's border box the device pixels compared reach, in
/// CSS pixels: past the 5 px its child spills, so the child's own edges,
/// which the clip hides, would show if the clip were not there.
const AROUND: f32 = 8.0;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("clips-on-device-pixels"))
}

/// The case drawn at `scale`.
fn drawn(case: &Case, scale: f32) -> Option<Rendering> {
    Rendering::at_scale(case, scale).ok()
}

/// The border box of the element with `class`, as laid out.
fn border_box(rendering: &Rendering, class: &str) -> Option<Rect> {
    let (document, drawing) = (rendering.document()?, rendering.drawing()?);
    let node = document.descendants(document.root()).find(|id| {
        document.element(*id).is_some_and(|element| {
            element
                .attr("class")
                .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
        })
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    Some(drawing.layout.get(id)?.border_box)
}

/// A rectangle in 1/128 px, as Chrome's numbers are written.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a box on a 500 × 140 page, in 1/128 px, is far inside an i32"
)]
fn units(rect: Rect) -> [i32; 4] {
    [rect.left(), rect.top(), rect.size.width, rect.size.height]
        .map(|value| (value * 128.0).round() as i32)
}

/// The picture Chrome took at `scale`, beside the case.
fn chrome(case: &Case, scale: u8) -> Option<Canvas> {
    let name = if scale == 1 {
        "chrome.png".to_owned()
    } else {
        format!("chrome-at-{scale}.png")
    };
    let bytes = std::fs::read(case.expectation(&name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// A device pixel's place, from a CSS coordinate at `scale`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a coordinate on a 500 × 140 page, clamped at zero"
)]
fn device(value: f32, scale: f32) -> u32 {
    (value * scale).floor().max(0.0) as u32
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

/// The largest difference from Chrome along the row and the column of
/// device pixels through the middle of `rect`, from `AROUND` before it to
/// `AROUND` after it, and where it is.
fn worst_across(
    ours: &Canvas,
    chrome: &Canvas,
    rect: Rect,
    scale: f32,
) -> (u8, Option<(u32, u32)>) {
    let row = device(rect.top() + rect.size.height / 2.0, scale);
    let column = device(rect.left() + rect.size.width / 2.0, scale);
    let across = device(rect.left() - AROUND, scale)..device(rect.right() + AROUND, scale);
    let down = device(rect.top() - AROUND, scale)..device(rect.bottom() + AROUND, scale);
    let points = across.map(|x| (x, row)).chain(down.map(|y| (column, y)));
    let mut worst = (0, None);
    for (x, y) in points {
        let found = difference(ours, chrome, x, y);
        if found > worst.0 {
            worst = (found, Some((x, y)));
        }
    }
    worst
}

#[test]
fn every_box_that_clips_is_laid_out_where_chrome_lays_it_out() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Some(rendering) = drawn(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two) in CHROME {
            let chrome = if pick == 0 { at_one } else { at_two };
            let Some(rect) = border_box(&rendering, class) else {
                panic!("{class} is laid out at {scale}");
            };
            assert_eq!(units(rect), chrome, "{class} at {scale}");
        }
    }
}

/// Every device pixel along each box's middle row and column, and 8 px
/// either side of it, is Chrome's exactly at one and at two: the clip's
/// edges are on the same whole device pixels, with nothing smoothed across
/// them. Clipping to the padding box as laid out left `plain`'s child a
/// fraction into its edge pixels, and `backed`'s red background showing in
/// them.
#[test]
fn every_clip_is_on_the_device_pixels_chrome_clips_on() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, number) in [(1.0, 1), (2.0, 2)] {
        let (Some(rendering), Some(chrome)) = (drawn(&case, scale), chrome(&case, number)) else {
            panic!("the case renders at {scale}, and Chrome's picture is beside it");
        };
        let Some(drawing) = rendering.drawing() else {
            panic!("the case is drawn at {scale}");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        for (class, ..) in CHROME {
            let Some(rect) = border_box(&rendering, class) else {
                panic!("{class} is laid out at {scale}");
            };
            let (worst, at) = worst_across(ours, &chrome, rect, scale);
            assert_eq!(worst, 0, "{class} at {scale}: {worst} of 255 at {at:?}");
        }
    }
}

/// How far past the rounded box's corners a curve's smoothing reaches, in
/// CSS pixels: its `4px` radius and one more.
const CURVE: f32 = 5.0;

/// The largest difference from Chrome at a curved corner, of 255, at one
/// and at two: the rounded box's dark child clipped over its own red
/// background, both smoothed along the curve. This engine samples a soft
/// edge sixteen by sixteen (queue item 459) and Chrome covers it by area;
/// the same curves drawn without a clip were 37 and 43 off in
/// `spread-shadow-corners`.
const AT_A_CURVE: [u8; 2] = [43, 51];

/// Every device pixel of the whole picture is Chrome's exactly, at one and
/// at two, except within `CURVE` of the rounded box's four corners, where
/// it is within `AT_A_CURVE`.
#[test]
fn the_whole_picture_is_chromes_but_for_the_curves() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, number) in [(1.0, 1_u8), (2.0, 2)] {
        let (Some(rendering), Some(chrome)) = (drawn(&case, scale), chrome(&case, number)) else {
            panic!("the case renders at {scale}, and Chrome's picture is beside it");
        };
        let (Some(drawing), Some(rounded)) =
            (rendering.drawing(), border_box(&rendering, "rounded"))
        else {
            panic!("the rounded box is drawn at {scale}");
        };
        let near = |at: u32, from: f32, to: f32| {
            let at = f32::from(u16::try_from(at).unwrap_or(u16::MAX)) / scale;
            (at >= from - 1.0 && at < from + CURVE) || (at >= to - CURVE && at < to + 1.0)
        };
        let bound = AT_A_CURVE[usize::from(number - 1)];
        let mut curved = 0;
        for y in 0..chrome.height() {
            for x in 0..chrome.width() {
                let found = difference(&drawing.canvas, &chrome, x, y);
                if near(x, rounded.left(), rounded.right())
                    && near(y, rounded.top(), rounded.bottom())
                {
                    assert!(found <= bound, "at {scale}, ({x}, {y}) is {found}");
                    curved += usize::from(found > 0);
                } else {
                    assert_eq!(found, 0, "at {scale}, ({x}, {y}) is {found} from Chrome's");
                }
            }
        }
        assert!(curved > 0, "at {scale} the curves were compared");
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = drawn(&case, 2.0).and_then(|rendering| rendering.drawing()) else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
