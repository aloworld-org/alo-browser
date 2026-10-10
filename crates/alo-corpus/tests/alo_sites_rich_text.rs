/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' rich text, in numbers: the page that opened queue item 400.
//!
//! `cases/alo-sites-rich-text` pins the whole page as files. This says out
//! loud what item 400 is closed by. The page has no style sheet, so every
//! number here is the user-agent sheet's. Until item 400 that sheet gave a
//! list inside a list the same `1em` above and below as any other, so the
//! nested point stood 16 pixels under the point it belongs to; and it had no
//! rule for `<s>`, so the checklist's finished task was drawn as plain text.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::DisplayItem;

/// The case, read and rendered.
fn rich_text() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("alo-sites-rich-text"))?;
    Rendering::of(&case).ok()
}

/// The element boxes made for elements named `tag`, in document order, as
/// `(box, border box)`.
fn all(rendering: &Rendering, tag: &str) -> Vec<(alo_box::BoxId, Rect)> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|element| &*element.name.local == tag)
        })
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

/// The boxes of the runs of text straight inside the element box `parent`.
fn text_in(rendering: &Rendering, parent: alo_box::BoxId) -> Vec<alo_box::BoxId> {
    let Some(drawing) = rendering.drawing() else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    boxes
        .ids()
        .filter(|id| {
            boxes.get(*id).is_some_and(|node| {
                node.parent == Some(parent) && matches!(node.kind, alo_box::BoxKind::Text { .. })
            })
        })
        .collect()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn a_list_in_a_list_starts_where_the_line_above_it_ends() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let (lists, items) = (all(&rendering, "ul"), all(&rendering, "li"));
    let ([(_, outer), (_, nested), ..], [_, (_, holding), ..]) =
        (lists.as_slice(), items.as_slice())
    else {
        panic!("lists and items: {lists:?} {items:?}");
    };
    // The text is 16 pixels at `normal`, 18.625 a line. The outer list is
    // under the subheading, 16 below a 1em margin: the second point starts
    // one line down, at 185.1575.
    assert!(near(outer.origin.y, 166.5325), "{outer:?}");
    assert!(near(holding.origin.y, 185.1575), "{holding:?}");
    // The nested list starts where the point's own line ends, with no margin,
    // 40 further in; the point holding it is two lines tall, not two lines
    // and 16.
    assert!(near(nested.origin.y, 185.1575 + 18.625), "{nested:?}");
    assert!(near(nested.origin.x, 48.0), "{nested:?}");
    assert!(near(holding.size.height, 2.0 * 18.625), "{holding:?}");
    // And it gives nothing below either: the numbered list after the outer
    // one is the outer one's 1em below its end, not 1em and 16.
    let numbered = all(&rendering, "ol");
    let [(_, numbered)] = numbered.as_slice() else {
        panic!("one numbered list: {numbered:?}");
    };
    assert!(
        near(numbered.origin.y, 166.5325 + 3.0 * 18.625 + 16.0),
        "{numbered:?}"
    );
}

#[test]
fn the_finished_task_is_struck_through_half_way_up_its_letters() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let struck = all(&rendering, "s");
    let [(struck, line)] = struck.as_slice() else {
        panic!("one <s>: {struck:?}");
    };
    let runs = text_in(&rendering, *struck);
    let [run] = runs.as_slice() else {
        panic!("one run of text in the <s>: {runs:?}");
    };
    let strokes: Vec<_> = drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill { box_id, path, .. } if box_id == run => path.bounds(),
            _ => None,
        })
        .collect();
    let [(left, top, right, bottom)] = strokes.as_slice() else {
        panic!("one line drawn for \"Publish\": {strokes:?}");
    };
    // As wide as the word, and inside the line's upper half and a quarter:
    // through the letters, not under them as an underline would be.
    assert!(near(*left, line.origin.x), "{strokes:?} {line:?}");
    assert!(near(*right, line.origin.x + line.size.width), "{strokes:?}");
    assert!(
        *top > line.origin.y + line.size.height * 0.25,
        "{strokes:?}"
    );
    assert!(
        *bottom < line.origin.y + line.size.height * 0.75,
        "{strokes:?}"
    );
}

#[test]
fn nothing_else_on_the_page_is_struck_or_underlined_but_the_link() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    // Two lines on the page: the task's strike and the link's underline. The
    // link's `<u>` underlines the same words in the same place and colour.
    let lines = drawing
        .display
        .items()
        .iter()
        .filter(|item| matches!(item, DisplayItem::Fill { .. }))
        .count();
    assert_eq!(lines, 2);
}
