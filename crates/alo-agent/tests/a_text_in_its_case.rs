/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ What an agent reads of a text in a case (ADR 0049 § 4).
//!
//! Everything drawn from the document's text is read as drawn — text, a
//! name from content, a name from a label — because that is what a person
//! sees and what Chrome gives a screen reader. Everything a page gives as an
//! attribute, and a field's value, is read as written: no style applies to
//! the first, and the second is what the form submits.

use alo_agent::{AgentTree, Target, Verb, apply, perform};
use alo_box::{BoxTree, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{BlockFont, LayoutTree, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

const SHEET: &str = "body { margin: 0 } .up { text-transform: uppercase } \
                     .down { text-transform: lowercase }";

/// The document's boxes and their layout.
fn drawn(document: &Document) -> (BoxTree, LayoutTree) {
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(document, &sheets, &MediaContext::default());
    let boxes = build_boxes(document, &styles);
    let layout = compute(&boxes, &styles, Size::new(400.0, 400.0), &BlockFont);
    (boxes, layout)
}

/// The outline's lines with their positions taken off, which are layout's
/// to assert, not this file's.
fn read(document: &Document) -> Vec<String> {
    let (boxes, layout) = drawn(document);
    AgentTree::new(document, &boxes, &layout)
        .to_outline()
        .lines()
        .map(|line| line.split(" at (").next().unwrap_or(line).trim().to_owned())
        .collect()
}

#[test]
fn text_and_a_name_from_content_are_read_as_drawn() {
    let document = parse_document(
        "<h2 class=up>My calendars</h2><span class=down>Can edit</span>\
         <button class=up>Save draft</button><a class=up href=/x>Link text</a>",
    );
    let outline = read(&document);
    for line in [
        "heading \"MY CALENDARS\" [level=2]",
        "text \"can edit\"",
        "button \"SAVE DRAFT\"",
        "link \"LINK TEXT\"",
    ] {
        assert!(
            outline.iter().any(|read| read == line),
            "{line}: {outline:#?}"
        );
    }
}

#[test]
fn a_name_from_a_label_is_the_label_as_drawn() {
    let document = parse_document(
        "<label class=up>Wrapped label <input value=kept></label>\
         <label class=up for=p>Pointing label</label><input id=p>\
         <label for=h style='display: none'>Hidden label</label><input id=h>",
    );
    let outline = read(&document);
    for line in [
        "textbox \"WRAPPED LABEL\"",
        "textbox \"POINTING LABEL\"",
        "textbox \"Hidden label\"",
    ] {
        assert!(
            outline.iter().any(|read| read == line),
            "{line}: {outline:#?}"
        );
    }
}

#[test]
fn an_attribute_and_a_fields_value_are_read_as_written() {
    let document = parse_document(
        "<button class=up aria-label='Close dialog'>x</button>\
         <img class=up alt='A broken picture'>\
         <input class=up aria-label=Field value='typed value'>\
         <textarea class=up aria-label=Notes>written notes</textarea>",
    );
    let outline = read(&document);
    for line in [
        "button \"Close dialog\"",
        "image \"A broken picture\" [broken]",
        "text \"typed value\"",
        "text \"written notes\"",
    ] {
        assert!(
            outline.iter().any(|read| read == line),
            "{line}: {outline:#?}"
        );
    }
    assert!(
        !outline
            .iter()
            .any(|read| read.contains("TYPED") || read.contains("WRITTEN")),
        "{outline:#?}"
    );
}

#[test]
fn an_agent_finds_what_the_page_wrote_and_writes_as_it_is_told() {
    let mut document = parse_document(
        "<h2 class=up>My calendars</h2><label class=up for=n>Name</label><input class=up id=n>",
    );
    let (boxes, layout) = drawn(&document);
    let tree = AgentTree::new(&document, &boxes, &layout);
    let found: Vec<Option<String>> = tree
        .named("My calendars")
        .iter()
        .map(alo_agent::AgentNode::name)
        .collect();
    assert_eq!(found, [Some("MY CALENDARS".to_owned())]);
    let decided = perform(
        &tree,
        &Target::Named("name".to_owned()),
        &Verb::PutText("ada lovelace".to_owned()),
    );
    let outcome = decided.expect("the field is found by the name the page wrote");
    apply(&mut document, &boxes, &outcome);
    let outline = read(&document);
    assert!(
        outline.iter().any(|read| read == "text \"ada lovelace\""),
        "{outline:#?}"
    );
}
