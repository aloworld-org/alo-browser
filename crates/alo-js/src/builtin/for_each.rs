/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Array.prototype.forEach` (queue item 331): the first builtin that keeps
//! state across the calls it asks for (ADR 0031).
//!
//! The specification reads the length **once**, into `len`, then for each
//! `k` below it asks whether the object has `k` and, if it does, reads it
//! and calls the callback with the element, `k` and the object. A callback
//! that pushes onto the array it is walking does not make the walk longer,
//! and a `length` getter runs once rather than once per element — so `len`
//! and `k` are kept, in the two slots this builtin declares
//! ([`KEPT`]), across every call it asks for.
//!
//! # Every step that may run the script is asked for
//!
//! - `length` may be a getter — a `NodeList`'s is — and is called with the
//!   object as its `this` ([`READ_LENGTH`]).
//! - What it answered may be an object, whose `valueOf` runs
//!   ([`CONVERTED_LENGTH`]).
//! - An element may be a getter ([`READ_ELEMENT`]).
//! - The callback ([`CALLED_BACK`]).
//!
//! None of them is refused: each is laid out by the interpreter like any
//! other call, and a throw from any of them ends the walk where it was, as
//! the specification's `?` does. `HasProperty` runs no script on any
//! object this engine has, so it is answered here.
//!
//! # A walk over holes asks the embedder's stop
//!
//! `forEach` over `{ length: 2 ** 53 - 1 }` asks for nothing at all: every
//! index is a hole. The interpreter asks the embedder's stop at each call
//! and each backward jump, and here there is neither, so this asks it on
//! every pass (ADR 0031 § 7) — otherwise a page could hold the thread for
//! longer than anybody would wait.
//!
//! # What it refuses
//!
//! A primitive `this` needs the wrapper objects item 73 builds, and is
//! [`Missing::AWrapperObject`] as `values` is. `null` and `undefined` are
//! the `TypeError` the specification gives.

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native, Want};
use crate::object::{Found, Value};

use super::array_like::{key_of, to_length};
use super::array_prototype::object_of;

/// The native: `forEach`, keeping `len` and `k`.
pub(super) const FOR_EACH: Native = Native::new("forEach", for_each).keeping(KEPT);

/// How many slots it keeps.
const KEPT: usize = 2;
/// The slot `len` is kept in, once it is known.
const LENGTH: usize = 0;
/// The slot `k` is kept in: the index being visited.
const INDEX: usize = 1;

/// Come back here with what the `length` getter answered.
const READ_LENGTH: u32 = 1;
/// Come back here with the primitive the length converted to.
const CONVERTED_LENGTH: u32 = 2;
/// Come back here once the callback has returned for index `k`.
const CALLED_BACK: u32 = 3;
/// Come back here with what an element's getter answered.
const READ_ELEMENT: u32 = 4;

/// `Array.prototype.forEach(callbackfn, thisArg)`.
fn for_each(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let object = object_of(call, "forEach")?;
    match call.step() {
        0 => read_length(call, object),
        READ_LENGTH => convert_length(call, call.answer()?),
        CONVERTED_LENGTH => {
            let primitive =
                Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let length = to_length(convert::to_number(call.seen(), primitive, call.at())?);
            begin(call, object, length)
        }
        CALLED_BACK => {
            let next = call.kept_number(INDEX)? + 1.0;
            call.keep(INDEX, Value::Number(next))?;
            walk(call, object)
        }
        READ_ELEMENT => {
            let element = call.answer()?;
            let index = call.kept_number(INDEX)?;
            Ok(call_back(call, object, element, index))
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `Get(O, "length")`: an array's own, read directly; anything else's by
/// `[[Get]]`, asking for its getter if it has one.
fn read_length(call: &mut Call<'_>, object: Ref) -> Result<Answer, Escape> {
    if let Some(array) = call.seen().as_array(object) {
        let length = f64::from(array.length());
        return begin(call, object, length);
    }
    let at = call.at();
    let units: Vec<u16> = "length".encode_utf16().collect();
    // Interning may allocate; `object` is `this`, on the interpreter's stack.
    let key = call
        .objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))?;
    match call.seen().get(object, key)? {
        Found::Value(value) => convert_length(call, value),
        Found::Missing | Found::Getter(Value::Undefined) => convert_length(call, Value::Undefined),
        Found::Getter(getter) => Ok(Answer::want(
            Want::Call {
                callee: getter,
                receiver: Value::Object(object),
                arguments: Vec::new(),
            },
            READ_LENGTH,
        )),
    }
}

/// `ToLength(value)`, asking for `ToPrimitive` first if it is an object.
fn convert_length(call: &mut Call<'_>, value: Value) -> Result<Answer, Escape> {
    let Some(primitive) = Primitive::of(value) else {
        return Ok(Answer::want(
            Want::Primitive {
                of: value,
                hint: Hint::Number,
            },
            CONVERTED_LENGTH,
        ));
    };
    let length = to_length(convert::to_number(call.seen(), primitive, call.at())?);
    let object = object_of(call, "forEach")?;
    begin(call, object, length)
}

/// With `len` known: the callback must be callable — checked after the
/// length, as the specification orders it, so a `length` getter has run by
/// the time a missing callback throws — then keep `len` and `k = 0` and
/// walk.
fn begin(call: &mut Call<'_>, object: Ref, length: f64) -> Result<Answer, Escape> {
    let callable = match call.argument(0) {
        Value::Object(held) => call.seen().callable(held).is_some(),
        _ => false,
    };
    if !callable {
        return Err(Escape::type_error(
            "Array.prototype.forEach needs a function to call for each element",
            call.at(),
        ));
    }
    call.keep(LENGTH, Value::Number(length))?;
    call.keep(INDEX, Value::Number(0.0))?;
    walk(call, object)
}

/// From `k`, skip every index the object does not have, and at the first it
/// has, read it — or ask for its getter — and call back. Past `len`, the
/// walk is over and answers `undefined`.
fn walk(call: &mut Call<'_>, object: Ref) -> Result<Answer, Escape> {
    let length = call.kept_number(LENGTH)?;
    loop {
        let index = call.kept_number(INDEX)?;
        if index >= length {
            return Ok(Answer::Value(Value::Undefined));
        }
        if call.stop_asked() {
            return Err(Escape::Interrupted);
        }
        // May allocate, for an index past 2³² − 2; nothing is held in a Rust
        // local across it but numbers and `this`.
        let key = key_of(call, index)?;
        if call.seen().has(object, key)? {
            return Ok(match call.seen().get(object, key)? {
                Found::Value(element) => call_back(call, object, element, index),
                Found::Missing | Found::Getter(Value::Undefined) => {
                    call_back(call, object, Value::Undefined, index)
                }
                Found::Getter(getter) => Answer::want(
                    Want::Call {
                        callee: getter,
                        receiver: Value::Object(object),
                        arguments: Vec::new(),
                    },
                    READ_ELEMENT,
                ),
            });
        }
        call.keep(INDEX, Value::Number(index + 1.0))?;
    }
}

/// `Call(callbackfn, thisArg, « kValue, 𝔽(k), O »)`.
///
/// Built last, of values already on the stack — the callback and `thisArg`
/// are arguments, the object is `this`, and the element was read from it or
/// answered into the answer slot — and nothing allocates before it is
/// returned.
fn call_back(call: &Call<'_>, object: Ref, element: Value, index: f64) -> Answer {
    Answer::want(
        Want::Call {
            callee: call.argument(0),
            receiver: call.argument(1),
            arguments: vec![element, Value::Number(index), Value::Object(object)],
        },
        CALLED_BACK,
    )
}
