/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the DOM standard allows a tree to become, and the refusal it names
//! for each thing it does not.
//!
//! These are the standard's *ensure pre-insertion validity* and the checks
//! at the head of *replace a child*, rule for rule, plus the name check
//! `createElement` makes. They live here rather than in the bindings because
//! a script and an agent change one document (ADR 0017 § 5), and the
//! operations in [`crate::mutation`] ask them before touching anything — so a
//! refusal always leaves the tree exactly as it was.

use crate::document::Document;
use crate::node::{NodeId, NodeKind};
use core::fmt;

/// Why a change was refused, named as the DOM standard names it.
///
/// The bindings turn each into the `DOMException` of the same name; the
/// message says which rule it was, for a person reading a console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `HierarchyRequestError`: the tree would become one the standard does
    /// not allow — a cycle, a second element under a document, text where a
    /// document cannot hold it.
    HierarchyRequest(&'static str),
    /// `NotFoundError`: a node named is not where the change says it is.
    NotFound(&'static str),
    /// `InvalidCharacterError`: a name no element can have.
    InvalidCharacter(&'static str),
}

impl Refusal {
    /// The exception's name, exactly as the standard spells it.
    pub fn name(self) -> &'static str {
        match self {
            Self::HierarchyRequest(_) => "HierarchyRequestError",
            Self::NotFound(_) => "NotFoundError",
            Self::InvalidCharacter(_) => "InvalidCharacterError",
        }
    }

    /// Which rule refused, in words.
    pub fn message(self) -> &'static str {
        match self {
            Self::HierarchyRequest(message)
            | Self::NotFound(message)
            | Self::InvalidCharacter(message) => message,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.message())
    }
}

impl std::error::Error for Refusal {}

/// The answer for an id that names nothing in this document: one minted by
/// another, or one whose node was released. Not a case the standard has —
/// a script never holds a node that does not exist — and the closest name it
/// has for "that node is not here".
const NO_SUCH_NODE: Refusal = Refusal::NotFound("no node in this document has that id");

/// The kinds of node the rules tell apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Document,
    Fragment,
    Doctype,
    Element,
    Text,
    /// A comment or a processing instruction: character data that is not
    /// text.
    OtherData,
}

fn shape(document: &Document, id: NodeId) -> Result<Shape, Refusal> {
    Ok(match document.kind(id).ok_or(NO_SUCH_NODE)? {
        NodeKind::Document => Shape::Document,
        NodeKind::Fragment => Shape::Fragment,
        NodeKind::Doctype { .. } => Shape::Doctype,
        NodeKind::Element(_) => Shape::Element,
        NodeKind::Text(_) => Shape::Text,
        NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } => Shape::OtherData,
    })
}

/// *Ensure pre-insertion validity* of `node` into `parent` before `child`.
pub(crate) fn ensure_pre_insertion(
    document: &Document,
    node: NodeId,
    parent: NodeId,
    child: Option<NodeId>,
) -> Result<(), Refusal> {
    let (parent_shape, node_shape) = common(document, node, parent)?;
    if let Some(child) = child
        && document.parent(child) != Some(parent)
    {
        return Err(Refusal::NotFound(
            "the node to insert before is not a child of this parent",
        ));
    }
    kinds(node_shape, parent_shape)?;
    if parent_shape != Shape::Document {
        return Ok(());
    }

    let doctype_after_child = child.is_some_and(|child| follows(document, child, Shape::Doctype));
    let child_is_doctype = child.is_some_and(|child| is(document, child, Shape::Doctype));
    match node_shape {
        Shape::Fragment => {
            if fragment_is_not_one_element(document, node) {
                return Err(ONE_ELEMENT_NO_TEXT);
            }
            if has_child(document, node, Shape::Element, None)
                && (has_child(document, parent, Shape::Element, None)
                    || child_is_doctype
                    || doctype_after_child)
            {
                return Err(SECOND_ELEMENT);
            }
        }
        Shape::Element => {
            if has_child(document, parent, Shape::Element, None) {
                return Err(SECOND_ELEMENT);
            }
            if child_is_doctype || doctype_after_child {
                return Err(ELEMENT_BEFORE_DOCTYPE);
            }
        }
        Shape::Doctype => {
            if has_child(document, parent, Shape::Doctype, None) {
                return Err(SECOND_DOCTYPE);
            }
            let element_in_the_way = match child {
                Some(child) => precedes(document, child, Shape::Element),
                None => has_child(document, parent, Shape::Element, None),
            };
            if element_in_the_way {
                return Err(DOCTYPE_AFTER_ELEMENT);
            }
        }
        Shape::Document | Shape::Text | Shape::OtherData => {}
    }
    Ok(())
}

