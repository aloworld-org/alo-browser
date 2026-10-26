/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An empty `<textarea>` under `overflow: clip` stands on a line, in
//! numbers: the case that closes queue item 669.
//!
//! `cases/an-empty-clipped-textarea-stands-on-a-line` pins the page as
//! files. This says what Chrome 155.0.8059.86 gives for the same bytes,
//! headless, its sheet linked after the doctype and every element set in the
//! corpus's `DejaVu Sans` (measured once the font had loaded), 2026-10-26: in
//! sixty-fourths of a pixel, each row's top and height, where the label
//! beside the textarea starts down, and the textarea's border box. Until
//! item 669 an empty textarea recorded no line, so under `clip` it stood on
//! its bottom margin edge and the label dropped to its foot.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// A row: the textarea's `id`, the row's top and height, the label's top,
/// and the textarea's border box (left, top, width, height), in
/// sixty-fourths.
type Row = (&'static str, [u16; 2], u16, [u16; 4]);

/// Each row, as Chrome 155 lays it out.
const ROWS: [Row; 9] = [
    ("clip", [512, 3584], 704, [1863, 512, 5504, 3584]),
    ("clip-filled", [4736, 3584], 4928, [1863, 4736, 5504, 3584]),
    ("normal", [8960, 3584], 9152, [1863, 8960, 5504, 3584]),
    (
        "normal-filled",
        [13184, 3584],
        13376,
        [1863, 13184, 5504, 3584],
    ),
    ("plain", [17408, 2304], 17408, [1863, 17408, 10624, 2304]),
    (
        "plain-filled",
        [20352, 2304],
        20352,
        [1863, 20352, 10624, 2304],
    ),
    ("padded", [23296, 4096], 24000, [1863, 23296, 5504, 4096]),
    ("hinted", [28032, 3584], 28224, [1863, 28032, 5504, 3584]),
    ("auto", [32256, 3904], 34880, [1863, 32256, 5504, 3584]),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("an-empty-clipped-textarea-stands-on-a-line"))?;
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

/// The row holding the textarea of `id`, the top of its label's text, and
/// the textarea's border box.
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
