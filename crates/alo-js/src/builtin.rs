/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The objects a realm has before a script has run a line (queue item 218).
//!
//! ADR 0013 § 3 — *absent beats approximate* — is why there were none of these
//! until now, and it is also why there are exactly two: `Object.prototype` and
//! `Function.prototype`. Those two are not a library, they are **what an
//! ordinary object and an ordinary function are**. Until they existed, `{}` had
//! no prototype at all, so `({}) + ''` was a `TypeError` rather than
//! `"[object Object]"` and no page could have run.
//!
//! # An intrinsic is rooted, and everything else hangs off it
//!
//! [`Intrinsics`] holds two [`Root`]s. Every builtin method is a property of
//! one of those two objects, so the collector reaches all of them from the
//! realm and none of them needs a root of its own. That is also the reason the
//! two are made in the order they are: `Object.prototype` first with a null
//! prototype, then `Function.prototype` as a function *whose* prototype is
//! `Object.prototype`, then the methods on both.
//!
//! # What is deliberately not here
//!
//! No `Object` and no `Function` on the global object: both are constructors,
//! `new` is queue item 212, and a constructor that cannot construct is a stub.
//! `Object.prototype` is still reachable from a script — `({}).__proto__` — so
//! nothing here is untestable from the language it belongs to.
//!
//! No `Array`, `Math`, `JSON`, `Error`, `String`, `Number` or `Boolean`, no
//! well-known symbols and no weak collections. Each is named in the queue
//! rather than half-built here.

pub mod function_prototype;
pub mod object_prototype;

use crate::abrupt::Escape;
use crate::heap::{Ref, Root};
use crate::object::native::Body;
use crate::object::{Fault, Native, Objects, Property, Value};

/// The objects a realm owns.
#[derive(Debug)]
pub struct Intrinsics {
    object_prototype: Root,
    function_prototype: Root,
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

        let intrinsics = Self {
            object_prototype,
            function_prototype,
        };
        object_prototype::furnish(objects, &intrinsics)?;
        function_prototype::furnish(objects, &intrinsics)?;
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
            .holding(&self.object_prototype)
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
            .holding(&self.function_prototype)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }
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
