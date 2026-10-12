/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A control's form owner, worked out when it is asked for (ADR 0051 § 2).
//!
//! The `form` that the control's **`form` attribute** names by id in its own
//! tree; no owner when the attribute names anything else or nothing; and
//! without the attribute, its **nearest ancestor `form`**. Nothing remembers
//! the answer on the node, so there is nothing to keep in step as the tree
//! changes.
//!
//! HTML's reset of a form owner lands on exactly this answer in every case
//! but one: the parser's *form element pointer*, which ties a control to an
//! unclosed `<form>` that is not its ancestor in misnested markup. That is
//! the legacy tail, and law 1 refuses it.

use crate::document::Document;
use crate::node::NodeId;

/// The elements that take part in a form's submission: HTML's
/// *submittable elements*, but for `object`, which only a plugin gives a
/// value to.
const SUBMITTABLE: &[&str] = &["button", "input", "select", "textarea"];

/// Whether `node` is a submittable element.
pub fn is_submittable(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| SUBMITTABLE.iter().any(|name| element.name.is_html(name)))
}

/// The form `control` belongs to, if it belongs to one.
pub fn form_owner(document: &Document, control: NodeId) -> Option<NodeId> {
    let element = document.element(control)?;
    if let Some(id) = element.attr("form") {
        let root = root_of(document, control);
        let named = core::iter::once(root)
            .chain(document.descendants(root))
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.attr("id") == Some(id))
            })?;
        return is_form(document, named).then_some(named);
    }
    let mut at = document.parent(control);
    while let Some(node) = at {
        if is_form(document, node) {
            return Some(node);
        }
        at = document.parent(node);
    }
    None
}

/// The submittable elements `form` owns, in tree order: its own, and any in
/// its tree that name it by its id.
pub fn controls(document: &Document, form: NodeId) -> Vec<NodeId> {
    let root = root_of(document, form);
    document
        .descendants(root)
        .filter(|node| is_submittable(document, *node))
        .filter(|node| form_owner(document, *node) == Some(form))
        .collect()
}

/// Whether `node` is a `<form>`.
fn is_form(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| element.name.is_html("form"))
}

/// The root of the tree `node` is in.
pub(crate) fn root_of(document: &Document, node: NodeId) -> NodeId {
    let mut at = node;
    while let Some(parent) = document.parent(at) {
        at = parent;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn named(document: &Document, name: &str) -> Option<NodeId> {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("name") == Some(name))
        })
    }

    fn with_id(document: &Document, id: &str) -> Option<NodeId> {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some(id))
        })
    }

    #[test]
    fn an_owner_is_the_nearest_form_around_or_the_one_named() {
        let document = parse_document(
            "<form id=a><input name=inside><input name=away form=b></form>\
             <form id=b></form><input name=outside form=a>\
             <input name=nowhere form=nope><p id=p></p><input name=at-p form=p>\
             <input name=alone>",
        );
        let (a, b) = (with_id(&document, "a"), with_id(&document, "b"));
        let owner = |name| named(&document, name).and_then(|node| form_owner(&document, node));
        assert_eq!(owner("inside"), a);
        assert_eq!(owner("away"), b, "the attribute wins over the ancestor");
        assert_eq!(owner("outside"), a);
        assert_eq!(owner("nowhere"), None);
        assert_eq!(owner("at-p"), None, "an id that is not a form's");
        assert_eq!(owner("alone"), None);
    }

    #[test]
    fn a_forms_controls_are_its_own_and_those_naming_it_in_tree_order() {
        let document = parse_document(
            "<input name=before form=f><form id=f><input name=one><p>x</p>\
             <select name=two></select><input name=gone form=g><textarea name=three>\
             </textarea><button name=four>b</button></form><form id=g></form>\
             <input name=after form=f>",
        );
        let Some(form) = with_id(&document, "f") else {
            panic!("the form");
        };
        let names: Vec<&str> = controls(&document, form)
            .into_iter()
            .filter_map(|node| document.element(node).and_then(|e| e.attr("name")))
            .collect();
        assert_eq!(names, ["before", "one", "two", "three", "four", "after"]);
    }
}
