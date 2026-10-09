/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `new`: making a constructor, and constructing with one (queue item 212).
//!
//! Two of the specification's operations, one at each end of a constructor's
//! life. `MakeConstructor` runs when a plain `function` is **made**, and gives
//! it the `prototype` object every instance will inherit from. `[[Construct]]`
//! runs at `new f()`, makes the instance from that `prototype`, and runs the
//! body with the instance as its `this`.
//!
//! # A construction is a call with a different landing
//!
//! [`Op::Construct`](crate::code::Op) finds the stack in the shape a call
//! leaves — the constructor, a place for its `this`, the arguments — so once
//! the instance is written into the `this` slot, entering the body is
//! [`Engine::enter_at`] unchanged: the same frame, the same bounds, the same
//! `RangeError` for a constructor that constructs itself for ever. What differs
//! is [`After::Construct`], which `return` reads. A body that returns an object
//! answers with it; one that returns anything else — `undefined` by running off
//! its end, or `return 1` — answers with the instance, which is still in the
//! `this` slot because nothing in the language can assign to `this`.
//!
//! # What may be constructed is decided where it was written
//!
//! A plain `function`, declared or as an expression, has a `[[Construct]]`. An
//! arrow, a method, a getter and a setter are callable and nothing more.
//! [`Chunk::constructs`](crate::code::Chunk::constructs) records which, so
//! the check is one question rather than a list of exceptions.
//!
//! A builtin constructs when it was made as one
//! ([`Native::instance`](crate::object::Native::instance)), and the `Error`
//! constructors are the first that are (queue item 227); `Object` and
//! `Function` are still item 73's. Its instance is made by
//! [`Engine::make_instance`] rather than by [`Engine::construct`], because a
//! builtin constructor called **without** `new` is given one too — `TypeError('x')`
//! is the same object as `new TypeError('x')` — and making it in the one place
//! both paths pass through is what keeps the two from differing. An
//! embedder's constructor ([`Instance::Made`], ADR 0018 § 8's `Event`) is
//! given one only when constructed, since Web IDL makes calling it without
//! `new` a `TypeError`.
//!
//! # What is not here
//!
//! Classes, `super`, `new.target` and private names are queue item 223, and the
//! compiler refuses each by name. A class constructor is the case that makes
//! `new.target` differ from the callee and the instance be made by a parent
//! rather than here, and neither can be written until a class can.

use crate::abrupt::{Escape, Internal};
use crate::heap::Ref;
use crate::object::native::Instance;
use crate::object::{Code, Found, Property, Value};

use super::Engine;
use super::frame::{After, Run};

impl Engine {
    /// `Op::Construct`: the constructor, a place for its `this` and `argc`
    /// arguments are on the stack.
    ///
    /// The constructor is checked **after** the arguments were evaluated,
    /// because that is the specification's order and a page can see it:
    /// `new 1(f())` calls `f` first.
    pub(super) fn construct(&mut self, run: &mut Run, argc: u32, at: usize) -> Result<(), Escape> {
        let argc = usize::try_from(argc).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        let height = self.height(run)?;
        let callee_at = height
            .checked_sub(argc.saturating_add(2))
            .filter(|place| *place >= run.base().unwrap_or(usize::MAX))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let callee = self.value_at(run, callee_at)?;
        let Some((constructor, native)) = self.constructor(callee) else {
            return Err(Escape::type_error(
                format!("{} is not a constructor", self.describe(callee)),
                at,
            ));
        };
        if native {
            // Entering it makes its instance, as calling it would.
            return self.enter_at(run, callee_at, argc, at, After::Construct);
        }

        // `OrdinaryCreateFromConstructor`: the instance inherits from the
        // constructor's `prototype` if that is an object, and from
        // `Object.prototype` if a page has written something else there.
        let key = self
            .objects
            .key(&units("prototype"))
            .map_err(|why| Escape::refused(why, at))?;
        let above = match self.objects.get(constructor, key)? {
            Found::Value(Value::Object(above)) => above,
            Found::Value(_) => self.realm.intrinsics().object_prototype(&self.objects)?,
            // Every constructor is given a `prototype` that is a data property
            // and may not be reconfigured, so it is there and it is data.
            Found::Missing | Found::Getter(_) => {
                return Err(Escape::Broken(Internal::ConstructorIsWrong));
            }
        };
        // A safepoint. The constructor is on the stack and `above` is either
        // its property or the realm's, so both survive it.
        let instance = self
            .objects
            .object(Some(above))
            .map_err(|why| Escape::refused(why, at))?;
        self.write_at(run, callee_at.saturating_add(1), Value::Object(instance))?;
        self.enter_at(run, callee_at, argc, at, After::Construct)
    }

