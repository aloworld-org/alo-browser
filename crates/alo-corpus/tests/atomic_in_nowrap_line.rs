/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An atomic box in a line that may not wrap stays on it, in numbers: the
//! case that closes queue item 447.
//!
//! `cases/atomic-in-nowrap-line` pins the page as files. This says what
//! Chrome 154 gives for the same bytes with the corpus's font: each row's
//! height, and for each `inline-block` how far across it stands, to within
//! Chrome's 1/64 px layout unit, and which line of its row it is on. Until
//! item 447 every box that did not fit started a new line, so the first five
//! rows were two lines tall here and one in Chrome.
//!
//! Each box stands half a pixel lower here than in Chrome, which floors the
//! half-leading below an `inline-block` to a whole pixel. That is the same
//! in every row and is not this item's, so only the line is asserted.

use alo_box::BoxId;
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("atomic-in-nowrap-line"))?;
    Rendering::of(&case).ok()
}

/// Every element that `wanted` picks, as its box and border box, in
/// document order.
fn all(rendering: &Rendering, wanted: impl Fn(&alo_dom::Element) -> bool) -> Vec<(BoxId, Rect)> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some_and(&wanted))
        .filter_map(|node| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|box_node| &box_node.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })?;
            Some((id, drawing.layout.get(id)?.border_box))
        })
        .collect()
}

/// Within Chrome's layout unit.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1.0 / 64.0
}

/// A row: its `id`, its height, and where across each of its boxes stands
/// and on which of its lines (20 px each).
type Row = (&'static str, f32, &'static [(f32, u8)]);

/// Each row, as Chrome 154 lays it out.
const ROWS: [Row; 9] = [
    ("kept", 20.0, &[(2115.0 / 64.0, 0)]),
    ("own", 20.0, &[(2115.0 / 64.0, 0)]),
    ("pair", 20.0, &[(8.0, 0), (68.0, 0)]),
    ("blocks-space", 20.0, &[(2115.0 / 64.0, 0)]),
    ("blocks-letter", 20.0, &[(2353.0 / 64.0, 0)]),
    ("spans-space", 40.0, &[(8.0, 1)]),
    ("wrapping", 60.0, &[(8.0, 1)]),
    ("wrapping-letter", 40.0, &[(8.0, 1)]),
    ("ended", 20.0, &[(2115.0 / 64.0, 0)]),
];

#[test]
fn each_box_is_on_the_line_chrome_puts_it_on() {
    let Some(rendering) = case() else {
        panic!("the case renders");
    };
    // Each row, and nothing else, has an `id`.
    let rows = all(&rendering, |element| element.attr("id").is_some());
    let boxes = all(&rendering, |element| {
        element
            .attr("class")
            .is_some_and(|classes| classes.split_whitespace().any(|name| name == "ib"))
    });
    assert_eq!(rows.len(), ROWS.len(), "{rows:?}");
    let mut boxes = boxes.iter();
    for ((_, row), (name, height, expected)) in rows.iter().zip(ROWS) {
        assert!(near(row.size.height, height), "{name}: {row:?}");
        for (x, line) in expected {
            let Some((_, placed)) = boxes.next() else {
                panic!("{name}: a box for each expected");
            };
            assert!(near(placed.origin.x, *x), "{name}: {placed:?}, not {x}");
            let on = ((placed.origin.y - row.origin.y) / 20.0).floor();
            assert!(near(on, f32::from(*line)), "{name}: {placed:?} on {on}");
        }
    }
    assert!(boxes.next().is_none(), "no box left over");
}

#[test]
fn a_box_kept_on_a_clipped_line_is_hidden_by_its_ellipsis() {
    let Some(rendering) = case() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let ended = all(&rendering, |element| element.attr("id") == Some("ended"));
    let boxes = all(&rendering, |element| element.attr("class") == Some("ib"));
    let (Some((row, _)), Some((kept, _))) = (ended.first(), boxes.last()) else {
        panic!("the ended row and its box");
    };
    assert!(drawing.layout.is_hidden(*kept), "the box is hidden whole");
    let ellipses = drawing.layout.ellipses(*row);
    let [only] = ellipses.as_slice() else {
        panic!("one ellipsis: {ellipses:?}");
    };
    // Straight after "ab ", where the box would have stood: in Chrome's
    // screenshot its three dots are in columns 35, 40 and 45–46.
    assert!(near(only.rect.origin.x, 2115.0 / 64.0), "{only:?}");
}
