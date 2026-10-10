/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' tickets section, in numbers: the page that opened queue item
//! 393.
//!
//! `cases/alo-sites-tickets` pins the whole page as files. This says out loud
//! what item 393 is closed by. The section's `tickets-layout-banner` undoes
//! the card's `max-width: 38rem` with `max-width: none`, and until item 393
//! layout refused `none` as a value it did not implement and recorded that
//! against the page. The box came out right only because the refusal fell
//! back to the initial value, which is `none`; the record said otherwise.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn tickets() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-tickets"))?;
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
    let Some((case, Rendering::Loaded(_, answered))) = tickets() else {
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
fn a_maximum_of_none_is_read_and_not_recorded_as_refused() {
    let Some((_, rendering)) = tickets() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    assert!(
        !issues.iter().any(|issue| issue.contains("max-width")),
        "{issues:#?}"
    );
    // What is left is the sheet's own list, the other alo Sites cases'
    // exactly: pseudo-elements, `@keyframes` and `prefers-reduced-motion`.
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")),
        "{issues:#?}"
    );
}

#[test]
fn the_banner_is_laid_out_as_the_sheet_says() {
    let Some((_, rendering)) = tickets() else {
        panic!("the case renders");
    };
    let (Some(section), Some(heading), Some(offer), Some(actions), Some(button)) = (
        found(&rendering, "section", "s-tickets"),
        found(&rendering, "h2", ""),
        found(&rendering, "p", ""),
        found(&rendering, "p", "actions"),
        found(&rendering, "a", "button"),
    ) else {
        panic!("the section and what is in it are laid out");
    };
    // `max-width: none` undoes the card's 38rem (608), so the section is the
    // whole 800. clamp(2rem, 5vw, 4rem) of padding is 40 at 800 wide, inside
    // a 1 px border.
    assert_eq!((section.origin.x, section.origin.y), (0.0, 0.0));
    assert!(near(section.size.width, 800.0), "{section:?}");
    assert_eq!((heading.origin.x, heading.origin.y), (41.0, 41.0));
    assert!(near(heading.size.width, 718.0), "{heading:?}");
    // 33.6 of heading and its 14 below.
    assert!(near(offer.origin.y, 88.6), "{offer:?}");
    // The offer's 27.2, and its 17 px margin collapsed with the actions'
    // 1rem.
    assert!(near(actions.origin.y, 132.8), "{actions:?}");
    // `.actions` is a flex row with no `justify-content`, so `text-align:
    // center` centres the text and leaves the button at the start, as a
    // browser does.
    assert!(near(button.origin.x, 41.0), "{button:?}");
    assert!(near(button.size.height, 48.4), "{button:?}");
    // 132.8 + 48.4, the actions' 17 below, and 40 + 1: 239.2.
    assert!(near(section.size.height, 239.2), "{section:?}");
}
