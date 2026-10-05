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
//! # Two of these ask the interpreter for something (queue item 219)
//!
//! `toLocaleString` is specified as `Invoke(O, "toString")` — a property read
//! that may be an accessor, and then a call — so it is three steps rather than
//! one. And `hasOwnProperty` and `propertyIsEnumerable` both begin with
//! `ToPropertyKey`, which for an **object** argument means running the script's
//! own `valueOf`: `({}).hasOwnProperty({})` was a refusal by name until this
//! item and is an ordinary `false` now.
//!
//! Both are written the same way, because the mechanism is the same one: return
//! [`Answer::Want`], name the step to come back at, and read [`Call::answer`]
//! there.
//!
//! # What is absent, and where each one is
//!
//! `Symbol.toStringTag`, which `toString` consults before anything else, needs
//! the well-known symbols (queue item 73). An array is `"[object Array]"`
//! (queue item 225) and an error `"[object Error]"` (item 227); the builtin
//! tags for a date and the three
//! wrapper kinds each need that builtin to exist, and until then every other
//! object that is not a function is `"[object Object]"`, which is what it
//! genuinely is.

use crate::abrupt::{Escape, Internal, Missing};
use crate::convert::{self, Hint, Primitive};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Want};
use crate::object::{Found, Key, Property, Value};

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
    super::method(objects, on, functions, "toLocaleString", to_locale_string)?;
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
fn to_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let tag = match call.this() {
        Value::Undefined => "[object Undefined]",
        Value::Null => "[object Null]",
        Value::Object(held) => {
            if call.seen().as_array(held).is_some() {
                "[object Array]"
            } else if call.seen().is_error(held) {
                "[object Error]"
            } else if call.seen().callable(held).is_some() {
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
    Ok(Answer::Value(Value::Text(held)))
}

/// `Object.prototype.toLocaleString`, which is `Invoke(this, "toString")`.
///
/// It exists so that a page may override `toLocaleString` on one kind of object
/// and have every other kind still answer something, and it is the *shape* that
/// matters here: a builtin that looks a method up on its own `this` and calls
/// it. `Array.prototype.join` and `toString` are the same shape, and so is
/// every method that takes a callback.
///
/// # It is three steps because a property read can be a call
///
/// `Invoke` is `GetV` and then `Call`, and `GetV` may find an accessor — so
/// finding the method is itself a call, exactly as it is in
/// [`convert::primitive_of`]. Step 0 looks, step 1 has what a getter answered
/// and calls it, step 2 has what the method answered and is done.
///
/// There is no `ToObject` here and that is the specification rather than a gap:
/// `toLocaleString` is written on the value, not on an object made from it,
/// which is why a primitive `this` reaches [`Missing::AWrapperObject`] through
/// the property read rather than before it.
fn to_locale_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let (callee, step) = match call.step() {
        // What a getter answered *is* the method, and a method it is not is the
        // `TypeError` `Invoke` gives for anything uncallable.
        1 => (call.answer()?, 2),
        2 => return Ok(Answer::Value(call.answer()?)),
        _ => {
            let held = object_of(call, "toLocaleString")?;
            let units: Vec<u16> = "toString".encode_utf16().collect();
            let key = call
                .objects()
                .key(&units)
                .map_err(|why| Escape::refused(why, at))?;
            match call.seen().get(held, key)? {
                // Not there, or an accessor with no getter, which reads as
                // `undefined` and so is not callable either.
                Found::Missing | Found::Getter(Value::Undefined) => (Value::Undefined, 2),
                Found::Value(method) => (method, 2),
                // Fetching the method before calling it, so the answer of this
                // call is the method rather than the result.
                Found::Getter(getter) => (getter, 1),
            }
        }
    };
    if !callable(call, callee) {
        return Err(Escape::type_error(
            "toLocaleString needs a toString to call, and this object's is not a function",
            at,
        ));
    }
    let receiver = call.this();
    Ok(Answer::want(
        Want::Call {
            callee,
            receiver,
            arguments: Vec::new(),
        },
        step,
    ))
}

/// `Object.prototype.valueOf`, which is `ToObject(this)` and nothing else.
///
/// It is the reason `1 + {}` is `"1[object Object]"` rather than a `TypeError`:
/// the conversion asks for `valueOf` first, this hands back the object it was
/// given, and the search moves on to `toString`.
fn value_of(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let held = object_of(call, "valueOf")?;
    Ok(Answer::Value(Value::Object(held)))
}

/// `Object.prototype.hasOwnProperty`.
fn has_own_property(call: &mut Call<'_>) -> Result<Answer, Escape> {
    // The specification converts the key before it touches `this`, so a bad key
    // is answered even when `this` is `null`.
    let key = match key_of(call, 0)? {
        Keyed::Wanting(answer) => return Ok(answer),
        Keyed::Key(key) => key,
    };
    let held = object_of(call, "hasOwnProperty")?;
    let there = call.seen().own_property(held, key)?.is_some();
    Ok(Answer::Value(Value::Bool(there)))
}

/// `Object.prototype.isPrototypeOf`.
///
/// The walk starts at the **prototype** of what it was given, which is what
/// makes `a.isPrototypeOf(a)` false. One object is not its own prototype, and
/// the walk that says so is [`Objects::reaches`](crate::object::Objects::reaches)
/// — the same one a prototype assignment uses to refuse a cycle, so the bound
/// on a chain is stated once.
fn is_prototype_of(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let Value::Object(other) = call.argument(0) else {
        // Not an object, so nothing is its prototype. Answered before `this` is
        // looked at, which is the specification's order.
        return Ok(Answer::Value(Value::Bool(false)));
    };
    let held = object_of(call, "isPrototypeOf")?;
    let above = call.seen().prototype(other)?;
    Ok(Answer::Value(Value::Bool(
        call.seen().reaches(above, held)?,
    )))
}

/// `Object.prototype.propertyIsEnumerable`.
fn property_is_enumerable(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let key = match key_of(call, 0)? {
        Keyed::Wanting(answer) => return Ok(answer),
        Keyed::Key(key) => key,
    };
    let held = object_of(call, "propertyIsEnumerable")?;
    let enumerable = call
        .seen()
        .own_property(held, key)?
        .is_some_and(Property::is_enumerable);
    Ok(Answer::Value(Value::Bool(enumerable)))
}

/// Reading `__proto__`.
fn proto_get(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let held = object_of(call, "__proto__")?;
    Ok(Answer::Value(match call.seen().prototype(held)? {
        Some(above) => Value::Object(above),
        None => Value::Null,
    }))
}

/// Writing `__proto__`.
///
/// Three of its four answers are **silence**, and that is the specification's
/// own shape rather than laxity here: a `this` that is not an object and a
/// value that is neither an object nor `null` are both ignored, because the
/// name is an accessor a page may reach on any value. Only a refusal by the
/// object model — a cycle, or an object that is not extensible — is an error,
/// and it is the error that says a page's own `Object.freeze` held.
fn proto_set(call: &mut Call<'_>) -> Result<Answer, Escape> {
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
        return Ok(Answer::Value(Value::Undefined));
    };
    let to = match call.argument(0) {
        Value::Object(above) => Some(above),
        Value::Null => None,
        _ => return Ok(Answer::Value(Value::Undefined)),
    };
    if call.objects().set_prototype(held, to)? {
        return Ok(Answer::Value(Value::Undefined));
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

/// What [`key_of`] has: a key, or the conversion that has to happen first.
enum Keyed {
    /// The key.
    Key(Key),
    /// The object needs the script's own `valueOf` run over it, which is what
    /// this asks for. The caller returns it unchanged and is entered again at
    /// step 1 with the primitive as its answer.
    Wanting(Answer),
}

/// `ToPropertyKey` of an argument, at step 0 or of what step 1 was answered.
///
/// An **object** argument is the case this needed queue item 219 for:
/// `ToPropertyKey` begins with `ToPrimitive`, which means calling the script's
/// own `valueOf` or `toString`. That algorithm is
/// [`convert::primitive_of`]'s and the interpreter drives it, so what is asked
/// for here is the conversion rather than either call — a builtin that spelled
/// out the search would be a second copy of a rule that has to agree with the
/// first.
///
/// `hasOwnProperty()` with no argument asks about the property named
/// `"undefined"`, which is the answer the language gives rather than a refusal.
fn key_of(call: &mut Call<'_>, which: usize) -> Result<Keyed, Escape> {
    let at = call.at();
    let given = if call.step() == 0 {
        call.argument(which)
    } else {
        call.answer()?
    };
    let Some(primitive) = Primitive::of(given) else {
        if call.step() != 0 {
            // A conversion answers a primitive or throws, so an object here is
            // this engine having resumed the wrong builtin.
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        }
        return Ok(Keyed::Wanting(Answer::want(
            Want::Primitive {
                of: given,
                // `ToPropertyKey` wants a string, and the hint decides which of
                // `valueOf` and `toString` is tried first.
                hint: Hint::String,
            },
            1,
        )));
    };
    Ok(Keyed::Key(convert::to_property_key(
        call.objects(),
        primitive,
        at,
    )?))
}

/// The specification's `IsCallable`.
fn callable(call: &Call<'_>, value: Value) -> bool {
    matches!(value, Value::Object(held) if call.seen().callable(held).is_some())
}
