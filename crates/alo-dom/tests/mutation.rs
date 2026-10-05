/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The DOM standard's tree operations, rule by rule (queue item 245,
//! ADR 0017 § 5).
//!
//! Every validity rule the standard lists for inserting and replacing is a
//! test here that refuses with the standard's name **and leaves the tree
//! exactly as it was** — every link of every node, the serialisation and the
//! change count. Then the operations that succeed, the count, and what a
//! script can throw at them: every id a script could hold, and some it
//! could not, in every position.

use alo_dom::{Document, NodeId, NodeKind, Refusal, parse_document};

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

type Links = (
    Option<NodeKind>,
    Option<NodeId>,
    Option<NodeId>,
    Option<NodeId>,
    Option<NodeId>,
    Option<NodeId>,
);

/// Everything a refusal must not have touched.
fn snapshot(document: &Document) -> (String, u64, usize, Vec<Links>) {
    let links = every_id(document)
        .into_iter()
        .map(|id| {
            (
                document.kind(id).cloned(),
                document.parent(id),
                document.first_child(id),
                document.last_child(id),
                document.previous_sibling(id),
                document.next_sibling(id),
            )
        })
        .collect();
    (
        document.serialize_node(document.root()),
        document.change_count(),
        document.node_count(),
        links,
    )
}

#[track_caller]
fn refuses(
    document: &mut Document,
    name: &str,
    change: impl FnOnce(&mut Document) -> Result<NodeId, Refusal>,
) {
    let before = snapshot(document);
    let result = change(document);
    assert!(result.is_err(), "the change is refused, not {result:?}");
    if let Err(refusal) = result {
        assert_eq!(refusal.name(), name, "{refusal}");
    }
    assert_eq!(snapshot(document), before, "a refusal changes nothing");
}

fn first_html(document: &Document, local: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|id| document.get(*id).is_some_and(|n| n.is_html_element(local)))
}

fn child_of_kind(
    document: &Document,
    parent: NodeId,
    wanted: fn(&NodeKind) -> bool,
) -> Option<NodeId> {
    document
        .children(parent)
        .find(|id| document.kind(*id).is_some_and(wanted))
}

fn is_doctype(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Doctype { .. })
}

fn is_comment(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Comment(_))
}

/// A page: a comment, a doctype and an `<html>` under the document, and a
/// body with a paragraph of text in it.
fn page() -> Document {
    parse_document("<!--c--><!DOCTYPE html><html><head></head><body><p>text</p></body></html>")
}

/// A fragment — a template's contents — holding what `fill` puts in it.
fn fragment(document: &mut Document, fill: &[&str]) -> Option<NodeId> {
    let template = document.create_element("template").ok()?;
    let contents = document.element(template)?.template_contents?;
    for what in fill {
        let node = if let Some(text) = what.strip_prefix('"') {
            document.create_text_node(text)
        } else {
            document.create_element(what).ok()?
        };
        document.append_child(contents, node).ok()?;
    }
    Some(contents)
}

// --- the rules both operations share ----------------------------------------

#[test]
fn only_a_document_a_fragment_or_an_element_holds_children() {
    let mut document = page();
    let root = document.root();
    let p = first_html(&document, "p").unwrap();
    let text = document.first_child(p).unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let node = document.create_element("b").unwrap();

    for parent in [text, doctype, comment] {
        refuses(&mut document, "HierarchyRequestError", |d| {
            d.append_child(parent, node)
        });
    }
    let replaced_in_text = document.create_element("i").unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(text, replaced_in_text, p)
    });
}

#[test]
fn a_node_cannot_go_inside_itself_or_its_descendants() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(p, p)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(p, body)
    });
    let text = document.first_child(p).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(p, body, text)
    });
}

#[test]
fn a_template_cannot_go_inside_its_own_contents() {
    let mut document = page();
    let template = document.create_element("template").unwrap();
    let contents = document
        .element(template)
        .and_then(|e| e.template_contents)
        .unwrap();
    let inside = document.create_element("div").unwrap();
    document.append_child(contents, inside).unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(contents, template)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(inside, template)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(contents, template, inside)
    });
}

