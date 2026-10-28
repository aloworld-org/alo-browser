/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A child moved by `position: relative` in a scroll container is padded
//! where it stood and taken as it is where it was moved to, in numbers: the
//! case that closes queue item 693.
//!
//! `cases/a-moved-box-is-padded-where-it-stood` pins the page as files.
//! This says what Chrome 156.0.8078.73 gives for the same bytes, headless,
//! its sheet inlined after the doctype (three runs alike), 2026-10-29: each
//! box's border box in sixty-fourths of a pixel, and its `scrollWidth` and
//! `scrollHeight`, which Chrome gives in whole pixels. Until item 693 a
//! child's margin box was padded where it had been moved to: 2 px too far
//! for a child moved 60 right at 2 px of padding (124 for Chrome's 122),
//! and 10 too far at 10 px.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};

/// A box: its `id`, its border box (left, top, width, height) in
/// sixty-fourths, and its `scrollWidth` and `scrollHeight`.
type Row = (&'static str, [u32; 4], f64, f64);

/// Each box, as Chrome 156 lays it out and scrolls it.
const ROWS: [Row; 81] = [
    ("block-left-even", [512, 512, 5_504, 3_584], 122.0, 54.0),
    ("block-top-even", [512, 4_480, 5_504, 3_584], 84.0, 82.0),
    ("block-both-even", [512, 8_448, 5_504, 3_584], 122.0, 82.0),
    ("block-right-even", [512, 12_416, 5_504, 3_584], 122.0, 54.0),
    ("block-bottom-even", [512, 16_384, 5_504, 3_584], 84.0, 82.0),
    (
        "block-negleft-even",
        [512, 20_352, 5_504, 3_584],
        104.0,
        54.0,
    ),
    (
        "block-negtop-even",
        [512, 24_320, 5_504, 3_584],
        104.0,
        54.0,
    ),
    (
        "block-margin-even",
        [512, 28_288, 5_504, 3_584],
        122.0,
        54.0,
    ),
    (
        "block-leftwins-even",
        [512, 32_256, 5_504, 3_584],
        122.0,
        54.0,
    ),
    ("flex-left-even", [512, 36_224, 5_504, 3_584], 122.0, 54.0),
    ("flex-top-even", [512, 40_192, 5_504, 3_584], 84.0, 82.0),
    ("flex-both-even", [512, 44_160, 5_504, 3_584], 122.0, 82.0),
    ("flex-right-even", [512, 48_128, 5_504, 3_584], 122.0, 54.0),
    ("flex-bottom-even", [512, 52_096, 5_504, 3_584], 84.0, 82.0),
    (
        "flex-negleft-even",
        [512, 56_064, 5_504, 3_584],
        104.0,
        54.0,
    ),
    ("flex-negtop-even", [512, 60_032, 5_504, 3_584], 104.0, 54.0),
    ("flex-margin-even", [512, 64_000, 5_504, 3_584], 122.0, 54.0),
    (
        "flex-leftwins-even",
        [512, 67_968, 5_504, 3_584],
        122.0,
        54.0,
    ),
    ("grid-left-even", [512, 71_936, 5_504, 3_584], 122.0, 54.0),
    ("grid-top-even", [512, 75_904, 5_504, 3_584], 84.0, 82.0),
    ("grid-both-even", [512, 79_872, 5_504, 3_584], 122.0, 82.0),
    ("grid-right-even", [512, 83_840, 5_504, 3_584], 122.0, 54.0),
    ("grid-bottom-even", [512, 87_808, 5_504, 3_584], 84.0, 82.0),
    (
        "grid-negleft-even",
        [512, 91_776, 5_504, 3_584],
        104.0,
        54.0,
    ),
    ("grid-negtop-even", [512, 95_744, 5_504, 3_584], 104.0, 54.0),
    ("grid-margin-even", [512, 99_712, 5_504, 3_584], 122.0, 54.0),
    (
        "grid-leftwins-even",
        [512, 103_680, 5_504, 3_584],
        122.0,
        54.0,
    ),
    ("block-left-right", [8_192, 512, 6_016, 3_584], 122.0, 54.0),
    ("block-top-right", [8_192, 4_480, 6_016, 3_584], 92.0, 82.0),
    (
        "block-both-right",
        [8_192, 8_448, 6_016, 3_584],
        122.0,
        82.0,
    ),
    (
        "block-right-right",
        [8_192, 12_416, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "block-bottom-right",
        [8_192, 16_384, 6_016, 3_584],
        92.0,
        82.0,
    ),
    (
        "block-negleft-right",
        [8_192, 20_352, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "block-negtop-right",
        [8_192, 24_320, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "block-margin-right",
        [8_192, 28_288, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "block-leftwins-right",
        [8_192, 32_256, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "flex-left-right",
        [8_192, 36_224, 6_016, 3_584],
        122.0,
        54.0,
    ),
    ("flex-top-right", [8_192, 40_192, 6_016, 3_584], 92.0, 82.0),
    (
        "flex-both-right",
        [8_192, 44_160, 6_016, 3_584],
        122.0,
        82.0,
    ),
    (
        "flex-right-right",
        [8_192, 48_128, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "flex-bottom-right",
        [8_192, 52_096, 6_016, 3_584],
        92.0,
        82.0,
    ),
    (
        "flex-negleft-right",
        [8_192, 56_064, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "flex-negtop-right",
        [8_192, 60_032, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "flex-margin-right",
        [8_192, 64_000, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "flex-leftwins-right",
        [8_192, 67_968, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "grid-left-right",
        [8_192, 71_936, 6_016, 3_584],
        122.0,
        54.0,
    ),
    ("grid-top-right", [8_192, 75_904, 6_016, 3_584], 92.0, 82.0),
    (
        "grid-both-right",
        [8_192, 79_872, 6_016, 3_584],
        122.0,
        82.0,
    ),
    (
        "grid-right-right",
        [8_192, 83_840, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "grid-bottom-right",
        [8_192, 87_808, 6_016, 3_584],
        92.0,
        82.0,
    ),
    (
        "grid-negleft-right",
        [8_192, 91_776, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "grid-negtop-right",
        [8_192, 95_744, 6_016, 3_584],
        112.0,
        54.0,
    ),
    (
        "grid-margin-right",
        [8_192, 99_712, 6_016, 3_584],
        122.0,
        54.0,
    ),
    (
        "grid-leftwins-right",
        [8_192, 103_680, 6_016, 3_584],
        122.0,
        54.0,
    ),
    ("block-left-wide", [15_872, 512, 6_528, 4_608], 130.0, 70.0),
    ("block-top-wide", [15_872, 5_504, 6_528, 4_608], 100.0, 90.0),
    (
        "block-both-wide",
        [15_872, 10_496, 6_528, 4_608],
        130.0,
        90.0,
    ),
    (
        "block-right-wide",
        [15_872, 15_488, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "block-bottom-wide",
        [15_872, 20_480, 6_528, 4_608],
        100.0,
        90.0,
    ),
    (
        "block-negleft-wide",
        [15_872, 25_472, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "block-negtop-wide",
        [15_872, 30_464, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "block-margin-wide",
        [15_872, 35_456, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "block-leftwins-wide",
        [15_872, 40_448, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "flex-left-wide",
        [15_872, 45_440, 6_528, 4_608],
        130.0,
        70.0,
    ),
    ("flex-top-wide", [15_872, 50_432, 6_528, 4_608], 100.0, 90.0),
    (
        "flex-both-wide",
        [15_872, 55_424, 6_528, 4_608],
        130.0,
        90.0,
    ),
    (
        "flex-right-wide",
        [15_872, 60_416, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "flex-bottom-wide",
        [15_872, 65_408, 6_528, 4_608],
        100.0,
        90.0,
    ),
    (
        "flex-negleft-wide",
        [15_872, 70_400, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "flex-negtop-wide",
        [15_872, 75_392, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "flex-margin-wide",
        [15_872, 80_384, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "flex-leftwins-wide",
        [15_872, 85_376, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "grid-left-wide",
        [15_872, 90_368, 6_528, 4_608],
        130.0,
        70.0,
    ),
    ("grid-top-wide", [15_872, 95_360, 6_528, 4_608], 100.0, 90.0),
    (
        "grid-both-wide",
        [15_872, 100_352, 6_528, 4_608],
        130.0,
        90.0,
    ),
    (
        "grid-right-wide",
        [15_872, 105_344, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "grid-bottom-wide",
        [15_872, 110_336, 6_528, 4_608],
        100.0,
        90.0,
    ),
    (
        "grid-negleft-wide",
        [15_872, 115_328, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "grid-negtop-wide",
        [15_872, 120_320, 6_528, 4_608],
        120.0,
        70.0,
    ),
    (
        "grid-margin-wide",
        [15_872, 125_312, 6_528, 4_608],
        130.0,
        70.0,
    ),
    (
        "grid-leftwins-wide",
        [15_872, 130_304, 6_528, 4_608],
        130.0,
        70.0,
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("a-moved-box-is-padded-where-it-stood"))?;
    Rendering::of(&case).ok()
}

/// The box of the element whose `id` is `id`.
fn box_of(rendering: &Rendering, id: &str) -> Option<BoxId> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))?;
    drawing.boxes.ids().find(|box_id| {
        matches!(
            drawing.boxes.get(*box_id).map(|held| &held.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// Within Chrome's layout unit.
fn near(ours: f32, chrome: f64) -> bool {
    (f64::from(ours) - chrome).abs() <= 1.0 / 64.0
}

fn pixels(units: u32) -> f64 {
    f64::from(units) / 64.0
}

#[test]
fn each_box_stands_where_chrome_stands_it() {
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("the case is drawn");
    for (id, [left, top, width, height], _, _) in ROWS {
        let held = box_of(&rendering, id).expect("the box is built");
        let border_box = drawing
            .layout
            .get(held)
            .expect("the box is laid out")
            .border_box;
        for (ours, chrome, what) in [
            (border_box.origin.x, left, "left"),
            (border_box.origin.y, top, "top"),
            (border_box.size.width, width, "width"),
            (border_box.size.height, height, "height"),
        ] {
            assert!(
                near(ours, pixels(chrome)),
                "{id}: its {what} is {ours}, Chrome's {}",
                pixels(chrome)
            );
        }
    }
}

#[test]
fn each_box_scrolls_as_far_as_chromes() {
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("the case is drawn");
    for (id, across, down) in ROWS.map(|(id, _, across, down)| (id, across, down)) {
        let held = box_of(&rendering, id).expect("the box is built");
        let area = drawing
            .layout
            .get(held)
            .expect("the box is laid out")
            .scrolling_area();
        // Chrome gives a whole pixel, so ours is within half of one of it.
        for (ours, chrome, what) in [(area.width, across, "across"), (area.height, down, "down")] {
            assert!(
                (f64::from(ours) - chrome).abs() <= 0.5,
                "{id}: it scrolls {ours} {what}, Chrome's {chrome}"
            );
        }
    }
}
