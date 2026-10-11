/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ A `<details>`, as an agent reads and opens it (ADR 0048 §§ 3–4, queue
//! item 429).
//!
//! A closed `details` hides its content from a person, so an agent reads its
//! summary, and on the summary that it is closed — and nothing of what it
//! hides, which would let it answer from text its person has never seen.
//! `Activate` on the summary opens it, as a person's click does, on a page
//! that never ran script as on one that has; that is HTML's activation
//! behaviour, in `alo-dom`, not a promise a script keeps.

use alo_agent::{AgentTree, Change, Target, Verb, apply, perform};
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

/// Activate the node called `name` and carry it into the document,
/// answering what changed, or [`None`] when the verb was refused.
fn activate(document: &mut Document, name: &str) -> Option<Vec<Change>> {
    let (boxes, layout) = drawn(document);
    let outcome = perform(
        &AgentTree::new(document, &boxes, &layout),
        &Target::Named(name.to_owned()),
        &Verb::Activate,
    )
    .ok()?;
    Some(apply(document, &boxes, &outcome))
}

/// The `details` element of a document holding one.
fn details(document: &Document) -> Option<alo_dom::NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("details"))
    })
}

const PAGE: &str = "<details><summary>How fresh is the coffee?</summary>\
                    <p>It ships the day it is roasted.</p></details>";

#[test]
fn a_closed_details_is_read_as_its_summary_which_says_it_is_closed() {
    let document = parse_document(PAGE);
    assert_eq!(
        read(&document),
        [
            "document",
            "group",
            "summary \"How fresh is the coffee?\" [expanded=false]",
        ],
        "nothing it hides is read"
    );
}

#[test]
fn activating_the_summary_opens_it_and_activating_it_again_closes_it() {
    let mut document = parse_document(PAGE);
    let Some(changes) = activate(&mut document, "How fresh is the coffee?") else {
        panic!("a summary is operable");
    };
    let Some(details) = details(&document) else {
        panic!("the page has a details");
    };
    assert_eq!(
        changes,
        [Change::Set {
            node: details,
            attribute: "open".to_owned(),
            value: String::new(),
        }],
        "the change is the details', not the summary's"
    );
    assert_eq!(
        read(&document),
        [
            "document",
            "group",
            "summary \"How fresh is the coffee?\" [expanded=true]",
            "paragraph",
            "text \"It ships the day it is roasted.\"",
        ],
    );

    let Some(changes) = activate(&mut document, "How fresh is the coffee?") else {
        panic!("a summary is operable");
    };
    assert_eq!(
        changes,
        [Change::Removed {
            node: details,
            attribute: "open".to_owned(),
        }]
    );
    assert_eq!(read(&document).len(), 3, "closed again");
}

#[test]
fn a_second_summary_opens_nothing() {
    let mut document = parse_document(
        "<details open><summary>One</summary><summary aria-label=Two>Two</summary></details>",
    );
    assert!(
        read(&document).iter().any(|line| line == "generic \"Two\""),
        "it is generic, not a summary: {:#?}",
        read(&document)
    );
    assert_eq!(activate(&mut document, "Two"), None, "and is not operated");
    assert!(
        details(&document).is_some_and(|details| alo_dom::details::is_open(&document, details)),
        "the details is still open"
    );
}
