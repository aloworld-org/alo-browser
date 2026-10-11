/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where two boxes meet, what came before decides whether the line may
//! break, in numbers: the case that closes queue item 449.
//!
//! `cases/soft-wrap-between-boxes` pins the page as files. This says what
//! Chrome 154 gives for the same bytes with the corpus's font: each row's
//! height, and for the box each row marks `.s`, how far across its last
//! piece starts, to within Chrome's 1/64 px layout unit, and which line of
//! its row that piece is on. Until item 449 a break before text was decided
//! by the text's own `white-space`, so `#space-block` was one line here and
//! two in Chrome, and `#word`, `#closed-from`, `#after-box` and
//! `#box-after-space` broke or did not where Chrome did the opposite.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("soft-wrap-between-boxes"))?;
    Rendering::of(&case).ok()
}

/// Every element that `wanted` picks, as the rectangles of the pieces its
/// box was laid out in, in document order.
fn all(rendering: &Rendering, wanted: impl Fn(&alo_dom::Element) -> bool) -> Vec<Vec<Rect>> {
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
            let pieces: Vec<Rect> = drawing
                .layout
                .fragments(id)
                .iter()
                .map(|fragment| fragment.rect)
                .collect();
            if pieces.is_empty() {
                Some(vec![drawing.layout.get(id)?.border_box])
            } else {
                Some(pieces)
            }
        })
        .collect()
}

/// Within Chrome's layout unit.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1.0 / 64.0
}

/// A row: its `id`, its height, and where across the marked box's last
/// piece stands and on which of the row's lines (20 px each).
type Row = (&'static str, f32, f32, u8);

/// Each row, as Chrome 154 lays it out, across measured from the page.
const ROWS: [Row; 14] = [
    ("space-block", 40.0, 8.0, 1),
    ("space-span", 40.0, 8.0, 1),
    ("space-kept", 20.0, 8.0 + 1603.0 / 64.0, 0),
    ("before-space", 40.0, 8.0, 1),
    ("space-closed", 20.0, 8.0 + 2817.0 / 64.0, 0),
    ("word", 20.0, 8.0 + 1841.0 / 64.0, 0),
    ("word-wraps", 20.0, 8.0 + 1841.0 / 64.0, 0),
    ("no-break-space", 20.0, 8.0 + 1603.0 / 64.0, 0),
    ("closed-into", 40.0, 8.0, 1),
    ("closed-from", 40.0, 8.0, 1),
    ("opened-after", 20.0, 8.0 + 2210.0 / 64.0, 0),
    ("after-box", 60.0, 8.0, 2),
    ("after-box-letter", 40.0, 8.0, 1),
    ("box-after-space", 40.0, 8.0, 1),
];

#[test]
fn each_marked_box_ends_on_the_line_chrome_puts_it_on() {
    let Some(rendering) = case() else {
        panic!("the case renders");
    };
    // Each row, and nothing else, has an `id`.
    let rows = all(&rendering, |element| element.attr("id").is_some());
    let marked = all(&rendering, |element| {
        element
            .attr("class")
            .is_some_and(|classes| classes.split_whitespace().any(|name| name == "s"))
    });
    assert_eq!(rows.len(), ROWS.len(), "{rows:?}");
    assert_eq!(marked.len(), ROWS.len(), "{marked:?}");
    for ((row, pieces), (name, height, x, line)) in rows.iter().zip(&marked).zip(ROWS) {
        let (Some(row), Some(last)) = (row.first(), pieces.last()) else {
            panic!("{name}: a row and a piece");
        };
        assert!(near(row.size.height, height), "{name}: {row:?}");
        assert!(near(last.origin.x, x), "{name}: {pieces:?}, not at {x}");
        let on = ((last.origin.y - row.origin.y) / 20.0).floor();
        assert!(near(on, f32::from(line)), "{name}: {pieces:?} on {on}");
    }
}
