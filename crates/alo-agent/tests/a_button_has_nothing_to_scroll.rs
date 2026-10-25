/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What has something to scroll is what Chrome would scroll (queue item 645).
//!
//! A `<button>Send</button>` in a body with no styles of its own was answered
//! `scrolled` when an agent asked to scroll it: its text, reached from the
//! corner of its padding box, is wider than its content box by the padding it
//! starts after. Chrome 155 measures the same button's `scrollHeight` and
//! `clientHeight` as 17 and 17, and its `scrollWidth` and `clientWidth` as 43
//! and 43, so there is nothing to scroll; and a box whose content passes its
//! content box by 0.5 pixels scrolls, where 0.45 does not.
//!
//! And a box that spills is scrolled only when it is a scroll container
//! (queue item 661): Chrome 155 leaves `scrollTop` at 0 for a box of
//! `overflow: visible` or `clip` however far its content reaches, and scrolls
//! one whose `overflow` is `hidden`, `scroll` or `auto` on either axis.
//!
//! A `<textarea>` is one of those (queue item 663): Chrome 155's sheet makes
//! it `overflow: auto` and keeps its lines, and scrolls one of two rows
//! holding six lines by 40.

use alo_agent::{AgentTree, Outcome, Refusal, ScrollBy, Target, Verb, perform};
use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};
use alo_layout::{BoxGeometry, Edges, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

/// Queue item 625's page, as far as its first button.
const PAGE: &str = "<!doctype html><body style=margin:0>\
     <h1 style=margin:0>Hello</h1>\
     <button>Send</button>";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    // A button's font is `Arial, sans-serif`, and a heading's the default.
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Sans");
    database
}

/// What an agent's scroll of the box of the element `id` names, down by 40,
/// is answered on `page` on a screen of this `scale`, with where that box
/// ended up; or [`None`] when `id` names no element with a box.
fn scroll_at(page: &str, id: &str, scale: f32) -> Option<(Result<Outcome, Refusal>, BoxGeometry)> {
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let styles = resolve(
        &document,
        &[SourcedSheet::new(Origin::UserAgent, &agent)],
        &MediaContext::default().at_scale(scale),
    );
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let layout = compute(
        &boxes,
        &styles,
        Size::new(480.0, 400.0),
        &TextMeasurer::new(&database),
    );
    let tree = AgentTree::new(&document, &boxes, &layout);
    let node = element(&document, id)?;
    let target = boxes
        .ids()
        .find(|&each| boxes.get(each).and_then(|held| held.kind.node()) == Some(node))?;
    let answer = perform(
        &tree,
        &Target::Node(target),
        &Verb::Scroll(ScrollBy::Pixels(40.0)),
    );
    Some((answer, layout.get(target)?))
}

/// [`scroll_at`] a scale of one.
fn scroll(page: &str, id: &str) -> Option<(Result<Outcome, Refusal>, BoxGeometry)> {
    scroll_at(page, id, 1.0)
}

/// The element whose `id` is `id`.
fn element(document: &Document, id: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|&each| document.attribute_by_name(each, "id") == Some(id))
}

#[test]
fn a_plain_button_is_refused_a_scroll_as_a_heading_is() {
    let page = PAGE
        .replace("<button>", "<button id=send>")
        .replace("<h1 ", "<h1 id=hello ");
    let (answer, button) = scroll(&page, "send").expect("the button has a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    let (answer, _) = scroll(&page, "hello").expect("the heading has a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );

    // In numbers: the button's text fills its content box exactly, reached
    // from its padding box's corner, 6 across and 1 down — so what the
    // content reaches is wider than the content box, and none of it spills.
    assert_eq!(
        button.padding,
        Edges {
            top: 1.0,
            right: 6.0,
            bottom: 1.0,
            left: 6.0,
        }
    );
    let content = button.content_box();
    assert_eq!(
        (content.origin.x, content.origin.y, content.size.height),
        (8.0, 41.0, 15.0)
    );
    assert!(
        (button.reach.width - (6.0 + content.size.width)).abs() < 1e-4,
        "{button:?}"
    );
    assert!((button.reach.height - 16.0).abs() < 1e-4, "{button:?}");
    assert!(button.reach.width > content.size.width);
    assert!(!button.overflows(1.0));
}