/// The checks at the head of *replace a child*: `child` in `parent`,
/// replaced by `node`.
pub(crate) fn ensure_replacement(
    document: &Document,
    node: NodeId,
    child: NodeId,
    parent: NodeId,
) -> Result<(), Refusal> {
    let (parent_shape, node_shape) = common(document, node, parent)?;
    if document.parent(child) != Some(parent) {
        return Err(Refusal::NotFound(
            "the node to replace is not a child of this parent",
        ));
    }
    kinds(node_shape, parent_shape)?;
    if parent_shape != Shape::Document {
        return Ok(());
    }

    let doctype_after_child = follows(document, child, Shape::Doctype);
    match node_shape {
        Shape::Fragment => {
            if fragment_is_not_one_element(document, node) {
                return Err(ONE_ELEMENT_NO_TEXT);
            }
            if has_child(document, node, Shape::Element, None)
                && (has_child(document, parent, Shape::Element, Some(child)) || doctype_after_child)
            {
                return Err(SECOND_ELEMENT);
            }
        }
        Shape::Element => {
            if has_child(document, parent, Shape::Element, Some(child)) {
                return Err(SECOND_ELEMENT);
            }
            if doctype_after_child {
                return Err(ELEMENT_BEFORE_DOCTYPE);
            }
        }
        Shape::Doctype => {
            if has_child(document, parent, Shape::Doctype, Some(child)) {
                return Err(SECOND_DOCTYPE);
            }
            if precedes(document, child, Shape::Element) {
                return Err(DOCTYPE_AFTER_ELEMENT);
            }
        }
        Shape::Document | Shape::Text | Shape::OtherData => {}
    }
    Ok(())
}

/// Whether `name` is a *valid element local name*, which `createElement`
/// requires and refuses with `InvalidCharacterError` otherwise.
///
/// The standard's current rule: a name that starts with an ASCII letter may
/// hold anything but ASCII whitespace, NUL, `/` and `>` — the characters the
/// HTML tokenizer ends a tag name on — and any other name must start with
/// `:`, `_` or a character past ASCII and continue with letters, digits,
/// `-`, `.`, `:`, `_` or characters past ASCII.
pub(crate) fn is_valid_element_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_alphabetic() {
        return !name
            .chars()
            .any(|c| c.is_ascii_whitespace() || matches!(c, '\0' | '/' | '>'));
    }
    let beyond_ascii = |c: char| u32::from(c) >= 0x80;
    (matches!(first, ':' | '_') || beyond_ascii(first))
        && chars.all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '_') || beyond_ascii(c)
        })
}

/// Whether `name` is a *valid attribute local name*, which `setAttribute`
/// requires and refuses with `InvalidCharacterError` otherwise.
///
/// The standard's current rule, looser than an element's because an
/// attribute's name is never the start of a tag: at least one character, and
/// none of ASCII whitespace, NUL, `/`, `=` and `>` — the characters the HTML
/// tokenizer ends an attribute name on.
pub(crate) fn is_valid_attribute_local_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_ascii_whitespace() || matches!(c, '\0' | '/' | '=' | '>'))
}

const SECOND_ELEMENT: Refusal = Refusal::HierarchyRequest("a document can hold only one element");
const SECOND_DOCTYPE: Refusal = Refusal::HierarchyRequest("a document can hold only one doctype");
const ONE_ELEMENT_NO_TEXT: Refusal = Refusal::HierarchyRequest(
    "a fragment put into a document may hold at most one element and no text",
);
const ELEMENT_BEFORE_DOCTYPE: Refusal =
    Refusal::HierarchyRequest("a document's element cannot come before its doctype");
const DOCTYPE_AFTER_ELEMENT: Refusal =
    Refusal::HierarchyRequest("a document's doctype cannot come after its element");

