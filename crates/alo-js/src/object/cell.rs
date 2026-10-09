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
//! # Thirteen kinds, and two of them are not a script's
//!
//! An [`Ordinary`] object, an [`Array`] (queue item 225), an error (queue item
//! 227), an [`ArrayIterator`] (queue item 230), a [`RegExp`] (queue item 74), a
//! [`Promise`] (queue item 333), a [`Date`] (queue item 356), a [`Function`], a [`Text`], a [`Symbol`] — and
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

use super::array::Array;
use super::array_iterator::ArrayIterator;
use super::date::Date;
use super::environment::Environment;
use super::function::Function;
use super::internal::{Exotic, Internal};
use super::ordinary::Ordinary;
use super::promise::Promise;
use super::regexp::RegExp;
use super::slots::Slots;
use super::symbol::Symbol;
use super::text::Text;

/// What one slot of this engine's heap holds.
#[derive(Debug)]
pub enum Cell {
    /// An ordinary object.
    Object(Ordinary),
    /// An array: an ordinary object whose `length` keeps up with its indices
    /// (queue item 225).
    Array(Array),
    /// An object an `Error` constructor made: an ordinary object with the
    /// `[[ErrorData]]` slot (queue item 227).
    ///
    /// The slot holds nothing and is only ever asked whether it is there —
    /// which is what `Object.prototype.toString` asks before it answers
    /// `"[object Error]"` — so it is a kind of cell rather than a field, and
    /// everything else about the object is the ordinary answer.
    Error(Ordinary),
    /// What `[].values()` answers: an ordinary object with the state of the
    /// generator the specification writes an array iterator as (queue item
    /// 230).
    ArrayIterator(ArrayIterator),
    /// What a regular expression literal makes: an ordinary object with the
    /// compiled pattern beside it (queue item 74).
    RegExp(RegExp),
    /// What `new Promise(…)` makes: an ordinary object with a promise's state
    /// beside it (queue item 333).
    Promise(Promise),
    /// What `new Date(…)` makes: an ordinary object with a time value beside
    /// it (queue item 356).
    Date(Date),
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
            Cell::Object(object) | Cell::Error(object) => Some(object),
            Cell::Array(array) => Some(array),
            Cell::ArrayIterator(iterator) => Some(iterator.ordinary()),
            Cell::RegExp(regexp) => Some(regexp.ordinary()),
            Cell::Promise(promise) => Some(promise.ordinary()),
            Cell::Date(date) => Some(date.ordinary()),
            Cell::Function(function) => Some(function),
            Cell::Foreign(exotic) => Some(exotic.as_ref()),
            Cell::Text(_) | Cell::Symbol(_) | Cell::Slots(_) | Cell::Environment(_) => None,
        }
    }

    /// The same, to be written through.
    pub fn internal_mut(&mut self) -> Option<&mut dyn Internal> {
        match self {
            Cell::Object(object) | Cell::Error(object) => Some(object),
            Cell::Array(array) => Some(array),
            Cell::ArrayIterator(iterator) => Some(iterator.ordinary_mut()),
            Cell::RegExp(regexp) => Some(regexp.ordinary_mut()),
            Cell::Promise(promise) => Some(promise.ordinary_mut()),
            Cell::Date(date) => Some(date.ordinary_mut()),
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

    /// The array this cell is, if it is one: `IsArray`, and what an assignment
    /// to `length` asks before it converts the value (queue item 225).
    pub const fn array(&self) -> Option<&Array> {
        match self {
            Cell::Array(array) => Some(array),
            _ => None,
        }
    }

    /// The array iterator this cell is, if it is one (queue item 230).
    pub const fn array_iterator(&self) -> Option<&ArrayIterator> {
        match self {
            Cell::ArrayIterator(iterator) => Some(iterator),
            _ => None,
        }
    }

    /// The same, to be written through.
    pub const fn array_iterator_mut(&mut self) -> Option<&mut ArrayIterator> {
        match self {
            Cell::ArrayIterator(iterator) => Some(iterator),
            _ => None,
        }
    }

    /// The `RegExp` object this cell is, if it is one (queue item 74).
    pub const fn regexp(&self) -> Option<&RegExp> {
        match self {
            Cell::RegExp(regexp) => Some(regexp),
            _ => None,
        }
    }

    /// The promise this cell is, if it is one (queue item 333).
    pub const fn promise(&self) -> Option<&Promise> {
        match self {
            Cell::Promise(promise) => Some(promise),
            _ => None,
        }
    }

    /// The same, to be written through.
    pub const fn promise_mut(&mut self) -> Option<&mut Promise> {
        match self {
            Cell::Promise(promise) => Some(promise),
            _ => None,
        }
    }

    /// The `Date` object this cell is, if it is one (queue item 356).
    pub const fn date(&self) -> Option<&Date> {
        match self {
            Cell::Date(date) => Some(date),
            _ => None,
        }
    }

    /// The same, to be written through.
    pub const fn date_mut(&mut self) -> Option<&mut Date> {
        match self {
            Cell::Date(date) => Some(date),
            _ => None,
        }
    }

    /// Whether this cell has the `[[ErrorData]]` slot (queue item 227).
    pub const fn is_error(&self) -> bool {
        matches!(self, Cell::Error(_))
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
            Cell::Array(_) => "an array",
            Cell::Error(_) => "an error",
            Cell::ArrayIterator(_) => "an array iterator",
            Cell::RegExp(_) => "a regular expression",
            Cell::Promise(_) => "a promise",
            Cell::Date(_) => "a date",
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
            Cell::Object(object) | Cell::Error(object) => object.trace(tracer),
            Cell::Array(array) => array.trace(tracer),
            Cell::ArrayIterator(iterator) => iterator.trace(tracer),
            Cell::RegExp(regexp) => regexp.trace(tracer),
            Cell::Promise(promise) => promise.trace(tracer),
            Cell::Date(date) => date.trace(tracer),
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
            Cell::Object(object) | Cell::Error(object) => object.footprint(),
            Cell::Array(array) => array.footprint(),
            Cell::ArrayIterator(iterator) => iterator.footprint(),
            Cell::RegExp(regexp) => regexp.footprint(),
            Cell::Promise(promise) => promise.footprint(),
            Cell::Date(date) => date.footprint(),
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
