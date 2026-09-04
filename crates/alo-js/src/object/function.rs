/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A function: an ordinary object that can also be called.
//!
//! ADR 0014 § 11 says internal methods are a trait and that *arrays, functions,
//! proxies and the DOM's oddities are one mechanism rather than two*. A function
//! is the first thing in this engine to take that up: everything a page can do
//! to it as an **object** — read a property, define one, ask for its prototype
//! — is the ordinary answer, delegated, and the only thing added is the one
//! internal method that makes it a function.
//!
//! # `[[Call]]` is not on [`Internal`], and that is deliberate
//!
//! [`Internal`](super::Internal) is *"the own-property internal methods … they
//! need nothing but the object itself"*. Calling needs the interpreter, the
//! stack and a frame, and a cell is borrowed out of the heap while its internal
//! methods run — so a `[[Call]]` on that trait would be a method that may not
//! do the one thing it is for. What is here instead is what the interpreter
//! needs in order to make the call itself: which chunk, of which program, over
//! which environment.
//!
//! # Three fields, and each is a rule
//!
//! - **The unit and the chunk** are the code. A [`Unit`] is reference-counted
//!   rather than in the heap because it holds no heap reference at all — a
//!   chunk is compiled with no heap in sight — so it is outside the collector's
//!   business, and two functions of the same program share one.
//! - **The environment** is what makes it a closure: the bindings that were in
//!   force where it was written, kept alive by this cell for exactly as long as
//!   this cell is. [`None`] is a function written at a script's top level, whose
//!   names are the realm's.
//! - **The `this` it captured**, which is [`Some`] for an **arrow** and [`None`]
//!   for everything else. An arrow has no `this` of its own, so the call cannot
//!   be given one, so it is decided where the arrow was made — and holding it
//!   here rather than walking a chain for it is what makes
//!   [`Op::This`](crate::code::Op) one instruction with no case in it.
//!
//! # What a function has not got yet
//!
//! No `[[Construct]]`: `new`, classes and `super` are queue item 212. No
//! `prototype`, `name`, `length`, `call`, `apply` or `bind` — those are
//! properties and methods of `Function.prototype`, which is a builtin (queue
//! item 73) — and no prototype at all, for the reason the realm's global object
//! has none: an object pretending to have one would be an object whose
//! `toString` a page could find and this engine could not call. ADR 0013 § 3,
//! absent beats approximate.

use std::rc::Rc;

use crate::heap::{Barrier, Field, Ref, Survivors, Trace, Tracer};
use crate::unit::Unit;

use super::internal::Internal;
use super::key::Key;
use super::ordinary::Ordinary;
use super::property::Property;
use super::value::{Stored, Value};

/// A function object.
#[derive(Debug)]
pub struct Function {
    ordinary: Ordinary,
    unit: Rc<Unit>,
    chunk: u32,
    environment: Field,
    captured: Option<Stored>,
}

impl Function {
    /// A function of this chunk, closing over this environment.
    ///
    /// `captured` is the `this` an arrow took from where it was written, and
    /// [`None`] for anything that gets its own.
    pub fn of(
        unit: Rc<Unit>,
        chunk: u32,
        environment: Option<Ref>,
        captured: Option<Value>,
    ) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(None),
            unit,
            chunk,
            environment: match environment {
                Some(held) => Field::holding(held),
                None => Field::empty(),
            },
            captured: captured.map(Stored::holding),
        }
    }

    /// The program its code is in.
    pub const fn unit(&self) -> &Rc<Unit> {
        &self.unit
    }

    /// Which chunk of that program it is.
    pub const fn chunk(&self) -> u32 {
        self.chunk
    }

    /// The environment it closed over.
    pub const fn environment(&self) -> Option<Ref> {
        self.environment.get()
    }

    /// The `this` it captured, which only an arrow has.
    pub fn captured(&self) -> Option<Value> {
        self.captured.as_ref().map(Stored::get)
    }
}

impl Trace for Function {
    fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        self.environment.trace(tracer);
        if let Some(captured) = &self.captured {
            captured.trace(tracer);
        }
    }

    fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }

    fn clear_weak(&mut self, _survivors: &Survivors) {}
}

impl Internal for Function {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.ordinary.own_property(key)
    }

    fn own_property_mut(&mut self, key: Key) -> Option<&mut Property> {
        self.ordinary.own_property_mut(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.ordinary.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.ordinary.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.ordinary.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.ordinary.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.ordinary.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.ordinary.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.ordinary.prevent_extensions()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::Function;
    use crate::object::internal::Internal;
    use crate::object::value::Value;
    use crate::unit::Unit;

    #[test]
    fn a_function_is_an_object_that_also_knows_its_code() {
        let unit = Rc::new(Unit::new());
        let function = Function::of(Rc::clone(&unit), 0, None, None);
        assert_eq!(function.chunk(), 0);
        assert_eq!(function.environment(), None);
        assert_eq!(function.captured(), None, "only an arrow captures one");
        assert!(function.is_extensible(), "and it is an ordinary object");
        assert!(function.own_keys().is_empty());
        assert!(Rc::ptr_eq(function.unit(), &unit), "the program is shared");
    }

    #[test]
    fn an_arrow_holds_the_this_it_was_written_beside() {
        let unit = Rc::new(Unit::new());
        let arrow = Function::of(unit, 0, None, Some(Value::Bool(true)));
        assert_eq!(arrow.captured(), Some(Value::Bool(true)));
    }
}
