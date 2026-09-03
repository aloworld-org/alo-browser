/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Malformed input still produces a usable tree.
//!
//! This is not tolerance for the legacy web — law 1 refuses that, and none of
//! these inputs needs quirks mode. It is that an HTML parser has a defined
//! repair for every input, and a renderer that returned an error instead of a
//! tree would be unusable the first time someone left a tag open. What we owe
//! in exchange is a record of what had to be repaired, which is
//! `Document::issues`.

use alo_dom::{Document, NodeKind, QualifiedName, parse_document, parse_fragment};

/// Every node the tree can be walked to, in document order, described.
fn shape(document: &Document) -> Vec<String> {
    document
        .descendants(document.root())
        .map(|id| match document.kind(id) {
            Some(NodeKind::Element(element)) => format!("<{}>", element.name),
            Some(NodeKind::Text(text)) => format!("{text:?}"),
            Some(NodeKind::Comment(_)) => "<!---->".to_owned(),
            Some(NodeKind::Doctype { name, .. }) => format!("<!DOCTYPE {name}>"),
            Some(NodeKind::Fragment) => "#fragment".to_owned(),
            Some(NodeKind::ProcessingInstruction { .. }) => "<?pi>".to_owned(),
            Some(NodeKind::Document) | None => "#unreachable".to_owned(),
        })
        .collect()
}

/// Walk every link in both directions and assert they agree. A tree whose
/// links disagree is a tree that traverses differently depending on which way
/// you came, and every later stage walks it both ways.
fn assert_links_are_consistent(document: &Document) {
    let mut seen = vec![document.root()];
    seen.extend(document.descendants(document.root()));

    for id in seen {
        let mut previous = None;
        for child in document.children(id) {
            assert_eq!(
                document.parent(child),
                Some(id),
                "{child} has a wrong parent"
            );
            assert_eq!(
                document.previous_sibling(child),
                previous,
                "{child} disagrees with the sibling before it",
            );
            if let Some(previous) = previous {
                assert_eq!(document.next_sibling(previous), Some(child));
            }
            previous = Some(child);
        }
        assert_eq!(document.first_child(id).is_some(), previous.is_some());
        assert_eq!(document.last_child(id), previous, "{id} has a wrong tail");
    }
}

#[test]
fn an_unclosed_element_is_closed_for_us() {
    let document = parse_document("<div><span>unclosed");
    assert_eq!(
        document.serialize_children(document.root()),
        "<html><head></head><body><div><span>unclosed</span></div></body></html>",
    );
    assert_links_are_consistent(&document);
}

#[test]
fn a_stray_end_tag_is_dropped_and_reported() {
    let document = parse_document("</p>stray");
    assert_eq!(document.text_content(document.root()), "stray");
    assert!(
        !document.issues().is_empty(),
        "the parser had something to say about it",
    );
    assert_links_are_consistent(&document);
}

#[test]
fn misnested_formatting_is_reparented_into_a_tree_that_holds_together() {
    // The adoption agency: `<b>` is reopened inside the second paragraph. This
    // is the input that drives the reparenting path hardest, which is why it is
    // here rather than a tidier one.
    let document = parse_document("<p>1<b>2<i>3</b>4</i>5</p>");
    assert_links_are_consistent(&document);
    assert_eq!(document.text_content(document.root()), "12345");

    let body = document
        .descendants(document.root())
        .find(|id| {
            document
                .get(*id)
                .is_some_and(|node| node.is_html_element("body"))
        })
        .expect("every document gets a body");
    assert_eq!(
        document.serialize_children(body),
        "<p>1<b>2<i>3</i></b><i>4</i>5</p>"
    );
}

#[test]
fn text_before_the_document_element_still_lands_in_the_body() {
    let document = parse_document("loose text<p>then a paragraph</p>");
    assert_eq!(
        document.text_content(document.root()),
        "loose textthen a paragraph",
    );
    assert_links_are_consistent(&document);
}

#[test]
fn a_document_with_no_markup_at_all_is_still_a_document() {
    let document = parse_document("");
    assert_eq!(
        document.serialize_children(document.root()),
        "<html><head></head><body></body></html>",
    );
    assert_eq!(document.text_content(document.root()), "");
    assert_links_are_consistent(&document);
}

#[test]
fn a_fragment_of_nonsense_is_still_a_fragment() {
    let document = parse_fragment("</div><b>text", &QualifiedName::html("div"));
    assert_eq!(document.serialize_children(document.root()), "<b>text</b>");
    assert_links_are_consistent(&document);
}

#[test]
fn a_fragment_in_a_context_that_rejects_it_drops_the_element_not_the_text() {
    // A `<td>` cannot open inside a `<ul>`; the parser refuses the element and
    // keeps the text, and says so.
    let document = parse_fragment("<td>cell", &QualifiedName::html("ul"));
    assert_eq!(document.serialize_children(document.root()), "cell");
    assert!(!document.issues().is_empty());
    assert_links_are_consistent(&document);
}

#[test]
fn a_missing_doctype_is_recorded_as_quirks_and_changes_nothing() {
    let quirky = parse_document("<p>no doctype here</p>");
    assert_eq!(quirky.quirks_signal(), alo_dom::QuirksSignal::Quirks);

    let standards = parse_document("<!DOCTYPE html><p>no doctype here</p>");
    assert_eq!(standards.quirks_signal(), alo_dom::QuirksSignal::NoQuirks);

    // Law 1: quirks mode is not implemented, so apart from the doctype node
    // itself the two trees are the same tree. The signal is a note for a
    // diagnostic, never a switch.
    let without_doctype = |document: &Document| {
        shape(document)
            .into_iter()
            .filter(|entry| !entry.starts_with("<!DOCTYPE"))
            .collect::<Vec<_>>()
    };
    assert_eq!(without_doctype(&quirky), without_doctype(&standards));
}

#[test]
fn every_issue_says_where_it_was() {
    let document = parse_document("<p>one\n<p>two\n</b>\n");
    assert!(!document.issues().is_empty());
    for issue in document.issues() {
        assert!(issue.line >= 1, "{issue} has no line");
        assert!(!issue.message.is_empty(), "an issue with nothing to say");
        assert!(issue.to_string().starts_with("line "));
    }
}

#[test]
fn deep_nesting_does_not_lose_the_tree() {
    let depth = 200;
    let input: String = "<div>".repeat(depth) + "deep" + &"</div>".repeat(depth);
    let document = parse_document(&input);
    assert_eq!(document.text_content(document.root()), "deep");
    assert_links_are_consistent(&document);

    let divs = document
        .descendants(document.root())
        .filter(|id| {
            document
                .get(*id)
                .is_some_and(|node| node.is_html_element("div"))
        })
        .count();
    assert_eq!(divs, depth);
}

#[test]
fn a_node_the_parser_discarded_stays_in_the_arena_holding_its_id() {
    // The fragment scaffolding is built and then taken out of the tree. Its
    // slot is not freed and its id is not handed to anything else, which is
    // what ADR 0003 promises: a stale id names a node that is gone rather than
    // a different node that is not.
    let document = parse_fragment("<b>x</b>", &QualifiedName::html("div"));
    let reachable = 1 + document.descendants(document.root()).count();
    assert!(
        document.node_count() > reachable,
        "{} nodes were created and {reachable} are reachable, so nothing was detached",
        document.node_count(),
    );
    assert_links_are_consistent(&document);
}
