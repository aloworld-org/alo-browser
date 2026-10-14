/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pictures laid out at tenths of a pixel, drawn on whole device pixels:
//! the case that closes the picture's box of queue item 587 (ADR 0057 § 2,
//! *Measured since*).
//!
//! `cases/pictures-on-device-pixels` places an `<img>` of each kind
//! measured — at its own size, at a size set, with a `2px` border, with a
//! `2.6px` border that is floored, with a padding of fractions, in the flow
//! inside a positioned box, on a line at a width and its ratio — at
//! fractions of a pixel, and two
//! more that start off the page, one left of it and one above it. Chrome
//! draws a picture into its content box with each edge rounded to a whole
//! device pixel on its own, `floor(x·s + 0.5) / s`. This engine drew it
//! from the content box's left edge rounded and its width rounded, so one
//! picture in two was a pixel narrow or short against Chrome's; and a
//! picture starting off the page was slid back onto it whole.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 100 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2` (never the emulated scale
//! of Chrome's developer tools), without `--disable-gpu`, taken
//! 2026-10-14. The boxes below are its `getBoundingClientRect()` in the
//! same runs.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::Rect;
use alo_paint::{Canvas, DisplayItem};

/// Chrome's border box of each picture, at one and at two, in 1/128 px:
/// left, top, width, height.
const CHROME: [(&str, [i32; 4], [i32; 4]); 9] = [
    ("plain", [1318, 1318, 2560, 2560], [1318, 1318, 2560, 2560]),
    ("sized", [6476, 1368, 2598, 2648], [6476, 1369, 2598, 2649]),
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
        "padded",
        [21868, 1298, 2378, 2564],
        [21868, 1299, 2379, 2565],
    ),
    (
        "above",
        [26912, -704, 2610, 2610],
        [26912, -704, 2611, 2611],
    ),
    (
        "nested",
        [37170, 1356, 2610, 2610],
        [37171, 1356, 2611, 2611],
    ),
    ("beside", [-704, 7730, 2624, 2624], [-704, 7731, 2624, 2624]),
    ("piece", [3928, 7718, 3878, 3878], [3929, 7718, 3878, 3878]),
];

/// Where each picture is drawn, at one and at two, in 1/128 px: its content
/// box from Chrome's border box above, less the border and the padding as
/// laid out, each edge rounded as `floor(x·s + 0.5) / s`. Every one of them
/// is what Chrome's two pictures show.
const DRAWN: [(&str, [i32; 4], [i32; 4]); 9] = [
    ("plain", [1280, 1280, 2560, 2560], [1344, 1344, 2560, 2560]),
    ("sized", [6528, 1408, 2560, 2560], [6464, 1344, 2624, 2688]),
    (
        "bordered",
        [11904, 1536, 2048, 2048],
        [11840, 1536, 2112, 2112],
    ),
    (
        "floored",
        [16896, 1664, 2176, 2048],
        [17024, 1664, 2048, 2112],
    ),
    (
        "padded",
        [22016, 1408, 2176, 2176],
        [22080, 1472, 2048, 2048],
    ),
    (
        "above",
        [26880, -640, 2688, 2560],
        [26944, -704, 2560, 2624],
    ),
    (
        "nested",
        [37120, 1408, 2688, 2560],
        [37184, 1344, 2624, 2624],
    ),
    ("beside", [-640, 7680, 2560, 2688], [-704, 7744, 2624, 2624]),
    ("piece", [3968, 7680, 3840, 3968], [3904, 7744, 3904, 3840]),
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("pictures-on-device-pixels"))
}

/// The case drawn at `scale`.
fn drawn(case: &Case, scale: f32) -> Option<Rendering> {
    Rendering::at_scale(case, scale).ok()
}

/// The box of the element with `class`.
fn box_of(rendering: &Rendering, class: &str) -> Option<alo_box::BoxId> {
    let (document, drawing) = (rendering.document()?, rendering.drawing()?);
    let node = document.descendants(document.root()).find(|id| {
        document.element(*id).is_some_and(|element| {
            element
                .attr("class")
                .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
        })
    })?;
    let boxes = &drawing.boxes;
    boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// The border box of the element with `class`, as laid out.
fn border_box(rendering: &Rendering, class: &str) -> Option<Rect> {
    let id = box_of(rendering, class)?;
    Some(rendering.drawing()?.layout.get(id)?.border_box)
}

/// Where the picture of the element with `class` is drawn, as the display
/// list says.
fn picture_rect(rendering: &Rendering, class: &str) -> Option<Rect> {
    let id = box_of(rendering, class)?;
    rendering
        .drawing()?
        .display
        .items()
        .iter()
        .find_map(|item| match item {
            DisplayItem::Picture { box_id, rect, .. } if *box_id == id => Some(*rect),
            _ => None,
        })
}

/// A rectangle in 1/128 px, as Chrome's numbers are written.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a box on a 500 × 100 page, in 1/128 px, is far inside an i32"
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
fn every_picture_is_laid_out_where_chrome_lays_it_out() {
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

/// Each picture is drawn into its content box with every edge on a whole
/// device pixel, at one and at two. `padded` is the content box rounded on
/// its own: the snapped border box less a padding of fractions would leave
/// its edges between pixels. `above` and `beside` start where they start,
/// off the page.
#[test]
fn every_picture_is_drawn_on_the_device_pixels_chrome_draws_it_on() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Some(rendering) = drawn(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two) in DRAWN {
            let wanted = if pick == 0 { at_one } else { at_two };
            let Some(rect) = picture_rect(&rendering, class) else {
                panic!("{class}'s picture is drawn at {scale}");
            };
            assert_eq!(units(rect), wanted, "{class} at {scale}");
        }
    }
}

/// Every device pixel of the whole picture is Chrome's exactly, at one and
/// at two: each picture's edges, its border's and its padding's, and the
/// page beside them. Drawing a picture from its rounded left edge and its
/// rounded width left `sized` a device pixel short of Chrome's at its
/// right, and `beside`'s left five pixels were drawn over the page's edge.
#[test]
fn the_whole_picture_is_chromes() {
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
        for y in 0..chrome.height() {
            for x in 0..chrome.width() {
                let found = difference(ours, &chrome, x, y);
                assert_eq!(found, 0, "at {scale}, ({x}, {y}) is {found} from Chrome's");
            }
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