/// The first two rules of both checks, and the kinds of the two nodes they
/// are about.
fn common(document: &Document, node: NodeId, parent: NodeId) -> Result<(Shape, Shape), Refusal> {
    let parent_shape = shape(document, parent)?;
    let node_shape = shape(document, node)?;
    if !matches!(
        parent_shape,
        Shape::Document | Shape::Fragment | Shape::Element
    ) {
        return Err(Refusal::HierarchyRequest(
            "only a document, a fragment or an element can hold children",
        ));
    }
    if is_host_including_inclusive_ancestor(document, node, parent) {
        return Err(Refusal::HierarchyRequest(
            "a node cannot be put inside itself or anything inside it",
        ));
    }
    Ok((parent_shape, node_shape))
}

/// The two rules about what may go where, whatever the parent's children.
fn kinds(node: Shape, parent: Shape) -> Result<(), Refusal> {
    match (node, parent) {
        (Shape::Document, _) => Err(Refusal::HierarchyRequest(
            "a document cannot be put inside anything",
        )),
        (Shape::Text, Shape::Document) => {
            Err(Refusal::HierarchyRequest("a document cannot hold text"))
        }
        (Shape::Doctype, Shape::Fragment | Shape::Element) => Err(Refusal::HierarchyRequest(
            "only a document can hold a doctype",
        )),
        _ => Ok(()),
    }
}

/// Whether `ancestor` is `of` or above it, walking through a template's
/// contents to the template.
///
/// Bounded by the arena's size, so a chain of links no tree should have — a
/// template inside its own contents — answers "yes", and the change is
/// refused, rather than spinning.
fn is_host_including_inclusive_ancestor(document: &Document, ancestor: NodeId, of: NodeId) -> bool {
    let mut current = of;
    for _ in 0..=document.node_count() {
        if current == ancestor {
            return true;
        }
        match document.parent(current).or_else(|| document.host(current)) {
            Some(next) => current = next,
            None => return false,
        }
    }
    true
}

fn is(document: &Document, id: NodeId, wanted: Shape) -> bool {
    shape(document, id) == Ok(wanted)
}

/// Whether `parent` has a child of this shape, not counting `except`.
fn has_child(document: &Document, parent: NodeId, wanted: Shape, except: Option<NodeId>) -> bool {
    document
        .children(parent)
        .any(|child| Some(child) != except && is(document, child, wanted))
}

/// Whether a sibling after `child` has this shape.
fn follows(document: &Document, child: NodeId, wanted: Shape) -> bool {
    let mut next = document.next_sibling(child);
    while let Some(sibling) = next {
        if is(document, sibling, wanted) {
            return true;
        }
        next = document.next_sibling(sibling);
    }
    false
}

/// Whether a sibling before `child` has this shape.
fn precedes(document: &Document, child: NodeId, wanted: Shape) -> bool {
    let mut previous = document.previous_sibling(child);
    while let Some(sibling) = previous {
        if is(document, sibling, wanted) {
            return true;
        }
        previous = document.previous_sibling(sibling);
    }
    false
}

/// A fragment with more than one element child, or with a text child.
fn fragment_is_not_one_element(document: &Document, fragment: NodeId) -> bool {
    document
        .children(fragment)
        .filter(|child| is(document, *child, Shape::Element))
        .nth(1)
        .is_some()
        || has_child(document, fragment, Shape::Text, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_named_as_the_standard_names_it() {
        assert_eq!(SECOND_ELEMENT.name(), "HierarchyRequestError");
        assert_eq!(NO_SUCH_NODE.name(), "NotFoundError");
        assert_eq!(
            Refusal::InvalidCharacter("x").to_string(),
            "InvalidCharacterError: x"
        );
    }

    #[test]
    fn element_names_follow_the_standards_rule() {
        for good in [
            "p",
            "my-widget",
            "A",
            "svg:rect",
            "x\u{1F600}",
            "_x",
            ":x",
            "é",
            "a\"b",
        ] {
            assert!(is_valid_element_local_name(good), "{good:?} is a name");
        }
        for bad in [
            "", "a b", "a/b", "a>b", "a\0", "1a", "-a", ".a", "_x y", "\u{e9}!",
        ] {
            assert!(!is_valid_element_local_name(bad), "{bad:?} is not a name");
        }
    }

    #[test]
    fn attribute_names_follow_the_standards_looser_rule() {
        for good in ["id", "data-x", "1a", "-a", "xlink:href", "a\"b", "\u{e9}!"] {
            assert!(is_valid_attribute_local_name(good), "{good:?} is a name");
        }
        for bad in ["", "a b", "a\tb", "a/b", "a=b", "a>b", "a\0"] {
            assert!(!is_valid_attribute_local_name(bad), "{bad:?} is not a name");
        }
    }
}
