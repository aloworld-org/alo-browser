/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether an element is disabled, as HTML says (ADR 0051 § 3).
//!
//! A control is disabled if it says so, or if it is inside a disabled
//! `<fieldset>` — except inside that fieldset's **first** `<legend>`, which
//! stays usable so that a person can still read and operate the group's own
//! controls. An `<option>` is also disabled when its `<optgroup>` is.
//!
//! This was `alo-css`' rule for `:disabled` until a form needed it too: a
//! disabled control is barred from constraint validation and sends nothing
//! when its form is submitted ([`crate::constraint`],
//! [`crate::submission`]). A rule about the document with several askers
//! lives here once, for ADR 0017 § 5's reason, and `alo-css` asks it.

use crate::document::Document;
use crate::node::{Element, NodeId};

/// Elements that a `disabled` attribute means something on.
const CAN_BE_DISABLED: &[&str] = &[
    "button", "input", "select", "textarea", "optgroup", "option", "fieldset",
];

/// The form controls a disabled ancestor `<fieldset>` reaches.
const DISABLED_BY_FIELDSET: &[&str] = &["button", "input", "select", "textarea"];

fn is_one_of(element: &Element, names: &[&str]) -> bool {
    names.iter().any(|name| element.name.is_html(name))
}

/// Whether an element is one that a `disabled` attribute applies to at all.
pub fn can_be_disabled(element: &Element) -> bool {
    is_one_of(element, CAN_BE_DISABLED)
}

/// Whether an element is disabled: it says so, its `<optgroup>` says so,
/// or a `<fieldset>` around it does and it is not in that fieldset's first
/// `<legend>`.
pub fn is_disabled(document: &Document, id: NodeId, element: &Element) -> bool {
    if !can_be_disabled(element) {
        return false;
    }
    if element.attr("disabled").is_some() {
        return true;
    }
    if element.name.is_html("option") {
        return document
            .parent(id)
            .and_then(|parent| document.element(parent))
            .is_some_and(|parent| {
                parent.name.is_html("optgroup") && parent.attr("disabled").is_some()
            });
    }
    if !is_one_of(element, DISABLED_BY_FIELDSET) {
        return false;
    }
    disabling_fieldset(document, id).is_some()
}

/// Whether the node `id` is an element and disabled.
pub fn is_disabled_node(document: &Document, id: NodeId) -> bool {
    document
        .element(id)
        .is_some_and(|element| is_disabled(document, id, element))
}

/// The nearest ancestor `<fieldset>` that disables this element, if there is
/// one.
fn disabling_fieldset(document: &Document, id: NodeId) -> Option<NodeId> {
    let mut child = id;
    let mut ancestor = document.parent(id);
    while let Some(current) = ancestor {
        if let Some(element) = document.element(current)
            && element.name.is_html("fieldset")
            && element.attr("disabled").is_some()
            && !is_inside_first_legend(document, current, child)
        {
            return Some(current);
        }
        child = current;
        ancestor = document.parent(current);
    }
    None
}

/// Whether the path from a fieldset down to a descendant passes through the
/// fieldset's first `<legend>` child.
fn is_inside_first_legend(
    document: &Document,
    fieldset: NodeId,
    child_of_fieldset: NodeId,
) -> bool {
    document
        .children(fieldset)
        .find(|candidate| {
            document
                .element(*candidate)
                .is_some_and(|element| element.name.is_html("legend"))
        })
        .is_some_and(|first_legend| first_legend == child_of_fieldset)
}