#[test]
fn the_reference_child_must_be_a_child_of_the_parent() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let head = first_html(&document, "head").unwrap();
    let node = document.create_element("b").unwrap();
    let detached = document.create_element("i").unwrap();

    refuses(&mut document, "NotFoundError", |d| {
        d.insert_before(body, node, Some(head))
    });
    refuses(&mut document, "NotFoundError", |d| {
        d.insert_before(body, node, Some(detached))
    });
    refuses(&mut document, "NotFoundError", |d| {
        d.replace_child(body, node, head)
    });
    refuses(&mut document, "NotFoundError", |d| {
        d.remove_child(body, head)
    });
    refuses(&mut document, "NotFoundError", |d| {
        d.remove_child(body, detached)
    });
}

#[test]
fn a_document_goes_nowhere() {
    let mut document = page();
    let root = document.root();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(body, root)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(body, root, p)
    });
    // Under a detached element the document is no ancestor, so only this
    // rule stands in the way.
    let loose = document.create_element("div").unwrap();
    let inside = document.create_element("span").unwrap();
    document.append_child(loose, inside).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(loose, root)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(loose, root, inside)
    });
}

#[test]
fn text_never_under_a_document_and_a_doctype_only_under_one() {
    let mut document = page();
    let root = document.root();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let text = document.create_text_node("loose");
    let contents = fragment(&mut document, &[]).unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, text)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, text, comment)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(body, doctype)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(contents, doctype)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(body, doctype, p)
    });
}

// --- a document's own children, when inserting ------------------------------

#[test]
fn a_fragment_into_a_document_holds_one_element_and_no_text() {
    let mut document = page();
    let root = document.root();
    let html = first_html(&document, "html").unwrap();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let two = fragment(&mut document, &["a", "b"]).unwrap();
    let with_text = fragment(&mut document, &["\"words"]).unwrap();
    let one = fragment(&mut document, &["main"]).unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, two)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, with_text)
    });
    // One element, but the document has one already.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, one)
    });

    // Without its element, one element is allowed — but not before the
    // doctype, nor where a doctype follows.
    document.remove_child(root, html).unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, one, Some(doctype))
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, one, Some(comment))
    });
    document.append_child(root, one).unwrap();
    assert!(document.children(one).next().is_none(), "left empty");
}

#[test]
fn a_document_holds_one_element_after_its_doctype() {
    let mut document = page();
    let root = document.root();
    let html = first_html(&document, "html").unwrap();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let second = document.create_element("html").unwrap();

    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, second)
    });
    document.remove_child(root, html).unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, second, Some(doctype))
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, second, Some(comment))
    });
    document.append_child(root, second).unwrap();
}

#[test]
fn a_document_holds_one_doctype_before_its_element() {
    let mut document = page();
    let root = document.root();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();

    // A document can be handed only the doctype it already has — the parser
    // makes one at most — and while it holds it, it holds a doctype, so even
    // moving that one is refused, as the standard's rule reads.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, doctype, Some(comment))
    });
    // Taken out, it cannot be appended after the element.
    document.remove_child(root, doctype).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.append_child(root, doctype)
    });
    // Before the comment, ahead of the element, it may go back.
    assert_eq!(
        document.insert_before(root, doctype, Some(comment)),
        Ok(doctype)
    );
    assert_eq!(document.next_sibling(doctype), Some(comment));
}

#[test]
fn a_doctype_cannot_follow_the_element_when_inserted_before_a_later_child() {
    let mut document =
        parse_document("<!DOCTYPE html><html><head></head><body></body></html><!--after-->");
    let root = document.root();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    let after = child_of_kind(&document, root, is_comment).unwrap();
    document.remove_child(root, doctype).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.insert_before(root, doctype, Some(after))
    });
}

// --- a document's own children, when replacing ------------------------------

#[test]
fn replacing_in_a_document_keeps_one_element_after_one_doctype() {
    let mut document = page();
    let root = document.root();
    let html = first_html(&document, "html").unwrap();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    let element = document.create_element("html").unwrap();
    let two = fragment(&mut document, &["a", "b"]).unwrap();
    let with_text = fragment(&mut document, &["\"words"]).unwrap();
    let one = fragment(&mut document, &["main"]).unwrap();

    // Fragments: too many elements, text, or an element other than the one
    // replaced, or with a doctype after the one replaced.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, two, html)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, with_text, html)
    });
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, one, comment)
    });
    // An element: the same two.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, element, comment)
    });
    // A doctype: a doctype other than the child is there.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, doctype, comment)
    });

    // What is allowed: the element for an element, a one-element fragment
    // for the element, the doctype for itself.
    assert_eq!(document.replace_child(root, element, html), Ok(html));
    assert_eq!(document.replace_child(root, one, element), Ok(element));
    assert_eq!(document.replace_child(root, doctype, doctype), Ok(doctype));
    assert!(
        document
            .get(first_html(&document, "main").unwrap())
            .is_some()
    );
}

