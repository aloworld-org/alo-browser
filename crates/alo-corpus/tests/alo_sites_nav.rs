/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' navigation, in numbers: the page that opened queue item 395.
//!
//! `cases/alo-sites-nav` pins the whole page as files. This says out loud
//! what item 395 is closed by. Every link in the menu is `display:
//! inline-flex` with `min-height: 2.75rem`, sitting in a list item's line,
//! and until item 395 layout stood an `inline-flex` box on its bottom edge
//! rather than on its first item's baseline. Each list item gained the
//! strut's descent under its link — 7.7 px at this font — and the bar grew
//! from 73.4 to 81.1.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn nav() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-nav"))?;
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
    let Some((case, Rendering::Loaded(_, answered))) = nav() else {
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
fn what_it_records_is_the_sheets_list_and_sticky() {
    let Some((_, rendering)) = nav() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    // The other alo Sites cases' list exactly — pseudo-elements, `@keyframes`
    // and `prefers-reduced-motion` — and `position: sticky`, which is item
    // 97's and needs a scroll position. At the top of a page that has not
    // scrolled, a sticky bar stands where a static one does, so nothing on
    // this page moves for it.
    let sticky = issues
        .iter()
        .filter(|issue| issue.contains("position: sticky"))
        .count();
    assert_eq!(sticky, 1, "{issues:#?}");
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")
            || issue.contains("position: sticky")),
        "{issues:#?}"
    );
}

#[test]
fn every_link_in_the_menu_stands_on_its_texts_baseline() {
    let Some((_, rendering)) = nav() else {
        panic!("the case renders");
    };
    let (items, links, lists, navs, headers) = (
        all(&rendering, "li"),
        all(&rendering, "a"),
        all(&rendering, "ul"),
        all(&rendering, "nav"),
        all(&rendering, "header"),
    );
    let (
        [home, pricing, order],
        [_skip, brand, home_link, pricing_link, order_link],
        [list],
        [bar],
        [header],
    ) = (
        items.as_slice(),
        links.as_slice(),
        lists.as_slice(),
        navs.as_slice(),
        headers.as_slice(),
    )
    else {
        panic!("three items, five links, a list, a nav and a header: {items:?} {links:?}");
    };
    // A link is `inline-flex`, `min-height: 2.75rem` (44), and its text is a
    // 27.2 line centred in it, so its baseline is its text's, 8.4 down plus
    // the text's own. That is lower in the link than the strut's is in the
    // item's line, so the link's top is the line's top and the item is as
    // tall as the link: 44. Stood on its bottom edge, the strut's descent
    // hung under it and the item was 51.7.
    for (item, link) in [(home, home_link), (pricing, pricing_link)] {
        assert!(near(item.size.height, 44.0), "{item:?}");
        assert!(near(link.size.height, 44.0), "{link:?}");
        assert!(near(link.origin.y, item.origin.y), "{link:?} in {item:?}");
    }
    // The button's 9.6 of padding and 1 of border each side make it 48.4,
    // taller than its minimum, and its item is that too, not 56.1.
    assert!(near(order.size.height, 48.4), "{order:?}");
    assert!(near(order_link.size.height, 48.4), "{order_link:?}");
    assert!(near(order_link.origin.y, order.origin.y), "{order_link:?}");
    // `align-items: center` puts the shorter items 2.2 below the button's.
    assert!(near(order.origin.y, 12.0), "{order:?}");
    assert!(near(home.origin.y, 14.2), "{home:?}");
    // The list is its tallest item; the bar is the list and its 0.75rem of
    // padding top and bottom, and the brand is centred in it beside the list.
    assert!(near(list.size.height, 48.4), "{list:?}");
    assert!(near(bar.size.height, 72.4), "{bar:?}");
    assert!(near(brand.size.height, 44.0), "{brand:?}");
    assert!(near(brand.origin.y, 14.2), "{brand:?}");
    // The header adds its 1 px border along the bottom.
    assert!(near(header.size.height, 73.4), "{header:?}");
    // Across: the menu is pushed to the end by the brand's `margin-right:
    // auto`, and ends at the nav's 1.25rem of padding.
    assert!(near(list.origin.x + list.size.width, 780.0), "{list:?}");
    assert!(near(brand.origin.x, 20.0), "{brand:?}");
}
