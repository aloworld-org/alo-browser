/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An inline block under `overflow: clip` stands on its line, in numbers:
//! the case that closes queue item 667.
//!
//! `cases/a-clipped-box-stands-on-its-line` pins the page as files. This says
//! what Chrome 155.0.8059.86 gives for the same bytes, headless, its sheet
//! linked after the doctype and every element set in the corpus's `DejaVu
//! Sans`, measured 2026-10-26: in sixty-fourths of a pixel, each row's top
//! and height, where the label beside the box starts down, and the box's
//! border box. Until item 667 every `overflow` but `visible` stood a box on
//! its bottom margin edge, so under `clip` the label dropped to the bottom
//! of the box and every row was a descent taller.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// A row: the box's `id`, the row's top and height, the label's top, and
/// the box's border box (left, top, width, height), in sixty-fourths.
type Row = (&'static str, [u16; 2], u16, [u16; 4]);

/// Each row, as Chrome 155 lays it out.
const ROWS: [Row; 11] = [
    ("clip", [512, 2560], 1792, [1863, 512, 1920, 2560]),
    ("clip-x", [3712, 2560], 4992, [1863, 3712, 1920, 2560]),
    ("clip-y", [6912, 2560], 8192, [1863, 6912, 1920, 2560]),
    ("cut-off", [10112, 2560], 11392, [1863, 10112, 1920, 1280]),
    ("edged", [13312, 3328], 14784, [1863, 13312, 2304, 2944]),
    ("flex", [17280, 2560], 17920, [1863, 17280, 1920, 2560]),
    ("inside", [20480, 2560], 21760, [1863, 20480, 1920, 2560]),
    ("hidden", [23680, 2880], 25280, [1863, 23680, 1920, 2560]),
    (
        "clip-hidden",
        [27200, 2880],
        28800,
        [1863, 27200, 1920, 2560],
    ),
    ("text-area", [30720, 3584], 32192, [1863, 30720, 5504, 3584]),
    (
        "text-area-auto",
        [34944, 3904],
        37568,
        [1863, 34944, 5504, 3584],
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("a-clipped-box-stands-on-its-line"))?;
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
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1.0 / 64.0
}

fn pixels(units: u16) -> f32 {
    f32::from(units) / 64.0
}

#[test]
fn each_box_stands_where_chrome_stands_it() {
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