#[test]
fn replacing_with_a_doctype_or_element_refuses_a_doctype_after() {
    let mut document = parse_document("<!--c--><!DOCTYPE html><html></html>");
    let root = document.root();
    let comment = child_of_kind(&document, root, is_comment).unwrap();
    let html = first_html(&document, "html").unwrap();
    let element = document.create_element("html").unwrap();
    document.remove_child(root, html).unwrap();
    // The comment comes before the doctype: an element in its place would
    // stand before the doctype.
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, element, comment)
    });
    let one = fragment(&mut document, &["main"]).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, one, comment)
    });

    // And a doctype replacing something after the element.
    let mut document = parse_document("<!DOCTYPE html><html></html><!--after-->");
    let root = document.root();
    let doctype = child_of_kind(&document, root, is_doctype).unwrap();
    let after = child_of_kind(&document, root, is_comment).unwrap();
    document.remove_child(root, doctype).unwrap();
    refuses(&mut document, "HierarchyRequestError", |d| {
        d.replace_child(root, doctype, after)
    });
}

// --- what succeeds ------------------------------------------------------------

#[test]
fn inserting_moves_a_node_and_answers_it() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let head = first_html(&document, "head").unwrap();
    let p = first_html(&document, "p").unwrap();
    let b = document.create_element("b").unwrap();

    assert_eq!(document.append_child(body, b), Ok(b));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [p, b]);
    assert_eq!(document.insert_before(body, b, Some(p)), Ok(b));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [b, p]);
    // Before itself is where it already is.
    assert_eq!(document.insert_before(body, b, Some(b)), Ok(b));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [b, p]);
    // From one parent to another.
    assert_eq!(document.append_child(head, p), Ok(p));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [b]);
    assert_eq!(document.parent(p), Some(head));
}

#[test]
fn a_fragment_gives_up_its_children_in_order() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();
    let contents = fragment(&mut document, &["a", "\"t", "b"]).unwrap();
    let moving: Vec<_> = document.children(contents).collect();

    assert_eq!(
        document.insert_before(body, contents, Some(p)),
        Ok(contents)
    );
    let mut expected = moving.clone();
    expected.push(p);
    assert_eq!(document.children(body).collect::<Vec<_>>(), expected);
    assert!(document.children(contents).next().is_none());
    assert_eq!(document.change_count(), 4, "three fills and one insertion");
}

#[test]
fn replacing_answers_the_replaced_node_detached() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();
    let a = document.create_element("a").unwrap();
    let b = document.create_element("b").unwrap();
    document.append_child(body, a).unwrap();
    document.append_child(body, b).unwrap();

    // By its own next sibling.
    assert_eq!(document.replace_child(body, a, p), Ok(p));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [a, b]);
    assert_eq!(document.parent(p), None);
    assert_eq!(document.text_content(p), "text", "it keeps its children");
    // By itself.
    assert_eq!(document.replace_child(body, a, a), Ok(a));
    assert_eq!(document.children(body).collect::<Vec<_>>(), [a, b]);
}

#[test]
fn removing_answers_what_was_removed_and_a_loose_node_is_left_alone() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();

    assert_eq!(document.remove_child(body, p), Ok(p));
    assert!(!document.is_attached(p));
    let count = document.change_count();
    assert!(!document.remove(p), "nothing holds it");
    assert!(!document.remove(document.root()), "nor the document");
    assert_eq!(document.change_count(), count, "and that is not a change");
    document.append_child(body, p).unwrap();
    assert!(document.remove(p));
}

#[test]
fn an_element_name_that_cannot_be_one_is_refused_and_makes_nothing() {
    let mut document = page();
    let before = document.node_count();
    for bad in ["", "two words", "a>b", "1st", "\0"] {
        let refusal = document.create_element(bad).unwrap_err();
        assert_eq!(refusal.name(), "InvalidCharacterError", "{bad:?}");
    }
    assert_eq!(document.node_count(), before);
    let long = "x".repeat(1 << 20);
    assert!(
        document.create_element(&long).is_ok(),
        "long is not invalid"
    );
}

