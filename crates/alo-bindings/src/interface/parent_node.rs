/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ParentNode`: the mixin `Document`, `Element` and `DocumentFragment`
//! include (queue item 329).
//!
//! Its one member here is `querySelectorAll(selectors)`, the DOM's *scope-
//! match a selectors string*:
//!
//! - The string is parsed by `alo-css`, the parser every style sheet is
//!   read with, and matched by `alo-css`'s one matcher — so a selector a
//!   page queries for and one it styles with mean the same thing. Text that
//!   is not a selector list, or names a selector this engine does not have
//!   (`:has()` among them, until style has it), is a `SyntaxError`
//!   `DOMException`; so is text nested deeper than `alo-css` will hand its
//!   parser, which a page's script chooses.
//! - What is matched is the node's **descendants**, in tree order — never
//!   the node itself, and never a `<template>`'s contents, which are not
//!   descendants. A selector may still look above the node: `div p` from
//!   inside a `div` finds the `p`, as the standard says.
//! - `:scope` is the element the query was asked of; asked of a document or
//!   a fragment, it is `:root`.
//! - The answer is a **static** `NodeList` ([`crate::node_list`]), in tree
//!   order, of each matching element's one wrapper.
//!
//! The walk asks the embedder's stop at each node, as a regular expression
//! does (ADR 0029 § 3): a document is bounded by the heap, and a selector by
//! what `alo-css` parses, but their product is a page's choice.
//!
//! **Not here:** `querySelector`, `children`, `append`, `prepend`,
//! `replaceChildren` and the element counts, absent until a page needs
//! them; `children` is a live collection and stage 3's (ADR 0017 § 8).

use alo_css::SelectorList;
use alo_css::matching::MatchContext;
use alo_dom::NodeKind;
use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};

use super::dom_exception;
use crate::define;
use crate::embed::{self, Wrapping};
use crate::idl::{self, Brand, Converted, This};
use crate::interface;
use crate::node_list;

/// The mixin's members, on the prototype of an interface that includes it.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(
        objects,
        prototype,
        function_prototype,
        "querySelectorAll",
        query_selector_all,
    )
}

/// `querySelectorAll(selectors)`.
fn query_selector_all(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::ParentNode, "querySelectorAll")?;
    idl::needs(call, 1, "querySelectorAll")?;
    let text = match idl::only_string(call)? {
        Converted::Ready(text) => text,
        Converted::Asked(asked) => return Ok(asked),
    };
    let Some(list) = SelectorList::parse_text(&text) else {
        return Err(dom_exception::thrown_named(
            call,
            this.owner,
            "SyntaxError",
            "the string is not a selector list this engine has",
        ));
    };
    let found = matched(call, this, &list)?;
    let at = call.at();
    let scope = call.objects().heap_mut().open();
    let answered = listed(call.objects(), this.owner, &found, at);
    call.objects().heap_mut().close(scope);
    answered.map(|list| Answer::Value(Value::Object(list)))
}

/// Every descendant of `this` that `list` matches, in tree order.
///
/// # Errors
///
/// [`Escape::Interrupted`] when the embedder asks the script to stop.
fn matched(
    call: &Call<'_>,
    this: This,
    list: &SelectorList,
) -> Result<Vec<alo_dom::NodeId>, Escape> {
    let document = idl::read(call, this.owner)?;
    let is_element = matches!(document.kind(this.node), Some(NodeKind::Element(_)));
    let mut context = if is_element {
        MatchContext::scoped(document, this.node)
    } else {
        MatchContext::new(document)
    };
    let mut found = Vec::new();
    for node in document.descendants(this.node) {
        if call.stop_asked() {
            return Err(Escape::Interrupted);
        }
        if list.iter().any(|selector| context.matches(selector, node)) {
            found.push(node);
        }
    }
    Ok(found)
}

/// A static `NodeList` of the wrappers of `nodes`, with a scope open that
/// holds each wrapper until the list does.
fn listed(
    objects: &mut Objects,
    owner: Ref,
    nodes: &[alo_dom::NodeId],
    at: usize,
) -> Result<Ref, Escape> {
    let mut wrappers = Vec::with_capacity(nodes.len());
    for node in nodes {
        let prototype = interface::prototype_of(objects, owner, *node);
        let wrapper = match embed::wrap(objects, owner, *node, prototype) {
            Ok(wrapper) => wrapper,
            Err(Wrapping::Refused(refused)) => return Err(Escape::refused(refused, at)),
            Err(Wrapping::NotADocument) => return Err(Escape::fault(Fault::NotAnObject)),
            // Each node was read from the document just now, and wrapping
            // one releases nothing a wrapper or the document holds.
            Err(Wrapping::NoSuchNode(_)) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
        };
        objects.heap_mut().hold(wrapper);
        wrappers.push(wrapper);
    }
    node_list::made(objects, owner, &wrappers, at)
}