    /// `MakeConstructor`, for a function that is on top of the stack.
    ///
    /// A fresh object inheriting from `Object.prototype`, with a `constructor`
    /// pointing back at the function — writable, configurable, not enumerable —
    /// becomes the function's `prototype`, which is writable and neither
    /// enumerable nor configurable. Those attributes are the specification's,
    /// and the last of them is what lets [`Engine::construct`] treat an
    /// accessor there as this engine's own bug.
    ///
    /// # The scope is the point of the function
    ///
    /// Making the object allocates and interning each name allocates, so the
    /// object and the two keys are held in a [`Scope`](crate::heap::Scope)
    /// until a property owns them — the same reason
    /// [`builtin::method`](crate::builtin) opens one.
    pub(super) fn make_constructor(&mut self, function: Ref, at: usize) -> Result<(), Escape> {
        let scope = self.objects.heap_mut().open();
        let outcome = self.furnish_constructor(function, at);
        self.objects.heap_mut().close(scope);
        outcome
    }

    /// [`Engine::make_constructor`], with the scope already open.
    fn furnish_constructor(&mut self, function: Ref, at: usize) -> Result<(), Escape> {
        let above = self.realm.intrinsics().object_prototype(&self.objects)?;
        let prototype = self
            .objects
            .object(Some(above))
            .map_err(|why| Escape::refused(why, at))?;
        self.objects.heap_mut().hold(prototype);

        let back = self.held_key("constructor", at)?;
        self.objects.define(
            prototype,
            back,
            Property::data(Value::Object(function), true, false, true),
        )?;
        let key = self.held_key("prototype", at)?;
        self.objects.define(
            function,
            key,
            Property::data(Value::Object(prototype), true, false, false),
        )?;
        Ok(())
    }

    /// Intern a name and hold the string that spells it in the open scope.
    fn held_key(&mut self, name: &str, at: usize) -> Result<crate::object::Key, Escape> {
        let key = self
            .objects
            .key(&units(name))
            .map_err(|why| Escape::refused(why, at))?;
        if let Some(held) = key.reference() {
            self.objects.heap_mut().hold(held);
        }
        Ok(key)
    }

    /// The function `value` is if it has a `[[Construct]]`, and whether it is
    /// a builtin.
    fn constructor(&self, value: Value) -> Option<(Ref, bool)> {
        let Value::Object(held) = value else {
            return None;
        };
        match self.objects.callable(held)?.code() {
            Code::Compiled { unit, chunk, .. } => {
                unit.chunk(*chunk).filter(|chunk| chunk.constructs())?;
                Some((held, false))
            }
            Code::Native(native) => native.instance().map(|_| (held, true)),
        }
    }

    /// Give a builtin constructor at `callee_at` its instance, in the `this`
    /// slot above it (queue item 227).
    ///
    /// `OrdinaryCreateFromConstructor` with the constructor itself as
    /// `NewTarget`, which is the only `NewTarget` this engine can have: a
    /// different one needs `Reflect.construct` or a derived class, and those
    /// are queue items 73 and 223. A builtin constructor's `prototype` is
    /// neither writable nor configurable, so it is always the object it was
    /// made with, and anything else there is this engine's own bug — the same
    /// argument [`Engine::construct`] makes for a script's.
    pub(super) fn make_instance(
        &mut self,
        run: &mut Run,
        callee_at: usize,
        constructor: Ref,
        instance: Instance,
        at: usize,
    ) -> Result<(), Escape> {
        let key = self
            .objects
            .key(&units("prototype"))
            .map_err(|why| Escape::refused(why, at))?;
        let Found::Value(Value::Object(above)) = self.objects.get(constructor, key)? else {
            return Err(Escape::Broken(Internal::ConstructorIsWrong));
        };
        // A safepoint. The constructor is on the stack and holds `above`.
        let made = match instance {
            Instance::Error => self.objects.error(Some(above)),
            Instance::Made(make) => self.objects.foreign(make(Some(above))),
            Instance::Promise => self.objects.promise(Some(above)),
            Instance::Date => self.objects.date(Some(above)),
        }
        .map_err(|why| Escape::refused(why, at))?;
        self.write_at(run, callee_at.saturating_add(1), Value::Object(made))
    }
}

/// A name as the code units a key is made of.
fn units(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}
