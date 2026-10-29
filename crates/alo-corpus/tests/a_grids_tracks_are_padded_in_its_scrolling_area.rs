/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A grid that scrolls reaches its tracks with its end padding after them,
//! and its items' border boxes unpadded, in numbers: the case that closes
//! queue item 694.
//!
//! `cases/a-grids-tracks-are-padded-in-its-scrolling-area` pins the page as
//! files. This says what Chrome 156.0.8078.73 gives for the same bytes,
//! headless, its sheet linked after the doctype (two runs alike),
//! 2026-10-29: each box's border box and its child's, in sixty-fourths of a
//! pixel, the child's against the box's, and the box's `scrollWidth` and
//! `scrollHeight`, which Chrome gives in whole pixels. Until item 694 a
//! grid's tracks were not in its scrolling area and its items' margin boxes
//! were padded: 84 across for Chrome's 124 with one column 120 wide, and 134
//! for Chrome's 102 with a child 100 wide and 30 of margin after it.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};

/// A box: its `id`; its border box (left, top, width, height) in
/// sixty-fourths; its child's border box, its left and top from the box's,
/// where it has a child; and the box's `scrollWidth` and `scrollHeight`.
type Row = (&'static str, [u32; 4], Option<[i32; 4]>, f64, f64);

/// Each box, as Chrome 156 lays it out and scrolls it.
const ROWS: [Row; 63] = [
    (
        "grid-cols-even",
        [512, 512, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-rows-even",
        [512, 4480, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        84.0,
        94.0,
    ),
    (
        "grid-both-even",
        [512, 8448, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        94.0,
    ),
    (
        "grid-gap-even",
        [512, 12_416, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        114.0,
        54.0,
    ),
    (
        "grid-rowgap-even",
        [512, 16_384, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        84.0,
        74.0,
    ),
    (
        "grid-implicit-even",
        [512, 20_352, 5504, 3584],
        Some([5312, 192, 1280, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-autorows-even",
        [512, 24_320, 5504, 3584],
        Some([192, 2752, 3840, 1280]),
        84.0,
        84.0,
    ),
    (
        "grid-minmax-even",
        [512, 28_288, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-percent-even",
        [512, 32_256, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-fr-even",
        [512, 36_224, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        84.0,
        54.0,
    ),
    (
        "grid-center-even",
        [512, 40_192, 5504, 3584],
        Some([-1088, 192, 3840, 1280]),
        104.0,
        54.0,
    ),
    (
        "grid-end-even",
        [512, 44_160, 5504, 3584],
        Some([2752, 192, 3840, 1280]),
        102.0,
        54.0,
    ),
    (
        "grid-spacebetween-even",
        [512, 48_128, 5504, 3584],
        Some([192, 192, 1280, 1280]),
        84.0,
        54.0,
    ),
    (
        "grid-narrow-even",
        [512, 52_096, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        84.0,
        54.0,
    ),
    (
        "grid-empty-even",
        [512, 56_064, 5504, 3584],
        None,
        124.0,
        94.0,
    ),
    (
        "grid-margin-even",
        [512, 60_032, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-absolute-even",
        [512, 64_000, 5504, 3584],
        Some([64, 64, 3840, 1280]),
        124.0,
        54.0,
    ),
    (
        "grid-sized-even",
        [512, 67_968, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        124.0,
        94.0,
    ),
    (
        "grid-overmargin-even",
        [512, 71_936, 5504, 3584],
        Some([192, 192, 6400, 1280]),
        102.0,
        54.0,
    ),
    (
        "grid-undermargin-even",
        [512, 75_904, 5504, 3584],
        Some([192, 192, 3840, 3840]),
        84.0,
        62.0,
    ),
    (
        "nested-even",
        [512, 79_872, 5504, 3584],
        Some([192, 192, 3840, 1280]),
        122.0,
        54.0,
    ),
    (
        "grid-cols-right",
        [8192, 512, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-rows-right",
        [8192, 4480, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        92.0,
        94.0,
    ),
    (
        "grid-both-right",
        [8192, 8448, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        94.0,
    ),
    (
        "grid-gap-right",
        [8192, 12_416, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        122.0,
        54.0,
    ),
    (
        "grid-rowgap-right",
        [8192, 16_384, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        92.0,
        74.0,
    ),
    (
        "grid-implicit-right",
        [8192, 20_352, 6016, 3584],
        Some([5312, 192, 1280, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-autorows-right",
        [8192, 24_320, 6016, 3584],
        Some([192, 2752, 3840, 1280]),
        92.0,
        84.0,
    ),
    (
        "grid-minmax-right",
        [8192, 28_288, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-percent-right",
        [8192, 32_256, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-fr-right",
        [8192, 36_224, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        92.0,
        54.0,
    ),
    (
        "grid-center-right",
        [8192, 40_192, 6016, 3584],
        Some([-1088, 192, 3840, 1280]),
        112.0,
        54.0,
    ),
    (
        "grid-end-right",
        [8192, 44_160, 6016, 3584],
        Some([2752, 192, 3840, 1280]),
        102.0,
        54.0,
    ),
    (
        "grid-spacebetween-right",
        [8192, 48_128, 6016, 3584],
        Some([192, 192, 1280, 1280]),
        92.0,
        54.0,
    ),
    (
        "grid-narrow-right",
        [8192, 52_096, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        92.0,
        54.0,
    ),
    (
        "grid-empty-right",
        [8192, 56_064, 6016, 3584],
        None,
        132.0,
        94.0,
    ),
    (
        "grid-margin-right",
        [8192, 60_032, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-absolute-right",
        [8192, 64_000, 6016, 3584],
        Some([64, 64, 3840, 1280]),
        132.0,
        54.0,
    ),
    (
        "grid-sized-right",
        [8192, 67_968, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        132.0,
        94.0,
    ),
    (
        "grid-overmargin-right",
        [8192, 71_936, 6016, 3584],
        Some([192, 192, 6400, 1280]),
        102.0,
        54.0,
    ),
    (
        "grid-undermargin-right",
        [8192, 75_904, 6016, 3584],
        Some([192, 192, 3840, 3840]),
        92.0,
        62.0,
    ),
    (
        "nested-right",
        [8192, 79_872, 6016, 3584],
        Some([192, 192, 3840, 1280]),
        122.0,
        54.0,
    ),
    (
        "grid-cols-wide",
        [15_872, 512, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-rows-wide",
        [15_872, 5504, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        100.0,
        110.0,
    ),
    (
        "grid-both-wide",
        [15_872, 10_496, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        110.0,
    ),
    (
        "grid-gap-wide",
        [15_872, 15_488, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        130.0,
        70.0,
    ),
    (
        "grid-rowgap-wide",
        [15_872, 20_480, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        100.0,
        90.0,
    ),
    (
        "grid-implicit-wide",
        [15_872, 25_472, 6528, 4608],
        Some([5824, 704, 1280, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-autorows-wide",
        [15_872, 30_464, 6528, 4608],
        Some([704, 3264, 3840, 1280]),
        100.0,
        100.0,
    ),
    (
        "grid-minmax-wide",
        [15_872, 35_456, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-percent-wide",
        [15_872, 40_448, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-fr-wide",
        [15_872, 45_440, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        100.0,
        70.0,
    ),
    (
        "grid-center-wide",
        [15_872, 50_432, 6528, 4608],
        Some([-576, 704, 3840, 1280]),
        120.0,
        70.0,
    ),
    (
        "grid-end-wide",
        [15_872, 55_424, 6528, 4608],
        Some([3264, 704, 3840, 1280]),
        110.0,
        70.0,
    ),
    (
        "grid-spacebetween-wide",
        [15_872, 60_416, 6528, 4608],
        Some([704, 704, 1280, 1280]),
        100.0,
        70.0,
    ),
    (
        "grid-narrow-wide",
        [15_872, 65_408, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        100.0,
        70.0,
    ),
    (
        "grid-empty-wide",
        [15_872, 70_400, 6528, 4608],
        None,
        140.0,
        110.0,
    ),
    (
        "grid-margin-wide",
        [15_872, 75_392, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-absolute-wide",
        [15_872, 80_384, 6528, 4608],
        Some([64, 64, 3840, 1280]),
        140.0,
        70.0,
    ),
    (
        "grid-sized-wide",
        [15_872, 85_376, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        140.0,
        110.0,
    ),
    (
        "grid-overmargin-wide",
        [15_872, 90_368, 6528, 4608],
        Some([704, 704, 6400, 1280]),
        110.0,
        70.0,
    ),
    (
        "grid-undermargin-wide",
        [15_872, 95_360, 6528, 4608],
        Some([704, 704, 3840, 3840]),
        100.0,
        70.0,
    ),
    (
        "nested-wide",
        [15_872, 100_352, 6528, 4608],
        Some([704, 704, 3840, 1280]),
        130.0,
        70.0,
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case =
        Case::read(&cases_directory().join("a-grids-tracks-are-padded-in-its-scrolling-area"))?;
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

/// The box of an element's one child element.
fn child_of(rendering: &Rendering, parent: BoxId) -> Option<BoxId> {
    let drawing = rendering.drawing()?;
    drawing.boxes.children(parent).find(|child| {
        matches!(
            drawing.boxes.get(*child).map(|held| &held.kind),
            Some(BoxKind::Element { .. })
        )
    })
}

/// Within Chrome's layout unit.
fn near(ours: f32, chrome: f64) -> bool {
    (f64::from(ours) - chrome).abs() <= 1.0 / 64.0
}

fn pixels(units: i32) -> f64 {
    f64::from(units) / 64.0
}

#[test]
fn each_box_and_its_child_stand_where_chrome_stands_them() {
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("the case is drawn");
    for (id, [left, top, width, height], child, _, _) in ROWS {
        let held = box_of(&rendering, id).expect("the box is built");
        let border_box = drawing
            .layout
            .get(held)
            .expect("the box is laid out")
            .border_box;
        let [left, top, width, height] = [left, top, width, height]
            .map(|units| i32::try_from(units).expect("a box's units fit"));
        let mut sides = vec![
            (border_box.origin.x, left, "left"),
            (border_box.origin.y, top, "top"),
            (border_box.size.width, width, "width"),
            (border_box.size.height, height, "height"),
        ];
        match child {
            Some([across, down, wide, tall]) => {
                let child = child_of(&rendering, held).expect("the child is built");
                let placed = drawing
                    .layout
                    .get(child)
                    .expect("the child is laid out")
                    .border_box;
                sides.extend([
                    (
                        placed.origin.x - border_box.origin.x,
                        across,
                        "child's left",
                    ),
                    (placed.origin.y - border_box.origin.y, down, "child's top"),
                    (placed.size.width, wide, "child's width"),
                    (placed.size.height, tall, "child's height"),
                ]);
            }
            None => assert!(child_of(&rendering, held).is_none(), "{id} holds a child"),
        }
        for (ours, chrome, what) in sides {
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
    for (id, across, down) in ROWS.map(|(id, _, _, across, down)| (id, across, down)) {
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