/// A list of 20 pixels that scrolls, holding `tall` pixels of one item.
fn a_list_holding(tall: &str) -> String {
    format!(
        "<!doctype html><body style=margin:0>\
         <ul id=list style='height:20px;overflow:auto;margin:0;padding:7px 0'>\
         <li style='height:{tall}px'></li></ul>"
    )
}

#[test]
fn a_box_has_something_to_scroll_once_it_spills_by_half_a_pixel() {
    // Chrome 155: 20.5 pixels in 20 scrolls, by 1, and 20.45 does not.
    let (answer, list) = scroll(&a_list_holding("20.5"), "list").expect("a box");
    assert!(matches!(answer, Ok(Outcome::Scrolled { .. })), "{answer:?}");
    // In numbers: a padding box of 34 (`clientHeight`), and a scrolling
    // area of 7 + 20.5 + 7 (`scrollHeight`), the end padding counted.
    assert!((list.padding_box().size.height - 34.0).abs() < 1e-4);
    assert!(
        (list.scrolling_area().height - 34.5).abs() < 1e-4,
        "{list:?}"
    );

    let (answer, list) = scroll(&a_list_holding("20.45"), "list").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    // On the grid of sixty-fourths, 20.45 is 20.4375: 0.4375 past the edge.
    assert!(
        (list.scrolling_area().height - 34.4375).abs() < 1e-4,
        "{list:?}"
    );
}

#[test]
fn half_a_pixel_is_the_screens_so_at_two_it_is_a_quarter() {
    // Chrome 155 at a scale of two: 0.25 past the content box scrolls, by
    // half a pixel, and 0.24 does not. 20.3 is 20.296875 on either grid.
    let (answer, _) = scroll_at(&a_list_holding("20.3"), "list", 2.0).expect("a box");
    assert!(matches!(answer, Ok(Outcome::Scrolled { .. })), "{answer:?}");
    let (answer, _) = scroll_at(&a_list_holding("20.3"), "list", 1.0).expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    let (answer, _) = scroll_at(&a_list_holding("20.2"), "list", 2.0).expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
}

/// A list of 10 pixels, styled `style`, holding seven lines of 18.
fn a_short_list(style: &str) -> String {
    format!(
        "<!doctype html><body style=margin:0>\
         <ul id=list style='height:10px;margin:0;padding:0;line-height:18px;{style}'>\
         <li>a<li>b<li>c<li>d<li>e<li>f<li>g</ul>"
    )
}

#[test]
fn a_box_that_spills_is_scrolled_only_as_a_scroll_container() {
    // Chrome 155: 126 in 10 with `overflow: visible` or `clip` leaves
    // `scrollTop` 0; `overflow-x: hidden`, which computes `overflow-y` as
    // `auto`, scrolls it by 40, and so does `hidden` beside `clip`.
    for style in [
        "",
        "overflow: visible",
        "overflow: clip",
        "overflow-x: clip",
    ] {
        let (answer, list) = scroll(&a_short_list(style), "list").expect("a box");
        assert!(
            matches!(answer, Err(Refusal::DoesNotScroll { .. })),
            "{style}: {answer:?}"
        );
        // In numbers: it spills, by 116, and is still not scrolled.
        assert!((list.padding_box().size.height - 10.0).abs() < 1e-4);
        assert!(
            (list.scrolling_area().height - 126.0).abs() < 1e-4,
            "{style}: {list:?}"
        );
        assert!(list.overflows(1.0), "{style}");
    }
    for style in [
        "overflow: auto",
        "overflow: hidden",
        "overflow-x: hidden",
        "overflow-y: scroll",
        "overflow-x: hidden; overflow-y: clip",
    ] {
        let (answer, _) = scroll(&a_short_list(style), "list").expect("a box");
        assert!(
            matches!(answer, Ok(Outcome::Scrolled { .. })),
            "{style}: {answer:?}"
        );
    }
}

