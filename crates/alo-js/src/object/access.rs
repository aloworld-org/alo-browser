/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Get, set, define, delete and own keys — one interface, for every object.
//!
//! ADR 0014 § 11: *one interface for property access, because the
//! representation behind it is the thing an engine changes when it gets fast.
//! Hidden classes and inline caches are refused now under law 3 and are allowed
//! later without an ADR, provided the semantics and the order are unchanged.*
//! This file is that interface, and [`table`](super::table) is the
//! representation it hides.
//!
//! The five operations are written **once**, in terms of the own-property
//! internal methods in [`Internal`], which is how the specification itself
//! defines them: `OrdinaryGet` is `OrdinaryGetOwnProperty` plus a walk. So an
//! exotic object — an array, an `HTMLCollection`, a node — gets all five by
//! answering the own-property questions, and there is no second copy of the
//! prototype walk to keep in step.
//!
//! # What a getter does here, and why that is not a stub
//!
//! Reading an accessor property means **calling a function**, and calling one
//! part way through a property access means re-entering the interpreter from
//! inside an instruction — which is queue item 214. So [`Found::Getter`] hands
//! back the function to call rather than pretending to have called it. That is
//! ADR 0013 § 3's *absent beats approximate* in its most literal form: the
//! answer is a value the caller must act on, so an interpreter that has not
//! learned to call one will not compile, where an engine that quietly answered
//! `undefined` would run and be wrong.
//!
//! # A chain that does not end
//!
//! Every walk is bounded, and the bound is exact rather than chosen: a chain
//! longer than there are **slots in the heap** has visited a slot twice, so it
//! is a cycle. Nothing a page can write makes one — [`Objects::set_prototype`]
//! refuses a cycle, which is the specification's own rule — but an embedder's
//! [`Exotic`](super::Exotic) answers [`Internal::prototype`] with whatever it
//! likes, and a renderer that hung on a lying object would be a denial of
//! service in the process that parses hostile bytes (ADR 0005). So the walk
//! answers [`Fault::ChainDoesNotEnd`] rather than running for ever, and no
//! legal chain is ever refused by it.

use std::fmt;

use crate::heap::{Barrier, Ref};

use super::internal::Internal;
use super::key::Key;
use super::property::Property;
use super::value::Value;
use super::{Objects, Refused};

/// The engine asked something of a reference that could not answer.
///
/// None of these is a page's mistake — a script accessing a property of a
/// string gets a wrapper object, and a script naming nothing gets a
/// `TypeError` the interpreter raises. Each of these is the *engine* holding a
/// reference to the wrong thing, which is why they are errors rather than
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The reference names a cell that is not an object.
    NotAnObject,
    /// The reference names a cell that is not a symbol.
    NotASymbol,
    /// The reference names nothing: its slot was emptied, retired or filled
    /// again (ADR 0014 § 3), which means a root was missed.
    Gone,
    /// A prototype chain visited more slots than the heap has, so it is a
    /// cycle — which only an embedder's object can produce.
    ChainDoesNotEnd,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::NotAnObject => write!(out, "that reference does not name an object"),
            Fault::NotASymbol => write!(out, "that reference does not name a symbol"),
            Fault::Gone => write!(out, "that reference names nothing"),
            Fault::ChainDoesNotEnd => {
                write!(out, "that prototype chain has a cycle in it")
            }
        }
    }
}

/// What reading a property found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Found {
    /// Nothing along the chain has the key, which a script reads as
    /// `undefined` — and which is *not* the same as a property whose value is
    /// `undefined`, since `in` and `Object.hasOwn` can tell them apart.
    Missing,
    /// A data property's value.
    Value(Value),
    /// An accessor's getter, which the caller must call (queue item 214).
    ///
    /// [`Value::Undefined`] is an accessor with a setter and no getter, which
    /// reads as `undefined` without calling anything.
    Getter(Value),
}

/// What shape a property that is about to be written to has.
///
/// Read out of the heap and held by value, so that the borrow is over before
/// anything is changed.
enum Shape {
    /// A data property, and whether it may be written.
    Data { writable: bool },
    /// An accessor, and its setter.
    Accessor(Value),
}

/// What writing a property did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Set {
    /// The value was stored.
    Done,
    /// It was not, and in strict mode that is a `TypeError`: the property is
    /// not writable, or the object is not extensible, or a prototype's
    /// non-writable data property shadows it.
    Refused,
    /// An accessor's setter, which the caller must call with the value.
    ///
    /// [`Value::Undefined`] is an accessor with a getter and no setter, which
    /// is a refusal — but a *different* one, and the caller decides what to say
    /// about it.
    Setter(Value),
}

impl Objects {
    /// `[[Get]]`: the value of a property, following the prototype chain.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object or a chain that
    /// does not end.
    pub fn get(&self, object: Ref, key: Key) -> Result<Found, Fault> {
        let Some((holder, _)) = self.holder_of(object, key)? else {
            return Ok(Found::Missing);
        };
        let Some(property) = self.own(holder)?.own_property(key) else {
            return Ok(Found::Missing);
        };
        Ok(match property.value() {
            Some(value) => Found::Value(value),
            None => Found::Getter(property.getter().unwrap_or(Value::Undefined)),
        })
    }

