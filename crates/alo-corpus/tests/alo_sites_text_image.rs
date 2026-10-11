/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' text-and-image section, in numbers: the page that opened
//! queue item 380.
//!
//! `cases/alo-sites-text-image` pins the whole page as files. This says out
//! loud what item 380 is closed by. The section is `text-image-overlap`: a
//! two-column grid whose text card is `margin-inline-start: -4rem`, so it
//! reaches 4rem back over the picture's column. Layout read no inline-axis
//! logical property, and the card sat in its own column with no margin, its
//! text 64 narrower than the sheet makes it and its sentence on two lines.
//!
//! Every number below was also measured in Google Chrome 154, headless, on
//! the same page and sheet at 800 wide: the card 428 × 155.78 at (352, 48),
//! its heading 364 wide at (384, 80), the section 251.78 tall (Chrome's
//! layout unit rounds 27.2 to 27.1875). The page's one picture is in no
//! repository, so it is a broken picture with an `alt`, in Chrome as here
//! (ADR 0046).
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked
//! for and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn text_image() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-text-image"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border boxes of every element named `tag`, in document order.
fn all(rendering: &Rendering, tag: &str) -> Vec<Rect> {
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
            Some(drawing.layout.get(id)?.border_box)
        })
        .collect()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = text_image() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(
        case.address.as_deref(),
        Some("https://nordwind.alosites.com/")
    );
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
}

#[test]
fn what_it_records_is_the_sheets_list_and_its_missing_picture() {
    let Some((_, rendering)) = text_image() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    // The other alo Sites cases' list — pseudo-elements, `@keyframes` and
    // `prefers-reduced-motion` — its picture, which no repository holds,
    // and nothing about a logical property.
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")
            || issue == "no picture was loaded for \"/assets/img/9hK3vQ2mR8pT1xWz4bC5dg\""),
        "{issues:#?}"
    );
}

#[test]
fn the_text_card_reaches_four_rem_back_over_the_picture() {
    let Some((_, rendering)) = text_image() else {
        panic!("the case renders");
    };
    let (sections, figures, cards) = (
        all(&rendering, "section"),
        all(&rendering, "figure"),
        all(&rendering, "div"),
    );
    let ([section], [figure], [card]) = (sections.as_slice(), figures.as_slice(), cards.as_slice())
    else {
        panic!("a section, a figure and a card: {sections:?} {figures:?} {cards:?}");
    };
    // Two `1fr` columns of 760 less the 2rem gap: 364 each, at 20 and 416.
    assert!(
        near(figure.origin.x, 20.0) && near(figure.size.width, 364.0),
        "{figure:?}"
    );
    // The card's column starts at 416; its margin-inline-start of -4rem
    // starts it at 352, and a stretched grid item takes the margin's 64
    // into its width: 428.
    assert!(
        near(card.origin.x, 352.0) && near(card.size.width, 428.0),
        "{card:?}"
    );
    assert!(
        card.origin.x < figure.origin.x + figure.size.width,
        "the card overlaps the picture: {card:?} {figure:?}"
    );
    // 3rem of section padding above it. Inside, `clamp(1.5rem, 4vw, 3rem)`
    // is 32 of padding above and below; the heading is `clamp(1.5rem,
    // 3.5vw, 2.125rem)`, 28 px, at 1.2 (33.6) with 0.5em below (14); the
    // sentence one line of 17 px at 1.6 (27.2) with 1em below (17).
    assert!(near(card.origin.y, 48.0), "{card:?}");
    assert!(
        near(card.size.height, 32.0 + 33.6 + 14.0 + 27.2 + 17.0 + 32.0),
        "{card:?}"
    );
    // 48 above and below the card, the tallest item.
    assert!(
        near(section.size.height, 48.0 + card.size.height + 48.0),
        "{section:?}"
    );
}

#[test]
fn the_cards_text_is_its_width_less_its_padding() {
    let Some((_, rendering)) = text_image() else {
        panic!("the case renders");
    };
    let (headings, paragraphs) = (all(&rendering, "h2"), all(&rendering, "p"));
    let ([heading], [line]) = (headings.as_slice(), paragraphs.as_slice()) else {
        panic!("a heading and a paragraph: {headings:?} {paragraphs:?}");
    };
    // 352 + 32 of padding, 428 − 64 wide.
    assert!(near(heading.origin.x, 384.0), "{heading:?}");
    assert!(near(heading.size.width, 364.0), "{heading:?}");
    assert!(near(line.size.width, 364.0), "{line:?}");
    // The sentence, 316.44 in DejaVu Sans at 17 px, is one line in 364.
    assert!(near(line.size.height, 27.2), "{line:?}");
}
