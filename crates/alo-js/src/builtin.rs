/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The objects a realm has before a script has run a line (queue item 218).
//!
//! ADR 0013 § 3 — *absent beats approximate* — is why there were none of these
//! until item 218, and it is also why there are so few: `Object.prototype`,
//! `Function.prototype` and `Array.prototype`, which are not a library but
//! **what an ordinary object, an ordinary function and an array are**; the
//! seven error constructors ([`error`], queue item 227), which are what a
//! `catch` binds; and what `for…of` reads through (queue item 230) — two
//! well-known symbols, `%IteratorPrototype%` and `%ArrayIteratorPrototype%`.
//! Until the first three existed, `{}` had no prototype at all, so `({}) + ''`
//! was a `TypeError` rather than `"[object Object]"` and no page could have
//! run.
//!
//! # An intrinsic is rooted, and everything else hangs off it
//!
//! [`Intrinsics`] holds a [`Root`] for each. Every builtin method is a property
//! of one of those objects, so the collector reaches all of them from the
//! realm and none of them needs a root of its own. That is also the reason
//! they are made in the order they are: `Object.prototype` first with a null
//! prototype, then `Function.prototype` as a function *whose* prototype is
//! `Object.prototype`, then the methods on both.
//!
//! `Array.prototype` **is itself an array**, of length zero, which is the
//! specification's and is what `Object.prototype.toString` says about it
//! (queue item 225). Its only methods are the three that make an iterator
//! ([`array_prototype`], queue item 230): `[].push` is still `undefined`, which
//! a page's own feature test reads correctly, where a `push` that did half of
//! what the specification says would not be (item 73).
//!
//! # A well-known symbol is an intrinsic too
//!
//! `Symbol.iterator` is a key on `Array.prototype` and `%IteratorPrototype%`,
//! and `Symbol.toStringTag` one on `%ArrayIteratorPrototype%`, so each is made
//! once with the realm and rooted beside the objects it is a key of. No page
//! can name either yet — the `Symbol` function that carries them is item 73's —
//! and nothing here needs a page to: `for…of` reaches `Symbol.iterator` through
//! an instruction of its own.
//!
//! # What is deliberately not here
//!
//! No `Object` and no `Function` on the global object: both are constructors,
//! and `new` (queue item 212) constructs only a function a script wrote — a
//! builtin with a `[[Construct]]` is item 73's, and a constructor that cannot
//! construct is a stub.
//! `Object.prototype` is still reachable from a script — `({}).__proto__` — so
//! nothing here is untestable from the language it belongs to.
//!
//! No `Array` constructor and no array method but the three iterators, no
//! `Math`, `JSON`, `String`, `Number` or `Boolean`, no `AggregateError` (queue
//! item 229), no `Symbol` and eleven of the thirteen well-known symbols, and no
//! weak collections. Each is named in the queue rather than half-built here.

pub mod array_iterator;
pub mod array_prototype;
pub mod error;
pub mod function_prototype;
pub mod iterator_prototype;
pub mod object_prototype;

use crate::abrupt::Escape;
use crate::heap::{Ref, Root};
use crate::object::native::Body;
use crate::object::symbol::WellKnown;
use crate::object::{Fault, Found, Key, Native, Objects, Property, Value};
pub use error::Family;

/// The objects a realm owns.
#[derive(Debug)]
pub struct Intrinsics {
    /// `Object.prototype`.
    object: Root,
    /// `Function.prototype`.
    function: Root,
    /// `Array.prototype`.
    array: Root,
    /// The seven error constructors, in [`Family::ALL`]'s order. Each holds its
    /// prototype, which may be neither changed nor deleted.
    errors: Vec<Root>,
    /// The well-known symbols, in [`WellKnown::ALL`]'s order (queue item 230).
    symbols: Vec<Root>,
    /// `%IteratorPrototype%`, which every iterator the language makes inherits
    /// from.
    iterator: Root,
    /// `%ArrayIteratorPrototype%`, which an array iterator inherits from.
    array_iterator: Root,
}