    /// `[[GetOwnProperty]]`: the property this object has itself, ignoring its
    /// prototype.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn own_property(&self, object: Ref, key: Key) -> Result<Option<&Property>, Fault> {
        Ok(self.own(object)?.own_property(key))
    }

    /// `[[HasProperty]]`: whether anything along the chain has the key.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object or a chain that
    /// does not end.
    pub fn has(&self, object: Ref, key: Key) -> Result<bool, Fault> {
        Ok(self.holder_of(object, key)?.is_some())
    }

    /// `[[OwnPropertyKeys]]`: every own key, in the order a page can see.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn own_keys(&self, object: Ref) -> Result<Vec<Key>, Fault> {
        Ok(self.own(object)?.own_keys())
    }

    /// `[[DefineOwnProperty]]` with a complete descriptor.
    ///
    /// Answers whether it was allowed: `false` is what
    /// `Object.defineProperty` turns into a `TypeError` and what
    /// `Reflect.defineProperty` hands back as it is. The rules are
    /// [`Property::may_replace`] and extensibility, and they are here rather
    /// than in each object so that an exotic one cannot drift from them.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn define(&mut self, object: Ref, key: Key, property: Property) -> Result<bool, Fault> {
        {
            let internal = self.own(object)?;
            let allowed = match internal.own_property(key) {
                Some(existing) => existing.may_replace(&property),
                None => internal.is_extensible(),
            };
            if !allowed {
                return Ok(false);
            }
        }
        self.write(object, |internal, barrier| {
            internal.define_own(barrier, key, property)
        })
    }

    /// `[[Set]]`: store a value, following the chain to find out whether that
    /// is allowed and whether it is a setter's business.
    ///
    /// A property that is found on a **prototype** and is a writable data
    /// property becomes an own property of `object` — which is the rule that
    /// makes `child.x = 1` shadow the parent rather than writing through it,
    /// and getting it the other way round is how one page's object quietly
    /// edits another's.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object or a chain that
    /// does not end.
    pub fn set(&mut self, object: Ref, key: Key, value: Value) -> Result<Set, Fault> {
        if let Some((holder, _)) = self.holder_of(object, key)? {
            // Read what shape the property is and let go of the heap before
            // touching it: a borrow held across the write is the borrow checker
            // catching what an engine in another language finds out at runtime.
            let shape = match self.own(holder)?.own_property(key) {
                Some(property) if property.is_data() => Some(Shape::Data {
                    writable: property.is_writable(),
                }),
                Some(property) => Some(Shape::Accessor(
                    property.setter().unwrap_or(Value::Undefined),
                )),
                None => None,
            };
            match shape {
                Some(Shape::Accessor(setter)) => return Ok(Set::Setter(setter)),
                Some(Shape::Data { writable: false }) => return Ok(Set::Refused),
                Some(Shape::Data { writable: true }) if holder == object => {
                    // The one case that is a store rather than a definition: an
                    // own writable data property keeps its attributes and its
                    // place in the order, so this must not go through
                    // [`Objects::define`].
                    let wrote = self.write(object, |internal, barrier| {
                        internal
                            .own_property_mut(key)
                            .is_some_and(|property| property.write(barrier, value))
                    })?;
                    return Ok(if wrote { Set::Done } else { Set::Refused });
                }
                Some(Shape::Data { writable: true }) | None => {}
            }
        }

        // Either nothing along the chain has it, or a prototype has a writable
        // data property — and that is shadowed by an own property here rather
        // than written through, which [`Objects::define`] refuses if this
        // object is not extensible.
        let defined = self.define(object, key, Property::plain(value))?;
        Ok(if defined { Set::Done } else { Set::Refused })
    }

    /// `[[Delete]]`: take a property away, answering whether it went.
    ///
    /// A property that is not there was already not there, so deleting it
    /// answers `true` — which is the language's answer and is what
    /// `delete a.nothing` evaluates to.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn delete(&mut self, object: Ref, key: Key) -> Result<bool, Fault> {
        match self.own(object)?.own_property(key) {
            None => return Ok(true),
            Some(property) if !property.is_configurable() => return Ok(false),
            Some(_) => {}
        }
        self.write(object, |internal, _| internal.delete_own(key))
    }

    /// `[[GetPrototypeOf]]`.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn prototype(&self, object: Ref) -> Result<Option<Ref>, Fault> {
        Ok(self.own(object)?.prototype())
    }

    /// `[[SetPrototypeOf]]`, refusing a cycle and a non-extensible object.
    ///
    /// The cycle rule is the specification's and it is a **refusal rather than
    /// a detection later**: `a.__proto__ = b; b.__proto__ = a` answers `false`
    /// on the second assignment, so a chain in this heap never has a cycle in
    /// it. The walk in [`Objects::get`] is bounded anyway, because an
    /// embedder's object answers this question for itself.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object or a chain that
    /// does not end.
    pub fn set_prototype(&mut self, object: Ref, to: Option<Ref>) -> Result<bool, Fault> {
        let internal = self.own(object)?;
        if internal.prototype() == to {
            return Ok(true);
        }
        if !internal.is_extensible() {
            return Ok(false);
        }
        if self.reaches(to, object)? {
            return Ok(false);
        }
        self.write(object, |internal, barrier| {
            internal.set_prototype(barrier, to)
        })
    }

    /// `[[IsExtensible]]`.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn is_extensible(&self, object: Ref) -> Result<bool, Fault> {
        Ok(self.own(object)?.is_extensible())
    }

    /// `[[PreventExtensions]]`: no more own properties, ever.
    ///
    /// # Errors
    ///
    /// [`Fault`], for a reference that does not name an object.
    pub fn prevent_extensions(&mut self, object: Ref) -> Result<bool, Fault> {
        self.write(object, |internal, _| internal.prevent_extensions())
    }

    /// Define a property under a name, which is the common shape of every
    /// caller: intern the name, then define.
    ///
    /// The interning is a safepoint and the definition is not, so nothing can
    /// collect between the two — which is what makes this safe to write as one
    /// call rather than as a rooted pair.
    ///
    /// # Errors
    ///
    /// [`Refused`] if the name cannot be interned, and [`Fault`] wrapped in
    /// [`Named::Fault`] if the reference does not name an object.
    pub fn define_named(
        &mut self,
        object: Ref,
        name: &[u16],
        property: Property,
    ) -> Result<bool, Named> {
        let key = self.key(name)?;
        Ok(self.define(object, key, property)?)
    }

    /// Read a property by name.
    ///
    /// # Errors
    ///
    /// As [`Objects::define_named`].
    pub fn get_named(&mut self, object: Ref, name: &[u16]) -> Result<Found, Named> {
        let key = self.key(name)?;
        Ok(self.get(object, key)?)
    }

    /// The internal methods of a cell, or a fault saying why there are none.
    fn own(&self, object: Ref) -> Result<&dyn Internal, Fault> {
        let cell = self.heap().get(object).ok_or(Fault::Gone)?;
        cell.internal().ok_or(Fault::NotAnObject)
    }

    /// Change a cell through the barrier, with the same faults.
    ///
    /// The two [`None`]s are two different mistakes and are kept apart: the
    /// outer one is a reference that names nothing (ADR 0014 § 3), the inner
    /// one is a cell that is not an object.
    fn write<R>(
        &mut self,
        object: Ref,
        with: impl FnOnce(&mut dyn Internal, &mut Barrier) -> R,
    ) -> Result<R, Fault> {
        let outcome = self
            .heap_mut()
            .write(object, |cell, barrier| {
                cell.internal_mut().map(|internal| with(internal, barrier))
            })
            .ok_or(Fault::Gone)?;
        outcome.ok_or(Fault::NotAnObject)
    }

    /// Which object along the chain has this key, and how far up it was.
    ///
    /// The distance is what tells [`Objects::set`] whether a property is its
    /// own or inherited, which is the difference between writing to it and
    /// shadowing it.
    fn holder_of(&self, object: Ref, key: Key) -> Result<Option<(Ref, usize)>, Fault> {
        let mut current = object;
        let mut steps = 0_usize;
        loop {
            let internal = self.own(current)?;
            if internal.own_property(key).is_some() {
                return Ok(Some((current, steps)));
            }
            let Some(next) = internal.prototype() else {
                return Ok(None);
            };
            current = next;
            steps = steps.saturating_add(1);
            if steps > self.heap().slots() {
                return Err(Fault::ChainDoesNotEnd);
            }
        }
    }

    /// Whether the chain from `from` reaches `object`, which is the cycle a
    /// prototype assignment must refuse.
    fn reaches(&self, from: Option<Ref>, object: Ref) -> Result<bool, Fault> {
        let mut current = from;
        let mut steps = 0_usize;
        while let Some(held) = current {
            if held == object {
                return Ok(true);
            }
            current = self.own(held)?.prototype();
            steps = steps.saturating_add(1);
            if steps > self.heap().slots() {
                return Err(Fault::ChainDoesNotEnd);
            }
        }
        Ok(false)
    }
}

/// What a by-name operation can refuse: interning the name, or the object.
///
/// Two errors rather than one because they are answered in different places —
/// a heap at its ceiling stops the tab (ADR 0014 § 9) and a fault is the
/// engine's own mistake — and flattening them into one would lose that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// The name could not be interned.
    Refused(Refused),
    /// The reference did not name an object.
    Fault(Fault),
}

impl From<Refused> for Named {
    fn from(refused: Refused) -> Self {
        Self::Refused(refused)
    }
}

impl From<Fault> for Named {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl fmt::Display for Named {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Named::Refused(refused) => refused.fmt(out),
            Named::Fault(fault) => fault.fmt(out),
        }
    }
}
