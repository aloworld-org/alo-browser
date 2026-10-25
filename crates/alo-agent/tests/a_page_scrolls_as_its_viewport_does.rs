/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The root scrolls as the viewport does, and a body whose `overflow` the
//! viewport takes scrolls nothing (queue item 662).
//!
//! Measured against Chrome 155, a page of 500 in a viewport shorter than it
//! is scrolled through `documentElement.scrollTop` whatever the root's
//! `overflow` — `visible`, `hidden`, `auto`, `clip`, and `overflow: hidden;
//! height: 100px` — and its `scrollHeight` is 500. A root as tall as its
//! content was refused here, because its own box had nothing past it.
//!
//! Under a root of `visible`, a `<body style="overflow:auto;height:50px">`
//! is not scrolled (`body.scrollTop` stays 0): the viewport takes its
//! `overflow`, the page scrolls instead, and its `scrollHeight` is still
//! 500. Under a root of `auto` the body scrolls itself and the page's
//! `scrollHeight` is 50. The agent's tree reads a body of no meaning of
//! its own as part of the page, so the body is asked through the layout
//! tree's `scrolls`, which is what an agent's node asks.

use alo_agent::{AgentTree, Outcome, Refusal, ScrollBy, Target, Verb, perform};
use alo_box::{BoxTree, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{LayoutTree, NoText, Size, compute, viewport};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

/// The window every page here is laid out in.
fn window() -> Size {
    Size::new(480.0, 113.0)
}

/// A page whose root has the style `root` and whose body has `body`,
/// holding one block 500 tall.
fn page(root: &str, body: &str) -> String {
    format!(
        "<!doctype html><html style='{root}'><body style='margin:0;{body}'>\
         <div style='height:500px'></div>"
    )
}

/// What a page laid out in [`window`] is: its boxes and their layout.
fn laid_out(page: &str) -> (BoxTree, LayoutTree) {
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let styles = resolve(
        &document,
        &[SourcedSheet::new(Origin::UserAgent, &agent)],
        &MediaContext::default(),
    );
    let boxes = build_boxes(&document, &styles);
    let layout = compute(&boxes, &styles, window(), &NoText);
    (boxes, layout)
}

/// What an agent's scroll down by 40 of the root's box is answered on
/// `page`.
fn scroll(page: &str) -> Option<Result<Outcome, Refusal>> {
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let styles = resolve(
        &document,
        &[SourcedSheet::new(Origin::UserAgent, &agent)],
        &MediaContext::default(),
    );
    let boxes = build_boxes(&document, &styles);
    let layout = compute(&boxes, &styles, window(), &NoText);
    let tree = AgentTree::new(&document, &boxes, &layout);
    Some(perform(
        &tree,
        &Target::Node(boxes.root()?),
        &Verb::Scroll(ScrollBy::Pixels(40.0)),
    ))
}

/// Whether the body of `page` would be scrolled, as the agent's tree asks
/// it ([`LayoutTree::scrolls`]); the agent's tree reads a body of no meaning
/// of its own as part of the page rather than as a node.
fn body_scrolls(page: &str) -> Option<bool> {
    let (boxes, layout) = laid_out(page);
    Some(layout.scrolls(boxes.body()?))
}

/// The page's scrolling area, as the root element's `scrollHeight` reads it.
fn page_area(boxes: &BoxTree, layout: &LayoutTree) -> Option<Size> {
    let root = boxes.root()?;
    Some(viewport::scrolling_area(
        layout.viewport(),
        layout.get(root),
    ))
}

/// Whether an agent's scroll of `page`'s root is answered `Scrolled`.
fn is_scrolled(page: &str) -> bool {
    matches!(scroll(page), Some(Ok(Outcome::Scrolled { .. })))
}

/// Whether an agent's scroll of `page`'s root is refused `DoesNotScroll`.
fn is_refused(page: &str) -> bool {
    matches!(scroll(page), Some(Err(Refusal::DoesNotScroll { .. })))
}

#[test]
fn a_long_page_is_scrolled_whatever_the_roots_overflow() {
    for root in [
        "",
        "overflow: visible",
        "overflow: hidden",
        "overflow: auto",
        "overflow: clip",
        "overflow-x: clip",
        "overflow-y: clip",
        "overflow: hidden; height: 100px",
    ] {
        let page = page(root, "");
        assert!(is_scrolled(&page), "{root}");
        let (boxes, layout) = laid_out(&page);
        assert_eq!(
            page_area(&boxes, &layout),
            Some(Size::new(480.0, 500.0)),
            "{root}"
        );
    }
}

#[test]
fn the_root_answers_from_the_viewport_not_from_its_own_box() {
    // The root is exactly as tall as what it holds, so its own box has
    // nothing past it; the window of 113 does.
    let (boxes, layout) = laid_out(&page("", ""));
    let root = boxes.root().expect("the root has a box");
    let geometry = layout.get(root).expect("the root was laid out");
    assert_eq!(geometry.border_box.size, Size::new(480.0, 500.0));
    assert!(
        !geometry.overflows(1.0),
        "its own box has nothing to scroll"
    );
    assert!(layout.scrolls(root));

    // A page shorter than the window has nothing to scroll.
    let short = "<!doctype html><body style=margin:0><div style='height:100px'></div>";
    assert!(is_refused(short));
    let (boxes, layout) = laid_out(short);
    assert_eq!(page_area(&boxes, &layout), Some(window()));
}

#[test]
fn a_body_whose_overflow_the_viewport_takes_is_not_scrolled() {
    for body in [
        "overflow: auto; height: 50px",
        "overflow: hidden; height: 50px",
        "overflow: scroll; height: 50px",
        "overflow: clip; height: 50px",
    ] {
        let page = page("", body);
        assert_eq!(body_scrolls(&page), Some(false), "{body}");
        assert!(is_scrolled(&page), "{body}");
        // In numbers: the body is 50 tall and what it holds reaches the
        // page's scrolling area, 500, as Chrome's `scrollHeight` is.
        let (boxes, layout) = laid_out(&page);
        let held = boxes.body().and_then(|id| layout.get(id));
        let held = held.expect("the body was laid out");
        assert_eq!(held.border_box.size, Size::new(480.0, 50.0), "{body}");
        assert_eq!(
            page_area(&boxes, &layout),
            Some(Size::new(480.0, 500.0)),
            "{body}"
        );
    }
}

#[test]
fn under_a_root_that_is_not_visible_the_body_scrolls_itself() {
    for root in ["overflow: auto", "overflow: clip"] {
        let page = page(root, "overflow: auto; height: 50px");
        assert_eq!(body_scrolls(&page), Some(true), "{root}");
        // The page is 50 tall, shorter than the window: Chrome's
        // `scrollHeight` is 50 and the viewport's own 113 is larger.
        assert!(is_refused(&page), "{root}");
        let (boxes, layout) = laid_out(&page);
        assert_eq!(page_area(&boxes, &layout), Some(window()), "{root}");
    }
}

#[test]
fn a_margin_collapses_through_a_body_whose_overflow_the_viewport_takes() {
    // Chrome 155: a first child's top margin of 30 puts the body's top at 30
    // under a root of `visible`, and at 0 under a root of `auto`, where the
    // body of `hidden` holds the margin in.
    let page = |root: &str| {
        format!(
            "<!doctype html><html style='{root}'><body style='margin:0;overflow:hidden'>\
             <div style='margin-top:30px;height:10px'></div>"
        )
    };
    let top = |root: &str| {
        let (boxes, layout) = laid_out(&page(root));
        boxes
            .body()
            .and_then(|id| layout.get(id))
            .map(|body| body.border_box.origin.y)
    };
    assert_eq!(top(""), Some(30.0));
    assert_eq!(top("overflow: auto"), Some(0.0));
}
