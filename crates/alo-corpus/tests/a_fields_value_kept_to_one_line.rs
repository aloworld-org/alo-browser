/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A one-line field's value is one line, cut off at the field's content
//! box, in numbers: the case that closes queue item 715.
//!
//! `cases/a-fields-value-kept-to-one-line` pins the page as files and its
//! picture as `render.png`. Found by 701: an `<input>` holding more than its
//! width wrapped its value onto lines below it, over what followed. Measured
//! in Chrome 156.0.8078.76, headless, on the same fields set in Arial
//! (1 November 2026): the value on one line whatever the field's
//! `white-space` (`normal`, `pre-wrap`) or `overflow` (`visible` computes
//! `clip`), every space kept, nothing drawn below the field, and the text
//! cut where the content box ends — a field padded 10 pixels across stops
//! its `W`s 10 pixels short of its border, not at the padding edge — and a
//! long hint one line as well. Asserted here in the engine's numbers: each
//! value's text one piece, on the field's line, as wide as the whole value;
//! the paragraph after the first field where it would be with a short
//! value; and in the picture, no ink of a value past its field's content
//! box, nor between a field and what follows it.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// Each field, by `id`: whether it shows a hint rather than a value.
const FIELDS: [(&str, bool); 6] = [
    ("long", false),
    ("normal", false),
    ("visible", false),
    ("spaces", false),
    ("padded", false),
    ("hint", true),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("a-fields-value-kept-to-one-line"))?;
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

/// A field's border box, the box it holds its line in (its content box),
/// and every piece of the text it shows.
fn field(rendering: &Rendering, id: &str) -> Option<(Rect, Rect, Vec<Rect>)> {
    let drawing = rendering.drawing()?;
    let held = box_of(rendering, id)?;
    let inside = drawing.boxes.children(held).next()?;
    let text = drawing.boxes.children(inside).next()?;
    drawing.boxes.get(text)?.text()?;
    Some((
        drawing.layout.get(held)?.border_box,
        drawing.layout.get(inside)?.border_box,
        drawing
            .layout
            .fragments(text)
            .iter()
            .map(|fragment| fragment.rect)
            .collect(),
    ))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0 / 64.0
}

/// Whether the pixel at `x`, `y` is ink: darker than the field's white and
/// its hint's grey alike.
fn ink(rendering: &Rendering, x: u32, y: u32) -> bool {
    rendering
        .drawing()
        .and_then(|drawing| drawing.canvas.at(x, y))
        .is_some_and(|pixel| {
            let (red, green, blue, _) = pixel.to_rgba8();
            u32::from(red) + u32::from(green) + u32::from(blue) < 3 * 200
        })
}

/// A whole pixel's place, for a coordinate on the canvas.
fn pixel(at: f32) -> u32 {
    format!("{}", at.max(0.0).floor()).parse().unwrap_or(0)
}

#[test]
fn each_value_is_one_line_on_the_fields_line() {
    let rendering = case().expect("the case renders");
    for (id, _) in FIELDS {
        let (border_box, inside, pieces) = field(&rendering, id).expect("the field is laid out");
        let [piece] = pieces.as_slice() else {
            panic!("{id}: {} pieces, not one line", pieces.len());
        };
        assert!(close(piece.origin.x, inside.origin.x), "{id}: {piece:?}");
        assert!(close(piece.origin.y, inside.origin.y), "{id}: {piece:?}");
        assert!(close(piece.size.height, 15.0), "{id}: one line, {piece:?}");
        assert!(
            piece.size.width > inside.size.width + 200.0,
            "{id}: the whole value, past the field: {piece:?} in {inside:?}"
        );
        // The field itself is the size it would be holding nothing: one
        // line of 15 between its padding and border, 21 tall.
        assert!(close(border_box.size.height, 21.0), "{id}: {border_box:?}");
    }
    // The paragraph after the first field stands where it would after a
    // field holding a short value: under it, the column's gap of 8 below.
    let drawing = rendering.drawing().expect("drawn");
    let paragraph = drawing
        .boxes
        .ids()
        .find(|id| drawing.boxes.get(*id).and_then(|node| node.text()) == Some("Below the field"))
        .and_then(|id| drawing.layout.fragments(id).first().map(|f| f.rect))
        .expect("the paragraph is laid out");
    assert!(close(paragraph.origin.y, 8.0 + 21.0 + 8.0), "{paragraph:?}");
}

#[test]
fn every_space_of_a_value_takes_its_room() {
    let rendering = case().expect("the case renders");
    let (_, inside, _) = field(&rendering, "spaces").expect("the field is laid out");
    let (top, bottom) = (
        pixel(inside.origin.y),
        pixel(inside.origin.y + inside.size.height),
    );
    let inked: Vec<u32> = (pixel(inside.origin.x)..pixel(inside.origin.x + inside.size.width))
        .filter(|x| (top..bottom).any(|y| ink(&rendering, *x, y)))
        .collect();
    // The first gap in the ink is between `a` and `b`: four spaces of
    // `DejaVu Sans` at 13.33 px, 4.24 each, are 17 pixels; one would be 4.
    let gap = inked
        .windows(2)
        .find_map(|pair| match pair {
            [a, b] if b - a > 1 => Some(b - a - 1),
            _ => None,
        })
        .expect("ink in two runs");
    assert!(
        (15..=19).contains(&gap),
        "the gap between `a` and `b` is {gap}"
    );
}

#[test]
fn no_value_is_drawn_past_its_fields_content_box_nor_below_it() {
    let rendering = case().expect("the case renders");
    for (id, _) in FIELDS {
        let (border_box, inside, _) = field(&rendering, id).expect("the field is laid out");
        let top = pixel(inside.origin.y);
        let bottom = pixel(inside.origin.y + inside.size.height);
        let first = pixel(inside.origin.x);
        let end = pixel(inside.origin.x + inside.size.width);
        let inked = |x: u32| (top..bottom).any(|y| ink(&rendering, x, y));
        // Ink up to the content box's end — the value runs to it…
        assert!(
            (end.saturating_sub(4)..end).any(inked),
            "{id}: no ink at the content box's end"
        );
        // …and none from there to the border, nor past the field.
        let border_end = pixel(border_box.origin.x + border_box.size.width) - 2;
        for x in end..border_end {
            assert!(
                !inked(x),
                "{id}: ink at {x}, past the content box's end {end}"
            );
        }
        for x in pixel(border_box.origin.x + border_box.size.width) + 1..400 {
            assert!(!inked(x), "{id}: ink at {x}, past the field");
        }
        assert!(first < end);
        // Nothing of it below the field, where a second line would stand.
        let below = pixel(border_box.origin.y + border_box.size.height) + 1;
        for y in below..below + 6 {
            for x in first..end {
                assert!(
                    !ink(&rendering, x, y),
                    "{id}: ink at {x}, {y}, below the field"
                );
            }
        }
    }
}
