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
//! in the bold face and drawn in the regular one, and item 401: not one list
//! item had a marker.

use alo_agent::AgentTree;
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

/// A line of the page's 16 px text at `normal`: `DejaVu Sans`' ascent and
/// descent, 14.85 and 3.77, each in whole pixels (ADR 0050 § 1). Chrome
/// 154 makes every one of this page's lines 19 tall too.
const LINE: f32 = 19.0;

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
    // The text is 16 pixels at `normal`, 19 a line. The outer list is
    // under the subheading, 16 below a 1em margin: the second point starts
    // one line down, at 186.6875. The `h1`'s 0.67em margin above it is
    // 21.4375 on the grid of 1/64 px (ADR 0054 § 1), where it was 21.44.
    assert!(near(outer.origin.y, 167.6875), "{outer:?}");
    assert!(near(holding.origin.y, 186.6875), "{holding:?}");
    // The nested list starts where the point's own line ends, with no margin,
    // 40 further in; the point holding it is two lines tall, not two lines
    // and 16.
    assert!(near(nested.origin.y, 186.6875 + LINE), "{nested:?}");
    assert!(near(nested.origin.x, 48.0), "{nested:?}");
    assert!(near(holding.size.height, 2.0 * LINE), "{holding:?}");
    // And it gives nothing below either: the numbered list after the outer
    // one is the outer one's 1em below its end, not 1em and 16.
    let numbered = all(&rendering, "ol");
    let [(_, numbered)] = numbered.as_slice() else {
        panic!("one numbered list: {numbered:?}");
    };
    assert!(
        near(numbered.origin.y, 167.6875 + 3.0 * LINE + 16.0),
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
    // The underline steps aside for the `g` of "guide" (queue item 579), so
    // it is two pieces on one row: a line is counted by its row.
    let mut rows: Vec<f32> = drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill { path, .. } => path.bounds().map(|(_, top, _, _)| top),
            _ => None,
        })
        .collect();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| near(*a, *b));
    assert_eq!(rows.len(), 2, "{rows:?}");
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

/// Each list item's border box, its marker's text and its marker's box, in
/// document order.
fn markers(rendering: &Rendering) -> Vec<(Rect, String, Rect)> {
    let Some(drawing) = rendering.drawing() else {
        return Vec::new();
    };
    all(rendering, "li")
        .into_iter()
        .filter_map(|(item, rect)| {
            let marker = drawing.boxes.marker_of(item)?;
            let text = drawing.boxes.get(marker)?.text()?.to_owned();
            Some((rect, text, drawing.layout.get(marker)?.border_box))
        })
        .collect()
}

/// ADR 0044 § 4, worked by hand. The page is `DejaVu Sans` at 16 pixels, whose
/// ascent is 1901/2048 of an em, 14.85, and 15 as Chromium's whole-pixel
/// metrics have it: a disc or a circle starts ⌊15 × 2 / 3⌋ + 8 = 18 before
/// its item's content edge, and a number ends at it. Each stands on its
/// item's first line, in the same face as the line, so its box's top is the
/// line's.
#[test]
fn each_point_has_its_marker_outside_its_first_line() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let found = markers(&rendering);
    let texts: Vec<&str> = found.iter().map(|(_, text, _)| text.as_str()).collect();
    assert_eq!(texts, ["• ", "• ", "◦ ", "3. ", "4. ", "• "]);
    for (item, text, marker) in &found {
        // The item has no padding, so its content edge is its border box's.
        let start = item.origin.x;
        if text.starts_with(char::is_numeric) {
            assert!(
                near(marker.origin.x + marker.size.width, start),
                "{text} {marker:?}"
            );
        } else {
            assert!(near(marker.origin.x, start - 18.0), "{text} {marker:?}");
        }
        assert!(
            near(marker.origin.y, item.origin.y),
            "{text} {marker:?} {item:?}"
        );
        assert!(near(marker.size.height, LINE), "{text} {marker:?}");
    }
    // The second point holds a list: its marker is on its first line only.
    let Some((holding, _, disc)) = found.get(1) else {
        panic!("the second point: {found:?}");
    };
    assert!(near(holding.size.height, 2.0 * LINE), "{holding:?}");
    assert!(near(disc.origin.y, 186.6875), "{disc:?}");
    // The numbers are as wide as DejaVu Sans makes "3. ", so they start
    // 20.35 before the line.
    let Some((_, _, three)) = found.get(3) else {
        panic!("the third step: {found:?}");
    };
    assert!(near(three.origin.x, 27.648_438), "{three:?}");
}

/// Whether any pixel in the columns `left..right` of the rows of `line` is
/// darker than mid-grey.
fn inked(rendering: &Rendering, line: Rect, left: u32, right: u32) -> bool {
    ink_ends(rendering, line, left, right).is_some()
}

#[test]
fn each_marker_is_inked_where_it_stands_and_nowhere_else() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    for (item, text, marker) in markers(&rendering) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a line on an 800 × 600 page"
        )]
        let (left, start) = (marker.origin.x.floor() as u32, item.origin.x as u32);
        // Ink from where the marker starts to the item's edge, and none in
        // the eight pixels before it nor in the gap its suffix leaves.
        let Some(ends) = ink_ends(&rendering, marker, left, start) else {
            panic!("no ink for {text:?} at {marker:?}");
        };
        assert!(
            !inked(&rendering, marker, left.saturating_sub(8), left),
            "{text:?} ink before {left}"
        );
        assert!(
            ends + 3 < start,
            "{text:?}'s ink runs to {ends}, at the line"
        );
    }
}

#[test]
fn an_agent_reads_each_marker_as_a_property_and_each_point_by_what_it_says() {
    let Some(rendering) = rich_text() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("a document and a drawing");
    };
    let outline = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    for line in [
        "listitem \"Choose one useful outcome\" [marker=\"•\"]",
        "listitem \"Make the next step visible Keep supporting detail nested\" [marker=\"•\"]",
        "listitem \"Keep supporting detail nested\" [marker=\"◦\"]",
        "listitem \"Draft\" [marker=\"3.\"]",
        "listitem \"Review\" [marker=\"4.\"]",
        "listitem \"Publish\" [marker=\"•\"]",
    ] {
        assert!(outline.contains(line), "{line} in\n{outline}");
    }
    // Never as text of its own, and never in a name.
    assert!(!outline.contains("text \"•"), "{outline}");
    assert!(!outline.contains("\"3. Draft\""), "{outline}");
}
