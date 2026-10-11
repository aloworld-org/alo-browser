/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' blog index, in numbers: the page that opened queue item 354.
//!
//! `cases/alo-sites-blog-index` pins the whole page as files. This says out
//! loud what item 354 is closed by. Every card ends in `Read article<span
//! class="sr-only">: Launch notes</span>`, a link whose `<span>` is
//! `position: absolute`, one pixel square, for a reader of the screen. The
//! engine broke the link around the `<span>` as around a block: the link was
//! two pieces and an agent read it at (44, 768) in two, and the `<span>`
//! stood at the start of a line of its own, 115 to the left of where it was
//! written.
//!
//! Measured in Google Chrome 154, headless, on the same page and sheet at
//! 800 wide with the corpus' `DejaVu Sans` given to it: the link is one
//! rectangle, and the `<span>` stands at the end of "Read article" less its
//! own `margin: -1px`, a pixel above the top of the link's line — 159.4375
//! across where the link starts at 45 and is 115.4375 wide.
//!
//! The cards are side by side, as they are in Chrome: two columns of 368 at
//! x 20 and 412. The grid is `repeat(auto-fit, minmax(min(100%, 18rem),
//! 1fr))`, a track list `taffy` throws away because its smallest size is a
//! `min()`; until queue item 422 (ADR 0047) it was, and the cards were one
//! column of rows 760 wide.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked
//! for and answered is said once there and only checked here.

use alo_agent::AgentTree;
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, read and rendered.
fn blog_index() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-blog-index"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The boxes of every element whose `class` names `class`, in document
/// order: each one's own border box and its parent box's.
fn with_class(rendering: &Rendering, class: &str) -> Vec<(Rect, Rect)> {
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
        .filter_map(|node| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|box_node| &box_node.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })?;
            let parent = boxes.get(id)?.parent?;
            Some((
                drawing.layout.get(id)?.border_box,
                drawing.layout.get(parent)?.border_box,
            ))
        })
        .collect()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = blog_index() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(
        case.address.as_deref(),
        Some("https://north.sites.test/blog?page=2")
    );
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
}

#[test]
fn what_it_records_is_the_sheets_list_and_its_missing_cover() {
    let Some((_, rendering)) = blog_index() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let issues = drawing.issues(document);
    // The other alo Sites cases' list, the cover nobody has, and nothing
    // about a box out of flow: its containing block is the page's.
    assert!(
        issues.iter().all(|issue| issue.contains("::before")
            || issue.contains("::after")
            || issue.contains("@keyframes")
            || issue.contains("prefers-reduced-motion")
            || issue == "no picture was loaded for \"/assets/img/blob_cover\""),
        "{issues:#?}"
    );
}

#[test]
fn each_read_link_is_one_line_with_its_span_at_the_end() {
    let Some((_, rendering)) = blog_index() else {
        panic!("the case renders");
    };
    let links = with_class(&rendering, "blog-read");
    let spans = with_class(&rendering, "sr-only");
    assert_eq!(links.len(), 2, "one link a card, unbroken: {links:?}");
    assert_eq!(spans.len(), 2, "{spans:?}");
    for ((link, line), (span, _)) in links.iter().zip(&spans) {
        // The link's line is the run that holds it: one line of 17 px at
        // 1.6, 27.2 floored to 1/64 (ADR 0050 § 3), where the link starts.
        assert!(near(line.size.height, 1740.0 / 64.0), "{line:?}");
        assert!(near(link.origin.x, line.origin.x), "{link:?} {line:?}");
        // One pixel square, at the end of the link's text and the top of
        // its line, less its margin of -1px each way.
        assert!(
            near(span.size.width, 1.0) && near(span.size.height, 1.0),
            "{span:?}"
        );
        assert!(
            near(span.origin.x, link.origin.x + link.size.width - 1.0),
            "{span:?} {link:?}"
        );
        assert!(
            near(span.origin.y, line.origin.y - 1.0),
            "{span:?} {line:?}"
        );
    }
}

#[test]
fn an_agent_reads_each_read_link_in_one_piece() {
    let Some((_, rendering)) = blog_index() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let tree = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    let read: Vec<&str> = tree
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("link \"Read article"))
        .collect();
    assert_eq!(read.len(), 2, "{tree}");
    assert!(read.iter().all(|line| !line.contains("pieces")), "{tree}");
    // Each where its card's padding starts: the first card's at 45, as in
    // Chrome, and the second's 392 to the right of it, in the second column.
    assert!(
        read.first().is_some_and(|line| line.contains(" at (45, "))
            && read.get(1).is_some_and(|line| line.contains(" at (437, ")),
        "{tree}"
    );
}

#[test]
fn its_cards_are_two_columns_of_368_as_chromes_are() {
    let Some((_, rendering)) = blog_index() else {
        panic!("the case renders");
    };
    let cards = with_class(&rendering, "blog-card");
    assert_eq!(cards.len(), 2, "{cards:?}");
    let (first, grid) = cards[0];
    let (second, _) = cards[1];
    // Chrome 154, on the same page and sheet at 800 wide: `.blog-grid` is
    // 760 at x 20, and `min(100%, 18rem)` is 288 there, so two tracks and a
    // gap of 1.5rem fit, each 368 by its `1fr`.
    assert!(
        near(grid.origin.x, 20.0) && near(grid.size.width, 760.0),
        "{grid:?}"
    );
    assert!(
        near(first.origin.x, 20.0) && near(first.size.width, 368.0),
        "{first:?}"
    );
    assert!(
        near(second.origin.x, 412.0) && near(second.size.width, 368.0),
        "{second:?}"
    );
    assert!(
        near(first.origin.y, second.origin.y),
        "one row: {first:?} {second:?}"
    );
}
