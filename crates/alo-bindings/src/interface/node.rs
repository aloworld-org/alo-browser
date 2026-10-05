/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Node`: what every node is — its place in a tree, its text, and the four
//! changes a script makes to a tree's shape (queue item 249, item 80's
//! members only).
//!
//! - `parentNode`, `firstChild`, `lastChild`, `previousSibling` and
//!   `nextSibling`, read-only. A `<template>`'s contents are not its
//!   children, and their fragment has no parent, as the standard says.
//! - `textContent`: `null` on a document or a doctype; an element's or a
//!   fragment's descendant text; character data's own. Setting it replaces
//!   an element's or a fragment's children with one text node (*string
//!   replace all*), replaces character data's data, and does nothing on a
//!   document or a doctype. `null` sets the empty string.
//! - `appendChild`, `insertBefore`, `removeChild` and `replaceChild`, each
//!   one of `alo-dom`'s operations under the standard's validity rules, a
//!   refusal thrown as the `DOMException` it names.
//!
//! **Not here** (ADR 0017 § 8): `childNodes`, a live list and stage 3's;
//! `nodeType`, `nodeName`, `ownerDocument`, `contains`, `cloneNode`,
//! `isConnected` and the rest — each absent, so `typeof` answers
//! `"undefined"`, until a page or an item needs it.

use alo_dom::{Document, NodeId, NodeKind};
use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::dom_exception;
use crate::define;
use crate::idl::{self, Brand, Converted};

/// `Node.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("parentNode", parent_node as fn(&mut Call<'_>) -> _),
        ("firstChild", first_child),
        ("lastChild", last_child),
        ("previousSibling", previous_sibling),
        ("nextSibling", next_sibling),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "textContent",
        text_content,
        Some(set_text_content),
    )?;
    let operations = [
        ("appendChild", append_child as fn(&mut Call<'_>) -> _),
        ("insertBefore", insert_before),
        ("removeChild", remove_child),
        ("replaceChild", replace_child),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    Ok(())
}

/// A neighbour of `this`, as its wrapper or `null`.
fn neighbour(
    call: &mut Call<'_>,
    member: &'static str,
    which: fn(&Document, NodeId) -> Option<NodeId>,
) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, member)?;
    let found = which(idl::read(call, this.owner)?, this.node);
    idl::answer_node(call, this.owner, found)
}

/// `get parentNode`.
fn parent_node(call: &mut Call<'_>) -> Result<Answer, Escape> {
    neighbour(call, "parentNode", Document::parent)
}

/// `get firstChild`.
fn first_child(call: &mut Call<'_>) -> Result<Answer, Escape> {
    neighbour(call, "firstChild", Document::first_child)
}

/// `get lastChild`.
fn last_child(call: &mut Call<'_>) -> Result<Answer, Escape> {
    neighbour(call, "lastChild", Document::last_child)
}

/// `get previousSibling`.
fn previous_sibling(call: &mut Call<'_>) -> Result<Answer, Escape> {
    neighbour(call, "previousSibling", Document::previous_sibling)
}

/// `get nextSibling`.
fn next_sibling(call: &mut Call<'_>) -> Result<Answer, Escape> {
    neighbour(call, "nextSibling", Document::next_sibling)
}

/// `get textContent`.
fn text_content(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "textContent")?;
    let document = idl::read(call, this.owner)?;
    let text = match document.kind(this.node) {
        Some(NodeKind::Element(_) | NodeKind::Fragment) => Some(document.text_content(this.node)),
        Some(
            NodeKind::Text(data)
            | NodeKind::Comment(data)
            | NodeKind::ProcessingInstruction { data, .. },
        ) => Some(data.clone()),
        Some(NodeKind::Document | NodeKind::Doctype { .. }) | None => None,
    };
    idl::answer_text(call, text)
}

/// `set textContent`. Step 1 has the string an object's `toString` made.
fn set_text_content(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "textContent")?;
    let data = match call.step() {
        0 => match call.argument(0) {
            Value::Null => String::new(),
            value => match idl::string(call, value, 1)? {
                Converted::Ready(data) => data,
                Converted::Asked(asked) => return Ok(asked),
            },
        },
        1 => idl::answered_string(call)?,
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    idl::change(call, this.owner, |document| {
        match document.kind(this.node) {
            Some(NodeKind::Element(_) | NodeKind::Fragment) => {
                document.replace_all_with_text(this.node, &data).map(drop)
            }
            Some(
                NodeKind::Text(_) | NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. },
            ) => document.set_data(this.node, &data),
            Some(NodeKind::Document | NodeKind::Doctype { .. }) | None => None,
        }
    })?;
    Ok(Answer::Value(Value::Undefined))
}

/// `appendChild(node)`, answering `node`.
fn append_child(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "appendChild")?;
    idl::needs(call, 1, "appendChild")?;
    let node = idl::node(call, 0, this, "appendChild")?;
    let changed = idl::change(call, this.owner, |document| {
        document.append_child(this.node, node)
    })?;
    answered(call, this.owner, changed, 0)
}

/// `insertBefore(node, child)`, answering `node`.
fn insert_before(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "insertBefore")?;
    idl::needs(call, 2, "insertBefore")?;
    let node = idl::node(call, 0, this, "insertBefore")?;
    let child = idl::nullable_node(call, 1, this, "insertBefore")?;
    let changed = idl::change(call, this.owner, |document| {
        document.insert_before(this.node, node, child)
    })?;
    answered(call, this.owner, changed, 0)
}

/// `removeChild(child)`, answering `child`.
fn remove_child(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "removeChild")?;
    idl::needs(call, 1, "removeChild")?;
    let child = idl::node(call, 0, this, "removeChild")?;
    let changed = idl::change(call, this.owner, |document| {
        document.remove_child(this.node, child)
    })?;
    answered(call, this.owner, changed, 0)
}

/// `replaceChild(node, child)`, answering `child`.
fn replace_child(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Node, "replaceChild")?;
    idl::needs(call, 2, "replaceChild")?;
    let node = idl::node(call, 0, this, "replaceChild")?;
    let child = idl::node(call, 1, this, "replaceChild")?;
    let changed = idl::change(call, this.owner, |document| {
        document.replace_child(this.node, node, child)
    })?;
    answered(call, this.owner, changed, 1)
}

/// A change's answer: argument `which`, the wrapper the page passed, or the
/// refusal thrown as its `DOMException`.
fn answered(
    call: &mut Call<'_>,
    owner: Ref,
    changed: Result<NodeId, alo_dom::Refusal>,
    which: usize,
) -> Result<Answer, Escape> {
    match changed {
        Ok(_) => Ok(Answer::Value(call.argument(which))),
        Err(refusal) => Err(dom_exception::thrown(call, owner, refusal)),
    }
}
