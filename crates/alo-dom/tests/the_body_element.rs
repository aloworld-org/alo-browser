/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 253: HTML's *the body element*, as `document.body` reads and
//! replaces it.
//!
//! The getter's three conditions — the document element is an HTML `html`,
//! the child is a `body` or a `frameset`, the first such child — each have a
//! test that would answer differently without it. Every refusal leaves the
//! tree exactly as it was, and the hostile half hands `set_body` every id a
//! caller could hold, and some it could not.

use alo_dom::{Document, NodeId, Refusal, parse_document};

/// Ids `0..=count + 2`, minted by a scratch document: every node `document`
/// has, and a few it never made.
fn every_id(document: &Document) -> Vec<NodeId> {
    let mut scratch = Document::new();
    let mut ids = vec![scratch.root()];
    for _ in 0..=document.node_count().saturating_add(1) {
        ids.push(scratch.create_text_node(""));
    }
    ids
}

/// What a refusal must not have touched: the serialisation, the change
/// count, the node count, and every node's parent.
fn snapshot(document: &Document) -> (String, u64, usize, Vec<Option<NodeId>>) {
    (
        document.serialize_node(document.root()),
        document.change_count(),
        document.node_count(),
        every_id(document)
            .into_iter()
            .map(|id| document.parent(id))
            .collect(),
    )
}

#[track_caller]
fn refuses(document: &mut Document, new: Option<NodeId>, message: &str) {
    let before = snapshot(document);
    let result = document.set_body(new);
    assert!(
        matches!(result, Err(Refusal::HierarchyRequest(said)) if said == message),
        "refused as `HierarchyRequestError: {message}`, not {result:?}"
    );
    assert_eq!(snapshot(document), before, "a refusal changes nothing");
}

fn first_html(document: &Document, local: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|id| document.get(*id).is_some_and(|n| n.is_html_element(local)))
}

fn page() -> Document {
    parse_document("<!DOCTYPE html><html><head></head><body><p>text</p></body></html>")
}

const NOT_A_BODY: &str = "the body must be a body or a frameset element";

// --- the getter ---------------------------------------------------------------

#[test]
fn a_parsed_page_has_its_body_and_an_empty_document_has_none() {
    let document = page();
    assert_eq!(document.body(), first_html(&document, "body"));
    assert!(document.body().is_some());
    assert_eq!(Document::new().body(), None);
}

#[test]
fn a_page_still_in_its_head_has_no_body_yet() {
    let (mut parsing, mut document) = alo_dom::Parsing::start(
        "<!DOCTYPE html><html><head><script>x</script></head><body><p>later</p></body></html>",
    );
    let reached = parsing.resume(&mut document);
    assert!(
        matches!(reached, alo_dom::Reached::Script(_)),
        "stopped at the head's script, not {reached:?}"
    );
    assert_eq!(document.body(), None, "the parser has not made it yet");
    let _ = parsing.resume(&mut document);
    assert_eq!(document.body(), first_html(&document, "body"));
}

#[test]
fn a_frameset_is_the_body_element_though_nothing_renders_it() {
    let document = parse_document("<!DOCTYPE html><html><head></head><frameset></frameset></html>");
    assert!(
        first_html(&document, "body").is_none(),
        "the parser made none"
    );
    assert_eq!(document.body(), first_html(&document, "frameset"));
}

#[test]
fn the_first_body_or_frameset_child_of_the_html_element_and_no_deeper() {
    let mut document = page();
    let (Some(html), Some(body)) = (first_html(&document, "html"), first_html(&document, "body"))
    else {
        panic!("the page has an html and a body");
    };
    // A second body after the first is not it, and a frameset before the
    // first is.
    let second = document.create_element("body").unwrap();
    document.append_child(html, second).unwrap();
    assert_eq!(document.body(), Some(body));
    let frameset = document.create_element("frameset").unwrap();
    document.insert_before(html, frameset, Some(body)).unwrap();
    assert_eq!(document.body(), Some(frameset));
    // With every child of the html element gone, a body deeper down is not
    // the body element.
    for gone in [frameset, body, second] {
        document.remove_child(html, gone).unwrap();
    }
    let deeper = document.create_element("body").unwrap();
    let head = first_html(&document, "head").unwrap();
    document.append_child(head, deeper).unwrap();
    assert_eq!(document.body(), None, "a body inside the head is not it");
}

#[test]
fn a_document_element_that_is_not_html_has_no_body_element() {
    let mut document = page();
    let (Some(html), Some(body)) = (first_html(&document, "html"), first_html(&document, "body"))
    else {
        panic!("the page has an html and a body");
    };
    let root = document.create_element("div").unwrap();
    let parent = document.root();
    document.replace_child(parent, root, html).unwrap();
    document.append_child(root, body).unwrap();
    assert_eq!(document.parent(body), Some(root));
    assert_eq!(
        document.body(),
        None,
        "the html element is the html one only"
    );
}

// --- the setter ---------------------------------------------------------------

