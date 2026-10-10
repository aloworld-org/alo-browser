/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' booking section when it is not taking bookings, in numbers:
//! the page that opened queue item 394.
//!
//! `cases/alo-sites-booking-closed` pins the whole page as files. This says
//! out loud what item 394 is closed by. The section is
//! `booking-layout-split`, a grid of two `1fr` columns, and with no day
//! field its six blocks fill both columns in turn. Until item 394 layout
//! asked a paragraph how narrow it could be by measuring it with no width at
//! all, which is how wide it would *like* to be; the closed notice, one line
//! of 366, then claimed that as its narrowest, and the two columns came out
//! 320 and 366 rather than 343 each.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn closed() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-booking-closed"))?;
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
    let Some((case, Rendering::Loaded(_, answered))) = closed() else {
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
fn two_fr_columns_are_equal_and_the_notice_wraps_in_its_own() {
    let Some((_, rendering)) = closed() else {
        panic!("the case renders");
    };
    let (Some(section), Some(heading), Some(name), Some(place), Some(notice)) = (
        found(&rendering, "section", "s-booking"),
        found(&rendering, "h2", ""),
        found(&rendering, "h3", ""),
        found(&rendering, "p", "booking-where"),
        found(&rendering, "p", "booking-closed"),
    ) else {
        panic!("the section and what is in it are laid out");
    };
    // 800, less 1.5rem of padding and a 1 px border on each side, is 750;
    // less the 4rem column gap, 686, and two `1fr` columns are 343 each.
    assert!(near(section.size.width, 800.0), "{section:?}");
    assert_eq!((heading.origin.x, heading.origin.y), (25.0, 25.0));
    assert!(near(heading.size.width, 343.0), "{heading:?}");
    assert!(near(name.origin.x, 432.0), "{name:?}");
    assert!(near(name.size.width, 343.0), "{name:?}");
    // The notice is 366 on one line, so in 343 it is two lines of 27.2,
    // and the place beside it stretches to the row.
    assert!(near(notice.origin.x, 432.0), "{notice:?}");
    assert!(near(notice.size.width, 343.0), "{notice:?}");
    assert!(near(notice.size.height, 54.4), "{notice:?}");
    assert!(near(place.size.height, 54.4), "{place:?}");
    // Rows of 47.6 and 44.2 (a block and its margin) with 1rem between
    // them put the third at 148.8; its 54.4 and 17 of margin, then 24 + 1,
    // make the section 245.2.
    assert!(near(notice.origin.y, 148.8), "{notice:?}");
    assert!(near(section.size.height, 245.2), "{section:?}");
}
