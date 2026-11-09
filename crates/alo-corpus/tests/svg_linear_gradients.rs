/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An SVG `<linearGradient>` as a fill and a stroke: the case that closes
//! queue item 275's first part (ADR 0107).
//!
//! `cases/svg-linear-gradients` draws eleven 40 px `<svg>`s on whole CSS
//! pixels over white — the defaults, a diagonal squashed by a wide box, a
//! translucent middle stop, `userSpaceOnUse`, `reflect`, `repeat`, a
//! `gradientTransform`, `fill-opacity`, a stroke, a turned shape and a
//! hard edge — and then the owner's icon at 80, whose teal swoosh is the
//! gradient that opened the item.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome
//! 157.0.8103.38's screenshots of the same bytes, headless, without
//! `--disable-gpu`, at `--force-device-scale-factor` 1 and 2, taken
//! 2026-11-09; `chrome-at-1.txt` and `chrome-at-2.txt` its boxes, the same
//! day. Inside every gradient this engine's colour is within one level of
//! Chrome's — the translucent stop included, which is how mixing as
//! straight colours was told from mixing premultiplied: premultiplied, the
//! middle of `g3` is 38 levels off (a mutation, tried). What differs more is a shape's
//! edge, which Chrome smooths in quarters of a pixel and this engine by the
//! area covered ([`EDGES`]).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::display::DisplayItem;
use alo_paint::{Canvas, Paint, Ramp};

/// Each drawing: its class, and its border box in CSS pixels — left, top,
/// width, height — as the case's sheet places it and Chrome measured it.
const DRAWINGS: [(&str, [f32; 4]); 12] = [
    ("g1", [10.0, 10.0, 40.0, 40.0]),
    ("g2", [60.0, 10.0, 80.0, 40.0]),
    ("g3", [150.0, 10.0, 40.0, 40.0]),
    ("g4", [200.0, 10.0, 40.0, 40.0]),
    ("g5", [250.0, 10.0, 40.0, 40.0]),
    ("g6", [10.0, 60.0, 40.0, 40.0]),
    ("g7", [60.0, 60.0, 40.0, 40.0]),
    ("g8", [110.0, 60.0, 40.0, 40.0]),
    ("g9", [160.0, 60.0, 40.0, 40.0]),
    ("g10", [210.0, 60.0, 40.0, 40.0]),
    ("g11", [260.0, 60.0, 40.0, 40.0]),
    ("logo", [10.0, 110.0, 80.0, 80.0]),
];

/// How far any pixel of a drawing may be from Chrome's, of 255, at one and
/// at two: the gradients that fill their whole box are within a level
/// everywhere; the others differ at their shapes' edges. Measured: at one,
/// `g8` 9, `g9` 26, `g10` 20 and the icon 80, the most at its thin arcs'
/// edges; at two, 7, 27, 37 and 89. Chrome's edge pixels on the icon are
/// half the navy over the cream again and again, where this engine's are
/// the area each covers.
const EDGES: [(&str, u8, u8); 12] = [
    ("g1", 1, 1),
    ("g2", 1, 1),
    ("g3", 1, 1),
    ("g4", 1, 1),
    ("g5", 1, 1),
    ("g6", 1, 1),
    ("g7", 1, 1),
    ("g8", 9, 7),
    ("g9", 26, 27),
    ("g10", 20, 37),
    ("g11", 0, 0),
    ("logo", 80, 89),
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("svg-linear-gradients"))
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

/// Every ramp the box with `class` is filled with, in order.
fn ramps_of(rendering: &Rendering, class: &str) -> Vec<Ramp> {
    let (Some(id), Some(drawing)) = (box_of(rendering, class), rendering.drawing()) else {
        return Vec::new();
    };
    drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill {
                box_id,
                paint: Paint::Ramp(ramp),
                ..
            } if *box_id == id => Some(ramp.clone()),
            _ => None,
        })
        .collect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0e-3
}

