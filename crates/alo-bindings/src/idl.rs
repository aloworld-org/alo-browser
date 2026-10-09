/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What every member does before and after its own work: Web IDL's half.
//!
//! - **The brand check** (ADR 0017 § 4): a member asks for its `this` as a
//!   wrapper of the kind of node its interface is, and anything else — a
//!   plain object, a `Text` where an `Element` is required — is a
//!   `TypeError`, never a guess.
//! - **Arguments**: too few is a `TypeError`; a `Node` argument must be a
//!   wrapper **of the same document** as `this`, since a cross-document write
//!   is what ADR 0017 § 4 refuses; a `DOMString` argument is `ToString`,
//!   which for an object runs the page's own `toString` and so is asked of
//!   the interpreter rather than done here; an `HTMLElement?` is an element
//!   in the HTML namespace, or `null`.
//! - **Answers**: a node is answered as its one wrapper (ADR 0014 § 6), made
//!   if it has none, and text as a string in the heap.
//!
//! # A string crosses into the document as UTF-8
//!
//! `alo-dom` holds text as Rust strings and a script holds UTF-16, so a lone
//! surrogate a script puts into the document becomes U+FFFD there — as it
//! does when the parser reads one. A page reading it back sees the
//! replacement character. Holding the document's text as UTF-16 is the cure,
//! and it waits for a page that needs it.

use alo_dom::{Document, Namespace, NodeId, NodeKind};
use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Fault, Value};

use crate::document_cell::DocumentCell;
use crate::embed::{self, Wrapping};
use crate::interface;

/// What a member requires its `this` to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Brand {
    /// Any node.
    Node,
    /// An element.
    Element,
    /// An element in the HTML namespace.
    HtmlElement,
    /// An element in the SVG namespace.
    SvgElement,
    /// The document node.
    Document,
    /// A node the `ChildNode` mixin is on: an element, a doctype or
    /// character data.
    ChildNode,
    /// A node the `ParentNode` mixin is on: a document, an element or a
    /// fragment.
    ParentNode,
}

impl Brand {
    /// The interface's name, for the message.
    const fn name(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Element => "Element",
            Self::HtmlElement => "HTMLElement",
            Self::SvgElement => "SVGElement",
            Self::Document => "Document",
            Self::ChildNode => "ChildNode",
            Self::ParentNode => "ParentNode",
        }
    }

    /// Whether a node of `kind` is one.
    const fn admits(self, kind: &NodeKind) -> bool {
        match self {
            Self::Node => true,
            Self::Element => matches!(kind, NodeKind::Element(_)),
            Self::HtmlElement => matches!(
                kind,
                NodeKind::Element(element) if matches!(element.name.ns, Namespace::Html)
            ),
            Self::SvgElement => matches!(
                kind,
                NodeKind::Element(element) if matches!(element.name.ns, Namespace::Svg)
            ),
            Self::Document => matches!(kind, NodeKind::Document),
            Self::ChildNode => matches!(
                kind,
                NodeKind::Element(_)
                    | NodeKind::Doctype { .. }
                    | NodeKind::Text(_)
                    | NodeKind::Comment(_)
                    | NodeKind::ProcessingInstruction { .. }
            ),
            Self::ParentNode => matches!(
                kind,
                NodeKind::Document | NodeKind::Element(_) | NodeKind::Fragment
            ),
        }
    }
}

/// A member's `this`, once the brand check has passed: the node, and the
/// cell of the document it is in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct This {
    /// The document cell.
    pub owner: Ref,
    /// The node.
    pub node: NodeId,
}

/// The brand check: `this` as a wrapper of a node `brand` admits.
///
/// # Errors
///
/// A `TypeError` naming `member` for anything else.
pub(crate) fn this(call: &Call<'_>, brand: Brand, member: &'static str) -> Result<This, Escape> {
    let refused = || {
        Escape::type_error(
            format!(
                "'{member}' was called on something that is not a {}",
                brand.name()
            ),
            call.at(),
        )
    };
    let Value::Object(held) = call.this() else {
        return Err(refused());
    };
    let (owner, node) = embed::node_of(call.seen(), held).ok_or_else(refused)?;
    let kind = embed::document(call.seen(), owner)
        .and_then(|document| document.kind(node))
        .ok_or_else(refused)?;
    if brand.admits(kind) {
        Ok(This { owner, node })
    } else {
        Err(refused())
    }
}

/// Web IDL's argument count: `member` needs `count` arguments.
///
/// # Errors
///
/// A `TypeError` when fewer were passed.
pub(crate) fn needs(call: &Call<'_>, count: usize, member: &'static str) -> Result<(), Escape> {
    if call.count() >= count {
        return Ok(());
    }
    Err(Escape::type_error(
        format!(
            "'{member}' needs {count} argument{} and was given {}",
            if count == 1 { "" } else { "s" },
            call.count()
        ),
        call.at(),
    ))
}

