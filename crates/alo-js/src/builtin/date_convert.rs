/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A date as something else (queue item 356):
//! `Date.prototype[Symbol.toPrimitive]` and `Date.prototype.toJSON`.
//!
//! # `Symbol.toPrimitive` is why `date + ''` is text
//!
//! An object with no `Symbol.toPrimitive` is asked for `valueOf` first when no
//! hint is given, and a date's `valueOf` is its number. A date is the one
//! object the language makes differently: its `Symbol.toPrimitive` treats
//! *no hint* as *a string*, so `date + ''` is the date's text and `date - 0`
//! its number. This engine's `ToPrimitive` asks for the symbol first for that
//! reason ([`convert`](crate::convert)), and this method's own last step is
//! `OrdinaryToPrimitive` — the search alone, asked for as
//! [`Want::Ordinary`] so that it does not find this method again.
//!
//! Its text is `toString`'s, which is item 357's, so `date + ''` is refused by
//! name there — a refusal, where answering the number would have been a
//! wrong answer that read like a right one.
//!
//! # `toJSON` works on anything
//!
//! It is generic: `ToObject(this)`, its number, `null` for one that is not
//! finite, and otherwise whatever its `toISOString` answers — called by name,
//! so a page that replaced it is the page's choice. A primitive `this` needs
//! the wrapper objects item 73 builds ([`Missing::AWrapperObject`]).

use crate::abrupt::{Escape, Internal, Missing};
use crate::convert::{Hint, Primitive};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native, Want};
use crate::object::{Found, Key, Objects, Property, Value};

/// `Date.prototype[Symbol.toPrimitive]`.
const TO_PRIMITIVE: Native = Native::new("[Symbol.toPrimitive]", to_primitive);
/// `Date.prototype.toJSON`.
const TO_JSON: Native = Native::new("toJSON", to_json);

/// `[Symbol.toPrimitive]`: come back here with what `OrdinaryToPrimitive`
/// answered.
const ORDINARY: u32 = 1;
/// `toJSON`: come back here with `this` as a number-first primitive.
const PRIMITIVE: u32 = 1;
/// `toJSON`: come back here with what a `toISOString` getter answered.
const READ_METHOD: u32 = 2;
/// `toJSON`: come back here with what `toISOString` answered.
const CALLED: u32 = 3;

/// Put `toJSON` and `[Symbol.toPrimitive]` on `Date.prototype`.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference
/// this engine has lost.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
    to_primitive: Key,
) -> Result<(), Escape> {
    super::native_method(objects, prototype, function_prototype, TO_JSON)?;
    // Not writable, unlike every other method: the specification fixes it,
    // so a page that wants a date to convert otherwise must redefine it.
    // The symbol is the realm's, rooted, so only the function needs holding.
    let scope = objects.heap_mut().open();
    let outcome = objects
        .native(TO_PRIMITIVE, Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))
        .and_then(|function| {
            objects.heap_mut().hold(function);
            objects
                .define(
                    prototype,
                    to_primitive,
                    Property::data(Value::Object(function), false, false, true),
                )
                .map(drop)
                .map_err(Escape::from)
        });
    objects.heap_mut().close(scope);
    outcome
}

/// `Date.prototype[Symbol.toPrimitive](hint)`.
fn to_primitive(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == ORDINARY {
        return Ok(Answer::Value(call.answer()?));
    }
    let this = call.this();
    if !matches!(this, Value::Object(_)) {
        return Err(Escape::type_error(
            "Date.prototype[Symbol.toPrimitive] was called on something that is not an object",
            call.at(),
        ));
    }
    let hint = match call.argument(0) {
        Value::Text(held) => match call.seen().units(held) {
            Some(units) if units == utf16("string") || units == utf16("default") => {
                Some(Hint::String)
            }
            Some(units) if units == utf16("number") => Some(Hint::Number),
            _ => None,
        },
        _ => None,
    };
    let Some(hint) = hint else {
        return Err(Escape::type_error(
            "a date is converted with the hint 'string', 'number' or 'default'",
            call.at(),
        ));
    };
    Ok(Answer::want(Want::Ordinary { of: this, hint }, ORDINARY))
}

/// A name's code units, to compare a hint against.
fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `Date.prototype.toJSON(key)`.
fn to_json(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let object = match call.this() {
        Value::Object(held) => held,
        Value::Undefined | Value::Null => {
            return Err(Escape::type_error(
                "Date.prototype.toJSON was called on null or undefined",
                call.at(),
            ));
        }
        Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
            return Err(Escape::NotBuiltYet(Missing::AWrapperObject));
        }
    };
    match call.step() {
        0 => Ok(Answer::want(
            Want::Primitive {
                of: Value::Object(object),
                hint: Hint::Number,
            },
            PRIMITIVE,
        )),
        PRIMITIVE => {
            let tv = call.answer()?;
            if Primitive::of(tv).is_none() {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            }
            if let Value::Number(number) = tv
                && !number.is_finite()
            {
                return Ok(Answer::Value(Value::Null));
            }
            let at = call.at();
            // Interning may allocate; `object` is `this`, on the stack.
            let key = call
                .objects()
                .key(&utf16("toISOString"))
                .map_err(|why| Escape::refused(why, at))?;
            Ok(match call.seen().get(object, key)? {
                Found::Value(method) => invoke(object, method),
                Found::Missing | Found::Getter(Value::Undefined) => {
                    invoke(object, Value::Undefined)
                }
                Found::Getter(getter) => Answer::want(
                    Want::Call {
                        callee: getter,
                        receiver: Value::Object(object),
                        arguments: Vec::new(),
                    },
                    READ_METHOD,
                ),
            })
        }
        READ_METHOD => Ok(invoke(object, call.answer()?)),
        CALLED => Ok(Answer::Value(call.answer()?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `Invoke(O, "toISOString")`'s call: one that is not callable is the
/// `TypeError` every call of a non-function is, thrown where calls throw it.
fn invoke(object: Ref, method: Value) -> Answer {
    Answer::want(
        Want::Call {
            callee: method,
            receiver: Value::Object(object),
            arguments: Vec::new(),
        },
        CALLED,
    )
}
