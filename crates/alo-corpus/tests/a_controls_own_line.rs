/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A control's own line, in numbers: the cases that close queue item 467.
//!
//! A button or a field holds what it shows in a box at least one line tall,
//! and that line is the line a line of its text would be: with `normal`, its
//! font's ascent, descent and line gap, each a whole pixel (ADR 0050). Until
//! item 467 it was the computed `line-height`, which answers `normal` with
//! 1.2 em, so `a-filled-form`'s 13 px fields were 19.59 tall here and 19 in
//! Chrome 154, and `a-script-hears-an-event`'s 14 px button 20.8 here and
//! 22 in Chrome.
//!
//! Chrome's numbers were measured from the same bytes with the corpus's
//! font, in standards mode, as `getBoundingClientRect()` in its 1/64 px
//! layout unit.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The named case, rendered.
fn case(name: &str) -> Option<Rendering> {
    let case = Case::read(&cases_directory().join(name))?;
    Rendering::of(&case).ok()
}

/// The border box of each element called `name`, in document order.
fn boxes_of(rendering: &Rendering, name: &str) -> Vec<Rect> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some_and(|e| e.name.is_html(name)))
        .filter_map(|node| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|held| &held.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })?;
            drawing.layout.get(id).map(|held| held.border_box)
        })
        .collect()
}

/// `units` of Chrome's layout unit, 1/64 px.
fn units(units: f32) -> f32 {
    units / 64.0
}

fn within_a_unit(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0 / 64.0
}

#[test]
fn a_filled_forms_fields_are_chromes_height_and_stand_where_chromes_do() {
    let Some(rendering) = case("a-filled-form") else {
        panic!("the case renders");
    };
    // Its two text fields, and then the checkbox, which has no line of its
    // own and is here to show it did not move.
    let fields = boxes_of(&rendering, "input");
    assert_eq!(fields.len(), 3, "{fields:?}");
    // Each field is 13 px text's 12 and 3, a pixel of padding and a pixel
    // of border on each side: 19, as Chrome's is. And so each thing below
    // the first stands where Chrome's does.
    for (field, top) in fields.iter().zip([1600.0, 3776.0, 5184.0]) {
        assert!(within_a_unit(field.top(), units(top)), "{field:?}");
    }
    for field in &fields[..2] {
        assert!(within_a_unit(field.size.height, 19.0), "{field:?}");
    }
    let labels = boxes_of(&rendering, "label");
    let tops: Vec<f32> = labels.iter().map(|label| label.top()).collect();
    for (top, chrome) in tops.iter().zip([640.0, 2816.0, 6208.0]) {
        assert!(within_a_unit(*top, units(chrome)), "{tops:?}");
    }
    let Some(form) = boxes_of(&rendering, "form").first().copied() else {
        panic!("the form is laid out");
    };
    assert!(within_a_unit(form.size.height, units(6528.0)), "{form:?}");
}

#[test]
fn a_buttons_line_is_its_labels_and_only_its_border_is_not_chromes() {
    let Some(rendering) = case("a-script-hears-an-event") else {
        panic!("the case renders");
    };
    let Some(button) = boxes_of(&rendering, "button").first().copied() else {
        panic!("the button is laid out");
    };
    // Chrome's button is 22 tall: 14 px text's 13 and 3, a pixel of padding
    // above and below, and a border of 2 on each side. Inside its border it
    // is 18, and so is this one. Its border is 1 here, which is item 505's,
    // so it is 20 tall where Chrome's is 22; until item 467 its line was
    // 16.8 and it was 20.8, nearer by accident.
    let chrome = units(1408.0);
    let inside_its_border = button.size.height - 2.0;
    assert!(within_a_unit(inside_its_border, chrome - 4.0), "{button:?}");
    assert!(within_a_unit(button.size.height, 20.0), "{button:?}");
}
