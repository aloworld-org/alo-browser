/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' features section, in numbers: the page that opened queue items
//! 379 and 381.
//!
//! `cases/alo-sites-features` pins the whole page as files. This says out
//! loud what item 379 is closed by. The page is the `features-bento` variant,
//! whose first card the sheet gives `padding-block: 2.5rem` over the 1.25rem
//! every presented card has. Until item 379 no logical shorthand was split,
//! so the card kept 1.25rem above and below.
//!
//! It also says what item 381 is opened by: the section is drawn at opacity
//! 0, because only an `IntersectionObserver` would make it visible and this
//! engine has none. The picture is white for that reason, so the card's
//! padding is shown as a picture by `cases/block-axis-spacing` instead.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::DisplayItem;

/// The case, read and rendered.
fn features() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-features"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The box of the first element named `tag` whose `class` attribute starts
/// with `class`, and its border box.
fn found(rendering: &Rendering, tag: &str, class: &str) -> Option<(alo_box::BoxId, Rect)> {
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
    Some((id, drawing.layout.get(id)?.border_box))
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = features() else {
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
fn the_first_bento_card_is_padded_by_its_block_axis() {
    let Some((_, rendering)) = features() else {
        panic!("the case renders");
    };
    let Some((_, card)) = found(&rendering, "li", "") else {
        panic!("the card is laid out");
    };
    let Some((_, heading)) = found(&rendering, "h3", "") else {
        panic!("the card's heading is laid out");
    };
    let Some((_, said)) = found(&rendering, "p", "") else {
        panic!("a paragraph is laid out");
    };
    // The grid's two `minmax(0, 1fr)` tracks, and the card spans `1 / -1`.
    assert_eq!((card.origin.x, card.size.width), (20.0, 760.0), "{card:?}");
    assert!((card.origin.y - 171.8).abs() < 1e-3, "{card:?}");
    // `padding-block: 2.5rem` (40) inside a 1 px border above the heading,
    // and the inline sides still `.section-cards`' 1.25rem (20).
    assert!(
        (heading.origin.y - (card.origin.y + 1.0 + 40.0)).abs() < 1e-3,
        "{heading:?} in {card:?}"
    );
    assert!(
        (heading.origin.x - (card.origin.x + 1.0 + 20.0)).abs() < 1e-3,
        "{heading:?}"
    );
    // 1 + 40 + 21.6 (the heading) + 9 (its 0.5em) + 27.2 (the paragraph)
    // + 17 (its 1em) + 40 + 1. Before item 379 it was 116.8: 20 each end.
    assert!((card.size.height - 156.8).abs() < 1e-3, "{card:?}");
    // The first paragraph is the intro, above the card, and is not in it.
    assert!(said.origin.y < card.origin.y, "{said:?}");
}

#[test]
fn the_section_waits_for_an_observer_this_engine_does_not_have() {
    let Some((_, rendering)) = features() else {
        panic!("the case renders");
    };
    let Some((section, _)) = found(&rendering, "section", "s-features") else {
        panic!("the section is laid out");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn");
    };
    // `.js .section-motion { opacity: 0 }` and `.js .section-enter-fade-up
    // { transform: translateY(1.5rem) }`, until an `IntersectionObserver`
    // adds `is-visible` (item 381).
    let mut faded = None;
    let mut moved = None;
    for item in drawing.display.items() {
        match item {
            DisplayItem::PushGroup { box_id, opacity } if *box_id == section => {
                faded = Some(*opacity);
            }
            DisplayItem::PushTransform { box_id, matrix } if *box_id == section => {
                moved = Some((matrix.e, matrix.f));
            }
            _ => {}
        }
    }
    assert_eq!(faded.map(f32::to_bits), Some(0.0_f32.to_bits()));
    assert_eq!(moved, Some((0.0, 24.0)));
    // So nothing of it reaches the picture: the card's middle is white.
    let pixel = drawing.canvas.at(400, 250).map(alo_value::Rgba::to_rgba8);
    assert_eq!(pixel, Some((255, 255, 255, 255)));
}
