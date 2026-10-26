/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<textarea>` scrolls across no further than its left padding and its
//! lines, in numbers: the case that closes queue item 674.
//!
//! `cases/a-textarea-scrolls-across-no-further-than-its-lines` pins the page
//! as files. This says what Chrome 155.0.8059.86 gives for the same bytes,
//! headless, its sheet inlined after the doctype and `monospace` set in the
//! corpus's `DejaVu Sans Mono` (measured once it had loaded, three runs
//! alike), 2026-10-26: each box's border box in sixty-fourths of a pixel,
//! and its `scrollWidth` and `scrollHeight`, which Chrome gives in whole
//! pixels. Until item 674 a textarea's scrolling area across held its right
//! padding after its lines, as a `div`'s does, and was 2 px wider than
//! Chrome's at 2 px of padding and 10 px at 10; the `div`s beside them still
//! hold theirs.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};

/// A box: its `id`, its border box (left, top, width, height) in
/// sixty-fourths, and its `scrollWidth` and `scrollHeight`.
type Row = (&'static str, [u32; 4], f64, f64);

/// Each box, as Chrome 155 lays it out and scrolls it.
const ROWS: [Row; 30] = [
    (
        "textarea-even-spaces-nowrap",
        [512, 512, 5504, 3584],
        89.0,
        54.0,
    ),
    (
        "textarea-even-spaces-pre",
        [512, 4480, 5504, 3584],
        89.0,
        54.0,
    ),
    ("div-even-spaces", [512, 8448, 5504, 3584], 91.0, 54.0),
    (
        "textarea-even-words-nowrap",
        [512, 12_416, 5504, 3584],
        137.0,
        54.0,
    ),
    (
        "textarea-even-words-pre",
        [512, 16_384, 5504, 3584],
        137.0,
        54.0,
    ),
    ("div-even-words", [512, 20_352, 5504, 3584], 139.0, 54.0),
    (
        "textarea-even-indent-nowrap",
        [512, 24_320, 5504, 3584],
        108.0,
        54.0,
    ),
    (
        "textarea-even-indent-pre",
        [512, 28_288, 5504, 3584],
        108.0,
        54.0,
    ),
    ("div-even-indent", [512, 32_256, 5504, 3584], 110.0, 54.0),
    ("div-even-block", [512, 36_224, 5504, 3584], 134.0, 54.0),
    (
        "textarea-right-spaces-nowrap",
        [512, 40_192, 6016, 3584],
        92.0,
        54.0,
    ),
    (
        "textarea-right-spaces-pre",
        [512, 44_160, 6016, 3584],
        92.0,
        54.0,
    ),
    ("div-right-spaces", [512, 48_128, 6016, 3584], 99.0, 54.0),
    (
        "textarea-right-words-nowrap",
        [512, 52_096, 6016, 3584],
        137.0,
        54.0,
    ),
    (
        "textarea-right-words-pre",
        [512, 56_064, 6016, 3584],
        137.0,
        54.0,
    ),
    ("div-right-words", [512, 60_032, 6016, 3584], 147.0, 54.0),
    (
        "textarea-right-indent-nowrap",
        [512, 64_000, 6016, 3584],
        108.0,
        54.0,
    ),
    (
        "textarea-right-indent-pre",
        [512, 67_968, 6016, 3584],
        108.0,
        54.0,
    ),
    ("div-right-indent", [512, 71_936, 6016, 3584], 118.0, 54.0),
    ("div-right-block", [512, 75_904, 6016, 3584], 142.0, 54.0),
    (
        "textarea-wide-spaces-nowrap",
        [512, 79_872, 6528, 4608],
        100.0,
        70.0,
    ),
    (
        "textarea-wide-spaces-pre",
        [512, 84_864, 6528, 4608],
        100.0,
        70.0,
    ),
    ("div-wide-spaces", [512, 89_856, 6528, 4608], 107.0, 70.0),
    (
        "textarea-wide-words-nowrap",
        [512, 94_848, 6528, 4608],
        145.0,
        70.0,
    ),
    (
        "textarea-wide-words-pre",
        [512, 99_840, 6528, 4608],
        145.0,
        70.0,
    ),
    ("div-wide-words", [512, 104_832, 6528, 4608], 155.0, 70.0),
    (
        "textarea-wide-indent-nowrap",
        [512, 109_824, 6528, 4608],
        116.0,
        70.0,
    ),
    (
        "textarea-wide-indent-pre",
        [512, 114_816, 6528, 4608],
        116.0,
        70.0,
    ),
    ("div-wide-indent", [512, 119_808, 6528, 4608], 126.0, 70.0),
    ("div-wide-block", [512, 124_800, 6528, 4608], 150.0, 70.0),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case =
        Case::read(&cases_directory().join("a-textarea-scrolls-across-no-further-than-its-lines"))?;
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
        assert!(
            (f64::from(area.width) - across).abs() <= 0.5,
            "{id}: it scrolls {} across, Chrome's {across}",
            area.width
        );
        assert!(
            near(area.height, down),
            "{id}: it scrolls {} down, Chrome's {down}",
            area.height
        );
    }
}
