/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ A field's hint, as an agent reads it (ADR 0043 § 5, queue item 389).
//!
//! A placeholder looks like text in the field and is not in it. An agent
//! that read it as the field's content would believe the field filled and
//! submit an empty form; one that never learned of it loses the one hint the
//! author gave about what goes in. So it is the field's `placeholder`
//! property — ARIA's `aria-placeholder` — while the field holds nothing,
//! never its text, and its name only when nothing else names it.

use alo_agent::{AgentTree, Target, Verb, apply, perform};
use alo_box::{BoxTree, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{BlockFont, LayoutTree, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

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

/// Put `text` into the node called `name` and carry it into the document,
/// answering whether the verb ran.
fn put(document: &mut Document, name: &str, text: &str) -> bool {
    let (boxes, layout) = drawn(document);
    let decided = perform(
        &AgentTree::new(document, &boxes, &layout),
        &Target::Named(name.to_owned()),
        &Verb::PutText(text.to_owned()),
    );
    match decided {
        Ok(outcome) => {
            apply(document, &boxes, &outcome);
            true
        }
        Err(_) => false,
    }
}

#[test]
fn a_labelled_field_keeps_its_label_and_carries_its_hint() {
    let document = parse_document(
        "<label for=e>Email</label><input id=e type=email placeholder='you@company.eu'>",
    );
    let outline = read(&document);
    assert!(
        outline
            .iter()
            .any(|line| line == "textbox \"Email\" [placeholder=\"you@company.eu\"]"),
        "{outline:#?}"
    );
    assert!(
        !outline.iter().any(|line| line.starts_with("text ")),
        "the hint is not text in the field: {outline:#?}"
    );
}

#[test]
fn a_hint_names_a_field_only_when_nothing_else_does() {
    let document = parse_document(
        "<input placeholder=Search>\
         <input title=Find placeholder=Search>\
         <input aria-label=Query placeholder=Search>\
         <input type=password placeholder=Secret>",
    );
    let outline = read(&document);
    for expected in [
        "textbox \"Search\" [placeholder=\"Search\"]",
        "textbox \"Find\" [placeholder=\"Search\"]",
        "textbox \"Query\" [placeholder=\"Search\"]",
        "generic \"Secret\" [placeholder=\"Secret\"]",
    ] {
        assert!(
            outline.iter().any(|line| line == expected),
            "{expected} in {outline:#?}"
        );
    }
    // A filled field has no hint to give, and is still called by it.
    let filled = parse_document("<input placeholder=Search value=alo>");
    let outline = read(&filled);
    assert!(
        outline.iter().any(|line| line == "textbox \"Search\""),
        "{outline:#?}"
    );
    assert!(
        outline.iter().any(|line| line == "text \"alo\""),
        "{outline:#?}"
    );
}

#[test]
fn an_authors_aria_placeholder_is_read_for_their_own_widget() {
    let document = parse_document(
        "<div role=textbox aria-label=Notes aria-placeholder='Write here'></div>\
         <input aria-label=Both placeholder=Own aria-placeholder=Aria>\
         <input aria-label=Held value=x aria-placeholder=Aria>",
    );
    let outline = read(&document);
    for expected in [
        "textbox \"Notes\" [placeholder=\"Write here\"]",
        "textbox \"Both\" [placeholder=\"Own\"]",
        "textbox \"Held\"",
    ] {
        assert!(
            outline.iter().any(|line| line == expected),
            "{expected} in {outline:#?}"
        );
    }
}

#[test]
fn a_value_hides_the_hint_and_emptying_the_field_brings_it_back() {
    let mut document = parse_document(
        "<label for=e>Email</label><input id=e placeholder='you@company.eu'>\
         <label for=n>Notes</label><textarea id=n placeholder='one\ntwo'></textarea>",
    );
    assert!(put(&mut document, "Email", "someone@alo.build"));
    assert!(put(&mut document, "Notes", "written"));
    let outline = read(&document);
    assert!(
        outline.iter().any(|line| line == "textbox \"Email\""),
        "{outline:#?}"
    );
    assert!(
        outline
            .iter()
            .any(|line| line == "text \"someone@alo.build\""),
        "{outline:#?}"
    );
    assert!(
        !outline.iter().any(|line| line.contains("placeholder=")),
        "{outline:#?}"
    );

    assert!(put(&mut document, "Email", ""));
    let outline = read(&document);
    assert!(
        outline
            .iter()
            .any(|line| line == "textbox \"Email\" [placeholder=\"you@company.eu\"]"),
        "{outline:#?}"
    );
}

#[test]
fn a_hostile_hint_is_read_or_left_out_without_a_panic() {
    let long = "x".repeat(100_000);
    let document = parse_document(&format!("<input aria-label=Long placeholder='{long}'>"));
    let outline = read(&document);
    assert!(
        outline.iter().any(
            |line| line.starts_with("textbox \"Long\" [placeholder=\"xxx") && line.len() > 100_000
        ),
        "the whole hint is read"
    );

    // Nothing but line breaks is stripped to nothing: no hint, no name.
    let breaks = "\n".repeat(50_000);
    let document = parse_document(&format!("<input placeholder='{breaks}'>"));
    let outline = read(&document);
    assert!(outline.iter().any(|line| line == "textbox"), "{outline:#?}");

    // Control characters are read as written, quoted.
    let document = parse_document("<input aria-label=C placeholder='a\u{7}b\tc'>");
    let outline = read(&document);
    assert!(
        outline
            .iter()
            .any(|line| line == "textbox \"C\" [placeholder=\"a\\u{7}b\\tc\"]"),
        "{outline:#?}"
    );
}
