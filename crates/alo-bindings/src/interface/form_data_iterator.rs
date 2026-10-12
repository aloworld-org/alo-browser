/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The `FormData Iterator` prototype (queue item 472): Web IDL's *iterator
//! prototype object* for `FormData`, inheriting from `%IteratorPrototype%`,
//! so `for…of` and a spread find `[Symbol.iterator]` returning the iterator
//! itself, as Chrome 154's does (measured).
//!
//! Its one member is `next`: while the index is short of the target's list
//! it answers `{ value: [name, value], done: false }`, a new array each
//! time, and moves on; after that `{ value: undefined, done: true }`. The
//! list is read again at every call, as Web IDL's default iterator reads it.
//! Called on anything but such an iterator it is a `TypeError`.
//!
//! **Absent:** its `Symbol.toStringTag`, `"FormData Iterator"`, as every
//! interface's tag is absent here ([`crate::interface`]), so
//! `Object.prototype.toString` names it `[object Object]` where Chrome says
//! `[object FormData Iterator]`.

use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Key, Objects, Property};
use alo_js::{Escape, Fault, Value};

use crate::define;
use crate::form_data::FormData;
use crate::form_data_iterator::FormDataIterator;

/// The iterator prototype's member.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "next", next)
}

/// `next()`.
fn next(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let Value::Object(held) = call.this() else {
        return Err(not_an_iterator(call));
    };
    let Some((target, index)) = call
        .seen()
        .embedded::<FormDataIterator>(held)
        .map(|iterator| (iterator.target(), iterator.index()))
    else {
        return Err(not_an_iterator(call));
    };
    let entry = target
        .and_then(|target| call.seen().embedded::<FormData>(target))
        .ok_or(Escape::fault(Fault::Gone))?
        .list()
        .get(index)
        .cloned();
    let Some((name, value)) = entry else {
        return result(call, Value::Undefined, true);
    };
    call.objects()
        .write_embedded::<FormDataIterator, _>(held, |iterator, _| iterator.advance())
        .ok_or(Escape::fault(Fault::Gone))?;
    let scope = call.objects().heap_mut().open();
    let made = pair(call, &name, &value);
    call.objects().heap_mut().close(scope);
    let pair = made?;
    result(call, pair, false)
}

/// `[name, value]`, an array from `Array.prototype`, with the scope open:
/// each string and the array are held from the moment each exists.
fn pair(call: &mut Call<'_>, name: &str, value: &str) -> Result<Value, Escape> {
    let at = call.at();
    let mut texts = Vec::with_capacity(2);
    for text in [name, value] {
        let made = call
            .objects()
            .text(text.encode_utf16().collect())
            .map_err(|why| Escape::refused(why, at))?;
        call.objects().heap_mut().hold(made);
        texts.push(Value::Text(made));
    }
    let above = call.intrinsics()?.array_prototype(call.seen())?;
    let array = call
        .objects()
        .array(Some(above), 2)
        .map_err(|why| Escape::refused(why, at))?;
    for (which, text) in (0_u32..).zip(texts) {
        let key = Key::index(which).ok_or(Escape::fault(Fault::Gone))?;
        call.objects().define(array, key, Property::plain(text))?;
    }
    Ok(Value::Object(array))
}

/// `CreateIterResultObject(value, done)`: `value` is held in a scope from
/// before the first allocation until the object owns it.
fn result(call: &mut Call<'_>, value: Value, done: bool) -> Result<Answer, Escape> {
    let scope = call.objects().heap_mut().open();
    let made = resulted(call, value, done);
    call.objects().heap_mut().close(scope);
    made.map(Answer::Value)
}

/// [`result`], with the scope open.
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

/// The `TypeError` for `next` called on anything but a `FormData`'s
/// iterator.
fn not_an_iterator(call: &Call<'_>) -> Escape {
    Escape::type_error(
        "'next' was called on something that is not a FormData Iterator",
        call.at(),
    )
}
