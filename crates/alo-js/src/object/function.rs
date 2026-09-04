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
//! # Two kinds of code, and the rest is the same object
//!
//! A function's body is either a chunk this engine compiled from a script
//! ([`Code::Compiled`]) or a piece of Rust this engine wrote
//! ([`Code::Native`], queue item 218). Everything else about the two is
//! identical — the same cell, the same `typeof`, the same ordinary properties —
//! which is why it is one enumeration inside one object rather than a second
//! kind of callable thing for the interpreter to know about.
//!
//! # Compiled: three fields, and each is a rule
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
//! `prototype`, and no own `name` or `length` — those are queue item 220, and a
//! `length` without a `name` would be half an answer. `call`, `apply` and
//! `bind` are methods of `Function.prototype` and each has to re-enter the
//! script, which is queue item 219. ADR 0013 § 3, absent beats approximate.

use std::rc::Rc;

use crate::heap::{Barrier, Field, Ref, Survivors, Trace, Tracer};
use crate::unit::Unit;

use super::internal::Internal;
use super::key::Key;
use super::native::Native;
use super::ordinary::Ordinary;
use super::property::Property;
use super::value::{Stored, Value};

/// Where a function's body came from.
#[derive(Debug)]
pub enum Code {
    /// A chunk this engine compiled from a script (queue item 209).
    Compiled {
        /// The program the chunk is in.
        unit: Rc<Unit>,
        /// Which chunk of it.
        chunk: u32,
        /// The environment it closed over, empty for one written at a script's
        /// top level.
        environment: Field,
        /// The `this` an arrow took from where it was written.
        captured: Option<Stored>,
    },
    /// A builtin this engine wrote in Rust (queue item 218).
    Native(Native),
}

/// A function object.
#[derive(Debug)]
pub struct Function {
    ordinary: Ordinary,
    code: Code,
}

impl Function {
    /// A function of this chunk, closing over this environment.
    ///
    /// `captured` is the `this` an arrow took from where it was written, and
    /// [`None`] for anything that gets its own. `prototype` is the realm's
    /// `Function.prototype` and is [`None`] only where there is no realm at all
    /// — which a test may make and a script never can.
    pub fn of(
        unit: Rc<Unit>,
        chunk: u32,
        environment: Option<Ref>,
        captured: Option<Value>,
        prototype: Option<Ref>,
    ) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            code: Code::Compiled {
                unit,
                chunk,
                environment: match environment {
                    Some(held) => Field::holding(held),
                    None => Field::empty(),
                },
                captured: captured.map(Stored::holding),
            },
        }
    }

    /// A builtin, whose body is Rust.
    pub fn native(native: Native, prototype: Option<Ref>) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            code: Code::Native(native),
        }
    }

    /// Where its body came from, which is what a call asks first.
    pub const fn code(&self) -> &Code {
        &self.code
    }
}

impl Trace for Function {
    fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        match &self.code {
            Code::Compiled {
                environment,
                captured,
                ..
            } => {
                environment.trace(tracer);
                if let Some(captured) = captured {
                    captured.trace(tracer);
                }
            }
            // A native holds a function pointer and a `&'static str`, neither of
            // which can ever be an edge — see the module comment on
            // [`native`](super::native).
            Code::Native(_) => {}
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

    use super::{Code, Function};
    use crate::abrupt::Escape;
    use crate::object::internal::Internal;
    use crate::object::native::{Call, Native};
    use crate::object::value::Value;
    use crate::unit::Unit;

    #[test]
    fn a_function_is_an_object_that_also_knows_its_code() {
        let unit = Rc::new(Unit::new());
        let function = Function::of(Rc::clone(&unit), 0, None, None, None);
        let Code::Compiled {
            unit: held,
            chunk,
            environment,
            captured,
        } = function.code()
        else {
            panic!("a compiled function's code is a chunk");
        };
        assert_eq!(*chunk, 0);
        assert_eq!(environment.get(), None);
        assert!(captured.is_none(), "only an arrow captures one");
        assert!(Rc::ptr_eq(held, &unit), "the program is shared");
        assert!(function.is_extensible(), "and it is an ordinary object");
        assert!(function.own_keys().is_empty());
    }

    #[test]
    fn an_arrow_holds_the_this_it_was_written_beside() {
        let unit = Rc::new(Unit::new());
        let arrow = Function::of(unit, 0, None, Some(Value::Bool(true)), None);
        let Code::Compiled {
            captured: Some(captured),
            ..
        } = arrow.code()
        else {
            panic!("an arrow captured a this");
        };
        assert_eq!(captured.get(), Value::Bool(true));
    }

    #[test]
    fn a_native_is_the_same_object_with_a_different_body() {
        #[expect(
            clippy::unnecessary_wraps,
            reason = "the signature is `native::Body`, which every builtin shares"
        )]
        fn nothing(_: &mut Call<'_>) -> Result<Value, Escape> {
            Ok(Value::Undefined)
        }
        let function = Function::native(Native::new("nothing", nothing), None);
        let Code::Native(native) = function.code() else {
            panic!("a builtin's code is Rust");
        };
        assert_eq!(native.name(), "nothing");
        assert!(function.is_extensible(), "a page may hang a property on it");
        assert!(function.own_keys().is_empty());
    }
}
