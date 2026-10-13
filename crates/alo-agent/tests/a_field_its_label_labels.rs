/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A field is named by a `<label>` only when that label labels it — HTML's
//! *labeled control*, asked of `alo-dom` (queue item 554), which also
//! decides what a hovered label hovers.
//!
//! Each name is Chrome 154's, read from its accessibility tree over CDP
//! (`Accessibility.getPartialAXTree` at each field), the source not read:
//! a field wrapped by a label whose `for` points elsewhere is not named by
//! it, nor is the second field inside a label, nor a field after a `<div>`
//! that took its id first, nor a field wrapped by a label with an empty
//! `for`; a hidden input inside a label is passed over for the field after
//! it, and a label inside another names the field it points at.
//!
//! A label that labels nothing is read as text, as Chrome's is; one that
//! labels a field has been read already, as the field's name, and is not
//! read twice.

use alo_agent::AgentTree;
use alo_box::{BoxTree, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{BlockFont, LayoutTree, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

/// The page Chrome was given.
const PAGE: &str = "<label for=x>Pointing elsewhere <input id=w1></label><input id=x>\
     <label>Two inside <input id=w2><input id=w3></label>\
     <label for=d>Div first</label><div id=d></div><input id=d>\
     <label for=hh>Hidden</label><input type=hidden id=hh>\
     <label>Wrap hidden <input type=hidden id=hz><input id=w4></label>\
     <label>Wrapper <label for=y>Inner</label></label><input id=y>\
     <label for=\"\">Empty for <input id=e1></label>\
     <label>Upper hidden <input type=HIDDEN id=e2><input id=e3></label>";

/// The document's boxes and their layout.
fn drawn(document: &Document) -> (BoxTree, LayoutTree) {
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet("body { margin: 0 }");
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(document, &sheets, &MediaContext::default());
    let boxes = build_boxes(document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 400.0), &BlockFont);
    (boxes, layout)
}

/// The outline's lines that start with `kind `, or are `kind`, in order,
/// with their positions taken off — which are layout's to assert.
fn lines(document: &Document, kind: &str) -> Vec<String> {
    let (boxes, layout) = drawn(document);
    AgentTree::new(document, &boxes, &layout)
        .to_outline()
        .lines()
        .map(|line| line.split(" at (").next().unwrap_or(line).trim().to_owned())
        .filter(|line| line.split(' ').next() == Some(kind))
        .collect()
}

#[test]
fn each_field_is_named_by_the_label_that_labels_it_as_chrome_names_it() {
    let document = parse_document(PAGE);
    assert_eq!(
        lines(&document, "textbox"),
        [
            "textbox",                        // w1: its label points at x
            "textbox \"Pointing elsewhere\"", // x
            "textbox \"Two inside\"",         // w2
            "textbox",                        // w3: the second inside
            "textbox",                        // d: a div took the id first
            "textbox \"Wrap hidden\"",        // w4, past the hidden input
            "textbox \"Inner\"",              // y
            "textbox",                        // e1: an empty for
            "textbox \"Upper hidden\"",       // e3, past HIDDEN
        ]
    );
}

#[test]
fn a_label_that_labels_nothing_is_read_as_text_and_one_that_does_is_not() {
    assert_eq!(
        lines(&parse_document(PAGE), "text"),
        [
            "text \"Div first\"",
            "text \"Hidden\"",
            "text \"Wrapper\"",
            "text \"Empty for\"",
        ]
    );
}