#[test]
fn a_short_button_spills_and_is_not_scrolled_unless_it_is_a_scroll_container() {
    // Chrome 155: a button of `height: 8px` holding three lines is not
    // scrolled; under `overflow: auto` it is.
    let page = |style: &str| {
        format!(
            "<!doctype html><body style=margin:0>\
             <button id=b style='height:8px;{style}'>a<br>b<br>c</button>"
        )
    };
    let (answer, button) = scroll(&page(""), "b").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    assert!(button.overflows(1.0), "{button:?}");
    let (answer, _) = scroll(&page("overflow:auto"), "b").expect("a box");
    assert!(matches!(answer, Ok(Outcome::Scrolled { .. })), "{answer:?}");
}

#[test]
fn a_field_of_text_scrolls_along_its_line_and_a_date_field_does_not() {
    // Chrome 155 scrolls a long value's field by `scrollLeft` 40, though it
    // computes the field's `overflow` as `clip` even under `visible`.
    let long = "<!doctype html><body style=margin:0>\
         <input id=f style=overflow:visible \
         value=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa>";
    let (answer, field) = scroll(long, "f").expect("a box");
    assert!(matches!(answer, Ok(Outcome::Scrolled { .. })), "{answer:?}");
    assert!(
        field.scrolling_area().width > field.padding_box().size.width + 0.5,
        "{field:?}"
    );
    let short = "<!doctype html><body style=margin:0><input id=f value=a>";
    let (answer, _) = scroll(short, "f").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "a field with nothing past its edge: {answer:?}"
    );
    // A date field held in segments is not scrolled, however narrow.
    let date = "<!doctype html><body style=margin:0>\
         <input id=f type=date style=width:20px value=2026-10-25>";
    let (answer, _) = scroll(date, "f").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
}

/// A `<textarea>` of two rows holding six lines, under `style`.
fn a_text_area(style: &str) -> String {
    format!(
        "<!doctype html><body style=margin:0>\
         <textarea id=t rows=2 style='{style}'>1\n2\n3\n4\n5\n6</textarea>"
    )
}

#[test]
fn a_text_area_holds_its_lines_past_its_edge_and_scrolls_to_them() {
    // Chrome 155 (queue item 663): six lines of 15 in a `<textarea rows=2>`
    // make a scrolling area of 94, its padding of 2 above and below
    // included, and `scrollTop` 40 scrolls it by 40.
    let (answer, area) = scroll(&a_text_area(""), "t").expect("a box");
    assert!(matches!(answer, Ok(Outcome::Scrolled { .. })), "{answer:?}");
    assert!(
        (area.scrolling_area().height - 94.0).abs() < 1e-4,
        "{area:?}"
    );
    // Its own height is the sheet's `3em`, 40, and 44 with its padding:
    // Chrome's is two rows, 34 with its padding (queue item 523).
    assert!(
        (area.padding_box().size.height - 44.0).abs() < 1e-4,
        "{area:?}"
    );
    assert!(
        (area.scrolling_area().width - area.padding_box().size.width).abs() < 1e-4,
        "nothing passes it across: {area:?}"
    );
    // Two lines are held inside it, and there is nothing to scroll.
    let short = "<!doctype html><body style=margin:0><textarea id=t>1\n2</textarea>";
    let (answer, area) = scroll(short, "t").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    // Its scrolling area is its padding box, as Chrome's `scrollHeight` is
    // its `clientHeight` (34 and 34 in Chrome's smaller box).
    assert!(
        (area.scrolling_area().height - area.padding_box().size.height).abs() < 1e-4,
        "{area:?}"
    );
}

#[test]
fn a_text_area_scrolls_whatever_its_author_says_but_clip() {
    // Chrome 155 computes a `<textarea>`'s `overflow: visible` as `auto`
    // and scrolls it; it scrolls `hidden` and `scroll`, and not `clip`.
    for style in [
        "overflow:visible",
        "overflow-y:visible",
        "overflow:hidden",
        "overflow:scroll",
        "overflow-x:clip",
        "overflow:visible;display:block",
    ] {
        let (answer, _) = scroll(&a_text_area(style), "t").expect("a box");
        assert!(
            matches!(answer, Ok(Outcome::Scrolled { .. })),
            "{style}: {answer:?}"
        );
    }
    let (answer, area) = scroll(&a_text_area("overflow:clip"), "t").expect("a box");
    assert!(
        matches!(answer, Err(Refusal::DoesNotScroll { .. })),
        "{answer:?}"
    );
    assert!(
        area.overflows(1.0),
        "it spills and is not scrolled: {area:?}"
    );
}