// --- the change count ---------------------------------------------------------

#[test]
fn the_count_moves_on_every_change_and_never_on_a_refusal() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();
    assert_eq!(document.change_count(), 0, "parsing is not a change");

    let b = document.create_element("b").unwrap();
    assert_eq!(document.change_count(), 0, "neither is making a node");
    let mut expected = 0;
    let mut step = |document: &Document, moved: bool| {
        if moved {
            expected += 1;
        }
        assert_eq!(document.change_count(), expected);
    };

    let ok = document.append_child(body, b).is_ok();
    step(&document, ok);
    let ok = document.append_child(b, body).is_ok();
    step(&document, ok);
    let ok = document.insert_before(body, b, Some(p)).is_ok();
    step(&document, ok);
    let ok = document.replace_child(body, b, b).is_ok();
    step(&document, ok);
    let ok = document.remove_child(b, p).is_ok();
    step(&document, ok);
    let ok = document.remove_child(body, b).is_ok();
    step(&document, ok);
    let ok = document.set_attribute(p, "class", "x").is_some();
    step(&document, ok);
    let ok = document.remove_attribute(p, "class") == Some(true);
    step(&document, ok);
    let ok = document.remove_attribute(p, "class") == Some(true);
    step(&document, ok);
    let ok = document.remove(b);
    step(&document, ok);
    assert_eq!(expected, 6);
}

// --- releasing ----------------------------------------------------------------

#[test]
fn a_released_tree_answers_nothing_and_the_next_id_is_still_new() {
    let mut document = page();
    let body = first_html(&document, "body").unwrap();
    let p = first_html(&document, "p").unwrap();
    let text = document.first_child(p).unwrap();
    document.remove_child(body, p).unwrap();
    let highest = document.node_count();

    assert_eq!(document.release(p), Some(2));
    assert!(document.get(p).is_none());
    assert!(document.get(text).is_none());
    assert!(!document.serialize_node(document.root()).contains("text"));
    let next = document.create_element("p").unwrap();
    assert_eq!(next.as_usize(), highest, "one past the highest ever made");

    // A released id refuses as a node that is not there.
    refuses(&mut document, "NotFoundError", |d| d.append_child(body, p));
    refuses(&mut document, "NotFoundError", |d| d.append_child(p, next));
    refuses(&mut document, "NotFoundError", |d| d.remove_child(body, p));
    assert!(!document.remove(p));
}

// --- what a script can throw at it -------------------------------------------

/// The tree is still a tree: every walk ends, every link has its partner,
/// and the document holds what the standard lets it hold.
fn assert_a_tree(document: &Document, ids: &[NodeId]) {
    let bound = ids.len();
    for &id in ids {
        if document.get(id).is_none() {
            continue;
        }
        let mut current = id;
        let mut steps = 0;
        while let Some(parent) = document.parent(current) {
            current = parent;
            steps += 1;
            assert!(steps <= bound, "{id}'s ancestors go round");
        }
        let children: Vec<_> = document.children(id).take(bound + 1).collect();
        assert!(children.len() <= bound, "{id}'s children go round");
        assert_eq!(document.first_child(id), children.first().copied());
        assert_eq!(document.last_child(id), children.last().copied());
        for child in &children {
            assert_eq!(document.parent(*child), Some(id));
        }
        for pair in children.windows(2) {
            if let [before, after] = *pair {
                assert_eq!(document.previous_sibling(after), Some(before));
                assert_eq!(document.next_sibling(before), Some(after));
            }
        }
    }
    let root = document.root();
    let kinds: Vec<_> = document
        .children(root)
        .filter_map(|id| document.kind(id))
        .collect();
    let elements = kinds
        .iter()
        .filter(|k| matches!(k, NodeKind::Element(_)))
        .count();
    let doctypes = kinds.iter().filter(|k| is_doctype(k)).count();
    assert!(elements <= 1 && doctypes <= 1, "{kinds:?}");
    assert!(!kinds.iter().any(|k| matches!(k, NodeKind::Text(_))));
    if let (Some(d), Some(e)) = (
        kinds.iter().position(|k| is_doctype(k)),
        kinds.iter().position(|k| matches!(k, NodeKind::Element(_))),
    ) {
        assert!(d < e, "the doctype comes first");
    }
}

