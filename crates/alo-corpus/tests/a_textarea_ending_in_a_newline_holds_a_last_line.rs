/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<textarea>` whose text ends in a newline holds a last, empty line, in
//! numbers: the case that closes queue item 670.
//!
//! `cases/a-textarea-ending-in-a-newline-holds-a-last-line` pins the page as
//! files. This says what Chrome 155.0.8059.86 gives for the same bytes,
//! headless, its sheet linked after the doctype and every element set in the
//! corpus's `DejaVu Sans` (measured once the font had loaded, three runs
//! alike), 2026-10-26: in sixty-fourths of a pixel, each row's top and
//! height, where the label beside the box starts down, and the box's border
//! box; and in pixels, how far each scrolling textarea scrolls. Until item
//! 670 such a textarea held only the lines a box would, so it stood a line
//! short, and a kept newline on a line holding nothing made no line at all.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// A row: the box's `id`, the row's top and height, the label's top, and
/// the box's border box (left, top, width, height), in sixty-fourths.
type Row = (&'static str, [u16; 2], u16, [u16; 4]);

/// Each row, as Chrome 155 lays it out.
const ROWS: [Row; 10] = [
    ("ring", [512, 3584], 704, [1863, 512, 5504, 3584]),
    ("ring-newline", [4736, 3584], 6208, [1863, 4736, 5504, 3584]),
    ("newline", [8960, 3584], 10432, [1863, 8960, 5504, 3584]),
    (
        "two-newlines",
        [13184, 4032],
        15936,
        [1863, 13184, 5504, 3584],
    ),
    (
        "two-lines-newline",
        [17856, 4032],
        20608,
        [1863, 17856, 5504, 3584],
    ),
    (
        "box-newline",
        [22528, 1664],
        22720,
        [1863, 22528, 5504, 1664],
    ),
    ("box-blank", [24832, 4224], 27584, [1863, 24832, 5504, 4224]),
    ("box-alone", [29696, 1664], 29888, [1863, 29696, 5504, 1664]),
    ("four", [32000, 3904], 34624, [1863, 32000, 5504, 3584]),
    (
        "four-newline",
        [36544, 3904],
        39168,
        [1863, 36544, 5504, 3584],
    ),
];

/// Each scrolling textarea's `scrollHeight` in Chrome 155: four lines of 20
/// inside 2 of padding above and below, and with the newline five.
const SCROLLED: [(&str, f32); 2] = [("four", 84.0), ("four-newline", 104.0)];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case =
        Case::read(&cases_directory().join("a-textarea-ending-in-a-newline-holds-a-last-line"))?;
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

#[test]
fn each_scrolling_textarea_scrolls_as_far_as_chromes() {
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("the case is drawn");
    for (id, chrome) in SCROLLED {
        let held = box_of(&rendering, id).expect("the textarea has a box");
        let geometry = drawing.layout.get(held).expect("the textarea is laid out");
        let ours = geometry.scrolling_area().height;
        assert!(
            near(ours, chrome),
            "{id}: it scrolls {ours} tall, Chrome's {chrome}"
        );
    }
}
