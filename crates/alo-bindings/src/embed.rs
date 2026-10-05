/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Putting a document and its nodes into a heap, and having them back.
//!
//! Three questions, and every DOM member a later file adds is made of them:
//! **adopt** a page's document into its heap (ADR 0017 § 2), **wrap** a node
//! so a script can hold it — the same object every time it is asked for
//! (ADR 0014 § 6) — and ask what node an object **is**, which is how a native
//! reaches its node through the object it was called on and nothing ambient
//! (ADR 0017 § 4).
//!
//! # The one hazard, and how it is closed
//!
//! Making a wrapper is an allocation, an allocation may collect, and a
//! collection releases every detached tree no wrapper holds — including the
//! tree of the node being wrapped, if nothing holds it yet. That is exactly
//! `document.createElement('p')`: a node in a tree of its own, with no
//! wrapper, whose wrapper is about to be made. So before allocating, the
//! document cell is told the node is **pending**, and its sweep keeps a
//! pending node's tree; the wrapper is recorded and the mark cleared the moment
//! the allocation answers, with nothing allocated in between.

use core::fmt;

use alo_dom::{Document, NodeId};
use alo_js::heap::Ref;
use alo_js::object::{Objects, Refused};

use crate::document_cell::DocumentCell;
use crate::wrapper::Wrapper;

/// Why a node could not be wrapped.
#[derive(Debug, Clone, PartialEq)]
pub enum Wrapping {
    /// The reference given as the document cell is not one.
    NotADocument,
    /// The document has no live node with that id: it came from another
    /// document, or its tree was released.
    NoSuchNode(NodeId),
    /// The heap refused the wrapper.
    Refused(Refused),
}

impl fmt::Display for Wrapping {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Wrapping::NotADocument => out.write_str("that is not a page's document"),
            Wrapping::NoSuchNode(node) => write!(out, "the document has no node {node}"),
            Wrapping::Refused(refused) => write!(out, "no wrapper could be made: {refused}"),
        }
    }
}

impl From<Refused> for Wrapping {
    fn from(refused: Refused) -> Self {
        Self::Refused(refused)
    }
}

/// Move `document` into the heap, as the one cell that owns it from now on.
///
/// The caller roots the answer (ADR 0017 § 2: the renderer holds it by one
/// `Root`); unrooted, it lives exactly as long as some wrapper of one of its
/// nodes does.
///
/// # Errors
///
/// [`Refused`] when the heap cannot hold the cell — a document already larger
/// than the heap's ceiling is one, since the cell's footprint is its size.
pub fn adopt(objects: &mut Objects, document: Document) -> Result<Ref, Refused> {
    objects.foreign(Box::new(DocumentCell::new(document)))
}

/// The wrapper of `node`, in the document `cell` holds — the one it already
/// has, or a new one inheriting from `prototype`.
///
/// **A safepoint** when the wrapper is new. `cell` must be rooted by the
/// caller and `prototype` held, as for any allocation; the node itself is
/// kept for the duration (see the module's *one hazard*).
///
/// # Errors
///
/// [`Wrapping::NotADocument`] if `cell` is not a document cell,
/// [`Wrapping::NoSuchNode`] if the document has no such live node, and
/// [`Wrapping::Refused`] when the heap is full.
pub fn wrap(
    objects: &mut Objects,
    cell: Ref,
    node: NodeId,
    prototype: Option<Ref>,
) -> Result<Ref, Wrapping> {
    let held = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Wrapping::NotADocument)?;
    if held.document().get(node).is_none() {
        return Err(Wrapping::NoSuchNode(node));
    }
    if let Some(wrapper) = held.wrapper(node) {
        return Ok(wrapper);
    }

    objects.write_embedded::<DocumentCell, _>(cell, |held, _| held.pending = Some(node));
    let made = objects.foreign(Box::new(Wrapper::new(node, cell, prototype)));
    let recorded = objects.write_embedded::<DocumentCell, _>(cell, |held, barrier| {
        held.pending = None;
        match made {
            Ok(wrapper) if held.document().get(node).is_some() => {
                held.remember(barrier, node, wrapper);
                Ok(wrapper)
            }
            // Not reached: a pending node's tree is kept through the
            // collection. Answered rather than assumed, and the unrecorded
            // wrapper is garbage at the next one.
            Ok(_) => Err(Wrapping::NoSuchNode(node)),
            Err(refused) => Err(Wrapping::Refused(refused)),
        }
    });
    recorded.unwrap_or(Err(Wrapping::NotADocument))
}

/// The document cell and node an object is the wrapper of, or [`None`] for
/// any other value a script could pass — the brand check's question (ADR
/// 0017 § 4).
pub fn node_of(objects: &Objects, held: Ref) -> Option<(Ref, NodeId)> {
    let wrapper = objects.embedded::<Wrapper>(held)?;
    Some((wrapper.document()?, wrapper.node()))
}

/// The document `cell` holds, to read — style, layout, paint and the agent
/// borrow it here for as long as a read lasts (ADR 0017 § 2).
pub fn document(objects: &Objects, cell: Ref) -> Option<&Document> {
    objects
        .embedded::<DocumentCell>(cell)
        .map(DocumentCell::document)
}

/// Change the document `cell` holds with `change`, through `alo-dom`'s own
/// operations (ADR 0017 § 5). [`None`] if `cell` is not a document cell.
///
/// Not a safepoint: nothing in `change` can reach the heap.
pub fn change_document<R>(
    objects: &mut Objects,
    cell: Ref,
    change: impl FnOnce(&mut Document) -> R,
) -> Option<R> {
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| change(held.document_mut()))
}
