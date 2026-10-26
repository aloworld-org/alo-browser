/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<textarea>` keeps its newlines and spaces whatever its `white-space`,
//! in numbers: the case that closes queue item 673.
//!
//! `cases/a-textarea-keeps-its-newlines-whatever-its-white-space` pins the
//! page as files. This says what Chrome 155.0.8059.86 gives for the same
//! bytes, headless, its sheet inlined after the doctype, the labels set in
//! the corpus's `DejaVu Sans` and the textareas in its `DejaVu Sans Mono`
//! (measured once both had loaded, three runs alike), 2026-10-26: in
//! sixty-fourths of a pixel, each row's top and height, where the label
//! beside the textarea starts down, and the textarea's border box; and in
//! pixels, how far each scrolling textarea's lines reach across and how far
//! it scrolls down. Until
//! item 673 a textarea under `normal` or `nowrap` collapsed its newlines
//! and spaces as any box would, and one under `pre-line` its spaces, so it
//! held a line fewer or stood on a shorter one.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// A row: the textarea's `id`, the row's top and height, the label's top,
/// and the textarea's border box (left, top, width, height), in
/// sixty-fourths.
type Row = (&'static str, [u32; 2], u32, [u32; 4]);

/// Each row, as Chrome 155 lays it out.
const ROWS: [Row; 34] = [
    (
        "normal-newline-clip",
        [512, 3584],
        1984,
        [1863, 512, 5504, 3584],
    ),
    (
        "normal-newline-auto",
        [4736, 3904],
        7360,
        [1863, 4736, 5504, 3584],
    ),
    (
        "normal-spaces-clip",
        [9280, 3584],
        10_752,
        [1863, 9280, 5504, 3584],
    ),
    (
        "normal-spaces-auto",
        [13_504, 3904],
        16_128,
        [1863, 13_504, 5504, 3584],
    ),
    (
        "normal-words-clip",
        [18_048, 4032],
        20_800,
        [1863, 18_048, 5504, 3584],
    ),
    (
        "normal-words-auto",
        [22_720, 3904],
        25_344,
        [1863, 22_720, 5504, 3584],
    ),
    (
        "nowrap-newline-clip",
        [27_264, 3584],
        28_736,
        [1863, 27_264, 5504, 3584],
    ),
    (
        "nowrap-newline-auto",
        [31_488, 3904],
        34_112,
        [1863, 31_488, 5504, 3584],
    ),
    (
        "nowrap-spaces-clip",
        [36_032, 3584],
        36_224,
        [1863, 36_032, 5504, 3584],
    ),
    (
        "nowrap-spaces-auto",
        [40_256, 3904],
        42_880,
        [1863, 40_256, 5504, 3584],
    ),
    (
        "nowrap-words-clip",
        [44_800, 3584],
        44_992,
        [1863, 44_800, 5504, 3584],
    ),
    (
        "nowrap-words-auto",
        [49_024, 3904],
        51_648,
        [1863, 49_024, 5504, 3584],
    ),
    (
        "nowrap-indent-clip",
        [53_568, 3584],
        55_040,
        [1863, 53_568, 5504, 3584],
    ),
    (
        "nowrap-indent-auto",
        [57_792, 3904],
        60_416,
        [1863, 57_792, 5504, 3584],
    ),
    (
        "pre-line-newline-clip",
        [62_336, 3584],
        63_808,
        [1863, 62_336, 5504, 3584],
    ),
    (
        "pre-line-newline-auto",
        [66_560, 3904],
        69_184,
        [1863, 66_560, 5504, 3584],
    ),
    (
        "pre-line-spaces-clip",
        [71_104, 3584],
        72_576,
        [1863, 71_104, 5504, 3584],
    ),
    (
        "pre-line-spaces-auto",
        [75_328, 3904],
        77_952,
        [1863, 75_328, 5504, 3584],
    ),
    (
        "pre-line-words-clip",
        [79_872, 4032],
        82_624,
        [1863, 79_872, 5504, 3584],
    ),
    (
        "pre-line-words-auto",
        [84_544, 3904],
        87_168,
        [1863, 84_544, 5504, 3584],
    ),
    (
        "pre-newline-clip",
        [89_088, 3584],
        90_560,
        [1863, 89_088, 5504, 3584],
    ),
    (
        "pre-newline-auto",
        [93_312, 3904],
        95_936,
        [1863, 93_312, 5504, 3584],
    ),
    (
        "pre-spaces-clip",
        [97_856, 3584],
        98_048,
        [1863, 97_856, 5504, 3584],
    ),
    (
        "pre-spaces-auto",
        [102_080, 3904],
        104_704,
        [1863, 102_080, 5504, 3584],
    ),
    (
        "pre-words-clip",
        [106_624, 3584],
        106_816,
        [1863, 106_624, 5504, 3584],
    ),
    (
        "pre-words-auto",
        [110_848, 3904],
        113_472,
        [1863, 110_848, 5504, 3584],
    ),
    (
        "pre-indent-clip",
        [115_392, 3584],
        116_864,
        [1863, 115_392, 5504, 3584],
    ),
    (
        "pre-indent-auto",
        [119_616, 3904],
        122_240,
        [1863, 119_616, 5504, 3584],
    ),
    (
        "pre-wrap-newline-clip",
        [124_160, 3584],
        125_632,
        [1863, 124_160, 5504, 3584],
    ),
    (
        "pre-wrap-newline-auto",
        [128_384, 3904],
        131_008,
        [1863, 128_384, 5504, 3584],
    ),
    (
        "pre-wrap-spaces-clip",
        [132_928, 3584],
        134_400,
        [1863, 132_928, 5504, 3584],
    ),
    (
        "pre-wrap-spaces-auto",
        [137_152, 3904],
        139_776,
        [1863, 137_152, 5504, 3584],
    ),
    (
        "pre-wrap-words-clip",
        [141_696, 4032],
        144_448,
        [1863, 141_696, 5504, 3584],
    ),
    (
        "pre-wrap-words-auto",
        [146_368, 3904],
        148_992,
        [1863, 146_368, 5504, 3584],
    ),
];

/// Each scrolling textarea's `scrollWidth` and `scrollHeight` in Chrome
/// 155, which it gives in whole pixels. Across, that is the widest of its
/// padding box and its left padding with the room its lines take — Chrome
/// adds no padding after them, where this engine's scrolling area does
/// (queue item 674), so it is the lines' room that is asserted here. Down,
/// it is its lines with its padding above and below.
const SCROLLED: [(&str, f64, f64); 17] = [
    ("normal-newline-auto", 84.0, 54.0),
    ("normal-spaces-auto", 84.0, 54.0),
    ("normal-words-auto", 84.0, 64.0),
    ("nowrap-newline-auto", 84.0, 54.0),
    ("nowrap-spaces-auto", 89.0, 54.0),
    ("nowrap-words-auto", 137.0, 54.0),
    ("nowrap-indent-auto", 108.0, 54.0),
    ("pre-line-newline-auto", 84.0, 54.0),
    ("pre-line-spaces-auto", 84.0, 54.0),
    ("pre-line-words-auto", 84.0, 64.0),
    ("pre-newline-auto", 84.0, 54.0),
    ("pre-spaces-auto", 89.0, 54.0),
    ("pre-words-auto", 137.0, 54.0),
    ("pre-indent-auto", 108.0, 54.0),
    ("pre-wrap-newline-auto", 84.0, 54.0),
    ("pre-wrap-spaces-auto", 84.0, 54.0),
    ("pre-wrap-words-auto", 84.0, 64.0),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(
        &cases_directory().join("a-textarea-keeps-its-newlines-whatever-its-white-space"),
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

/// The row holding the box of `id`, the top of its label's text, and the
/// box's border box.
fn row_of(rendering: &Rendering, id: &str) -> Option<(Rect, f32, Rect)> {
    let drawing = rendering.drawing()?;
    let held = box_of(rendering, id)?;
    let row = drawing
        .boxes
        .ids()
        .find(|each| drawing.boxes.children(*each).any(|child| child == held))?;
    let label = drawing
        .boxes
        .descendants(row)
        .into_iter()
        .find(|each| drawing.boxes.get(*each).and_then(|node| node.text()) == Some("Ab"))?;
    let label_top = drawing.layout.fragments(label).first()?.rect.origin.y;
    Some((
        drawing.layout.get(row)?.border_box,
        label_top,
        drawing.layout.get(held)?.border_box,
    ))
}

/// Within Chrome's layout unit.
fn near(ours: f32, chrome: f64) -> bool {
    (f64::from(ours) - chrome).abs() <= 1.0 / 64.0
}

fn pixels(units: u32) -> f64 {
    f64::from(units) / 64.0
}

#[test]
fn each_textarea_stands_where_chrome_stands_it() {
    let rendering = case().expect("the case renders");
    for (id, [row_top, row_height], label_top, [left, top, width, height]) in ROWS {
        let (row, label, border_box) = row_of(&rendering, id).expect("the row is laid out");
        for (ours, chrome, what) in [
            (row.origin.y, row_top, "row's top"),
            (row.size.height, row_height, "row's height"),
            (label, label_top, "label's top"),
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
fn each_scrolling_textarea_scrolls_as_far_as_chromes() {
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("the case is drawn");
    for (id, across, down) in SCROLLED {
        let held = box_of(&rendering, id).expect("the textarea has a box");
        let geometry = drawing.layout.get(held).expect("the textarea is laid out");
        // Its padding box: inside its border of 1, 84 wide.
        let padding_left = geometry.border_box.origin.x + 1.0;
        let padding_width = geometry.border_box.size.width - 2.0;
        let lines_end = drawing
            .boxes
            .descendants(held)
            .into_iter()
            .filter(|each| {
                drawing
                    .boxes
                    .get(*each)
                    .and_then(|node| node.text())
                    .is_some()
            })
            .flat_map(|each| drawing.layout.fragments(each).iter())
            .map(|piece| piece.rect.origin.x + piece.rect.size.width)
            .fold(padding_left, f32::max);
        let ours = (lines_end - padding_left).max(padding_width);
        // Chrome gives a whole pixel, so ours is within half of one of it.
        assert!(
            (f64::from(ours) - across).abs() <= 0.5,
            "{id}: its lines reach {ours} across, Chrome's {across}"
        );
        let height = geometry.scrolling_area().height;
        assert!(
            near(height, down),
            "{id}: it scrolls {height} tall, Chrome's {down}"
        );
    }
}
