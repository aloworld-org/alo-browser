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
//! It also found item 402: every run written `font-weight: bold` was laid out
//! in the bold face and drawn in the regular one.

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

/// The font weight the display list draws the run `run` in, if it is drawn.
fn drawn_weight(rendering: &Rendering, run: alo_box::BoxId) -> Option<u16> {
    let drawing = rendering.drawing()?;
    drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::Text { box_id, font, .. } if *box_id == run => Some(font.weight().value()),
        _ => None,
    })
}

/// The right-most column between `left` and `right` holding ink darker than
/// mid-grey in the rows of `line`.
fn ink_ends(rendering: &Rendering, line: Rect, left: u32, right: u32) -> Option<u32> {
    let drawing = rendering.drawing()?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a line on an 800 × 600 page"
    )]
    let (top, bottom) = (
        line.origin.y.floor() as u32,
        (line.origin.y + line.size.height).ceil() as u32,
    );
    (left..right).rev().find(|x| {
        (top..bottom).any(|y| {
            drawing.canvas.at(*x, y).is_some_and(|pixel| {
                let (red, green, blue, _) = pixel.to_rgba8();
                u32::from(red) + u32::from(green) + u32::from(blue) < 3 * 128
            })
        })
    })
}

#[test]
fn bold_text_is_drawn_in_the_bold_face_as_wide_as_it_was_laid_out() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    // The headings are bold by the user-agent sheet and the `<strong>` by its
    // own rule, each written `bold`: until item 402 paint read only a number
    // and drew all three in the regular face, which is narrower than the room
    // layout measured for them in the bold one.
    let mut bold = Vec::new();
    for tag in ["h1", "h2", "strong"] {
        for (element, _) in all(&rendering, tag) {
            let mut runs = text_in(&rendering, element);
            // The `<strong>` holds its words in an `<em>`.
            if runs.is_empty() {
                let inner: Vec<_> = drawing
                    .boxes
                    .ids()
                    .filter(|id| {
                        drawing
                            .boxes
                            .get(*id)
                            .is_some_and(|node| node.parent == Some(element))
                    })
                    .collect();
                runs = inner
                    .iter()
                    .flat_map(|id| text_in(&rendering, *id))
                    .collect();
            }
            bold.extend(runs);
        }
    }
    assert_eq!(bold.len(), 3, "{bold:?}");
    let laid: Vec<Rect> = bold
        .iter()
        .filter_map(|run| Some(drawing.layout.get(*run)?.border_box))
        .collect();
    let [h1, h2, strong] = laid.as_slice() else {
        panic!("three laid-out runs: {laid:?}");
    };
    // The widths layout measured in DejaVu Sans Bold, as `layout.txt` has
    // them.
    assert!(near(h1.size.width, 471.79688), "{h1:?}");
    assert!(near(h2.size.width, 333.73828), "{h2:?}");
    assert!(near(strong.size.width, 162.36719), "{strong:?}");
    for run in &bold {
        assert_eq!(drawn_weight(&rendering, *run), Some(700), "{run:?}");
    }
    // In pixels: each run's ink reaches to within three pixels of the right
    // edge of the room it was given, where the regular face stopped 55, 44
    // and 19 pixels short. Nothing follows a heading on its line, so its ink
    // must not run past that edge either.
    for (line, followed) in [(h1, false), (h2, false), (strong, true)] {
        let edge = line.origin.x + line.size.width;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a line on an 800 × 600 page"
        )]
        let (left, right) = (
            line.origin.x as u32,
            if followed { edge.floor() as u32 } else { 800 },
        );
        let Some(ends) = ink_ends(&rendering, *line, left, right) else {
            panic!("ink on the line {line:?}");
        };
        assert!(f64::from(ends) >= f64::from(edge) - 3.0, "{ends} {line:?}");
        assert!(f64::from(ends) <= f64::from(edge) + 1.0, "{ends} {line:?}");
    }
    // And the text around them is still the regular face.
    let paragraphs = all(&rendering, "p");
    let Some((first, _)) = paragraphs.first() else {
        panic!("a paragraph");
    };
    let plain = text_in(&rendering, *first);
    let Some(before) = plain.first() else {
        panic!("text before the <strong>");
    };
    assert_eq!(drawn_weight(&rendering, *before), Some(400));
}
