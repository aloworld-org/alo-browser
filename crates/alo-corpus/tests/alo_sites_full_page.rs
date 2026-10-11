/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' full page, in numbers: the page that opened queue items 286
//! and 430.
//!
//! `cases/alo-sites-full-page` pins the whole page as files. This says out
//! loud what item 286 is closed by. The brand is `<a class="brand"><img
//! class="logo" alt="Nordwind Coffee Roasters"></a>`: the nav makes the link
//! `inline-flex`, and the sheet makes every `<img>` `display: block`. The box
//! tree broke the link around the picture as CSS breaks a `<span>` around a
//! block — two empty links, 0 wide at x 20 and 403.29, with the picture a
//! block of its own between them at x 93.63 — where a flex container's
//! children are its items whatever their `display`.
//!
//! Measured in Google Chrome 154, headless, on the same page and sheet at
//! 800 wide with the corpus' `DejaVu Sans` given to it: the link is one box
//! at (20, 14.1875), 44 tall, and the picture in it at (20, 20.1875), 32
//! tall. Chrome's link is 309.65625 wide, 16 more than its `alt` here: the
//! icon it draws before a broken picture's text, which is queue item 418.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked
//! for and answered is said once there and only checked here.

use alo_agent::AgentTree;
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn full_page() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-full-page"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border boxes of every box made for an element whose `class` names
/// `class`, in document order, each with the border boxes of the boxes made
/// for that element's children.
fn with_class(rendering: &Rendering, class: &str) -> Vec<(Rect, Vec<Rect>)> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| {
            document.element(*id).is_some_and(|element| {
                element
                    .attr("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
            })
        })
        .flat_map(|node| {
            boxes.ids().filter(move |id| {
                matches!(
                    boxes.get(*id).map(|box_node| &box_node.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })
        })
        .filter_map(|id| {
            let children = boxes
                .children(id)
                .filter_map(|child| Some(drawing.layout.get(child)?.border_box))
                .collect();
            Some((drawing.layout.get(id)?.border_box, children))
        })
        .collect()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = full_page() else {
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
fn what_it_records_is_the_sheets_list_its_sticky_nav_and_its_pictures() {
    let Some((_, rendering)) = full_page() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")
            || issue.ends_with("position: sticky")
            || issue.starts_with("no picture was loaded for \"/assets/img/")),
        "{issues:#?}"
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.ends_with("position: sticky")),
        "{issues:#?}"
    );
}

#[test]
fn the_brand_is_one_link_holding_its_logo_where_chrome_puts_it() {
    let Some((_, rendering)) = full_page() else {
        panic!("the case renders");
    };
    let brands = with_class(&rendering, "brand");
    assert_eq!(brands.len(), 1, "one box, not pieces: {brands:?}");
    let (link, held) = &brands[0];
    // Chrome 154: (20, 14.1875), 44 tall, and the logo (20, 20.1875), 32
    // tall. Its width here is the `alt`'s, Chrome's 16 more (item 418).
    assert!(
        near(link.origin.x, 20.0) && near(link.origin.y, 14.2) && near(link.size.height, 44.0),
        "{link:?}"
    );
    assert_eq!(held.len(), 1, "it holds the logo: {held:?}");
    let logo = held[0];
    assert!(
        near(logo.origin.x, 20.0) && near(logo.origin.y, 20.2) && near(logo.size.height, 32.0),
        "{logo:?}"
    );
    assert!(near(link.size.width, logo.size.width), "{link:?} {logo:?}");
    let logos = with_class(&rendering, "logo");
    assert_eq!(logos.len(), 1, "{logos:?}");
    assert_eq!(logos[0].0, logo, "and the box it holds is the logo's");
}

#[test]
fn an_agent_reads_the_brand_once_holding_its_picture() {
    let Some((_, rendering)) = full_page() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let tree = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    let lines: Vec<&str> = tree.lines().collect();
    let at = lines.iter().position(|line| {
        line.trim_start()
            .starts_with("link \"Nordwind Coffee Roasters\"")
    });
    let Some(at) = at else {
        panic!("the brand is read as a link:\n{tree}");
    };
    let link = lines[at];
    assert!(!link.contains("pieces"), "{tree}");
    assert!(link.contains(" at (20, 14.2"), "{tree}");
    let next = lines.get(at + 1).copied().unwrap_or_default();
    let indent = |line: &str| line.len() - line.trim_start().len();
    assert!(
        next.trim_start()
            .starts_with("image \"Nordwind Coffee Roasters\" [broken]")
            && indent(next) > indent(link),
        "the picture is read inside it:\n{tree}"
    );
}

#[test]
fn the_custom_code_sections_border_is_whole_pixels_as_chrome_draws_it() {
    // Item 430. The section's `border-left: .3rem` is 4.8 pixels as written;
    // Chrome 154 snaps a border width to whole device pixels, lays it out 4
    // and starts what is inside at 44 — the 4 and `clamp(1.5rem, 5vw, 4rem)`'s
    // 40 at 800 wide. Here it was 44.8, and drawn 4.8 wide.
    let Some((_, rendering)) = full_page() else {
        panic!("the case renders");
    };
    let sections = with_class(&rendering, "s-custom-code");
    assert_eq!(sections.len(), 1, "{sections:?}");
    let (section, held) = &sections[0];
    assert!(!held.is_empty(), "it holds its heading and its frame");
    for inside in held {
        assert!(
            near(inside.origin.x, 44.0) && near(inside.size.width, 736.0),
            "{inside:?}"
        );
    }
    let Some(drawing) = rendering.drawing() else {
        panic!("the case has a drawing");
    };
    let display = drawing.display.to_outline();
    let border = format!(" at (0, {}) 4×{}", section.origin.y, section.size.height);
    assert!(
        display
            .lines()
            .any(|line| line.starts_with("fill ") && line.contains(&border)),
        "the border is drawn 4 wide, looked for{border}:\n{display}"
    );
}
