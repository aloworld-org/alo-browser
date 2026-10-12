/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ChildNode`: the mixin `Element`, `CharacterData` and `DocumentType`
//! include, and so a member on each of their prototypes rather than on
//! `Node`'s — a document has no `remove()` (queue item 249).
//!
//! - `remove()` takes the node out of whatever holds it and does nothing to a
//!   node nothing holds, as the standard says.
//! - `replaceWith(...nodes)` (queue item 472, opened by the contact form's
//!   script, which puts a paragraph where its form was): each argument is a
//!   node of this document or a string, which becomes a text node; one is
//!   itself, and several are gathered into a fragment, the DOM's *convert
//!   nodes into a node*. A node nothing holds is left alone. Otherwise the
//!   result takes this node's place — or, if gathering moved this node out
//!   of its parent, goes before its first following sibling that was not
//!   among the arguments. Refusals are `alo-dom`'s, thrown as the
//!   `DOMException` each names: Chrome 154 throws `HierarchyRequestError` for
//!   `i.replaceWith(i.parentNode)`, as this does.
//!
//! A string argument that is an object runs the page's `toString`. One such
//! object among the arguments is converted in full; a second is refused by
//! name after the first's has run ([`Missing::ASecondArgumentBehindACall`],
//! queue item 221), as every member with two string arguments is.
//!
//! **Not here:** `before` and `after`, absent until a page or an item needs
//! them.

use alo_dom::validity::Refusal;
use alo_dom::{Document, NodeId};
use alo_js::Escape;
use alo_js::Value;
use alo_js::abrupt::{Internal, Missing};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};

use super::dom_exception;
use crate::define;
use crate::embed;
use crate::idl::{self, Brand, Converted, This};

/// The mixin's members, on the prototype of an interface that includes it.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "remove", remove)?;
    define::operation(
        objects,
        prototype,
        function_prototype,
        "replaceWith",
        replace_with,
    )
}

/// `remove()`.
fn remove(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::ChildNode, "remove")?;
    idl::change(call, this.owner, |document| document.remove(this.node))?;
    Ok(Answer::Value(Value::Undefined))
}

/// One argument of `replaceWith`, converted.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Given {
    /// A node of this document.
    Node(NodeId),
    /// A string, to become a text node.
    Text(String),
}

/// `replaceWith(...nodes)`.
fn replace_with(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::ChildNode, "replaceWith")?;
    let given = match given(call, this)? {
        Ok(given) => given,
        Err(asked) => return Ok(asked),
    };
    let changed = idl::change(call, this.owner, |document| {
        replaced(document, this.node, &given)
    })?;
    match changed {
        Ok(()) => Ok(Answer::Value(Value::Undefined)),
        Err(refusal) => Err(dom_exception::thrown(call, this.owner, refusal)),
    }
}

/// Every argument as a node or a string, or what to ask for first: step 1
/// has the string the one object that is not a node made.
fn given(call: &Call<'_>, this: This) -> Result<Result<Vec<Given>, Answer>, Escape> {
    let mut answered = match call.step() {
        0 => None,
        1 => Some(idl::answered_string(call)?),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let asked = answered.is_some();
    let mut met_an_object = false;
    let mut out = Vec::with_capacity(call.count());
    for which in 0..call.count() {
        let argument = call.argument(which);
        let is_a_node = match argument {
            Value::Object(held) => embed::node_of(call.seen(), held).is_some(),
            _ => false,
        };
        if is_a_node {
            out.push(Given::Node(idl::node(call, which, this, "replaceWith")?));
            continue;
        }
        if !matches!(argument, Value::Object(_)) {
            match idl::string(call, argument, 1)? {
                Converted::Ready(text) => out.push(Given::Text(text)),
                Converted::Asked(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
            }
            continue;
        }
        if met_an_object {
            return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
        }
        met_an_object = true;
        match answered.take() {
            Some(text) => out.push(Given::Text(text)),
            None if !asked => match idl::string(call, argument, 1)? {
                Converted::Ready(text) => out.push(Given::Text(text)),
                Converted::Asked(answer) => return Ok(Err(answer)),
            },
            None => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }
    Ok(Ok(out))
}

/// The DOM's `replaceWith` steps, once every argument is converted.
fn replaced(document: &mut Document, child: NodeId, given: &[Given]) -> Result<(), Refusal> {
    let Some(parent) = document.parent(child) else {
        return Ok(());
    };
    let nodes: Vec<NodeId> = given
        .iter()
        .filter_map(|one| match one {
            Given::Node(node) => Some(*node),
            Given::Text(_) => None,
        })
        .collect();
    let mut viable = document.next_sibling(child);
    while let Some(sibling) = viable
        && nodes.contains(&sibling)
    {
        viable = document.next_sibling(sibling);
    }
    let node = into_a_node(document, given)?;
    if document.parent(child) == Some(parent) {
        document.replace_child(parent, node, child).map(|_| ())
    } else {
        document.insert_before(parent, node, viable).map(|_| ())
    }
}

/// The DOM's *convert nodes into a node*: one node is itself, a string a
/// new text node, and any other number of them a new fragment holding each
/// in order.
fn into_a_node(document: &mut Document, given: &[Given]) -> Result<NodeId, Refusal> {
    let mut made = Vec::with_capacity(given.len());
    for one in given {
        made.push(match one {
            Given::Node(node) => *node,
            Given::Text(text) => document.create_text_node(text),
        });
    }
    if let [only] = made.as_slice() {
        return Ok(*only);
    }
    let fragment = document.create_document_fragment();
    for node in made {
        document.append_child(fragment, node)?;
    }
    Ok(fragment)
}

#[cfg(test)]
mod tests {
    use alo_dom::parse_document;

    use super::*;

    /// A `<div>` holding `inner`, and the document.
    fn div(inner: &str) -> (Document, NodeId) {
        let document = parse_document(&format!("<div>{inner}</div>"));
        let div = document
            .descendants(document.root())
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("div"))
            })
            .unwrap();
        (document, div)
    }

    /// What `node` holds, written as markup.
    fn written(document: &Document, node: NodeId) -> String {
        document.serialize_children(node)
    }

    #[test]
    fn several_nodes_and_strings_take_one_place_as_chrome_puts_them() {
        // Chrome 154: `i.replaceWith('t', d.lastChild, 'u')` on
        // `<i></i><b></b>` leaves `t<b></b>u`.
        let (mut document, div) = div("<i></i><b></b>");
        let i = document.first_child(div).unwrap();
        let b = document.last_child(div).unwrap();
        let given = [
            Given::Text("t".to_owned()),
            Given::Node(b),
            Given::Text("u".to_owned()),
        ];
        replaced(&mut document, i, &given).unwrap();
        assert_eq!(written(&document, div), "t<b></b>u");
        assert_eq!(document.parent(i), None, "the node it replaced is out");
    }

    #[test]
    fn nothing_given_takes_the_node_away_and_a_lone_node_is_left_alone() {
        let (mut document, div) = div("<i></i><b></b>");
        let i = document.first_child(div).unwrap();
        replaced(&mut document, i, &[]).unwrap();
        assert_eq!(written(&document, div), "<b></b>");
        let count = document.change_count();
        let b = document.create_element("b").unwrap();
        replaced(&mut document, i, &[Given::Node(b)]).unwrap();
        assert_eq!(document.change_count(), count, "nothing held it");
    }

    #[test]
    fn a_node_replaced_with_itself_stays_and_its_parent_is_refused() {
        let (mut document, div) = div("<i></i><b></b>");
        let i = document.first_child(div).unwrap();
        replaced(&mut document, i, &[Given::Node(i)]).unwrap();
        assert_eq!(written(&document, div), "<i></i><b></b>");
        assert!(matches!(
            replaced(&mut document, i, &[Given::Node(div)]),
            Err(Refusal::HierarchyRequest(_))
        ));
        assert_eq!(written(&document, div), "<i></i><b></b>");
    }

    #[test]
    fn a_node_moved_out_by_gathering_is_followed_by_its_first_sibling_not_given() {
        // `i.replaceWith(b, i)`: gathering both moves `i` out of the div,
        // so the fragment goes before `i`'s next sibling not given — `c`.
        let (mut document, div) = div("<i></i><b></b><c></c>");
        let i = document.first_child(div).unwrap();
        let b = document.next_sibling(i).unwrap();
        replaced(&mut document, i, &[Given::Node(b), Given::Node(i)]).unwrap();
        assert_eq!(written(&document, div), "<b></b><i></i><c></c>");
    }
}