/// Argument `which` as a node of `this`'s document.
///
/// # Errors
///
/// A `TypeError` for anything that is not a wrapper, and for a wrapper of
/// another document.
pub(crate) fn node(
    call: &Call<'_>,
    which: usize,
    this: This,
    member: &'static str,
) -> Result<NodeId, Escape> {
    let held = match call.argument(which) {
        Value::Object(held) => embed::node_of(call.seen(), held),
        _ => None,
    };
    match held {
        Some((owner, node)) if owner == this.owner => Ok(node),
        Some(_) => Err(Escape::type_error(
            format!(
                "argument {} to '{member}' is a node of another document",
                which.saturating_add(1)
            ),
            call.at(),
        )),
        None => Err(Escape::type_error(
            format!(
                "argument {} to '{member}' is not a Node",
                which.saturating_add(1)
            ),
            call.at(),
        )),
    }
}

/// Argument `which` as a `Node?`: `null` and `undefined` are no node.
///
/// # Errors
///
/// As [`node`], for anything else.
pub(crate) fn nullable_node(
    call: &Call<'_>,
    which: usize,
    this: This,
    member: &'static str,
) -> Result<Option<NodeId>, Escape> {
    match call.argument(which) {
        Value::Null | Value::Undefined => Ok(None),
        _ => node(call, which, this, member).map(Some),
    }
}

/// Argument `which` as an `HTMLElement?`: `null` and `undefined` are no
/// element, and anything else must be an element in the HTML namespace of
/// `this`'s document.
///
/// The conversion asks the node's namespace, which is exactly what being
/// an `HTMLElement` is ([`crate::interface`]).
///
/// # Errors
///
/// A `TypeError` for anything that is not an HTML element, and as [`node`]
/// for a node of another document.
pub(crate) fn nullable_html_element(
    call: &Call<'_>,
    which: usize,
    this: This,
    member: &'static str,
) -> Result<Option<NodeId>, Escape> {
    let held = match call.argument(which) {
        Value::Null | Value::Undefined => return Ok(None),
        Value::Object(held) => embed::node_of(call.seen(), held),
        _ => None,
    };
    if held.is_some() {
        let node = node(call, which, this, member)?;
        let is_html = embed::document(call.seen(), this.owner)
            .and_then(|document| document.element(node))
            .is_some_and(|element| element.name.ns == Namespace::Html);
        if is_html {
            return Ok(Some(node));
        }
    }
    Err(Escape::type_error(
        format!(
            "argument {} to '{member}' is not an HTMLElement",
            which.saturating_add(1)
        ),
        call.at(),
    ))
}

/// A `DOMString` argument, converted — or what to ask the interpreter for
/// first.
#[derive(Debug)]
pub(crate) enum Converted {
    /// The string, ready.
    Ready(String),
    /// The value is an object, and the member must ask for it to be made a
    /// primitive and come back.
    Asked(Answer),
}

/// `ToString(value)`: ready for a primitive; for an object, the request
/// that runs its `toString`, coming back at `step`, where
/// [`answered_string`] reads the result.
///
/// # Errors
///
/// A `TypeError` for a symbol, as `ToString` throws.
pub(crate) fn string(call: &Call<'_>, value: Value, step: u32) -> Result<Converted, Escape> {
    match Primitive::of(value) {
        Some(primitive) => primitive_string(call, primitive).map(Converted::Ready),
        None => Ok(Converted::Asked(Answer::want(
            Want::Primitive {
                of: value,
                hint: Hint::String,
            },
            step,
        ))),
    }
}

