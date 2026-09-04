/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Object.prototype`: what every object in the language can do (item 218).
//!
//! Five methods and one accessor, and between them they are why `{}` is a
//! usable value at all. `({}) + ''` is the one that matters most and it reaches
//! three separate mechanisms — the operator asks for a primitive, the search
//! finds `toString` on this object, and the interpreter calls a **builtin** —
//! so it is the case that says this item works.
//!
//! # `ToObject` on `this` is where each of these begins
//!
//! Every method here is specified as `O = ToObject(this)`, and a builtin is
//! strict code, so `this` arrives exactly as the caller wrote it. Three answers
//! rather than one, and they go to three different people: an object is itself,
//! `undefined` and `null` are the `TypeError` the language specifies, and a
//! primitive needs a wrapper object this engine has not built —
//! [`Missing::AWrapperObject`], which no page can catch, because a page acting
//! on it would be acting on a lie.
//!
//! # What is absent, and where each one is
//!
//! `toLocaleString` calls `this.toString()`, which is a builtin re-entering the
//! script (queue item 219). `Symbol.toStringTag`, which `toString` consults
//! before anything else, needs the well-known symbols (queue item 73). And
//! `toString`'s builtin tags for an array, an error, a date and the three
//! wrapper kinds each need that builtin to exist; until then every object that
//! is not a function is `"[object Object]"`, which is what it genuinely is.

use crate::abrupt::{Escape, Missing};
use crate::convert::{self, Primitive};
use crate::heap::Ref;
use crate::object::native::Call;
use crate::object::{Key, Property, Value};

use super::Intrinsics;

/// Put the methods on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(
    objects: &mut crate::object::Objects,
    intrinsics: &Intrinsics,
) -> Result<(), Escape> {
    let on = intrinsics.object_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    super::method(objects, on, functions, "toString", to_string)?;
    super::method(objects, on, functions, "valueOf", value_of)?;
    super::method(objects, on, functions, "hasOwnProperty", has_own_property)?;
    super::method(objects, on, functions, "isPrototypeOf", is_prototype_of)?;
    super::method(
        objects,
        on,
        functions,
        "propertyIsEnumerable",
        property_is_enumerable,
    )?;
    super::accessor(objects, on, functions, "__proto__", proto_get, proto_set)?;
    Ok(())
}

/// `Object.prototype.toString`.
///
/// `undefined` and `null` are answered *before* `ToObject`, which is the one
/// place in this file where a primitive `this` is not an error: the two values
/// that have no wrapper are the two the specification names outright.
fn to_string(call: &mut Call<'_>) -> Result<Value, Escape> {
    let tag = match call.this() {
        Value::Undefined => "[object Undefined]",
        Value::Null => "[object Null]",
        Value::Object(held) => {
            if call.seen().callable(held).is_some() {
                "[object Function]"
            } else {
                "[object Object]"
            }
        }
        // A wrapper's tag is `"[object String]"` and the rest, which needs the
        // wrapper first.
        Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
            return Err(Escape::NotBuiltYet(Missing::AWrapperObject));
        }
    };
    let at = call.at();
    let units: Vec<u16> = tag.encode_utf16().collect();
    let held = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Value::Text(held))
}

/// `Object.prototype.valueOf`, which is `ToObject(this)` and nothing else.
///
/// It is the reason `1 + {}` is `"1[object Object]"` rather than a `TypeError`:
/// the conversion asks for `valueOf` first, this hands back the object it was
/// given, and the search moves on to `toString`.
fn value_of(call: &mut Call<'_>) -> Result<Value, Escape> {
    let held = object_of(call, "valueOf")?;
    Ok(Value::Object(held))
}

/// `Object.prototype.hasOwnProperty`.
fn has_own_property(call: &mut Call<'_>) -> Result<Value, Escape> {
    // The specification converts the key before it touches `this`, so a bad key
    // is answered even when `this` is `null`.
    let key = key_of(call, 0)?;
    let held = object_of(call, "hasOwnProperty")?;
    let there = call.seen().own_property(held, key)?.is_some();
    Ok(Value::Bool(there))
}

/// `Object.prototype.isPrototypeOf`.
///
/// The walk starts at the **prototype** of what it was given, which is what
/// makes `a.isPrototypeOf(a)` false. One object is not its own prototype, and
/// the walk that says so is [`Objects::reaches`](crate::object::Objects::reaches)
/// — the same one a prototype assignment uses to refuse a cycle, so the bound
/// on a chain is stated once.
fn is_prototype_of(call: &mut Call<'_>) -> Result<Value, Escape> {
    let Value::Object(other) = call.argument(0) else {
        // Not an object, so nothing is its prototype. Answered before `this` is
        // looked at, which is the specification's order.
        return Ok(Value::Bool(false));
    };
    let held = object_of(call, "isPrototypeOf")?;
    let above = call.seen().prototype(other)?;
    Ok(Value::Bool(call.seen().reaches(above, held)?))
}

/// `Object.prototype.propertyIsEnumerable`.
fn property_is_enumerable(call: &mut Call<'_>) -> Result<Value, Escape> {
    let key = key_of(call, 0)?;
    let held = object_of(call, "propertyIsEnumerable")?;
    let enumerable = call
        .seen()
        .own_property(held, key)?
        .is_some_and(Property::is_enumerable);
    Ok(Value::Bool(enumerable))
}

/// Reading `__proto__`.
fn proto_get(call: &mut Call<'_>) -> Result<Value, Escape> {
    let held = object_of(call, "__proto__")?;
    Ok(match call.seen().prototype(held)? {
        Some(above) => Value::Object(above),
        None => Value::Null,
    })
}

/// Writing `__proto__`.
///
/// Three of its four answers are **silence**, and that is the specification's
/// own shape rather than laxity here: a `this` that is not an object and a
/// value that is neither an object nor `null` are both ignored, because the
/// name is an accessor a page may reach on any value. Only a refusal by the
/// object model — a cycle, or an object that is not extensible — is an error,
/// and it is the error that says a page's own `Object.freeze` held.
fn proto_set(call: &mut Call<'_>) -> Result<Value, Escape> {
    let at = call.at();
    // `RequireObjectCoercible` first: `undefined` and `null` throw even though
    // every other non-object is quietly nothing.
    if matches!(call.this(), Value::Undefined | Value::Null) {
        return Err(Escape::type_error(
            "cannot set __proto__ of undefined or null",
            at,
        ));
    }
    let Value::Object(held) = call.this() else {
        return Ok(Value::Undefined);
    };
    let to = match call.argument(0) {
        Value::Object(above) => Some(above),
        Value::Null => None,
        _ => return Ok(Value::Undefined),
    };
    if call.objects().set_prototype(held, to)? {
        return Ok(Value::Undefined);
    }
    Err(Escape::type_error(
        "this object's prototype cannot be changed: it is not extensible, or the new prototype is already below it",
        at,
    ))
}

/// `ToObject(this)`, as the three answers it really has.
///
/// # Errors
///
/// The `TypeError` the language specifies for `undefined` and `null`, and
/// [`Missing::AWrapperObject`] for every other primitive.
fn object_of(call: &Call<'_>, method: &str) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) => Ok(held),
        Value::Undefined | Value::Null => Err(Escape::type_error(
            format!("Object.prototype.{method} was called on undefined or null"),
            call.at(),
        )),
        Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
            Err(Escape::NotBuiltYet(Missing::AWrapperObject))
        }
    }
}

/// `ToPropertyKey` of an argument.
///
/// An object argument is where a builtin would have to call the script's own
/// `valueOf`, which it cannot until queue item 219 — so that one case says so
/// and every other one is converted here. `hasOwnProperty()` with no argument
/// asks about the property named `"undefined"`, which is the answer the
/// language gives rather than a refusal.
fn key_of(call: &mut Call<'_>, which: usize) -> Result<Key, Escape> {
    let at = call.at();
    let Some(primitive) = Primitive::of(call.argument(which)) else {
        return Err(Escape::NotBuiltYet(Missing::AConversionInsideABuiltin));
    };
    convert::to_property_key(call.objects(), primitive, at)
}