#[test]
fn every_id_in_every_position_refuses_or_keeps_a_tree() {
    let mut start = parse_document(
        "<!--c--><!DOCTYPE html><html><body><p>a<b>b</b></p><template><i>t</i></template></body></html>",
    );
    let loose = start.create_element("div").unwrap();
    let inside = start.create_text_node("loose");
    start.append_child(loose, inside).unwrap();
    let gone = start.create_element("span").unwrap();
    start.release(gone).unwrap();
    let ids = every_id(&start);
    let mut moved = 0_usize;

    for &a in &ids {
        for &b in &ids {
            for c in ids.iter().copied().map(Some).chain([None]) {
                let mut document = start.clone();
                let before = document.change_count();
                let insert = document.insert_before(a, b, c).is_ok();
                assert_eq!(document.change_count() - before, u64::from(insert));
                moved += usize::from(insert);
                assert_a_tree(&document, &ids);

                if let Some(c) = c {
                    let mut document = start.clone();
                    let replace = document.replace_child(a, b, c).is_ok();
                    assert_eq!(document.change_count(), u64::from(replace) + before);
                    moved += usize::from(replace);
                    assert_a_tree(&document, &ids);
                }
            }
            let mut document = start.clone();
            let _ = document.remove_child(a, b);
            document.remove(a);
            let _ = document.release(b);
            assert_a_tree(&document, &ids);
        }
    }
    assert!(moved > 0, "some of them were allowed");
}

#[test]
fn text_content_replaces_every_child_with_one_text_node_as_one_change() {
    let mut document = parse_document("<p>a<b>b</b><!--c-->d</p><template>t</template>");
    let element = |document: &Document, name: &str| {
        document
            .descendants(document.root())
            .find(|id| document.element(*id).is_some_and(|e| e.name.is_html(name)))
            .unwrap()
    };
    let p = element(&document, "p");
    let b = element(&document, "b");
    let before = document.node_count();

    let made = document.replace_all_with_text(p, "new").unwrap().unwrap();
    assert_eq!(made.as_usize(), before, "the text node takes the next id");
    assert_eq!(document.children(p).collect::<Vec<_>>(), [made]);
    assert_eq!(document.text_content(p), "new");
    assert_eq!(
        document.change_count(),
        1,
        "four children went in one change"
    );
    assert_eq!(document.parent(b), None, "a child taken away is detached");
    assert_eq!(document.text_content(b), "b", "and keeps its own children");

    assert_eq!(document.replace_all_with_text(p, ""), Some(None));
    assert_eq!(document.first_child(p), None, "empty text leaves no node");
    assert_eq!(document.change_count(), 2);
    assert_eq!(document.replace_all_with_text(p, ""), Some(None));
    assert_eq!(document.change_count(), 2, "nothing taken and nothing put");

    // A template's contents are a fragment, and take text like an element.
    let contents = document
        .element(element(&document, "template"))
        .and_then(|template| template.template_contents)
        .unwrap();
    assert!(document.replace_all_with_text(contents, "u").is_some());
    assert_eq!(document.text_content(contents), "u");

    // Neither the document nor character data is replaced this way.
    let root = document.root();
    let count = document.change_count();
    assert_eq!(document.replace_all_with_text(root, "x"), None);
    assert_eq!(document.replace_all_with_text(made, "x"), None);
    assert_eq!(document.change_count(), count);
}

#[test]
fn character_data_is_replaced_whole_and_nothing_else_is() {
    let mut document = parse_document("<p>a<!--c--></p>");
    let p = document
        .descendants(document.root())
        .find(|id| document.element(*id).is_some_and(|e| e.name.is_html("p")))
        .unwrap();
    let text = document.first_child(p).unwrap();
    let comment = document.last_child(p).unwrap();

    assert_eq!(document.set_data(text, "b"), Some(()));
    assert_eq!(document.kind(text), Some(&NodeKind::Text("b".to_owned())));
    assert_eq!(document.set_data(comment, "d"), Some(()));
    assert_eq!(
        document.kind(comment),
        Some(&NodeKind::Comment("d".to_owned()))
    );
    assert_eq!(document.set_data(text, "b"), Some(()));
    assert_eq!(
        document.change_count(),
        3,
        "the same data again is a change"
    );

    assert_eq!(document.set_data(p, "x"), None);
    assert_eq!(document.set_data(document.root(), "x"), None);
    assert_eq!(document.change_count(), 3);
}