/// A member's one `DOMString` argument, the first: ready, or what to ask for
/// first. Step 1 has the string an object's `toString` made.
///
/// # Errors
///
/// As [`string`] and [`answered_string`], and this crate's own bug at any
/// other step.
pub(crate) fn only_string(call: &Call<'_>) -> Result<Converted, Escape> {
    match call.step() {
        0 => string(call, call.argument(0), 1),
        1 => answered_string(call).map(Converted::Ready),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// How a `DOMString` argument reads one value before it is converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spelled {
    /// `ToString` of whatever was given, `null` and `undefined` included.
    AsGiven,
    /// `[LegacyNullToEmptyString]`: `null` is `""`.
    NullIsEmpty,
    /// An optional `[LegacyNullToEmptyString]` argument whose default is
    /// `""`: `null`, `undefined` and nothing passed are each `""`.
    NullOrAbsentIsEmpty,
}

/// The arguments `spelled` describes, in order, each a `DOMString` — or what
/// to ask for first.
///
/// A native keeps one answer across what it asks for (`element.rs` says
/// why), so **one** object among them is converted in full: step 0 converts
/// the primitives and asks for the first object, coming back at step 1,
/// which reads that object's string and converts the primitives again,
/// which runs no script. A second object is refused by name
/// ([`Missing::ASecondArgumentBehindACall`], queue item 221) after the
/// first's `toString` has run.
///
/// # Errors
///
/// A `TypeError` for a symbol, the refusal above, and this crate's own bug
/// at any step but 0 and 1.
pub(crate) fn strings(
    call: &Call<'_>,
    spelled: &[Spelled],
) -> Result<Result<Vec<String>, Answer>, Escape> {
    let mut answered = match call.step() {
        0 => None,
        1 => Some(answered_string(call)?),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let asked = answered.is_some();
    let mut met_an_object = false;
    let mut out = Vec::with_capacity(spelled.len());
    for (which, spelled) in spelled.iter().enumerate() {
        let argument = call.argument(which);
        match (spelled, argument) {
            (Spelled::NullIsEmpty | Spelled::NullOrAbsentIsEmpty, Value::Null)
            | (Spelled::NullOrAbsentIsEmpty, Value::Undefined) => {
                out.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(primitive) = Primitive::of(argument) {
            out.push(primitive_string(call, primitive)?);
            continue;
        }
        if met_an_object {
            return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
        }
        met_an_object = true;
        match answered.take() {
            Some(string) => out.push(string),
            None if !asked => match string(call, argument, 1)? {
                Converted::Ready(string) => out.push(string),
                Converted::Asked(answer) => return Ok(Err(answer)),
            },
            None => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }
    Ok(Ok(out))
}

/// A member's one `unsigned long` argument, the first: Web IDL's
/// conversion — `ToNumber`, then modulo 2³² — or what to ask for first.
/// Step 1 has the primitive an object's `valueOf` or `toString` made.
///
/// # Errors
///
/// A `TypeError` for a symbol, as `ToNumber` throws, and this crate's own
/// bug at any step but 0 and 1.
pub(crate) fn only_unsigned_long(call: &Call<'_>) -> Result<Result<u32, Answer>, Escape> {
    let primitive = match call.step() {
        0 => match Primitive::of(call.argument(0)) {
            Some(primitive) => primitive,
            None => {
                return Ok(Err(Answer::want(
                    Want::Primitive {
                        of: call.argument(0),
                        hint: Hint::Number,
                    },
                    1,
                )));
            }
        },
        1 => Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?,
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let number = convert::to_number(call.seen(), primitive, call.at())?;
    Ok(Ok(convert::to_uint32(number)))
}

/// The string a conversion [`string`] asked for came back as.
///
/// # Errors
///
/// A `TypeError` for a symbol, and this crate's own bug when the answer is
/// not a primitive, since a conversion answers one or throws.
pub(crate) fn answered_string(call: &Call<'_>) -> Result<String, Escape> {
    let primitive =
        Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    primitive_string(call, primitive)
}

/// A primitive as the string the document holds.
fn primitive_string(call: &Call<'_>, primitive: Primitive) -> Result<String, Escape> {
    let units = convert::to_units(call.seen(), primitive, call.at())?;
    Ok(String::from_utf16_lossy(&units))
}

/// Read the document `cell` holds.
///
/// # Errors
///
/// A fault if `cell` is not a document cell, which the brand check has
/// already said it is.
pub(crate) fn read<'a>(call: &'a Call<'_>, owner: Ref) -> Result<&'a Document, Escape> {
    embed::document(call.seen(), owner).ok_or(Escape::fault(Fault::NotAnObject))
}

/// Change the document `cell` holds, through `alo-dom`'s own operations.
///
/// # Errors
///
/// As [`read`].
pub(crate) fn change<R>(
    call: &mut Call<'_>,
    owner: Ref,
    change: impl FnOnce(&mut Document) -> R,
) -> Result<R, Escape> {
    embed::change_document(call.objects(), owner, change).ok_or(Escape::fault(Fault::NotAnObject))
}

/// Answer `node` as its one wrapper, made inheriting from its interface's
/// prototype if it has none; `null` for no node.
///
/// **A safepoint** when the wrapper is new.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault if the node is
/// not in the document, which a member only asks about nodes it just read.
pub(crate) fn answer_node(
    call: &mut Call<'_>,
    owner: Ref,
    node: Option<NodeId>,
) -> Result<Answer, Escape> {
    let Some(node) = node else {
        return Ok(Answer::Value(Value::Null));
    };
    let at = call.at();
    let prototype = interface::prototype_of(call.seen(), owner, node);
    match embed::wrap(call.objects(), owner, node, prototype) {
        Ok(wrapper) => Ok(Answer::Value(Value::Object(wrapper))),
        Err(Wrapping::Refused(refused)) => Err(Escape::refused(refused, at)),
        Err(Wrapping::NotADocument | Wrapping::NoSuchNode(_)) => {
            Err(Escape::fault(Fault::NotAnObject))
        }
    }
}

/// Answer `text` as a string in the heap; `null` for none.
///
/// # Errors
///
/// A `RangeError` for a string longer than the engine makes, and
/// [`Escape::Full`] for a heap at its ceiling.
pub(crate) fn answer_text(call: &mut Call<'_>, text: Option<String>) -> Result<Answer, Escape> {
    let Some(text) = text else {
        return Ok(Answer::Value(Value::Null));
    };
    let at = call.at();
    let units: Vec<u16> = text.encode_utf16().collect();
    let held = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(held)))
}

/// The document cell's own type, for a member that reads more than the
/// document.
pub(crate) fn owner_cell<'a>(call: &'a Call<'_>, owner: Ref) -> Result<&'a DocumentCell, Escape> {
    call.seen()
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))
}