impl Intrinsics {
    /// Make them, in the one order that works.
    ///
    /// **This allocates repeatedly**, and each allocation is a safepoint. The
    /// two prototypes are rooted the instant they exist, which is what keeps
    /// the second from collecting the first.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] for a heap that was full before anything ran, and a
    /// fault for a root this engine has lost.
    pub fn new(objects: &mut Objects) -> Result<Self, Escape> {
        // `Object.prototype` has no prototype, and that is the end of every
        // chain in the heap rather than an omission.
        let object_prototype = objects
            .object(None)
            .map_err(|why| Escape::refused(why, 0))?;
        let object_prototype = objects.heap_mut().root(object_prototype);

        let above = objects
            .heap()
            .holding(&object_prototype)
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        // `Function.prototype` is itself callable and answers `undefined` for
        // any arguments, which is the specification's own description of it.
        let function_prototype = objects
            .native(Native::new("", function_prototype::nothing), Some(above))
            .map_err(|why| Escape::refused(why, 0))?;
        let function_prototype = objects.heap_mut().root(function_prototype);

        // `Array.prototype` is an array of length zero inheriting from
        // `Object.prototype`, which is still rooted above.
        let array_prototype = objects
            .array(Some(above), 0)
            .map_err(|why| Escape::refused(why, 0))?;
        let array_prototype = objects.heap_mut().root(array_prototype);

        let symbols = well_known(objects)?;

        // `%IteratorPrototype%` inherits from `Object.prototype`, and
        // `%ArrayIteratorPrototype%` from it — each rooted the instant it
        // exists, as the three above are.
        let iterator = objects
            .object(Some(above))
            .map_err(|why| Escape::refused(why, 0))?;
        let iterator = objects.heap_mut().root(iterator);
        let iterators = objects
            .heap()
            .holding(&iterator)
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        let array_iterator = objects
            .object(Some(iterators))
            .map_err(|why| Escape::refused(why, 0))?;
        let array_iterator = objects.heap_mut().root(array_iterator);

        let functions = objects
            .heap()
            .holding(&function_prototype)
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        let errors = error::make(objects, above, functions)?;

        let intrinsics = Self {
            object: object_prototype,
            function: function_prototype,
            array: array_prototype,
            errors,
            symbols,
            iterator,
            array_iterator,
        };
        object_prototype::furnish(objects, &intrinsics)?;
        function_prototype::furnish(objects, &intrinsics)?;
        iterator_prototype::furnish(objects, &intrinsics)?;
        array_iterator::furnish(objects, &intrinsics)?;
        array_prototype::furnish(objects, &intrinsics)?;
        Ok(intrinsics)
    }

