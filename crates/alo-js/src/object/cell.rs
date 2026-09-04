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
//! # Six kinds, and two of them are not a script's
//!
//! An [`Ordinary`] object, a [`Function`], a [`Text`], a [`Symbol`] — and
//! [`Cell::Foreign`], which is an [`Exotic`] an embedder supplied. That one is
//! ADR 0013 § 6 and ADR 0014 § 6 in a single line of code: the DOM is **in this
//! heap**, traced by this collector, in the same graph as the closure that
//! mentions it.
//!
//! [`Cell::Slots`] and [`Cell::Environment`] are the engine's own (queue items
//! 72 and 209): the interpreter's value stack, a realm's `let` bindings and a
//! function's own bindings are lists of values, and ADR 0014 § 2 says such a
//! list lives *in the heap* rather than in a Rust local, because a precise
//! collector can only keep what it can walk to. No script can name either —
//! [`Cell::internal`] answers [`None`] for both, so there is no property of one
//! to read.
//!
//! A [`Text`] and a [`Symbol`] are in the heap without being objects, and that
//! is the language rather than a shortcut. `"abc".foo` reads a property of a
//! *wrapper* the interpreter makes; the string itself has no properties at all,
//! which is why [`Cell::internal`] answers [`None`] for one and why an access
//! on a primitive is a [`Fault`](super::Fault) rather than an empty answer.

use crate::heap::{Survivors, Trace, Tracer};

use super::environment::Environment;
use super::function::Function;
use super::internal::{Exotic, Internal};
use super::ordinary::Ordinary;
use super::slots::Slots;
use super::symbol::Symbol;
use super::text::Text;

/// What one slot of this engine's heap holds.
#[derive(Debug)]
pub enum Cell {
    /// An ordinary object.
    Object(Ordinary),
    /// A function, which is an ordinary object that can also be called (queue
    /// item 209).
    Function(Function),
    /// A string.
    Text(Text),
    /// A symbol.
    Symbol(Symbol),
    /// An object an embedder supplied — a node, a console, a test harness.
    Foreign(Box<dyn Exotic>),
    /// A list of values the engine itself holds: an interpreter's stack, a
    /// realm's lexical bindings.
    Slots(Slots),
    /// A function's bindings, and the environment it was written inside — the
    /// cell a closure keeps alive after the call that made it has returned.
    Environment(Environment),
}

impl Cell {
    /// The internal methods of this cell, or [`None`] if it is not an object.
    ///
    /// One function, and it is the only place in the engine that asks what kind
    /// of thing a reference names. Everything else — get, set, define, delete,
    /// the prototype walk — goes through the trait, which is what ADR 0014
    /// § 11's *one mechanism rather than two* means when it is written down.
    /// A function answers here like anything else: `f.a = 1` is an ordinary
    /// property of an ordinary object.
    pub fn internal(&self) -> Option<&dyn Internal> {
        match self {
            Cell::Object(object) => Some(object),
            Cell::Function(function) => Some(function),
            Cell::Foreign(exotic) => Some(exotic.as_ref()),
            Cell::Text(_) | Cell::Symbol(_) | Cell::Slots(_) | Cell::Environment(_) => None,
        }
    }

    /// The same, to be written through.
    pub fn internal_mut(&mut self) -> Option<&mut dyn Internal> {
        match self {
            Cell::Object(object) => Some(object),
            Cell::Function(function) => Some(function),
            Cell::Foreign(exotic) => Some(exotic.as_mut()),
            Cell::Text(_) | Cell::Symbol(_) | Cell::Slots(_) | Cell::Environment(_) => None,
        }
    }

    /// The string this cell is, if it is one.
    pub const fn text(&self) -> Option<&Text> {
        match self {
            Cell::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The symbol this cell is, if it is one.
    pub const fn symbol(&self) -> Option<&Symbol> {
        match self {
            Cell::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    /// The ordinary object this cell is, if it is one.
    ///
    /// Narrower than [`Cell::internal`] on purpose: this is for the few things
    /// that are about an ordinary object *specifically*, and reaching for it
    /// where the trait would do is how an exotic object stops working. A
    /// function is **not** one of these, for that reason.
    pub const fn ordinary(&self) -> Option<&Ordinary> {
        match self {
            Cell::Object(object) => Some(object),
            _ => None,
        }
    }

    /// The function this cell is, if it is one — which is what the interpreter
    /// asks before it makes a call.
    pub const fn function(&self) -> Option<&Function> {
        match self {
            Cell::Function(function) => Some(function),
            _ => None,
        }
    }

    /// The list of values this cell is, if it is one.
    pub const fn slots(&self) -> Option<&Slots> {
        match self {
            Cell::Slots(slots) => Some(slots),
            _ => None,
        }
    }

    /// The same, to be written through.
    pub const fn slots_mut(&mut self) -> Option<&mut Slots> {
        match self {
            Cell::Slots(slots) => Some(slots),
            _ => None,
        }
    }

    /// The environment this cell is, if it is one.
    pub const fn environment(&self) -> Option<&Environment> {
        match self {
            Cell::Environment(environment) => Some(environment),
            _ => None,
        }
    }

    /// The same, to be written through.
    pub const fn environment_mut(&mut self) -> Option<&mut Environment> {
        match self {
            Cell::Environment(environment) => Some(environment),
            _ => None,
        }
    }

    /// What this cell is, for a message a person reads.
    pub fn describe(&self) -> &'static str {
        match self {
            Cell::Object(_) => "an object",
            Cell::Function(_) => "a function",
            Cell::Text(_) => "a string",
            Cell::Symbol(_) => "a symbol",
            Cell::Foreign(exotic) => exotic.describe(),
            Cell::Slots(_) => "the engine's own working memory",
            Cell::Environment(_) => "a function's own bindings",
        }
    }
}

impl Trace for Cell {
    fn trace(&self, tracer: &mut Tracer) {
        match self {
            Cell::Object(object) => object.trace(tracer),
            Cell::Function(function) => function.trace(tracer),
            Cell::Symbol(symbol) => symbol.trace(tracer),
            Cell::Foreign(exotic) => exotic.trace(tracer),
            Cell::Slots(slots) => slots.trace(tracer),
            Cell::Environment(environment) => environment.trace(tracer),
            // A string holds no reference at all, which is the other half of
            // why it is immutable: there is nothing in it that could ever
            // become an edge.
            Cell::Text(_) => {}
        }
    }

    fn footprint(&self) -> usize {
        match self {
            Cell::Object(object) => object.footprint(),
            Cell::Function(function) => function.footprint(),
            Cell::Text(text) => text.footprint(),
            Cell::Foreign(exotic) => exotic.footprint(),
            Cell::Slots(slots) => slots.footprint(),
            Cell::Environment(environment) => environment.footprint(),
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
