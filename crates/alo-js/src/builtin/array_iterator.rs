/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `%ArrayIteratorPrototype%`: `next`, and the tag that names what an array
//! iterator is (queue item 230).
//!
//! `next` is the specification's generator closure run one step at a time:
//! read the length — **again, every time**, so an array that grows while it is
//! iterated is iterated to its new end — and if the index has reached it,
//! finish; otherwise hand out the key, the element or both, and move on. Once
//! finished it answers `done` for ever without looking at the array again,
//! which is the generator being *completed*.
//!
//! # It never calls the script, and refuses where it would have to
//!
//! An element may be a getter, and an array-like's `length` may be a getter or
//! an object whose `valueOf` must run. Either is a call from inside a
//! generator, which makes the generator's *executing* state observable — a
//! getter that calls `next` again must get a `TypeError` — and a throw from it
//! must complete the iterator. This iterator keeps neither, so both are
//! [`Missing::AnIteratedValueBehindACall`] (queue item 231) rather than an
//! answer a page could tell was wrong. An accessor with **no** getter is not a
//! call: it reads as `undefined`, as it does everywhere else.
//!
//! An array's own `length` is never either: it is a data property holding a
//! whole number, which is why `for…of` over an ordinary array never reaches
//! the refusal.
//!
//! # What a result is
//!
//! `{ value, done }`, an ordinary object inheriting from `Object.prototype`
//! with both properties defined in that order — `CreateIterResultObject`. Each
//! allocation that makes it is a safepoint, so the value is held in a scope
//! from before the first until the object owns it.

use crate::abrupt::{Escape, Missing};
use crate::convert::{self, Primitive};
use crate::heap::Ref;
use crate::numeric;
use crate::object::array_iterator::{ArrayIterator, Kind};
use crate::object::native::{Answer, Call};
use crate::object::symbol::WellKnown;
use crate::object::{Found, Key, Objects, Property, Value};

use super::Intrinsics;

/// The largest length an array-like may have: 2⁵³−1, `ToLength`'s ceiling.
const LONGEST: f64 = 9_007_199_254_740_991.0;

/// Put `next` and the tag on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(objects: &mut Objects, intrinsics: &Intrinsics) -> Result<(), Escape> {
    let on = intrinsics.array_iterator_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    super::method(objects, on, functions, "next", next)?;

    // `%ArrayIteratorPrototype%[Symbol.toStringTag]` is `"Array Iterator"`:
    // not writable, not enumerable, configurable. The string is the one thing
    // allocated, and it is stored before anything else could be.
    let key = intrinsics.well_known_key(objects, WellKnown::ToStringTag)?;
    let tag = objects
        .text("Array Iterator".encode_utf16().collect())
        .map_err(|why| Escape::refused(why, 0))?;
    objects.define(
        on,
        key,
        Property::data(Value::Text(tag), false, false, true),
    )?;
    Ok(())
}

/// `%ArrayIteratorPrototype%.next`.
fn next(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let Value::Object(held) = call.this() else {
        return Err(not_an_iterator(at));
    };
    let Some(iterator) = call.seen().as_array_iterator(held) else {
        return Err(not_an_iterator(at));
    };
    let (iterated, index, kind) = (iterator.iterated(), iterator.next_index(), iterator.kind());
    let Some(iterated) = iterated else {
        return result(call, Value::Undefined, true);
    };

    if index >= length_of(call, iterated)? {
        // `index` is never past `length` when it is compared, and once it
        // reaches it the iterator lets go of the array for good.
        call.objects()
            .with_array_iterator(held, ArrayIterator::finish)
            .ok_or(Escape::fault(crate::object::Fault::Gone))?;
        return result(call, Value::Undefined, true);
    }

    let value = match kind {
        Kind::Keys => Value::Number(index),
        Kind::Values => element(call, iterated, index)?,
        Kind::Entries => {
            let element = element(call, iterated, index)?;
            pair(call, index, element)?
        }
    };
    call.objects()
        .with_array_iterator(held, |iterator, _| iterator.advance())
        .ok_or(Escape::fault(crate::object::Fault::Gone))?;
    result(call, value, false)
}