    /// `Object.prototype` — what every object literal inherits from.
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn object_prototype(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.object)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// `Array.prototype` — what every array literal inherits from (queue item
    /// 225).
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn array_prototype(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.array)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// The constructor of one family of errors, which the realm binds to its
    /// name and a `catch` makes an error this engine threw from
    /// (queue item 227).
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn error_constructor(&self, objects: &Objects, family: Family) -> Result<Ref, Escape> {
        self.errors
            .get(family.index())
            .and_then(|root| objects.heap().holding(root))
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// `Function.prototype` — what every function inherits from.
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn function_prototype(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.function)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// A well-known symbol: the one this realm made, which is the only one
    /// there is (queue item 230).
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn well_known(&self, objects: &Objects, which: WellKnown) -> Result<Ref, Escape> {
        self.symbols
            .get(which.index())
            .and_then(|root| objects.heap().holding(root))
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// The key a well-known symbol is.
    ///
    /// # Errors
    ///
    /// The same as [`Intrinsics::well_known`].
    pub fn well_known_key(&self, objects: &Objects, which: WellKnown) -> Result<Key, Escape> {
        let held = self.well_known(objects, which)?;
        Ok(objects.symbol_key(held)?)
    }

    /// `%IteratorPrototype%` (queue item 230).
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn iterator_prototype(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.iterator)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// `%ArrayIteratorPrototype%` (queue item 230).
    ///
    /// # Errors
    ///
    /// A fault if this engine has lost the root, which is its own bug.
    pub fn array_iterator_prototype(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.array_iterator)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }
}

/// Make the well-known symbols, each rooted the instant it exists.
///
/// The description is made first and held in a scope across the symbol's own
/// allocation, which is the one place between them a collection may run.
fn well_known(objects: &mut Objects) -> Result<Vec<Root>, Escape> {
    let mut symbols = Vec::with_capacity(WellKnown::ALL.len());
    for which in WellKnown::ALL {
        let scope = objects.heap_mut().open();
        let made = described(objects, which.description());
        objects.heap_mut().close(scope);
        symbols.push(objects.heap_mut().root(made?));
    }
    Ok(symbols)
}

/// A symbol with this description, with a scope open.
fn described(objects: &mut Objects, description: &str) -> Result<Ref, Escape> {
    let text = objects
        .text(description.encode_utf16().collect())
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(text);
    objects
        .symbol(Some(text))
        .map_err(|why| Escape::refused(why, 0))
}

/// Put a builtin method on an object.
///
/// The attributes are the specification's for every method of every prototype:
/// writable and configurable so a page may replace one, and **not enumerable**
/// so `for (const k in {})` lists nothing — which is the difference between a
/// prototype a page can work with and one that shows up in every loop.
///
/// # The scope is the point of the function
///
/// Interning the name allocates and making the function allocates, so between
/// the two there is a reference only a Rust local is holding — exactly the bug
/// ADR 0014 § 2 says is invisible in an ordinary run. Both are held in a
/// [`Scope`](crate::heap::Scope) until the property owns them.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference that
/// does not name an object.
pub(crate) fn method(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = defined(objects, on, function_prototype, name, body);
    objects.heap_mut().close(scope);
    outcome
}

/// [`method`], with the scope already open.
fn defined(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = objects.key(&units).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = key.reference() {
        objects.heap_mut().hold(held);
    }
    let function = objects
        .native(Native::new(name, body), Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(function);
    objects.define(
        on,
        key,
        Property::data(Value::Object(function), true, false, true),
    )?;
    Ok(())
}

/// Put a builtin method on an object under a well-known symbol:
/// `Array.prototype[Symbol.iterator]` is the shape (queue item 230).
///
/// The attributes are [`method`]'s. The key needs no scope — the symbol is an
/// intrinsic and rooted — so the function is the one thing held across the
/// definition.
///
/// # Errors
///
/// The same as [`method`].
pub(crate) fn symbol_method(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    key: Key,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = symbol_defined(objects, on, function_prototype, key, name, body);
    objects.heap_mut().close(scope);
    outcome
}

/// [`symbol_method`], with the scope already open.
fn symbol_defined(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    key: Key,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let function = objects
        .native(Native::new(name, body), Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(function);
    objects.define(
        on,
        key,
        Property::data(Value::Object(function), true, false, true),
    )?;
    Ok(())
}

/// Give a method an object already has a second name: the **same** function
/// object under `key`, as the specification makes `Array.prototype[
/// Symbol.iterator]` the very function `Array.prototype.values` is (queue item
/// 230). Nothing is allocated, so nothing needs holding.
///
/// # Errors
///
/// A fault for a method that is not there, which is this engine's own bug.
pub(crate) fn alias(
    objects: &mut Objects,
    on: Ref,
    name: &'static str,
    key: Key,
) -> Result<(), Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let Some(existing) = objects.existing_key(&units) else {
        return Err(Escape::fault(Fault::Gone));
    };
    let Found::Value(function) = objects.get(on, existing)? else {
        return Err(Escape::fault(Fault::Gone));
    };
    objects.define(on, key, Property::data(function, true, false, true))?;
    Ok(())
}

/// Put an accessor whose halves are builtins on an object.
///
/// `__proto__` is the only one of these, and it is an accessor rather than a
/// data property because reading it and writing it are two different
/// operations on the same name.
///
/// # Errors
///
/// The same as [`method`].
pub(crate) fn accessor(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    name: &'static str,
    get: Body,
    set: Body,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = accessed(objects, on, function_prototype, name, get, set);
    objects.heap_mut().close(scope);
    outcome
}

/// [`accessor`], with the scope already open.
fn accessed(
    objects: &mut Objects,
    on: Ref,
    function_prototype: Ref,
    name: &'static str,
    get: Body,
    set: Body,
) -> Result<(), Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = objects.key(&units).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = key.reference() {
        objects.heap_mut().hold(held);
    }
    let getter = objects
        .native(Native::new(name, get), Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(getter);
    let setter = objects
        .native(Native::new(name, set), Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(setter);
    objects.define(
        on,
        key,
        Property::accessor(Value::Object(getter), Value::Object(setter), false, true),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Intrinsics;
    use crate::object::{Found, Objects, Value};

    #[test]
    fn function_prototype_is_a_function_whose_prototype_is_object_prototype() {
        let mut objects = Objects::new();
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("an empty heap holds two intrinsics");
        };
        let Ok(object_prototype) = intrinsics.object_prototype(&objects) else {
            panic!("it is rooted");
        };
        let Ok(function_prototype) = intrinsics.function_prototype(&objects) else {
            panic!("so is it");
        };
        assert!(
            objects.callable(function_prototype).is_some(),
            "Function.prototype is itself a function"
        );
        assert_eq!(
            objects.prototype(function_prototype),
            Ok(Some(object_prototype))
        );
        assert_eq!(
            objects.prototype(object_prototype),
            Ok(None),
            "and Object.prototype ends every chain"
        );
    }

    #[test]
    fn the_intrinsics_are_built_with_the_collector_running_at_every_allocation() {
        // The suite's other files turn stress on *after* the engine exists, so
        // nothing there covers this: making the intrinsics is a dozen
        // allocations with a name interned between each pair, and the interned
        // string is held only by a weak table until the property owns it. Every
        // one of those is a reference in a Rust local across a safepoint unless
        // the scope in [`method`] is holding it.
        let mut objects = Objects::new();
        objects.heap_mut().stress(true);
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("an empty heap holds two intrinsics however often it collects");
        };
        objects.heap_mut().stress(false);
        assert_eq!(
            objects.heap().scoped(),
            0,
            "every scope opened while building them was closed"
        );

        let Ok(object_prototype) = intrinsics.object_prototype(&objects) else {
            panic!("it is rooted");
        };
        // Each method is still there, and each still names a live function.
        for name in [
            "toString",
            "valueOf",
            "hasOwnProperty",
            "isPrototypeOf",
            "propertyIsEnumerable",
        ] {
            let units: Vec<u16> = name.encode_utf16().collect();
            let Some(key) = objects.existing_key(&units) else {
                panic!("{name} was interned when it was defined");
            };
            let Ok(Found::Value(Value::Object(held))) = objects.get(object_prototype, key) else {
                panic!("{name} survived a collection at every allocation");
            };
            assert!(objects.callable(held).is_some(), "{name} is a function");
        }
        let units: Vec<u16> = "__proto__".encode_utf16().collect();
        let Some(key) = objects.existing_key(&units) else {
            panic!("__proto__ was interned when it was defined");
        };
        let Ok(Some(property)) = objects.own_property(object_prototype, key) else {
            panic!("and the accessor survived too");
        };
        assert!(matches!(property.getter(), Some(Value::Object(_))));
        assert!(matches!(property.setter(), Some(Value::Object(_))));
    }

    #[test]
    fn a_builtin_method_is_writable_and_configurable_and_never_enumerable() {
        let mut objects = Objects::new();
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("an empty heap holds two intrinsics");
        };
        let Ok(object_prototype) = intrinsics.object_prototype(&objects) else {
            panic!("it is rooted");
        };
        let name: Vec<u16> = "toString".encode_utf16().collect();
        let Some(key) = objects.existing_key(&name) else {
            panic!("the name was interned when the method was defined");
        };
        let Ok(Some(property)) = objects.own_property(object_prototype, key) else {
            panic!("Object.prototype has a toString");
        };
        assert!(property.is_writable());
        assert!(property.is_configurable());
        assert!(
            !property.is_enumerable(),
            "a for-in over an empty object must list nothing"
        );
        let Ok(Found::Value(Value::Object(held))) = objects.get(object_prototype, key) else {
            panic!("and it is a function");
        };
        assert!(objects.callable(held).is_some());
    }
}