#[test]
fn each_drawing_is_laid_out_where_chrome_lays_it_out() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1.0_f32, 2.0] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        let Some(drawing) = rendering.drawing() else {
            panic!("drawn at {scale}");
        };
        for (class, [left, top, width, height]) in DRAWINGS {
            let found = box_of(&rendering, class)
                .and_then(|id| drawing.layout.get(id))
                .map(|geometry| geometry.border_box);
            assert!(
                found.is_some_and(|rect| close(rect.left(), left)
                    && close(rect.top(), top)
                    && close(rect.size.width, width)
                    && close(rect.size.height, height)),
                "{class} at {scale}: {found:?}"
            );
        }
    }
}

/// A drawing's class, and three points on the page with how far along its
/// gradient each is.
type Along = (&'static str, [((f32, f32), f32); 3]);

/// Where on the page each gradient is at nothing, half and one, and past.
const ALONG: [Along; 10] = [
    // Left to right across the box by default.
    (
        "g1",
        [
            ((10.0, 30.0), 0.0),
            ((30.0, 11.0), 0.5),
            ((50.0, 49.0), 1.0),
        ],
    ),
    // Corner to corner of an 80 by 40 box: the other two corners are
    // both half way, which is the squash.
    (
        "g2",
        [
            ((60.0, 10.0), 0.0),
            ((140.0, 10.0), 0.5),
            ((60.0, 50.0), 0.5),
        ],
    ),
    (
        "g3",
        [
            ((150.0, 10.0), 0.0),
            ((170.0, 30.0), 0.5),
            ((190.0, 10.0), 1.0),
        ],
    ),
    // From 10 to 30 user units into the box.
    (
        "g4",
        [
            ((210.0, 10.0), 0.0),
            ((220.0, 40.0), 0.5),
            ((230.0, 10.0), 1.0),
        ],
    ),
    // A quarter to a half of the box, and past it, run again.
    (
        "g5",
        [
            ((260.0, 10.0), 0.0),
            ((270.0, 10.0), 1.0),
            ((280.0, 30.0), 2.0),
        ],
    ),
    (
        "g6",
        [
            ((20.0, 60.0), 0.0),
            ((30.0, 60.0), 1.0),
            ((10.0, 60.0), -1.0),
        ],
    ),
    // Turned an eighth about the middle of the box: its lines of one colour
    // run along the other diagonal.
    (
        "g7",
        [
            ((80.0, 80.0), 0.5),
            ((70.0, 90.0), 0.5),
            ((108.284_27, 80.0), 1.0),
        ],
    ),
    // Top to bottom of the circle's box, 2 to 38.
    (
        "g8",
        [
            ((130.0, 62.0), 0.0),
            ((112.0, 80.0), 0.5),
            ((130.0, 98.0), 1.0),
        ],
    ),
    // Across the circle's box, 6 to 34, the stroke measured on it too.
    (
        "g9",
        [
            ((166.0, 80.0), 0.0),
            ((180.0, 0.0), 0.5),
            ((194.0, 80.0), 1.0),
        ],
    ),
    (
        "g11",
        [
            ((260.0, 60.0), 0.0),
            ((280.0, 60.0), 0.5),
            ((300.0, 60.0), 1.0),
        ],
    ),
];

#[test]
fn each_gradient_runs_where_its_attributes_say() {
    let Some(rendering) = case().and_then(|case| Rendering::at_scale(&case, 1.0).ok()) else {
        panic!("the case renders");
    };
    for (class, points) in ALONG {
        let ramps = ramps_of(&rendering, class);
        assert_eq!(ramps.len(), 1, "{class}");
        for ((x, y), wanted) in points {
            let found = ramps.first().map(|ramp| ramp.along(x, y));
            assert!(
                found.is_some_and(|found| close(found, wanted)),
                "{class} at ({x}, {y}): {found:?}, not {wanted}"
            );
        }
    }
    // The icon's one gradient, on its swoosh.
    assert_eq!(ramps_of(&rendering, "logo").len(), 1);
}

#[test]
fn a_turned_shapes_gradient_is_turned_with_it() {
    let Some(rendering) = case().and_then(|case| Rendering::at_scale(&case, 1.0).ok()) else {
        panic!("the case renders");
    };
    // A shape turned with its gradient: along the rectangle's own width,
    // turned a twelfth about the middle of a 20-unit drawing at two pixels
    // a unit.
    let ramps = ramps_of(&rendering, "g10");
    let turned = |x: f32, y: f32| {
        let (sin, cos) = 30.0_f32.to_radians().sin_cos();
        let (dx, dy) = (x - 10.0, y - 10.0);
        (
            210.0 + 2.0 * (10.0 + dx * cos - dy * sin),
            60.0 + 2.0 * (10.0 + dx * sin + dy * cos),
        )
    };
    for ((x, y), wanted) in [((5.0, 2.0), 0.0), ((10.0, 18.0), 0.5), ((15.0, 9.0), 1.0)] {
        let (px, py) = turned(x, y);
        let found = ramps.first().map(|ramp| ramp.along(px, py));
        assert!(
            found.is_some_and(|found| close(found, wanted)),
            "g10 at ({x}, {y}): {found:?}"
        );
    }
}

#[test]
fn nothing_in_the_case_is_left_undrawn() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let issues = std::fs::read_to_string(case.expectation("issues.txt")).unwrap_or_default();
    assert!(issues.trim().is_empty(), "{issues}");
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
fn every_pixel_is_chromes_within_a_level_and_every_edge_within_its_smoothing() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, name) in [(1.0_f32, "chrome.png"), (2.0, "chrome-at-2.png")] {
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
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "every length in the case is a small whole number"
        )]
        let device = |length: f32| (length * scale) as u32;
        for ((class, [left, top, width, height]), (named, at_one, at_two)) in
            DRAWINGS.iter().zip(EDGES)
        {
            assert_eq!(class, &named);
            let bound = if scale > 1.0 { at_two } else { at_one };
            let mut worst = 0;
            for y in device(*top)..device(top + height) {
                for x in device(*left)..device(left + width) {
                    worst = worst.max(difference(ours, &chrome, x, y));
                }
            }
            assert!(
                worst <= bound,
                "{class} at {scale} is {worst} from Chrome's, past {bound}"
            );
        }
        // Away from every edge — wherever a pixel and the eight around it
        // are within a few levels of each other in both pictures — the
        // whole page is within a level of Chrome's.
        let mut even = 0_usize;
        for y in 1..ours.height().saturating_sub(1) {
            for x in 1..ours.width().saturating_sub(1) {
                if !(evenly_around(ours, x, y) && evenly_around(&chrome, x, y)) {
                    continue;
                }
                even += 1;
                let found = difference(ours, &chrome, x, y);
                assert!(found <= 1, "at {scale} ({x}, {y}) is {found} from Chrome's");
            }
        }
        // Measured: 53,685 such pixels at one and 240,974 at two.
        let least = if scale > 1.0 { 240_000 } else { 53_000 };
        assert!(even >= least, "{even} pixels compared at {scale}");
    }
}

/// Whether the pixel at `(x, y)` and the eight around it are within
/// [`EVEN`] levels of it, in every channel: not at a shape's edge.
fn evenly_around(canvas: &Canvas, x: u32, y: u32) -> bool {
    let Some(middle) = canvas.at(x, y).map(alo_value::Rgba::to_rgba8) else {
        return false;
    };
    (x - 1..=x + 1).all(|across| {
        (y - 1..=y + 1).all(|down| {
            canvas
                .at(across, down)
                .map(alo_value::Rgba::to_rgba8)
                .is_some_and(|(red, green, blue, _)| {
                    red.abs_diff(middle.0) <= EVEN
                        && green.abs_diff(middle.1) <= EVEN
                        && blue.abs_diff(middle.2) <= EVEN
                })
        })
    })
}

/// How many levels a pixel's neighbour may differ by and the two still be
/// one even stretch of colour, less than any edge: the steepest gradients
/// (`g5` and `g6`, near 20 levels a pixel at one) are passed over here and
/// held to their drawings' bounds instead, which are a level.
const EVEN: u8 = 12;

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(drawing) = case()
        .and_then(|case| Rendering::at_scale(&case, 2.0).ok())
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at two");
    };
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
