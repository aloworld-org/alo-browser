/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `text-overflow: ellipsis` through a whole document, in numbers (queue
//! item 442): which boxes end their lines, and that the "…" stays with its
//! box when the box is moved after its lines are placed.
//!
//! The test font is eight pixels a character, "…" included, so a line of ten
//! characters in a box of forty keeps four and puts the "…" at 32.

use alo_box::{BoxId, BoxKind, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{BlockFont, Ellipsis, LayoutTree, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// Every box that ends its lines in "…", and those ellipses, after laying
/// out `html` under `css` (and no margin on `body`) in an 800 × 600 window.
fn ellipses(html: &str, css: &str) -> (BoxTree, LayoutTree, Vec<(BoxId, Vec<Ellipsis>)>) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&format!("body {{ margin: 0 }}\n{css}"));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);
    let found = boxes
        .ids()
        .map(|id| (id, layout.ellipses(id)))
        .filter(|(_, held)| !held.is_empty())
        .collect();
    (boxes, layout, found)
}

/// The box of the element with this `class`, by its tag's position: the
/// tests below give each element they ask about a class of its own.
fn element_with_class(html: &str, boxes: &BoxTree, class: &str) -> Option<BoxId> {
    let document = parse_document(html);
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("class") == Some(class))
    })?;
    boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

const ENDS: &str = ".e { width: 40px; white-space: nowrap; text-overflow: ellipsis }";

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.001
}

#[test]
fn a_clipping_block_ends_its_line_four_characters_in() {
    let html = "<div class=e>aaaaaaaaaa</div>";
    let (boxes, layout, found) = ellipses(html, &format!("{ENDS} .e {{ overflow: hidden }}"));
    let div = element_with_class(html, &boxes, "e");
    let [(id, held)] = found.as_slice() else {
        panic!("one box ends its line: {found:?}");
    };
    assert_eq!(Some(*id), div);
    let [ellipsis] = held.as_slice() else {
        panic!("one line: {held:?}");
    };
    assert_eq!(ellipsis.line, 0);
    assert!(close(ellipsis.rect.origin.x, 32.0), "{ellipsis:?}");
    assert!(close(ellipsis.rect.size.width, 8.0), "{ellipsis:?}");
    // Layout itself did not move: the text is as wide as it is.
    let text = boxes.children(*id).next();
    let width = text
        .and_then(|text| layout.get(text))
        .map(|geometry| geometry.border_box.size.width);
    assert!(width.is_some_and(|width| close(width, 80.0)), "{width:?}");
}

#[test]
fn any_overflow_but_visible_on_either_axis_ends_it() {
    let html = "<div class=e>aaaaaaaaaa</div>";
    for overflow in [
        "overflow: hidden",
        "overflow: clip",
        "overflow: auto",
        "overflow: scroll",
        "overflow-x: hidden",
        "overflow-y: hidden",
        // Measured in Chrome 154: ended too, though CSS leaves `overflow-x`
        // at `visible` beside a `clip`.
        "overflow-y: clip",
    ] {
        let (_, _, found) = ellipses(html, &format!("{ENDS} .e {{ {overflow} }}"));
        assert_eq!(found.len(), 1, "{overflow}");
    }
    for nothing in ["overflow: visible", ""] {
        let (_, _, found) = ellipses(html, &format!("{ENDS} .e {{ {nothing} }}"));
        assert!(found.is_empty(), "{nothing:?}: {found:?}");
    }
    // And `clip`, the initial value, ends nothing however it clips.
    let (_, _, found) = ellipses(
        html,
        ".e { width: 40px; white-space: nowrap; overflow: hidden; text-overflow: clip }",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_run_beside_a_block_is_the_elements_and_a_flex_containers_text_is_not() {
    // The run of text before the `<p>` is an anonymous box of the `<div>`'s,
    // and its line is the `<div>`'s to end.
    let html = "<div class=e>aaaaaaaaaa<p>b</p></div>";
    let (boxes, _, found) = ellipses(html, &format!("{ENDS} .e {{ overflow: hidden }}"));
    let [(id, _)] = found.as_slice() else {
        panic!("the run ends its line: {found:?}");
    };
    assert!(
        matches!(
            boxes.get(*id).map(|node| &node.kind),
            Some(BoxKind::Anonymous { .. })
        ),
        "{id:?}"
    );
    // A flex container is not a block container: Chrome ends nothing in it.
    let (_, _, found) = ellipses(
        "<div class=e>aaaaaaaaaa</div>",
        &format!("{ENDS} .e {{ overflow: hidden; display: flex }}"),
    );
    assert!(found.is_empty(), "{found:?}");
    // Nor is the property inherited: a block inside one that ends its lines
    // ends none of its own.
    let (_, _, found) = ellipses(
        "<div class=e><div class=inner>aaaaaaaaaa</div></div>",
        &format!("{ENDS} .e {{ overflow: hidden }} .inner {{ width: 40px }}"),
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_ellipsis_moves_with_a_box_placed_after_its_lines() {
    // An inline-block is laid out on its own and then put in its line, 16
    // along after "xx"; an absolutely positioned box is put at its static
    // position, there too. The "…" is where the box's text ends, either way.
    for placed in ["display: inline-block", "position: absolute"] {
        let html = "<p>xx<span class=e>aaaaaaaaaa</span></p>";
        let (boxes, layout, found) =
            ellipses(html, &format!("{ENDS} .e {{ overflow: hidden; {placed} }}"));
        let span = element_with_class(html, &boxes, "e");
        let [(id, held)] = found.as_slice() else {
            panic!("{placed}: one box ends its line: {found:?}");
        };
        assert_eq!(Some(*id), span, "{placed}");
        let left = layout
            .get(*id)
            .map(|geometry| geometry.content_box().origin.x);
        assert!(
            left.is_some_and(|left| close(left, 16.0)),
            "{placed}: {left:?}"
        );
        assert!(
            close(held[0].rect.origin.x, 16.0 + 32.0),
            "{placed}: {held:?}"
        );
    }
}
