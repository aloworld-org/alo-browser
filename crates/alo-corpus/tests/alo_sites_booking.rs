/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' booking section, in numbers: the page that opened queue items
//! 384 and 385.
//!
//! `cases/alo-sites-booking` pins the whole page as files. This says out loud
//! what item 384 is closed by. The page's form holds a day field nobody has
//! typed into, beside its label. Until item 384 a field with no line stood
//! on its bottom margin edge, so its line was a font's descent taller than
//! the field, and the form, centred down the grid, stood higher than a
//! browser draws it.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn booking() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-booking"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border box of the first element named `tag` whose `class` attribute
/// starts with `class`.
fn found(rendering: &Rendering, tag: &str, class: &str) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document.descendants(document.root()).find(|id| {
        document.element(*id).is_some_and(|element| {
            &*element.name.local == tag
                && element.attr("class").unwrap_or_default().starts_with(class)
        })
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    Some(drawing.layout.get(id)?.border_box)
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = booking() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(
        case.address.as_deref(),
        Some("https://nordwind.alosites.com/")
    );
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
    assert!(answered.said.is_empty(), "{:?}", answered.said);
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
}

#[test]
fn the_empty_day_field_makes_a_line_its_own_height() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(form), Some(field), Some(button)) = (
        found(&rendering, "form", "booking-day"),
        found(&rendering, "input", ""),
        found(&rendering, "button", ""),
    ) else {
        panic!("the form, its field and its button are laid out");
    };
    // `width: 100%` of the second track: 343 wide at 432. A 27.2 line,
    // 0.6rem of padding above and below and a 1 px border: 48.4 tall.
    assert!(near(field.origin.x, 432.0), "{field:?}");
    assert!(near(field.size.width, 343.0), "{field:?}");
    assert!(near(field.size.height, 48.4), "{field:?}");
    // The field stands on the baseline of the line it would hold, so its line
    // is its own 48.4 and the button's paragraph starts the label's
    // paragraph's 17 px margin below the field. With the field on its bottom
    // edge the line was 56.11, the strut's 7.71 below the baseline added.
    assert!(
        near(button.origin.y, field.origin.y + field.size.height + 17.0),
        "{button:?} under {field:?}"
    );
    // So the form is 162 tall: 27.2 + 4 + 48.4 + 17 + 48.4 + 17. With its
    // 1rem top margin, 178 centred in the rows' 274.8 from 25: 89.4. Before
    // item 384 it was 169.71 at 85.54.
    assert!(near(form.size.height, 162.0), "{form:?}");
    assert!(near(form.origin.y, 89.4), "{form:?}");
    assert!(near(field.origin.y, 120.6), "{field:?}");
    assert!(near(button.origin.y, 186.0), "{button:?}");
}

#[test]
fn the_offer_is_laid_out_as_the_sheet_says() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(section), Some(heading), Some(name), Some(place)) = (
        found(&rendering, "section", "s-booking"),
        found(&rendering, "h2", ""),
        found(&rendering, "h3", ""),
        found(&rendering, "p", "booking-where"),
    ) else {
        panic!("the section and its offer are laid out");
    };
    // `booking-layout-split`'s 62rem is wider than the page, so the section
    // is the whole 800; 1.5rem of padding in a 1 px border.
    assert_eq!(section, Rect::new(0.0, 0.0, 800.0, 324.8));
    assert_eq!((heading.origin.x, heading.origin.y), (25.0, 25.0));
    // Two `1fr` tracks either side of a 4rem gap in 750: 343 each.
    assert!(near(heading.size.width, 343.0), "{heading:?}");
    // 33.6 of heading, its 14 below, and the 1rem row gap.
    assert!(near(name.origin.y, 88.6), "{name:?}");
    // The last paragraph ends at 255.6 + 27.2 + 17 = 299.8, the content's
    // end, and the section's 24 + 1 below make it 324.8.
    assert!(near(place.origin.y, 255.6), "{place:?}");
}
