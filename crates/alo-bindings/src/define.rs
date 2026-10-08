/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a member goes on an interface's prototype, as Web IDL says it does.
//!
//! An **operation** is a data property holding a native function — writable,
//! enumerable and configurable. An **attribute** is an accessor property
//! whose getter is a native and whose setter is one too, or `undefined` for
//! a read-only attribute — enumerable and configurable. That is all a member
//! is to the object model; what it *does* is its body, in its interface's
//! file.
//!
//! A `[LegacyUnforgeable]` attribute is the same accessor **not
//! configurable**, and goes on an interface's unforgeables object rather than
//! its prototype, to be copied onto each instance (ADR 0019 § 3).
//!
//! Both allocate — the name is interned, the functions are made — so every
//! one of them is held in a scope until the prototype owns it, and the
//! prototype itself must be held by the caller.
//!
//! An interface that is **iterable with an indexed getter** — a value
//! iterator over its own indices — makes none of its iteration members: Web
//! IDL gives it `Array.prototype`'s own `forEach`, `keys`, `values`,
//! `entries` and `[Symbol.iterator]`, the very functions, so a page can
//! compare them with `===` ([`array_iteration`], queue item 331).

use alo_js::Escape;
use alo_js::heap::Ref;
use alo_js::object::native::{Body, Native};
use alo_js::object::{Code, Found, Key, Objects, Property, Value};

/// Put the operation `name`, whose body is `body`, on `prototype`.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold it, and a fault for a
/// reference this engine has lost.
pub(crate) fn operation(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = held_operation(objects, prototype, function_prototype, name, body);
    objects.heap_mut().close(scope);
    outcome
}

/// [`operation`], with the scope open.
fn held_operation(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
    name: &'static str,
    body: Body,
) -> Result<(), Escape> {
    let key = held_key(objects, name)?;
    let function = held_native(objects, function_prototype, name, body)?;
    let property = Property::data(Value::Object(function), true, true, true);
    defined(objects, prototype, key, property)
}

/// Put the attribute `name` on `prototype`: read by `get`, and written by
/// `set` or, without one, read-only.
///
/// # Errors
///
/// As [`operation`].
pub(crate) fn attribute(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
    name: &'static str,
    get: Body,
    set: Option<Body>,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = held_attribute(
        objects,
        prototype,
        function_prototype,
        name,
        (get, set),
        Configurable::Yes,
    );
    objects.heap_mut().close(scope);
    outcome
}

/// Put the read-only `[LegacyUnforgeable]` attribute `name`, read by `get`,
/// on an interface's `unforgeables` object: enumerable and **not
/// configurable**, so that once copied onto an instance no page can delete
/// or redefine it.
///
/// # Errors
///
/// As [`operation`].
pub(crate) fn unforgeable_attribute(
    objects: &mut Objects,
    unforgeables: Ref,
    function_prototype: Ref,
    name: &'static str,
    get: Body,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = held_attribute(
        objects,
        unforgeables,
        function_prototype,
        name,
        (get, None),
        Configurable::No,
    );
    objects.heap_mut().close(scope);
    outcome
}

/// The members Web IDL takes from `Array.prototype` for an interface with an
/// indexed getter and a value iterator, in the order it defines them, and
/// the name the realm's own function has under each.
const FROM_ARRAY: [&str; 4] = ["entries", "keys", "values", "forEach"];

/// Put `Array.prototype`'s `entries`, `keys`, `values` and `forEach` on
/// `prototype` — writable, enumerable and configurable, as an operation is —
/// and its `values` again under `iterator`, `Symbol.iterator`, not
/// enumerable (Web IDL, *iterable declarations*; queue item 331).
///
/// The functions are **the realm's own**: each is checked to be the
/// engine's native of that name rather than whatever `array_prototype`
/// holds now, so a prototype furnished after a page had replaced one would
/// be refused rather than handed the page's function. Nothing is allocated.
///
/// # Errors
///
/// A fault for an `Array.prototype` that does not hold the realm's
/// functions, which is an embedder furnishing a realm a script has already
/// changed, or this engine's bug.
pub(crate) fn array_iteration(
    objects: &mut Objects,
    prototype: Ref,
    array_prototype: Ref,
    iterator: Key,
) -> Result<(), Escape> {
    for name in FROM_ARRAY {
        let units: Vec<u16> = name.encode_utf16().collect();
        let key = objects
            .existing_key(&units)
            .ok_or(Escape::fault(alo_js::Fault::Gone))?;
        let function = realms_own(objects, array_prototype, key, name)?;
        let property = Property::data(Value::Object(function), true, true, true);
        defined(objects, prototype, key, property)?;
    }
    let values = realms_own(objects, array_prototype, iterator, "values")?;
    let property = Property::data(Value::Object(values), true, false, true);
    defined(objects, prototype, iterator, property)
}

/// The function `array_prototype` holds under `key`, if it is the engine's
/// own native called `name`.
fn realms_own(
    objects: &Objects,
    array_prototype: Ref,
    key: Key,
    name: &str,
) -> Result<Ref, Escape> {
    let Found::Value(Value::Object(function)) = objects.get(array_prototype, key)? else {
        return Err(Escape::fault(alo_js::Fault::Gone));
    };
    match objects
        .callable(function)
        .map(alo_js::object::Function::code)
    {
        Some(Code::Native(native)) if native.name() == name => Ok(function),
        _ => Err(Escape::fault(alo_js::Fault::Gone)),
    }
}

/// Whether an attribute may be deleted or redefined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Configurable {
    /// An ordinary attribute.
    Yes,
    /// A `[LegacyUnforgeable]` one.
    No,
}

/// [`attribute`] or [`unforgeable_attribute`], with the scope open.
fn held_attribute(
    objects: &mut Objects,
    object: Ref,
    function_prototype: Ref,
    name: &'static str,
    (get, set): (Body, Option<Body>),
    configurable: Configurable,
) -> Result<(), Escape> {
    let key = held_key(objects, name)?;
    let getter = held_native(objects, function_prototype, name, get)?;
    let setter = match set {
        Some(body) => Value::Object(held_native(objects, function_prototype, name, body)?),
        None => Value::Undefined,
    };
    let property = Property::accessor(
        Value::Object(getter),
        setter,
        true,
        configurable == Configurable::Yes,
    );
    defined(objects, object, key, property)
}

/// Intern `name` and hold the string that spells it in the open scope.
fn held_key(objects: &mut Objects, name: &str) -> Result<Key, Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = objects.key(&units).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = key.reference() {
        objects.heap_mut().hold(held);
    }
    Ok(key)
}

/// Make a native and hold it in the open scope.
fn held_native(
    objects: &mut Objects,
    function_prototype: Ref,
    name: &'static str,
    body: Body,
) -> Result<Ref, Escape> {
    let function = objects
        .native(Native::new(name, body), Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(function);
    Ok(function)
}

/// Define the property, which a fresh prototype never refuses.
fn defined(
    objects: &mut Objects,
    prototype: Ref,
    key: Key,
    property: Property,
) -> Result<(), Escape> {
    if objects.define(prototype, key, property)? {
        Ok(())
    } else {
        // A fresh, extensible prototype refusing a new configurable property
        // is a reference to something else: this crate's bug, not a page's.
        Err(Escape::fault(alo_js::Fault::NotAnObject))
    }
}
