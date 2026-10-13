/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which control a `<label>` labels — HTML's *labeled control* (queue item
//! 554).
//!
//! **One rule, asked by everyone.** The agent names a field by its label
//! and leaves a label that names something out of what it reads; a hovered
//! label hovers its control ([`crate::hover`]). Both ask this, so a label
//! is never one control's name and another control's hover.
//!
//! HTML's rule, and Chrome 154's, measured over CDP (the accessibility
//! tree's names and `querySelectorAll(':hover')`), the source not read:
//!
//! - **With a `for`**, the control is the *first* element in the label's
//!   tree whose `id` is its value, if that element is labelable, and
//!   nothing otherwise. A `<div>` first and an `<input>` with the same id
//!   later is nothing; so is `for=""`, and so is a `for` naming a hidden
//!   input. A control the label wraps is not its control then: a `for` is
//!   never a fallback.
//! - **Without one**, it is the first labelable element inside the label,
//!   in tree order. A second control inside it is not labelled by it.
//! - **Labelable** is HTML's list — `button`, `input` that is not
//!   `type=hidden` (in any case), `meter`, `output`, `progress`, `select`,
//!   `textarea` — less the form-associated custom elements, which this
//!   engine does not have.

use crate::document::Document;
use crate::node::{Element, NodeId};
use crate::owner::root_of;

impl Document {
    /// The control the `<label>` `label` labels, if it labels one.
    ///
    /// [`None`] for anything that is not an HTML `<label>`.
    pub fn labelled_control(&self, label: NodeId) -> Option<NodeId> {
        let element = self.element(label)?;
        if !element.name.is_html("label") {
            return None;
        }
        if let Some(target) = element.attr("for") {
            if target.is_empty() {
                return None;
            }
            let root = root_of(self, label);
            let named = core::iter::once(root)
                .chain(self.descendants(root))
                .find(|node| {
                    self.element(*node)
                        .is_some_and(|element| element.attr("id") == Some(target))
                })?;
            return self
                .element(named)
                .is_some_and(is_labelable)
                .then_some(named);
        }
        self.descendants(label)
            .find(|node| self.element(*node).is_some_and(is_labelable))
    }
}

/// Whether `element` is one a `<label>` can label.
pub fn is_labelable(element: &Element) -> bool {
    if element.name.is_html("input") {
        return !element
            .attr("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("hidden"));
    }
    [
        "button", "meter", "output", "progress", "select", "textarea",
    ]
    .iter()
    .any(|name| element.name.is_html(name))
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::node::NodeId;

    fn find(document: &Document, wanted: &str) -> Option<NodeId> {
        document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))
    }

    /// The id of the control the label `label` labels, or `-`.
    fn control_of(document: &Document, label: &str) -> String {
        find(document, label)
            .and_then(|label| document.labelled_control(label))
            .and_then(|control| document.element(control))
            .and_then(|control| control.attr("id"))
            .unwrap_or("-")
            .to_owned()
    }

    #[test]
    fn a_for_names_the_first_element_with_its_id_when_that_is_labelable() {
        let document = crate::parse_document(
            "<label id=a for=f></label><div><input id=f></div>\
             <label id=b for=p></label><div id=p></div>\
             <label id=c for=q></label><div id=q></div><input id=q>\
             <label id=d for=h></label><input type=HIDDEN id=h>\
             <label id=e for=t></label><textarea id=t></textarea>\
             <label id=g for=nothing></label>",
        );
        assert_eq!(control_of(&document, "a"), "f");
        assert_eq!(control_of(&document, "b"), "-", "a div");
        assert_eq!(control_of(&document, "c"), "-", "the first with the id");
        assert_eq!(control_of(&document, "d"), "-", "a hidden input");
        assert_eq!(control_of(&document, "e"), "t");
        assert_eq!(control_of(&document, "g"), "-", "no such id");
    }

    #[test]
    fn a_for_is_never_a_fallback_to_the_control_wrapped() {
        let document = crate::parse_document(
            "<label id=a for=x><input id=w></label><input id=x>\
             <label id=b for=elsewhere><input id=v></label>\
             <label id=c for=\"\"><input id=u></label><input id=\"\">\
             <label id=d for=div><input id=r></label><div id=div></div>",
        );
        assert_eq!(control_of(&document, "a"), "x");
        assert_eq!(control_of(&document, "b"), "-");
        assert_eq!(control_of(&document, "c"), "-", "an empty for, an empty id");
        assert_eq!(control_of(&document, "d"), "-", "a for naming a div");
    }

    #[test]
    fn without_a_for_it_is_the_first_labelable_element_inside() {
        let document = crate::parse_document(
            "<label id=a><span>Two</span><input id=g><input id=g2></label>\
             <label id=b><input type=hidden id=h><select id=s></select></label>\
             <label id=c><meter id=m></meter></label>\
             <label id=d>Nothing</label>",
        );
        assert_eq!(control_of(&document, "a"), "g", "the first of two");
        assert_eq!(control_of(&document, "b"), "s", "past the hidden one");
        assert_eq!(control_of(&document, "c"), "m");
        assert_eq!(control_of(&document, "d"), "-");
    }

    #[test]
    fn what_is_not_a_label_labels_nothing() {
        let document = crate::parse_document("<div id=a for=f><input id=f></div>");
        assert_eq!(control_of(&document, "a"), "-");
        assert_eq!(document.labelled_control(NodeId(10_000)), None);
        assert_eq!(document.labelled_control(document.root()), None);
    }

    #[test]
    fn a_label_out_of_the_document_looks_in_its_own_tree() {
        let mut document = crate::parse_document(
            "<input id=f class=outside><div id=held><label id=a for=f></label>\
             <input id=f class=inside></div>",
        );
        let class = |document: &Document, node: Option<NodeId>| {
            node.and_then(|node| document.element(node))
                .and_then(|element| element.attr("class"))
                .map(str::to_owned)
        };
        let (Some(held), Some(a)) = (find(&document, "held"), find(&document, "a")) else {
            panic!("the page has its elements");
        };
        let before = document.labelled_control(a);
        assert_eq!(class(&document, before).as_deref(), Some("outside"));
        assert!(document.remove(held));
        let after = document.labelled_control(a);
        assert_eq!(class(&document, after).as_deref(), Some("inside"));
    }
}
