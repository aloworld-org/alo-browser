/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Reading and writing a property, either of which may be a call.
//!
//! [`access`](crate::object::access) answers *what is there* — a value, a
//! getter, a setter, nothing — and stops, because the object model has no stack
//! to put a frame on. This file is the other half: it turns
//! [`Found::Getter`] into a call whose answer lands where the access's own
//! operands were, and [`Set::Setter`] into one whose answer is thrown away.
//!
//! # The stack does the bookkeeping, so there is nothing to resume
//!
//! Every property access takes a known number of values off the stack and puts
//! one back: `a.b` takes one and leaves one, `a[b] = c` takes three and leaves
//! one. A call takes everything above its callee and leaves one in its place —
//! so if the callee is put exactly where the access's answer belongs, the
//! `return` that ends the getter *is* the end of the access. No instruction has
//! to run again and nothing has to be remembered.
//!
//! A setter is the one that does not fit, because `a.b = c` evaluates to `c`
//! rather than to what the setter returned. So the value is written into the
//! answer's place first and the call is laid out **above** it, with
//! [`After::Discard`] to drop what comes back.
//!
//! # An accessor with a missing half is not an error
//!
//! `{ get a() {} }` has no setter and reads perfectly well. So a getter that is
//! `undefined` reads as `undefined` without calling anything, and a *setter*
//! that is `undefined` is a refusal — silence in sloppy code and a `TypeError`
//! in strict code, which is the same pair every other failed write gets and is
//! why the message says which of the two it was.

use crate::abrupt::{Escape, Internal};
use crate::code::Half;
use crate::object::{Found, Key, Property, Set, Value};

use super::Engine;
use super::call::Ask;
use super::frame::{After, Run};

impl Engine {
    /// `[[Get]]`, where `depth` is how many stack values the access is made of
    /// — the object being the deepest — and one value is left in their place.
    pub(super) fn read_into(
        &mut self,
        run: &mut Run,
        object: Value,
        key: Key,
        depth: usize,
        at: usize,
    ) -> Result<(), Escape> {
        let held = self.object_of(object, key, at, "read")?;
        match self.objects.get(held, key)? {
            // Nothing has the key — or an accessor has it and has no getter, in
            // which case the property is there and reading it is `undefined`
            // rather than a call.
            Found::Missing | Found::Getter(Value::Undefined) => {
                self.replace(run, depth, Value::Undefined)
            }
            Found::Value(value) => self.replace(run, depth, value),
            Found::Getter(getter) => {
                if self.function_of(getter).is_none() {
                    return Err(self.not_a_function(getter, at));
                }
                let bottom = self.place(run, depth.saturating_sub(1))?;
                self.begin_call(
                    run,
                    bottom,
                    Ask {
                        callee: getter,
                        receiver: object,
                        arguments: &[],
                        at,
                        after: After::Answer,
                    },
                )
            }
        }
    }

    /// `[[Set]]`, where `depth` is how many stack values the assignment is made
    /// of — object, then any key, then the value — and the **value** is left in
    /// their place whatever the write did.
    pub(super) fn write_from(
        &mut self,
        run: &mut Run,
        key: Key,
        depth: usize,
        at: usize,
    ) -> Result<(), Escape> {
        let object = self.peek(run, depth.saturating_sub(1))?;
        let value = self.peek(run, 0)?;
        let strict = run.strict()?;
        let held = self.object_of(object, key, at, "write")?;
        match self.objects.set(held, key, value)? {
            Set::Done => self.replace(run, depth, value),
            Set::Setter(Value::Undefined) => {
                if strict {
                    return Err(Escape::type_error(
                        format!(
                            "{} has a getter and no setter, so it cannot be written",
                            self.describe_key(key)
                        ),
                        at,
                    ));
                }
                self.replace(run, depth, value)
            }
            Set::Setter(setter) => {
                if self.function_of(setter).is_none() {
                    return Err(self.not_a_function(setter, at));
                }
                self.begin_setter(run, setter, object, value, depth, at)
            }
            Set::Refused => {
                if strict {
                    return Err(Escape::type_error(
                        format!("{} cannot be written", self.describe_key(key)),
                        at,
                    ));
                }
                self.replace(run, depth, value)
            }
        }
    }

    /// The setter's call, with what the assignment evaluates to already under
    /// it.
    fn begin_setter(
        &mut self,
        run: &mut Run,
        setter: Value,
        receiver: Value,
        value: Value,
        depth: usize,
        at: usize,
    ) -> Result<(), Escape> {
        let bottom = self.place(run, depth.saturating_sub(1))?;
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, _| {
                slots.truncate(bottom);
                slots.push(value);
            })
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        self.begin_call(
            run,
            bottom.saturating_add(1),
            Ask {
                callee: setter,
                receiver,
                arguments: &[value],
                at,
                after: After::Discard,
            },
        )
    }

    /// `{ get a() {} }`: define one half of an accessor property of the object
    /// under the function, and take the function off.
    ///
    /// The half that is not being defined is **kept** when the property is
    /// already an accessor, which is what makes `{ get a() {}, set a(v) {} }`
    /// one property with two halves rather than two definitions of which the
    /// second wins. That is the specification's own reading of a *partial*
    /// descriptor, in the one place the language has one without
    /// `Object.defineProperty` (queue item 73).
    pub(super) fn define_accessor(
        &mut self,
        run: &mut Run,
        key: Key,
        half: Half,
        depth: usize,
    ) -> Result<(), Escape> {
        let object = self.peek(run, depth.saturating_sub(1))?;
        let function = self.peek(run, 0)?;
        let Value::Object(held) = object else {
            // Only an object literal defines, and it defines on the object it
            // has just made.
            return Err(Escape::Broken(Internal::StackIsWrong));
        };
        let (was_get, was_set) = match self.objects.own_property(held, key)? {
            Some(property) => (
                property.getter().unwrap_or(Value::Undefined),
                property.setter().unwrap_or(Value::Undefined),
            ),
            None => (Value::Undefined, Value::Undefined),
        };
        let (get, set) = match half {
            Half::Getter => (function, was_set),
            Half::Setter => (was_get, function),
        };
        // Enumerable and configurable, which is what an object literal's own
        // properties are.
        self.objects
            .define(held, key, Property::accessor(get, set, true, true))?;
        for _ in 1..depth {
            self.pop(run)?;
        }
        Ok(())
    }

    /// The function a value is, or [`None`] if it is not callable.
    pub(super) fn function_of(&self, value: Value) -> Option<crate::heap::Ref> {
        match value {
            Value::Object(held) if self.objects.callable(held).is_some() => Some(held),
            _ => None,
        }
    }
}
