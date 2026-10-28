/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A scroll container pads its own content and not what spills out of the
//! boxes it holds, in numbers: the case that closes queue item 675.
//!
//! `cases/a-box-pads-its-childrens-boxes-not-their-spill` pins the page as
//! files. This says what Chrome 156.0.8078.73 gives for the same bytes,
//! headless, its sheet inlined after the doctype and `monospace` set in the
//! corpus's `DejaVu Sans Mono` (measured once it had loaded, three runs
//! alike), 2026-10-28: each box's border box in sixty-fourths of a pixel,
//! and its `scrollWidth` and `scrollHeight`, which Chrome gives in whole
//! pixels. Until item 675 a block, flex or grid container scrolled by its
//! end padding past whatever spilled out of a child, and no further than
//! its children's border boxes where Chrome pads their margin boxes: 2 px
//! too far across a child's spilling line at 2 px of padding and 10 at 10,
//! and 10 short of a child's margin.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};

/// A box: its `id`, its border box (left, top, width, height) in
/// sixty-fourths, and its `scrollWidth` and `scrollHeight`.
type Row = (&'static str, [u32; 4], f64, f64);

/// Each box, as Chrome 156 lays it out and scrolls it.
const ROWS: [Row; 66] = [
    ("block-line-even", [512, 512, 5504, 3584], 137.0, 54.0),
    ("block-block-even", [512, 4480, 5504, 3584], 132.0, 54.0),
    ("block-own-even", [512, 8448, 5504, 3584], 134.0, 54.0),
    ("block-tall-even", [512, 12_416, 5504, 3584], 84.0, 122.0),
    ("block-clip-even", [512, 16_384, 5504, 3584], 84.0, 54.0),
    ("block-margin-even", [512, 20_352, 5504, 3584], 94.0, 84.0),
    ("block-mixed-even", [512, 24_320, 5504, 3584], 137.0, 54.0),
    ("block-abs-even", [512, 28_288, 5504, 3584], 120.0, 70.0),
    (
        "block-negative-even",
        [512, 32_256, 5504, 3584],
        102.0,
        54.0,
    ),
    ("block-flat-even", [512, 36_224, 5504, 3584], 94.0, 54.0),
    ("block-scroller-even", [512, 40_192, 5504, 3584], 84.0, 54.0),
    ("flex-negative-even", [512, 44_160, 5504, 3584], 102.0, 54.0),
    ("flex-line-even", [512, 48_128, 5504, 3584], 137.0, 54.0),
    ("flex-block-even", [512, 52_096, 5504, 3584], 132.0, 54.0),
    ("flex-own-even", [512, 56_064, 5504, 3584], 134.0, 54.0),
    ("flex-tall-even", [512, 60_032, 5504, 3584], 84.0, 122.0),
    ("flex-margin-even", [512, 64_000, 5504, 3584], 94.0, 84.0),
    ("grid-line-even", [512, 67_968, 5504, 3584], 137.0, 54.0),
    ("grid-block-even", [512, 71_936, 5504, 3584], 132.0, 54.0),
    ("grid-own-even", [512, 75_904, 5504, 3584], 134.0, 54.0),
    ("grid-tall-even", [512, 79_872, 5504, 3584], 84.0, 122.0),
    ("grid-margin-even", [512, 83_840, 5504, 3584], 94.0, 84.0),
    ("block-line-right", [8192, 512, 6016, 3584], 137.0, 54.0),
    ("block-block-right", [8192, 4480, 6016, 3584], 132.0, 54.0),
    ("block-own-right", [8192, 8448, 6016, 3584], 142.0, 54.0),
    ("block-tall-right", [8192, 12_416, 6016, 3584], 92.0, 122.0),
    ("block-clip-right", [8192, 16_384, 6016, 3584], 92.0, 54.0),
    (
        "block-margin-right",
        [8192, 20_352, 6016, 3584],
        102.0,
        84.0,
    ),
    ("block-mixed-right", [8192, 24_320, 6016, 3584], 137.0, 54.0),
    ("block-abs-right", [8192, 28_288, 6016, 3584], 120.0, 70.0),
    (
        "block-negative-right",
        [8192, 32_256, 6016, 3584],
        102.0,
        54.0,
    ),
    ("block-flat-right", [8192, 36_224, 6016, 3584], 102.0, 54.0),
    (
        "block-scroller-right",
        [8192, 40_192, 6016, 3584],
        92.0,
        54.0,
    ),
    (
        "flex-negative-right",
        [8192, 44_160, 6016, 3584],
        102.0,
        54.0,
    ),
    ("flex-line-right", [8192, 48_128, 6016, 3584], 137.0, 54.0),
    ("flex-block-right", [8192, 52_096, 6016, 3584], 132.0, 54.0),
    ("flex-own-right", [8192, 56_064, 6016, 3584], 142.0, 54.0),
    ("flex-tall-right", [8192, 60_032, 6016, 3584], 92.0, 122.0),
    ("flex-margin-right", [8192, 64_000, 6016, 3584], 102.0, 84.0),
    ("grid-line-right", [8192, 67_968, 6016, 3584], 137.0, 54.0),
    ("grid-block-right", [8192, 71_936, 6016, 3584], 132.0, 54.0),
    ("grid-own-right", [8192, 75_904, 6016, 3584], 142.0, 54.0),
    ("grid-tall-right", [8192, 79_872, 6016, 3584], 92.0, 122.0),
    ("grid-margin-right", [8192, 83_840, 6016, 3584], 102.0, 84.0),
    ("block-line-wide", [15_872, 512, 6528, 4608], 145.0, 70.0),
    ("block-block-wide", [15_872, 5504, 6528, 4608], 140.0, 70.0),
    ("block-own-wide", [15_872, 10_496, 6528, 4608], 150.0, 70.0),
    (
        "block-tall-wide",
        [15_872, 15_488, 6528, 4608],
        100.0,
        130.0,
    ),
    ("block-clip-wide", [15_872, 20_480, 6528, 4608], 100.0, 70.0),
    (
        "block-margin-wide",
        [15_872, 25_472, 6528, 4608],
        110.0,
        100.0,
    ),
    (
        "block-mixed-wide",
        [15_872, 30_464, 6528, 4608],
        145.0,
        70.0,
    ),
    ("block-abs-wide", [15_872, 35_456, 6528, 4608], 120.0, 70.0),
    (
        "block-negative-wide",
        [15_872, 40_448, 6528, 4608],
        110.0,
        70.0,
    ),
    ("block-flat-wide", [15_872, 45_440, 6528, 4608], 110.0, 70.0),
    (
        "block-scroller-wide",
        [15_872, 50_432, 6528, 4608],
        100.0,
        70.0,
    ),
    (
        "flex-negative-wide",
        [15_872, 55_424, 6528, 4608],
        110.0,
        70.0,
    ),
    ("flex-line-wide", [15_872, 60_416, 6528, 4608], 145.0, 70.0),
    ("flex-block-wide", [15_872, 65_408, 6528, 4608], 140.0, 70.0),
    ("flex-own-wide", [15_872, 70_400, 6528, 4608], 150.0, 70.0),
    ("flex-tall-wide", [15_872, 75_392, 6528, 4608], 100.0, 130.0),
    (
        "flex-margin-wide",
        [15_872, 80_384, 6528, 4608],
        110.0,
        100.0,
    ),
    ("grid-line-wide", [15_872, 85_376, 6528, 4608], 145.0, 70.0),
    ("grid-block-wide", [15_872, 90_368, 6528, 4608], 140.0, 70.0),
    ("grid-own-wide", [15_872, 95_360, 6528, 4608], 150.0, 70.0),
    (
        "grid-tall-wide",
        [15_872, 100_352, 6528, 4608],
        100.0,
        130.0,
    ),
    (
        "grid-margin-wide",
        [15_872, 105_344, 6528, 4608],
        110.0,
        100.0,
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case =
        Case::read(&cases_directory().join("a-box-pads-its-childrens-boxes-not-their-spill"))?;
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