#[test]
fn a_new_body_replaces_the_old_one_where_it_was() {
    let mut document = page();
    let (Some(html), Some(old), Some(p)) = (
        first_html(&document, "html"),
        first_html(&document, "body"),
        first_html(&document, "p"),
    ) else {
        panic!("the page has an html, a body and a p");
    };
    let new = document.create_element("body").unwrap();
    let text = document.create_text_node("new");
    document.append_child(new, text).unwrap();
    let count = document.change_count();
    assert_eq!(document.set_body(Some(new)), Ok(()));
    assert_eq!(document.body(), Some(new));
    assert_eq!(document.parent(new), Some(html));
    assert_eq!(document.parent(old), None, "the old body is detached");
    assert_eq!(document.parent(p), Some(old), "and keeps its children");
    assert_eq!(
        document.change_count(),
        count + 1,
        "one change, counted once"
    );
    assert_eq!(
        document.serialize_node(document.root()),
        "<!DOCTYPE html><html><head></head><body>new</body></html>"
    );
}

#[test]
fn the_body_it_already_is_changes_nothing() {
    let mut document = page();
    let body = document.body();
    let before = snapshot(&document);
    assert_eq!(document.set_body(body), Ok(()));
    assert_eq!(snapshot(&document), before, "not even the change count");
}

#[test]
fn a_frameset_replaces_a_body_and_a_body_replaces_a_frameset() {
    let mut document = page();
    let frameset = document.create_element("frameset").unwrap();
    assert_eq!(document.set_body(Some(frameset)), Ok(()));
    assert_eq!(document.body(), Some(frameset));
    let body = document.create_element("body").unwrap();
    assert_eq!(document.set_body(Some(body)), Ok(()));
    assert_eq!(document.body(), Some(body));
    assert_eq!(document.parent(frameset), None);
}

#[test]
fn a_body_from_elsewhere_in_the_tree_is_moved_into_place() {
    let mut document = page();
    let (Some(old), Some(p)) = (first_html(&document, "body"), first_html(&document, "p")) else {
        panic!("the page has a body and a p");
    };
    let new = document.create_element("body").unwrap();
    document.append_child(p, new).unwrap();
    assert_eq!(document.set_body(Some(new)), Ok(()));
    assert_eq!(document.body(), Some(new));
    assert_eq!(document.parent(new), first_html(&document, "html"));
    assert!(
        document.children(p).all(|child| child != new),
        "it left the paragraph"
    );
    assert_eq!(document.parent(p), Some(old));
}

#[test]
fn with_no_body_element_the_new_one_is_appended_to_the_document_element() {
    let mut document = page();
    let (Some(html), Some(old)) = (first_html(&document, "html"), first_html(&document, "body"))
    else {
        panic!("the page has an html and a body");
    };
    document.remove_child(html, old).unwrap();
    let new = document.create_element("body").unwrap();
    assert_eq!(document.set_body(Some(new)), Ok(()));
    assert_eq!(document.last_child(html), Some(new));
    assert_eq!(document.body(), Some(new));

    // A document element that is not `html` has no body element, so the
    // new one is appended to it.
    let root = document.create_element("div").unwrap();
    let parent = document.root();
    document.replace_child(parent, root, html).unwrap();
    let appended = document.create_element("body").unwrap();
    assert_eq!(document.set_body(Some(appended)), Ok(()));
    assert_eq!(document.last_child(root), Some(appended));
    assert_eq!(
        document.body(),
        None,
        "and it is still not the body element"
    );
}

#[test]
fn what_is_not_a_body_or_a_frameset_is_refused_by_name() {
    let mut document = page();
    let div = document.create_element("div").unwrap();
    let text = document.create_text_node("body");
    let html = first_html(&document, "html");
    let root = document.root();
    refuses(&mut document, None, NOT_A_BODY);
    refuses(&mut document, Some(div), NOT_A_BODY);
    refuses(&mut document, Some(text), NOT_A_BODY);
    refuses(&mut document, html, NOT_A_BODY);
    refuses(&mut document, Some(root), NOT_A_BODY);
}

#[test]
fn with_no_document_element_a_body_has_nowhere_to_go() {
    let mut document = Document::new();
    let body = document.create_element("body").unwrap();
    refuses(
        &mut document,
        Some(body),
        "there is no document element to put a body in",
    );
}

#[test]
fn what_the_append_refuses_is_refused_and_changes_nothing() {
    // The document element is a `body` and not an `html`, so there is no
    // body element, and appending the document element to itself is the
    // cycle the standard's pre-insertion refuses.
    let mut document = Document::new();
    let body = document.create_element("body").unwrap();
    let root = document.root();
    document.append_child(root, body).unwrap();
    assert_eq!(document.body(), None);
    refuses(
        &mut document,
        Some(body),
        "a node cannot be put inside itself or anything inside it",
    );
}

// --- what a caller could throw at it -----------------------------------------

#[test]
fn every_id_in_the_setter_answers_or_refuses_and_never_panics() {
    // A loose body and frameset, a body in a template's contents, and every
    // parsed node besides.
    let mut made = page();
    made.create_element("body").unwrap();
    made.create_element("frameset").unwrap();
    let template = made.create_element("template").unwrap();
    let inner = made.create_element("body").unwrap();
    if let Some(contents) = made.element(template).and_then(|e| e.template_contents) {
        made.append_child(contents, inner).unwrap();
    }
    for id in every_id(&made) {
        let mut document = made.clone();
        let before = snapshot(&document);
        match document.set_body(Some(id)) {
            Ok(()) => assert_eq!(document.body(), Some(id), "{id} is the body now"),
            Err(_) => assert_eq!(
                snapshot(&document),
                before,
                "{id} refused, changing nothing"
            ),
        }
    }
}
