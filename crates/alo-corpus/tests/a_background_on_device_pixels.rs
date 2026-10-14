/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Backgrounds laid out at tenths of a pixel, drawn on whole device pixels:
//! the case that closes queue item 565 (ADR 0057 § 2, *Measured since*).
//!
//! `cases/backgrounds-on-device-pixels` places a box of each kind Chrome
//! 154 was measured snapping — a colour, a colour on a half, a border, a
//! gradient, an inset shadow, a spread shadow, a shadow moved by its
//! offset, a rounded box, a box under a translation, a box inside a box, a
//! piece of an inline box, and a row of flex items meeting at fractions —
//! at fractions of a pixel. Chrome draws each from its border box with
//! every edge rounded to a whole device pixel, `floor(x·s + 0.5) / s`; a
//! shadow is that box grown and then moved by its offset, and a translated
//! box is snapped where it would be without its transform, then moved.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 140 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2` (never the emulated scale
//! of Chrome's developer tools), the sans-serif family set in the corpus's
//! `DejaVu Sans`, without `--disable-gpu`, taken 2026-10-13. The boxes
//! below are its `getBoundingClientRect()` in the same runs.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::Rect;
use alo_paint::Canvas;

/// Chrome's border box of each box at one and at two, in 1/128 px: left,
/// top, width, height. `moved`'s is without its `translate(0.5px,
/// 0.25px)`, which `getBoundingClientRect()` includes and layout does not.
const CHROME: [(&str, [i32; 4], [i32; 4]); 12] = [
    ("colour", [1318, 1318, 2610, 2610], [1318, 1318, 2611, 2611]),
    ("half", [5184, 1344, 2624, 2624], [5184, 1344, 2624, 2624]),
    (
        "bordered",
        [9048, 1304, 2598, 2598],
        [9049, 1305, 2598, 2598],
    ),
    (
        "gradient",
        [14104, 1356, 2598, 2610],
        [14105, 1356, 2598, 2611],
    ),
    (
        "inset",
        [17996, 1394, 2636, 2610],
        [17996, 1395, 2636, 2611],
    ),
    (
        "spread",
        [23090, 1336, 2584, 2610],
        [23091, 1337, 2585, 2611],
    ),
    (
        "offset",
        [28192, 1376, 2584, 2610],
        [28192, 1376, 2585, 2611],
    ),
    (
        "rounded",
        [33388, 1298, 2610, 2610],
        [33388, 1299, 2611, 2611],
    ),
    (
        "moved",
        [38400, 1280, 2598, 2598],
        [38400, 1280, 2598, 2598],
    ),
    (
        "nested",
        [43608, 1368, 2584, 2584],
        [43609, 1369, 2585, 2585],
    ),
    ("piece", [1324, 7730, 5196, 1920], [1324, 7731, 5196, 1920]),
    (
        "row",
        [1304, 12838, 10544, 2598],
        [1305, 12838, 10544, 2598],
    ),
];

/// How far past a box's border box the device pixels compared reach, in
/// CSS pixels: past its spread and its shadow's offset.
const AROUND: f32 = 8.0;

/// How far from Chrome's a pixel along a box's middle may be, of 255.
///
/// Every edge here is on a whole device pixel, Chrome's and this engine's,
/// and is drawn exactly — except the shadow moved by `4.3px 2.6px`, whose
/// edges are a fraction into a pixel because the offset is added after the
/// box is snapped. Chrome covers such a pixel by its exact area (0.6, so
/// 115); this engine samples an edge sixteen by sixteen (queue item 459),
/// 0.625, so 109. Snapping the box after moving it instead would fill that
/// row solid, 23, which is 92 from Chrome's: far outside the bound.
fn bound(class: &str) -> u8 {
    if class == "offset" { 6 } else { 0 }
}

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("backgrounds-on-device-pixels"))
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
fn every_box_is_laid_out_where_chrome_lays_it_out() {
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

#[test]
fn every_box_is_drawn_on_the_device_pixels_chrome_draws_it_on() {
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
            assert!(
                worst <= bound(class),
                "{class} at {scale}: {worst} of 255 from Chrome's at {at:?}"
            );
        }
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

/// How far a spread shadow's corner reaches past its box, in CSS pixels:
/// the case's spread, and one more so a rounded corner's edge is inside
/// what is compared.
const SPREAD: f32 = 4.0;

/// The spread shadow's corners are square, as Chrome draws them (queue
/// item 573): every device pixel within `SPREAD` of the box's four corners
/// and outside the box is Chrome's exactly. They were rounded by the
/// spread, which left the corner pixel near white where Chrome's is the
/// shadow's colour.
#[test]
fn the_spread_shadow_has_square_corners_as_chromes_has() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, number) in [(1.0, 1), (2.0, 2)] {
        let (Some(rendering), Some(chrome)) = (drawn(&case, scale), chrome(&case, number)) else {
            panic!("the case renders at {scale}, and Chrome's picture is beside it");
        };
        let (Some(drawing), Some(rect)) = (rendering.drawing(), border_box(&rendering, "spread"))
        else {
            panic!("the spread box is drawn at {scale}");
        };
        let across = device(rect.left() - SPREAD, scale)..device(rect.right() + SPREAD, scale);
        let down = device(rect.top() - SPREAD, scale)..device(rect.bottom() + SPREAD, scale);
        let near_a_corner = |at: u32, from: f32, to: f32| {
            at < device(from + SPREAD, scale) || at >= device(to - SPREAD, scale)
        };
        let mut compared = 0;
        for y in down {
            for x in across.clone() {
                if !near_a_corner(x, rect.left(), rect.right())
                    || !near_a_corner(y, rect.top(), rect.bottom())
                {
                    continue;
                }
                let found = difference(&drawing.canvas, &chrome, x, y);
                assert_eq!(found, 0, "at {scale}, ({x}, {y}) is {found} from Chrome's");
                compared += 1;
            }
        }
        assert!(
            compared >= 4 * 64,
            "at {scale} only {compared} were compared"
        );
    }
}
