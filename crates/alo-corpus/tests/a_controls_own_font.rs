/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A control's own font, in numbers: the case that closes queue item 506.
//!
//! Chrome 154 sets every `<input>`, `<button>`, `<select>` and `<textarea>`
//! in 13.3333px whatever the page around it is set in. `web-a-form` is the
//! HTML specification's pizza form, with no style of its own, so its 16 px
//! paragraphs hold 13.33 px controls. Until item 506 a control took the
//! page's font, and each field was 25 tall here and 21 in Chrome; every box
//! below the first field was 4 px further down for each field above it.
//!
//! Chrome's numbers were measured from the same bytes in standards mode,
//! with the corpus's fonts standing for what Chrome computes (`DejaVu Sans`
//! for Arial and the page's own family, `DejaVu Sans Mono` for
//! `monospace`), as `getBoundingClientRect()` in its 1/64 px layout unit.

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
fn a_pizza_forms_fields_are_chromes_height_and_stand_where_chromes_do() {
    let Some(rendering) = case("web-a-form") else {
        panic!("the case renders");
    };
    let fields = boxes_of(&rendering, "input");
    // Three text fields, three radios, four checkboxes and the time field.
    assert_eq!(fields.len(), 11, "{fields:?}");
    // Each text field is 13.33 px text's 12 and 3, a pixel of padding and 2
    // of border above and below: 21, as Chrome's is, where the page's 16 px
    // made it 25. So each stands where Chrome's does, at 16, 53 and 90.
    for (field, top) in fields.iter().zip([16.0, 53.0, 90.0]) {
        assert!(within_a_unit(field.top(), top), "{field:?}");
        assert!(within_a_unit(field.size.height, 21.0), "{field:?}");
        // Chrome's are 172 wide. Here they are 20 of the font's zeros,
        // 169.66 inside their padding and border, where Chrome's width is
        // a rule of its own: queue item 511. The font moved them 33.94
        // nearer, from 211.59.
        assert!(within_a_unit(field.size.width, units(11370.0)), "{field:?}");
    }
    // The radios and the checkboxes, below them, are Chrome's to the unit.
    for (toggle, top) in fields[3..10].iter().zip([
        10918.0, 13222.0, 15526.0, 21196.0, 23500.0, 25804.0, 28108.0,
    ]) {
        assert!(within_a_unit(toggle.top(), units(top)), "{toggle:?}");
    }
    let fieldsets = boxes_of(&rendering, "fieldset");
    let tops: Vec<f32> = fieldsets.iter().map(|set| set.top()).collect();
    assert_eq!(tops.len(), 2);
    for (top, chrome) in tops.iter().zip([8128.0, 18406.0]) {
        assert!(within_a_unit(*top, units(chrome)), "{tops:?}");
    }
}

#[test]
fn a_pizza_forms_button_is_set_in_chromes_font() {
    let Some(rendering) = case("web-a-form") else {
        panic!("the case renders");
    };
    let Some(button) = boxes_of(&rendering, "button").first().copied() else {
        panic!("the button is laid out");
    };
    // "Submit order" in 13.33 px, 6 px of padding and 2 of border each
    // side: Chrome's 103.078125 by 21. In the page's 16 px it was 120.52
    // by 25. It stands 7 px below Chrome's because the textarea above it
    // is not yet Chrome's size (queue item 510).
    //
    // One unit is allowed across, and only across. The words shape to
    // 5,573.75 units; Chrome puts them on 5,573 and this engine on 5,574,
    // which is a question about a line of text at a fractional size, not
    // about a control's font: queue item 514.
    assert!(
        (button.size.width - units(6597.0)).abs() <= 1.0 / 64.0,
        "{button:?}"
    );
    assert!(within_a_unit(button.size.height, 21.0), "{button:?}");
}
