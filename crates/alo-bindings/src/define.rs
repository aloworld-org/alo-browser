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
//! Both allocate — the name is interned, the functions are made — so every
//! one of them is held in a scope until the prototype owns it, and the
//! prototype itself must be held by the caller.

use alo_js::Escape;
use alo_js::heap::Ref;
use alo_js::object::native::{Body, Native};
use alo_js::object::{Key, Objects, Property, Value};

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
    let outcome = held_attribute(objects, prototype, function_prototype, name, get, set);
    objects.heap_mut().close(scope);
    outcome
}

/// [`attribute`], with the scope open.
fn held_attribute(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
    name: &'static str,
    get: Body,
    set: Option<Body>,
) -> Result<(), Escape> {
    let key = held_key(objects, name)?;
    let getter = held_native(objects, function_prototype, name, get)?;
    let setter = match set {
        Some(body) => Value::Object(held_native(objects, function_prototype, name, body)?),
        None => Value::Undefined,
    };
    let property = Property::accessor(Value::Object(getter), setter, true, true);
    defined(objects, prototype, key, property)
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