/// `LengthOfArrayLike(iterated)`, refusing where it would call the script.
fn length_of(call: &mut Call<'_>, iterated: Ref) -> Result<f64, Escape> {
    if let Some(array) = call.seen().as_array(iterated) {
        return Ok(f64::from(array.length()));
    }
    let at = call.at();
    let units: Vec<u16> = "length".encode_utf16().collect();
    // Interning may allocate; `iterated` is held by the iterator, which is
    // `this`, which is on the interpreter's stack.
    let key = call
        .objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))?;
    let value = match call.seen().get(iterated, key)? {
        Found::Value(value) => value,
        Found::Missing | Found::Getter(Value::Undefined) => Value::Undefined,
        Found::Getter(_) => return Err(Escape::NotBuiltYet(Missing::AnIteratedValueBehindACall)),
    };
    let Some(primitive) = Primitive::of(value) else {
        return Err(Escape::NotBuiltYet(Missing::AnIteratedValueBehindACall));
    };
    Ok(to_length(convert::to_number(call.seen(), primitive, at)?))
}

/// `ToLength`: a whole number from zero to 2⁵³−1, with `NaN` as zero.
fn to_length(number: f64) -> f64 {
    if number.is_nan() || number <= 0.0 {
        return 0.0;
    }
    number.trunc().min(LONGEST)
}

/// `Get(iterated, ToString(index))`, refusing a getter.
fn element(call: &mut Call<'_>, iterated: Ref, index: f64) -> Result<Value, Escape> {
    let key = key_of(call, index)?;
    match call.seen().get(iterated, key)? {
        Found::Value(value) => Ok(value),
        Found::Missing | Found::Getter(Value::Undefined) => Ok(Value::Undefined),
        Found::Getter(_) => Err(Escape::NotBuiltYet(Missing::AnIteratedValueBehindACall)),
    }
}

/// The key an index is: an array index below 2³²−1, and the canonical
/// spelling of the number above it, which is an ordinary string key.
fn key_of(call: &mut Call<'_>, index: f64) -> Result<Key, Escape> {
    if let Some(key) = crate::object::array::exact_length(index).and_then(Key::index) {
        return Ok(key);
    }
    let at = call.at();
    let units: Vec<u16> = numeric::text_of(index).encode_utf16().collect();
    call.objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))
}

/// `[index, element]`, an array from `Array.prototype`, for `entries()`.
fn pair(call: &mut Call<'_>, index: f64, element: Value) -> Result<Value, Escape> {
    let scope = call.objects().heap_mut().open();
    let made = paired(call, index, element);
    call.objects().heap_mut().close(scope);
    made
}

/// [`pair`], with the scope open.
fn paired(call: &mut Call<'_>, index: f64, element: Value) -> Result<Value, Escape> {
    let at = call.at();
    if let Some(held) = element.reference() {
        call.objects().heap_mut().hold(held);
    }
    let above = call.intrinsics()?.array_prototype(call.seen())?;
    let array = call
        .objects()
        .array(Some(above), 2)
        .map_err(|why| Escape::refused(why, at))?;
    for (which, value) in (0_u32..).zip([Value::Number(index), element]) {
        let key = Key::index(which).ok_or(Escape::fault(crate::object::Fault::Gone))?;
        call.objects().define(array, key, Property::plain(value))?;
    }
    Ok(Value::Object(array))
}

/// `CreateIterResultObject(value, done)`.
fn result(call: &mut Call<'_>, value: Value, done: bool) -> Result<Answer, Escape> {
    let scope = call.objects().heap_mut().open();
    let made = resulted(call, value, done);
    call.objects().heap_mut().close(scope);
    made.map(Answer::Value)
}

/// [`result`], with the scope open: the value, both names and the object are
/// held from the moment each exists.
fn resulted(call: &mut Call<'_>, value: Value, done: bool) -> Result<Value, Escape> {
    let at = call.at();
    if let Some(held) = value.reference() {
        call.objects().heap_mut().hold(held);
    }
    let mut keys = Vec::with_capacity(2);
    for name in ["value", "done"] {
        let units: Vec<u16> = name.encode_utf16().collect();
        let key = call
            .objects()
            .key(&units)
            .map_err(|why| Escape::refused(why, at))?;
        if let Some(held) = key.reference() {
            call.objects().heap_mut().hold(held);
        }
        keys.push(key);
    }
    let above = call.intrinsics()?.object_prototype(call.seen())?;
    let object = call
        .objects()
        .object(Some(above))
        .map_err(|why| Escape::refused(why, at))?;
    for (key, value) in keys.into_iter().zip([value, Value::Bool(done)]) {
        call.objects().define(object, key, Property::plain(value))?;
    }
    Ok(Value::Object(object))
}

/// The `TypeError` for a `next` called on anything but an array iterator.
fn not_an_iterator(at: usize) -> Escape {
    Escape::type_error(
        "%ArrayIteratorPrototype%.next was called on something that is not an array iterator",
        at,
    )
}
