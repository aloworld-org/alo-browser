/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `<svg>`s laid out at tenths of a pixel, clipped and drawn on whole
//! device pixels: the case that closes the `<svg>`'s viewport of queue item
//! 589 (ADR 0057 § 2, *Measured since*).
//!
//! `cases/svgs-on-device-pixels` places an `<svg>` of each kind measured —
//! one whose shape runs past it, one whose shape is inside it, one with
//! `overflow: visible`, two with a `2.6px` border and a padding of
//! fractions, one fitting a `viewBox`, one in the flow inside a positioned
//! box, one on a line — and an `<img>` showing an SVG file, bare and framed,
//! each at fractions of a pixel. Chrome clips an `<svg>` to its content box
//! with each edge rounded to a whole device pixel on its own,
//! `floor(x·s + 0.5) / s`, and draws it from its border box's corner rounded
//! so, plus its border and padding as laid out; a `viewBox` is fitted to the
//! content box as laid out. An SVG file shown by an `<img>` is a picture:
//! drawn into its content box rounded so, and grown to fill it. This engine
//! drew and clipped every one of them where layout put it, at fractions.
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
use alo_paint::{Canvas, DisplayItem, Path};

/// Chrome's border box of each element, at one and at two, in 1/128 px:
/// left, top, width, height.
const CHROME: [(&str, [i32; 4], [i32; 4]); 10] = [
    (
        "clipped",
        [1318, 1356, 2610, 2598],
        [1318, 1356, 2611, 2598],
    ),
    ("inner", [6476, 1368, 2598, 2648], [6476, 1369, 2598, 2649]),
    (
        "visible",
        [11616, 1304, 2610, 2610],
        [11616, 1305, 2611, 2611],
    ),
    (
        "padded",
        [16696, 1350, 2890, 3076],
        [16697, 1350, 3019, 3205],
    ),
    (
        "offset",
        [23148, 1298, 2890, 3076],
        [23148, 1299, 3019, 3205],
    ),
    (
        "fitted",
        [29472, 1330, 2648, 2598],
        [29472, 1331, 2649, 2598],
    ),
    (
        "nested",
        [37170, 1356, 2560, 2560],
        [37171, 1356, 2560, 2560],
    ),
    (
        "piece",
        [43608, 1318, 2610, 2610],
        [43609, 1318, 2611, 2611],
    ),
    ("file", [1356, 7718, 2648, 2598], [1356, 7718, 2649, 2598]),
    ("framed", [6456, 7750, 2890, 3076], [6457, 7750, 3019, 3205]),
];

/// What each element's drawing is clipped to, at one and at two, in 1/128
/// px: its content box from Chrome's border box above, less the border and
/// the padding as laid out, each edge rounded as `floor(x·s + 0.5) / s`.
/// `visible` is not clipped.
const CLIPPED: [(&str, [i32; 4], [i32; 4]); 9] = [
    (
        "clipped",
        [1280, 1408, 2688, 2560],
        [1344, 1344, 2560, 2624],
    ),
    ("inner", [6528, 1408, 2560, 2560], [6464, 1344, 2624, 2688]),
    (
        "padded",
        [17152, 1792, 2048, 2048],
        [17216, 1856, 2112, 2048],
    ),
    (
        "offset",
        [23552, 1664, 2176, 2176],
        [23680, 1792, 2048, 2048],
    ),
    (
        "fitted",
        [29440, 1280, 2688, 2688],
        [29504, 1344, 2624, 2560],
    ),
    (
        "nested",
        [37120, 1408, 2560, 2560],
        [37184, 1344, 2560, 2560],
    ),
    (
        "piece",
        [43648, 1280, 2560, 2688],
        [43584, 1344, 2624, 2560],
    ),
    ("file", [1408, 7680, 2560, 2688], [1344, 7744, 2688, 2560]),
    ("framed", [6912, 8192, 2048, 2048], [6976, 8256, 2112, 2048]),
];

/// Where each element's one shape is drawn, before its clip, at one and at
/// two, in 1/128 px. For an `<svg>`: its border box's corner rounded as
/// above, plus the border and the padding as laid out, plus the shape's own
/// place, its `viewBox` fitted to the content box as laid out (`fitted`) —
/// so `offset`'s square starts a fraction into a pixel. For an SVG file:
/// its `viewBox` fitted to the rounded content box (`file`, `framed`).
const SHAPES: [(&str, [i32; 4], [i32; 4]); 10] = [
    ("clipped", [640, 768, 3840, 3840], [704, 704, 3840, 3840]),
    ("inner", [6784, 1664, 1536, 1536], [6720, 1600, 1536, 1536]),
    (
        "visible",
        [11264, 896, 3328, 3328],
        [11264, 896, 3328, 3328],
    ),
    (
        "padded",
        [16460, 1190, 3328, 3328],
        [16588, 1190, 3328, 3328],
    ),
    (
        "offset",
        [23884, 1958, 1280, 1280],
        [23948, 2022, 1280, 1280],
    ),
    (
        "fitted",
        [29970, 1800, 1324, 1299],
        [30034, 1864, 1324, 1299],
    ),
    (
        "nested",
        [37376, 1664, 1536, 1536],
        [37440, 1600, 1536, 1536],
    ),
    ("piece", [43008, 640, 3840, 3840], [42944, 704, 3840, 3840]),
    ("file", [1920, 8218, 1280, 1344], [1882, 8256, 1344, 1280]),
    ("framed", [7322, 8602, 1024, 1024], [7398, 8666, 1056, 1024]),
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("svgs-on-device-pixels"))
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

