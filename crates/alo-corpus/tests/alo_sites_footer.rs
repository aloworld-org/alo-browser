/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' footer, in numbers: the page that opened queue item 355.
//!
//! `cases/alo-sites-footer` pins the whole page as files. This says out loud
//! what item 355 is closed by. The page's `<main>` is empty, so the footer's
//! `margin-top: 3rem` collapses through it and through `body`, and `body`
//! starts 48 down. The skip link, `position: absolute; top: 0`, has nothing
//! positioned above it and belongs at the top of the page (CSS 2 § 10.1);
//! layout placed every absolute box against its parent, and drew it 48 down
//! with `body`.
//!
//! The footer carries `section-motion` and is drawn at opacity 0 until
//! `IntersectionObserver` exists (item 381), so it is checked here in numbers
//! rather than in pixels. It is loaded as the call-to-action section is
//! (`alo_sites_cta.rs`), from the same frame, sheet and analytics script, so
//! how its sheet is asked for and answered is said once there and only
//! checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn footer() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-footer"))?;
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
    let Some((case, Rendering::Loaded(_, answered))) = footer() else {
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
fn what_it_records_is_the_sheets_list_and_nothing_of_its_skip_link() {
    let Some((_, rendering)) = footer() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    // The other alo Sites cases' list exactly — pseudo-elements, `@keyframes`
    // and `prefers-reduced-motion`. Nothing about where an absolute box goes.
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")),
        "{issues:#?}"
    );
}

#[test]
fn the_skip_link_is_at_the_top_of_the_page_while_the_body_is_not() {
    let Some((_, rendering)) = footer() else {
        panic!("the case renders");
    };
    let (bodies, links) = (all(&rendering, "body"), all(&rendering, "a"));
    let ([body], [skip, ..]) = (bodies.as_slice(), links.as_slice()) else {
        panic!("a body and links: {bodies:?} {links:?}");
    };
    // The footer's 3rem collapses through the empty `main` and `body`.
    assert!(near(body.origin.y, 48.0), "{body:?}");
    // `left: -999rem; top: 0` against the initial containing block.
    assert!(near(skip.origin.x, -999.0 * 16.0), "{skip:?}");
    assert!(near(skip.origin.y, 0.0), "{skip:?}");
    // One line of 17 px at 1.6 and 0.5rem of padding above and below.
    assert!(near(skip.size.height, 27.2 + 16.0), "{skip:?}");
}

#[test]
fn its_two_columns_split_the_footer_three_rem_apart() {
    let Some((_, rendering)) = footer() else {
        panic!("the case renders");
    };
    let (footers, navs, paragraphs) = (
        all(&rendering, "footer"),
        all(&rendering, "nav"),
        all(&rendering, "p"),
    );
    let ([footer], [nav], [line]) = (footers.as_slice(), navs.as_slice(), paragraphs.as_slice())
    else {
        panic!("a footer, a nav and a line: {footers:?} {navs:?} {paragraphs:?}");
    };
    // At y 48, 800 wide: 1 of border and 3rem of padding above and below one
    // line of 27.2.
    assert_eq!(*footer, Rect::new(0.0, 48.0, 800.0, 124.2));
    // 760 of content less the 3rem column gap, in halves of 356.
    assert!(
        near(nav.origin.x, 20.0) && near(nav.size.width, 356.0),
        "{nav:?}"
    );
    assert!(
        near(line.origin.x, 424.0) && near(line.size.width, 356.0),
        "{line:?}"
    );
    assert!(
        near(nav.origin.y, 97.0) && near(line.origin.y, 97.0),
        "{nav:?} {line:?}"
    );
}
