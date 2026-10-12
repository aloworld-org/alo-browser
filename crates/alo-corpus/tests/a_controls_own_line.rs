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
//! Item 505 gave the button Chrome's 2 px border, drawn as Chrome's own
//! picture, so it is Chrome's 22 and its picture is Chrome's screenshot's.
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
fn a_buttons_line_and_border_are_chromes() {
    let Some(rendering) = case("a-script-hears-an-event") else {
        panic!("the case renders");
    };
    let Some(button) = boxes_of(&rendering, "button").first().copied() else {
        panic!("the button is laid out");
    };
    // Chrome's button is 43.84375 by 22 at (12, 12): 14 px text's 13 and 3,
    // a pixel of padding above and below, and a border of 2 on each side.
    // Until item 505 its border was 1 here, and it was 20 tall.
    assert!(within_a_unit(button.origin.x, 12.0), "{button:?}");
    assert!(within_a_unit(button.origin.y, 12.0), "{button:?}");
    assert!(
        within_a_unit(button.size.width, units(2806.0)),
        "{button:?}"
    );
    assert!(
        within_a_unit(button.size.height, units(1408.0)),
        "{button:?}"
    );
}

/// The largest difference in any channel between two pixels.
fn difference(a: Option<alo_value::Rgba>, b: Option<alo_value::Rgba>) -> Option<u8> {
    let (ar, ag, ab, _) = a?.to_rgba8();
    let (br, bg, bb, _) = b?.to_rgba8();
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
}

#[test]
fn a_buttons_picture_is_chromes_within_the_measured_bound() {
    // `chrome.png` is Chrome 154's screenshot of the same bytes with the
    // corpus's font, at one to one, cut to the case's size. Chrome draws
    // the button's 2 px border as its own picture: a one-pixel rounded
    // edge over the fill (item 505). Compared here over the picture alone,
    // a band four pixels deep around the button's edge and one pixel
    // outside it, where no glyph is. Chrome's raster puts edge coverage in
    // quarters, so its corners are a few levels off any exact curve: the
    // largest difference measured was 7, and the mean 0.20. Before item 505
    // the band was 34 off on average and 137 at worst.
    let directory = cases_directory().join("a-script-hears-an-event");
    let Some(case) = Case::read(&directory) else {
        panic!("the case reads");
    };
    let Ok(rendering) = Rendering::of(&case) else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let Ok(bytes) = std::fs::read(directory.join("chrome.png")) else {
        panic!("Chrome's screenshot is committed");
    };
    let Ok(chrome) = alo_paint::from_png(&bytes) else {
        panic!("Chrome's screenshot reads");
    };
    // The button's border box is (12, 12) to (55.84, 34), drawn on whole
    // pixels from 12 to 56 across and 12 to 34 down.
    let (left, top, right, bottom) = (11_u32, 11_u32, 57_u32, 35_u32);
    let mut differences = Vec::new();
    for y in top..bottom {
        for x in left..right {
            let inside_the_band = x >= left + 4 && x < right - 4 && y >= top + 4 && y < bottom - 4;
            if inside_the_band {
                continue;
            }
            let Some(difference) = difference(drawing.canvas.at(x, y), chrome.at(x, y)) else {
                panic!("({x}, {y}) is on both pages");
            };
            differences.push(u32::from(difference));
        }
    }
    let worst = differences.iter().copied().max().unwrap_or_default();
    let total: u32 = differences.iter().sum();
    let count = u32::try_from(differences.len()).unwrap_or(u32::MAX);
    assert!(worst <= 8, "the largest difference is {worst}");
    assert!(total * 2 <= count, "the mean is {total} over {count}");
}
