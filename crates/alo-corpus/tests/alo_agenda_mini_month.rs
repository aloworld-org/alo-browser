/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Agenda's month navigator, in numbers: the page that opened queue
//! item 437.
//!
//! `cases/alo-agenda-mini-month` pins the whole page as files. This says out
//! loud what item 437 is closed by. The month's two arrows are each a
//! `<button>` with 2 px of padding holding a 15 px `<svg>` that is `display:
//! block`, and until item 437 the box a button holds its content in was at
//! least a line tall whatever it held — 19.5 here — so each arrow was 23.5
//! tall, the header row with it, and the whole grid stood 4 px low. Chrome
//! 154 draws each arrow 19 × 19 and the header as tall as its title's line;
//! every number below is the one it gives for the same frozen page, given
//! the corpus's font, to within its 1/64 px layout unit.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_value::Rgba;

/// The case, rendered.
fn agenda() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("alo-agenda-mini-month"))?;
    Rendering::of(&case).ok()
}

/// The border boxes of every element that `wanted` picks, in document order.
fn all(rendering: &Rendering, wanted: impl Fn(&alo_dom::Element) -> bool) -> Vec<Rect> {
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
            Some(drawing.layout.get(id)?.border_box)
        })
        .collect()
}

/// Whether an element's `class` names `class`.
fn has_class(element: &alo_dom::Element, class: &str) -> bool {
    element
        .attr("class")
        .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn it_records_only_the_resets_pseudo_elements() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    assert_eq!(issues.len(), 2, "{issues:#?}");
    assert!(
        issues
            .iter()
            .all(|issue| issue.contains("::before") || issue.contains("::after")),
        "{issues:#?}"
    );
}

#[test]
fn each_arrow_is_its_svg_and_its_padding_and_no_taller() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let arrows = all(&rendering, |element| {
        element.name.is_html("button") && element.attr("aria-label").is_some()
    });
    let icons = all(&rendering, |element| has_class(element, "lucide"));
    let ([previous, next], [left, right]) = (arrows.as_slice(), icons.as_slice()) else {
        panic!("two arrows, two icons: {arrows:?} {icons:?}");
    };
    // 15 of `<svg>` and 2 of padding each side: 19 × 19, where a line's
    // minimum had made each one 19 × 23.5.
    for (arrow, icon) in [(previous, left), (next, right)] {
        assert!(near(arrow.size.width, 19.0), "{arrow:?}");
        assert!(near(arrow.size.height, 19.0), "{arrow:?}");
        assert!(near(icon.size.height, 15.0), "{icon:?}");
        assert!(near(icon.origin.x, arrow.origin.x + 2.0), "{icon:?}");
        assert!(near(icon.origin.y, arrow.origin.y + 2.0), "{icon:?}");
    }
    // `.miniNav`'s 2 px gap between them, and the pair at the header's end:
    // the sidebar's 240, less its 1 px border and 16 of padding.
    assert!(near(next.origin.x, previous.origin.x + 21.0), "{next:?}");
    assert!(near(next.origin.x + 19.0, 223.0), "{next:?}");
    // Centred against the title's 19.5 line, a quarter pixel down.
    assert!(near(previous.origin.y, 16.25), "{previous:?}");
}

#[test]
fn the_grid_starts_under_the_titles_line_and_its_days_are_square() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let heads = all(&rendering, |element| has_class(element, "miniHead"));
    let grids = all(&rendering, |element| has_class(element, "miniGrid"));
    let days = all(&rendering, |element| has_class(element, "miniDay"));
    let ([head], [grid]) = (heads.as_slice(), grids.as_slice()) else {
        panic!("one header, one grid: {heads:?} {grids:?}");
    };
    // The header is its title's line, 13 px at 1.5, now that the arrows are
    // shorter than it; the grid is under it and its 0.5rem margin.
    assert!(near(head.size.height, 19.5), "{head:?}");
    assert!(near(grid.origin.y, 43.5), "{grid:?}");

    // Six Monday-first weeks, 28 September to 8 November.
    assert_eq!(days.len(), 42);
    // Seven `1fr` columns of 207 less six 1 px gaps, each as tall as it is
    // wide (`aspect-ratio: 1`), under the weekday row's 0.62rem line and 2 px.
    // That line is 1.5 times 9.92 px rounded to 1/64, floored to 1/64:
    // 9.921875 × 1.5 is 952.5 sixty-fourths, so 952 (ADR 0050 § 3).
    //
    // Each day is on the grid (ADR 0054 § 3): its edges are the ideal edges
    // floored, so the columns are 1,837 and 1,838 units wide in turn, and
    // the rows the same. Chrome's are every one 1,838, because it sizes the
    // columns a second time once the ratio has made the rows 1,838 tall;
    // that second pass is inside `taffy` and is queue item 499's.
    let side = (207.0 - 6.0) / 7.0;
    let weekdays = 952.0 / 64.0 + 2.0;
    let floored = |pixels: f32| (pixels * 64.0).floor() / 64.0;
    let span = |start: f32| (floored(start), floored(start + side) - floored(start));
    for (index, day) in days.iter().enumerate() {
        let (row, column) = (index / 7, index % 7);
        #[expect(clippy::cast_precision_loss, reason = "a row and a column of seven")]
        let (row, column) = (row as f32, column as f32);
        let (left, width) = span(column * (side + 1.0));
        let (top, height) = span(row * (side + 1.0));
        assert!(near(day.origin.x, 16.0 + left), "{index}: {day:?}");
        assert!(near(day.size.width, width), "{index}: {day:?}");
        assert!(
            near(day.origin.y, 43.5 + weekdays + 1.0 + top),
            "{index}: {day:?}"
        );
        assert!(near(day.size.height, height), "{index}: {day:?}");
    }
}

#[test]
fn the_picked_day_is_a_terracotta_circle() {
    let Some(rendering) = agenda() else {
        panic!("the case renders");
    };
    let picked = all(&rendering, |element| has_class(element, "miniSelected"));
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let [day] = picked.as_slice() else {
        panic!("one picked day: {picked:?}");
    };
    let pixel = |x: f32, y: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a pixel on the canvas"
        )]
        let (x, y) = (x as u32, y as u32);
        drawing
            .canvas
            .at(x, y)
            .map_or_else(|| panic!("({x}, {y}) is on the canvas"), Rgba::to_rgba8)
    };
    let middle = day.origin.y + day.size.height / 2.0;
    // Inside the circle, left of its "14": `--terracotta`.
    assert_eq!(pixel(day.origin.x + 4.0, middle), (231, 111, 81, 255));
    // Its corner is outside the circle (`--radius-full`): the sidebar's
    // `--bg-sunken`.
    assert_eq!(
        pixel(day.origin.x + 1.0, day.origin.y + 1.0),
        (240, 236, 230, 255)
    );
}
