/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a slot in the heap holds.
//!
//! Item 71 built [`Heap<T>`](crate::heap::Heap) generic in this, and said what
//! it was waiting for: *the heap will hold one enumeration of cell kinds when
//! [the object model] arrives, an embedder's node wrapper among them, and
//! nothing in that file changes when it does.* This is that enumeration, and
//! nothing in `heap.rs` changed.
//!
//! # Three kinds, and one of them is somebody else's
//!
//! An [`Ordinary`] object, a [`Text`], a [`Symbol`] — and [`Cell::Foreign`],
//! which is an [`Exotic`] an embedder supplied. The last one is ADR 0013 § 6
//! and ADR 0014 § 6 in a single line of code: the DOM is **in this heap**,
//! traced by this collector, in the same graph as the closure that mentions it.
//!
//! A [`Text`] and a [`Symbol`] are in the heap without being objects, and that
//! is the language rather than a shortcut. `"abc".foo` reads a property of a
//! *wrapper* the interpreter makes; the string itself has no properties at all,
//! which is why [`Cell::internal`] answers [`None`] for one and why an access
//! on a primitive is a [`Fault`](super::Fault) rather than an empty answer.

use crate::heap::{Survivors, Trace, Tracer};

use super::internal::{Exotic, Internal};
use super::ordinary::Ordinary;
use super::symbol::Symbol;
use super::text::Text;

/// What one slot of this engine's heap holds.
#[derive(Debug)]
pub enum Cell {
    /// An ordinary object.
    Object(Ordinary),
    /// A string.
    Text(Text),
    /// A symbol.
    Symbol(Symbol),
    /// An object an embedder supplied — a node, a console, a test harness.
    Foreign(Box<dyn Exotic>),
}

impl Cell {
    /// The internal methods of this cell, or [`None`] if it is not an object.
    ///
    /// One function, and it is the only place in the engine that asks what kind
    /// of thing a reference names. Everything else — get, set, define, delete,
    /// the prototype walk — goes through the trait, which is what ADR 0014
    /// § 11's *one mechanism rather than two* means when it is written down.
    pub fn internal(&self) -> Option<&dyn Internal> {
        match self {
            Cell::Object(object) => Some(object),
            Cell::Foreign(exotic) => Some(exotic.as_ref()),
            Cell::Text(_) | Cell::Symbol(_) => None,
        }
    }

    /// The same, to be written through.
    pub fn internal_mut(&mut self) -> Option<&mut dyn Internal> {
        match self {
            Cell::Object(object) => Some(object),
            Cell::Foreign(exotic) => Some(exotic.as_mut()),
            Cell::Text(_) | Cell::Symbol(_) => None,
        }
    }

    /// The string this cell is, if it is one.
    pub const fn text(&self) -> Option<&Text> {
        match self {
            Cell::Text(text) => Some(text),
            Cell::Object(_) | Cell::Symbol(_) | Cell::Foreign(_) => None,
        }
    }

    /// The symbol this cell is, if it is one.
    pub const fn symbol(&self) -> Option<&Symbol> {
        match self {
            Cell::Symbol(symbol) => Some(symbol),
            Cell::Object(_) | Cell::Text(_) | Cell::Foreign(_) => None,
        }
    }

    /// The ordinary object this cell is, if it is one.
    ///
    /// Narrower than [`Cell::internal`] on purpose: this is for the few things
    /// that are about an ordinary object *specifically*, and reaching for it
    /// where the trait would do is how an exotic object stops working.
    pub const fn ordinary(&self) -> Option<&Ordinary> {
        match self {
            Cell::Object(object) => Some(object),
            Cell::Text(_) | Cell::Symbol(_) | Cell::Foreign(_) => None,
        }
    }

    /// What this cell is, for a message a person reads.
    pub fn describe(&self) -> &'static str {
        match self {
            Cell::Object(_) => "an object",
            Cell::Text(_) => "a string",
            Cell::Symbol(_) => "a symbol",
            Cell::Foreign(exotic) => exotic.describe(),
        }
    }
}

impl Trace for Cell {
    fn trace(&self, tracer: &mut Tracer) {
        match self {
            Cell::Object(object) => object.trace(tracer),
            Cell::Symbol(symbol) => symbol.trace(tracer),
            Cell::Foreign(exotic) => exotic.trace(tracer),
            // A string holds no reference at all, which is the other half of
            // why it is immutable: there is nothing in it that could ever
            // become an edge.
            Cell::Text(_) => {}
        }
    }

    fn footprint(&self) -> usize {
        match self {
            Cell::Object(object) => object.footprint(),
            Cell::Text(text) => text.footprint(),
            Cell::Foreign(exotic) => exotic.footprint(),
            Cell::Symbol(_) => 0,
        }
    }

    fn clear_weak(&mut self, survivors: &Survivors) {
        // Only an embedder's object can hold weakness today. `WeakMap`,
        // `WeakSet`, `WeakRef` and `FinalizationRegistry` are builtins (queue
        // item 73) and each will be a cell of its own; what the heap already
        // owes them — the ephemeron fixpoint, the clearing, the report of what
        // was lost — is built and tested (item 71).
        if let Cell::Foreign(exotic) = self {
            exotic.clear_weak(survivors);
        }
    }
}