/// The smallest rectangle holding `path`.
fn bounds(path: &Path) -> Option<Rect> {
    let (left, top, right, bottom) = path.bounds()?;
    Some(Rect::new(left, top, right - left, bottom - top))
}

/// What the element with `class` clips its drawing to, as the display list
/// says; nothing for one that does not clip.
fn clip_of(rendering: &Rendering, class: &str) -> Option<Rect> {
    let id = box_of(rendering, class)?;
    rendering
        .drawing()?
        .display
        .items()
        .iter()
        .find_map(|item| match item {
            DisplayItem::PushClip { box_id, path } if *box_id == id => bounds(path),
            _ => None,
        })
}

/// Where the element with `class` draws its shape: the last thing it fills,
/// after its own background and border.
fn shape_of(rendering: &Rendering, class: &str) -> Option<Rect> {
    let id = box_of(rendering, class)?;
    rendering
        .drawing()?
        .display
        .items()
        .iter()
        .rev()
        .find_map(|item| match item {
            DisplayItem::Fill { box_id, path, .. } if *box_id == id => bounds(path),
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
fn every_svg_is_laid_out_where_chrome_lays_it_out() {
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

/// Each drawing is clipped to its content box with every edge on a whole
/// device pixel, at one and at two; `visible` is not clipped at all.
#[test]
fn every_drawing_is_clipped_on_the_device_pixels_chrome_clips_it_on() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Some(rendering) = drawn(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two) in CLIPPED {
            let wanted = if pick == 0 { at_one } else { at_two };
            let Some(rect) = clip_of(&rendering, class) else {
                panic!("{class}'s drawing is clipped at {scale}");
            };
            assert_eq!(units(rect), wanted, "{class} at {scale}");
        }
        assert!(
            clip_of(&rendering, "visible").is_none(),
            "visible is not clipped at {scale}"
        );
    }
}

/// Each shape is where Chrome draws it, to a unit of 1/128 px — the one
/// unit is `fitted`'s, a fifth of a width of tenths. `offset` starts a
/// fraction into a pixel: drawn from the content box rounded, it would be on
/// whole pixels. `file` and `framed` grow with their rounded box: drawn at
/// the box as laid out, they would be a fraction narrower.
#[test]
fn every_shape_is_drawn_where_chrome_draws_it() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Some(rendering) = drawn(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two) in SHAPES {
            let wanted = if pick == 0 { at_one } else { at_two };
            let Some(rect) = shape_of(&rendering, class) else {
                panic!("{class}'s shape is drawn at {scale}");
            };
            let found = units(rect);
            let off = found
                .iter()
                .zip(wanted)
                .map(|(found, wanted)| found.abs_diff(wanted))
                .max()
                .unwrap_or(u32::MAX);
            assert!(off <= 1, "{class} at {scale}: {found:?}, not {wanted:?}");
        }
    }
}

/// The case's plain colours: the page, the shapes, and the borders and
/// paddings.
const PLAIN: [(u8, u8, u8); 3] = [(255, 255, 255), (0, 0, 0), (0xc0, 0x39, 0x2b)];

/// Whether a pixel is one of [`PLAIN`].
fn is_plain(canvas: &Canvas, x: u32, y: u32) -> bool {
    canvas.at(x, y).is_some_and(|pixel| {
        let (red, green, blue, _) = pixel.to_rgba8();
        PLAIN.contains(&(red, green, blue))
    })
}

/// Wherever both pictures have one of the case's plain colours it is the
/// same, at one and at two: no clip, shape, border or padding is a pixel
/// from Chrome's. Where either smooths an edge across a pixel the two are
/// within 31 of 255 at one and 41 at two. The worst are at the corners of
/// `offset`'s, `fitted`'s and `framed`'s shapes, which lie a fraction into
/// a pixel both ways: Chrome covers such a pixel by the lesser of its two
/// edges' coverage, this engine by area (ADR 0057, item 565). Along a
/// straight edge the two are within 7, this engine's sampling of an edge
/// sixteen by sixteen (item 459).
#[test]
fn the_whole_picture_is_chromes() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, number, bound) in [(1.0, 1, 31), (2.0, 2, 41)] {
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
                if is_plain(ours, x, y) && is_plain(&chrome, x, y) {
                    assert_eq!(found, 0, "at {scale}, ({x}, {y}) is another colour");
                } else {
                    assert!(
                        found <= bound,
                        "at {scale}, ({x}, {y}) is {found} from Chrome's"
                    );
                }
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
