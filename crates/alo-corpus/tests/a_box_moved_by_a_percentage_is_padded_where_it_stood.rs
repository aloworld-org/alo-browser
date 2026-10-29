/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A child moved by `position: relative` through a percentage, in a scroll
//! container, is moved as far as Chrome moves it and padded where it stood,
//! in numbers: the case that closes queue item 695.
//!
//! `cases/a-box-moved-by-a-percentage-is-padded-where-it-stood` pins the
//! page as files. This says what Chrome 156.0.8078.73 gives for the same
//! bytes, headless, its sheet linked after the doctype (two runs alike),
//! 2026-10-29: each box's border box and its child's, in sixty-fourths of a
//! pixel, the child's against the box's, and the box's `scrollWidth` and
//! `scrollHeight`, which Chrome gives in whole pixels. Until item 695 a
//! child moved by a percentage was taken as standing where it was moved to,
//! so it was padded there: 104 across for Chrome's 102 at 2 px of padding.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};

/// A box: its `id`; its border box (left, top, width, height) in
/// sixty-fourths; its child's border box, its left and top from the box's;
/// and the box's `scrollWidth` and `scrollHeight`.
type Row = (&'static str, [u32; 4], [i32; 4], f64, f64);

/// Each box, as Chrome 156 lays it out, moves its child and scrolls it.
const ROWS: [Row; 93] = [
    (
        "block-left-even",
        [512, 512, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-top-even",
        [512, 4480, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "block-both-even",
        [512, 8448, 5504, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-right-even",
        [512, 12_416, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-bottom-even",
        [512, 16_384, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "block-negleft-even",
        [512, 20_352, 5504, 3584],
        [-1088, 192, 6400, 1280],
        104.0,
        54.0,
    ),
    (
        "block-calc-even",
        [512, 24_320, 5504, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "block-leftwins-even",
        [512, 28_288, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-autotop-even",
        [512, 32_256, 5504, 1664],
        [192, 192, 3840, 1280],
        84.0,
        24.0,
    ),
    (
        "block-autobottom-even",
        [512, 34_304, 5504, 1664],
        [192, 2112, 3840, 1280],
        84.0,
        52.0,
    ),
    (
        "flex-left-even",
        [512, 36_352, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-top-even",
        [512, 40_320, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "flex-both-even",
        [512, 44_288, 5504, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-right-even",
        [512, 48_256, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-bottom-even",
        [512, 52_224, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "flex-negleft-even",
        [512, 56_192, 5504, 3584],
        [-1088, 192, 6400, 1280],
        104.0,
        54.0,
    ),
    (
        "flex-calc-even",
        [512, 60_160, 5504, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "flex-leftwins-even",
        [512, 64_128, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-autotop-even",
        [512, 68_096, 5504, 1664],
        [192, 192, 3840, 1280],
        84.0,
        24.0,
    ),
    (
        "flex-autobottom-even",
        [512, 70_144, 5504, 1664],
        [192, 2112, 3840, 1280],
        84.0,
        52.0,
    ),
    (
        "grid-left-even",
        [512, 72_192, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-top-even",
        [512, 76_160, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "grid-both-even",
        [512, 80_128, 5504, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-right-even",
        [512, 84_096, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-bottom-even",
        [512, 88_064, 5504, 3584],
        [192, 1792, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "grid-negleft-even",
        [512, 92_032, 5504, 3584],
        [-1408, 192, 6400, 1280],
        104.0,
        54.0,
    ),
    (
        "grid-calc-even",
        [512, 96_000, 5504, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "grid-leftwins-even",
        [512, 99_968, 5504, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-autotop-even",
        [512, 103_936, 5504, 1664],
        [192, 832, 3840, 1280],
        84.0,
        32.0,
    ),
    (
        "grid-autobottom-even",
        [512, 105_984, 5504, 1664],
        [192, 832, 3840, 1280],
        84.0,
        32.0,
    ),
    (
        "grid-area-even",
        [512, 108_032, 5504, 3584],
        [1472, 1152, 3840, 1280],
        84.0,
        54.0,
    ),
    (
        "block-left-right",
        [8192, 512, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-top-right",
        [8192, 4480, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "block-both-right",
        [8192, 8448, 6016, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-right-right",
        [8192, 12_416, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-bottom-right",
        [8192, 16_384, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "block-negleft-right",
        [8192, 20_352, 6016, 3584],
        [-1088, 192, 6400, 1280],
        112.0,
        54.0,
    ),
    (
        "block-calc-right",
        [8192, 24_320, 6016, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "block-leftwins-right",
        [8192, 28_288, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "block-autotop-right",
        [8192, 32_256, 6016, 1664],
        [192, 192, 3840, 1280],
        92.0,
        24.0,
    ),
    (
        "block-autobottom-right",
        [8192, 34_304, 6016, 1664],
        [192, 2112, 3840, 1280],
        92.0,
        52.0,
    ),
    (
        "flex-left-right",
        [8192, 36_352, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-top-right",
        [8192, 40_320, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "flex-both-right",
        [8192, 44_288, 6016, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-right-right",
        [8192, 48_256, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-bottom-right",
        [8192, 52_224, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "flex-negleft-right",
        [8192, 56_192, 6016, 3584],
        [-1088, 192, 6400, 1280],
        112.0,
        54.0,
    ),
    (
        "flex-calc-right",
        [8192, 60_160, 6016, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "flex-leftwins-right",
        [8192, 64_128, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "flex-autotop-right",
        [8192, 68_096, 6016, 1664],
        [192, 192, 3840, 1280],
        92.0,
        24.0,
    ),
    (
        "flex-autobottom-right",
        [8192, 70_144, 6016, 1664],
        [192, 2112, 3840, 1280],
        92.0,
        52.0,
    ),
    (
        "grid-left-right",
        [8192, 72_192, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-top-right",
        [8192, 76_160, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "grid-both-right",
        [8192, 80_128, 6016, 3584],
        [2752, 1792, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-right-right",
        [8192, 84_096, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-bottom-right",
        [8192, 88_064, 6016, 3584],
        [192, 1792, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "grid-negleft-right",
        [8192, 92_032, 6016, 3584],
        [-1408, 192, 6400, 1280],
        112.0,
        54.0,
    ),
    (
        "grid-calc-right",
        [8192, 96_000, 6016, 3584],
        [2112, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "grid-leftwins-right",
        [8192, 99_968, 6016, 3584],
        [2752, 192, 3840, 1280],
        102.0,
        54.0,
    ),
    (
        "grid-autotop-right",
        [8192, 103_936, 6016, 1664],
        [192, 832, 3840, 1280],
        92.0,
        32.0,
    ),
    (
        "grid-autobottom-right",
        [8192, 105_984, 6016, 1664],
        [192, 832, 3840, 1280],
        92.0,
        32.0,
    ),
    (
        "grid-area-right",
        [8192, 108_032, 6016, 3584],
        [1472, 1152, 3840, 1280],
        92.0,
        54.0,
    ),
    (
        "block-left-ten",
        [15_872, 512, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "block-top-ten",
        [15_872, 5504, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "block-both-ten",
        [15_872, 10_496, 6528, 4608],
        [3264, 2304, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "block-right-ten",
        [15_872, 15_488, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "block-bottom-ten",
        [15_872, 20_480, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "block-negleft-ten",
        [15_872, 25_472, 6528, 4608],
        [-576, 704, 6400, 1280],
        120.0,
        70.0,
    ),
    (
        "block-calc-ten",
        [15_872, 30_464, 6528, 4608],
        [2624, 1664, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "block-leftwins-ten",
        [15_872, 35_456, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "block-autotop-ten",
        [15_872, 40_448, 6528, 2688],
        [704, 704, 3840, 1280],
        100.0,
        40.0,
    ),
    (
        "block-autobottom-ten",
        [15_872, 43_520, 6528, 2688],
        [704, 2624, 3840, 1280],
        100.0,
        60.0,
    ),
    (
        "flex-left-ten",
        [15_872, 46_592, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "flex-top-ten",
        [15_872, 51_584, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "flex-both-ten",
        [15_872, 56_576, 6528, 4608],
        [3264, 2304, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "flex-right-ten",
        [15_872, 61_568, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "flex-bottom-ten",
        [15_872, 66_560, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "flex-negleft-ten",
        [15_872, 71_552, 6528, 4608],
        [-576, 704, 6400, 1280],
        120.0,
        70.0,
    ),
    (
        "flex-calc-ten",
        [15_872, 76_544, 6528, 4608],
        [2624, 1664, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "flex-leftwins-ten",
        [15_872, 81_536, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "flex-autotop-ten",
        [15_872, 86_528, 6528, 2688],
        [704, 704, 3840, 1280],
        100.0,
        40.0,
    ),
    (
        "flex-autobottom-ten",
        [15_872, 89_600, 6528, 2688],
        [704, 2624, 3840, 1280],
        100.0,
        60.0,
    ),
    (
        "grid-left-ten",
        [15_872, 92_672, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "grid-top-ten",
        [15_872, 97_664, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "grid-both-ten",
        [15_872, 102_656, 6528, 4608],
        [3264, 2304, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "grid-right-ten",
        [15_872, 107_648, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "grid-bottom-ten",
        [15_872, 112_640, 6528, 4608],
        [704, 2304, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "grid-negleft-ten",
        [15_872, 117_632, 6528, 4608],
        [-896, 704, 6400, 1280],
        120.0,
        70.0,
    ),
    (
        "grid-calc-ten",
        [15_872, 122_624, 6528, 4608],
        [2624, 1664, 3840, 1280],
        100.0,
        70.0,
    ),
    (
        "grid-leftwins-ten",
        [15_872, 127_616, 6528, 4608],
        [3264, 704, 3840, 1280],
        110.0,
        70.0,
    ),
    (
        "grid-autotop-ten",
        [15_872, 132_608, 6528, 2688],
        [704, 1344, 3840, 1280],
        100.0,
        40.0,
    ),
    (
        "grid-autobottom-ten",
        [15_872, 135_680, 6528, 2688],
        [704, 1344, 3840, 1280],
        100.0,
        40.0,
    ),
    (
        "grid-area-ten",
        [15_872, 138_752, 6528, 4608],
        [1984, 1664, 3840, 1280],
        100.0,
        70.0,
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(
        &cases_directory().join("a-box-moved-by-a-percentage-is-padded-where-it-stood"),
    )?;
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
    for (id, [left, top, width, height], [across, down, wide, tall], _, _) in ROWS {
        let held = box_of(&rendering, id).expect("the box is built");
        let border_box = drawing
            .layout
            .get(held)
            .expect("the box is laid out")
            .border_box;
        let child = child_of(&rendering, held).expect("the child is built");
        let moved = drawing
            .layout
            .get(child)
            .expect("the child is laid out")
            .border_box;
        let [left, top, width, height] = [left, top, width, height]
            .map(|units| i32::try_from(units).expect("a box's units fit"));
        for (ours, chrome, what) in [
            (border_box.origin.x, left, "left"),
            (border_box.origin.y, top, "top"),
            (border_box.size.width, width, "width"),
            (border_box.size.height, height, "height"),
            (moved.origin.x - border_box.origin.x, across, "child's left"),
            (moved.origin.y - border_box.origin.y, down, "child's top"),
            (moved.size.width, wide, "child's width"),
            (moved.size.height, tall, "child's height"),
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
