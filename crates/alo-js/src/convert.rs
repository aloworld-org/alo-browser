/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Turning a value into another kind of value: the abstract operations every
//! operator is written in terms of.
//!
//! `ToBoolean`, `ToNumber`, `ToString`, `ToPrimitive`, `ToPropertyKey` and
//! `typeof`. They are here rather than beside the operators that use them
//! because they are one responsibility — *what a value is worth as something
//! else* — and because getting one of them subtly wrong is visible in a dozen
//! operators at once.
//!
//! # `ToPrimitive` on an object throws today, and that is correct rather than
//! missing
//!
//! `OrdinaryToPrimitive` calls `valueOf` and then `toString`, and throws a
//! `TypeError` when neither is callable. An object in this engine has **no
//! prototype at all** until the builtins arrive (queue item 73), so neither
//! method is there to call and the `TypeError` is the answer the specification
//! gives for exactly that object — the same answer a real engine gives for
//! `Object.create(null) + ""`. What is *not* built is finding a method and
//! calling it, and where this engine finds something it would have to call it
//! says so ([`Missing::ACall`]) rather than skipping it, because skipping would
//! be quietly answering a question it did not ask.
//!
//! # The names the engine needs are interned once
//!
//! Interning a name allocates, and allocating is a safepoint (ADR 0014 § 2), so
//! a conversion that interned `"valueOf"` on every call would be a collection
//! in the middle of every arithmetic operation on an object. [`Names`] interns
//! the handful this engine asks for when the engine is made, and **roots
//! them**: a key holds a reference to its own string, and a key whose string
//! was collected would name nothing.

use crate::abrupt::{Escape, Missing};
use crate::heap::Root;
use crate::numeric;
use crate::object::{Found, Key, Objects, Refused, Value};

/// Which primitive an object is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    /// No preference — `a + b`, and `==`.
    Default,
    /// A number is wanted: every arithmetic operator but `+`.
    Number,
    /// A string is wanted: a property key, and `ToString`.
    String,
}

/// The property names the engine itself has to be able to ask for.
///
/// Rooted for as long as the engine is, because a [`Key`] is a reference to the
/// string that spells it and this engine may be the only thing holding it.
#[derive(Debug)]
pub struct Names {
    value_of: Key,
    to_string: Key,
    /// The roots that keep the strings above alive. Never released: they live
    /// as long as the engine does, and the heap goes with it.
    _held: Vec<Root>,
}

impl Names {
    /// Intern them, and root them.
    ///
    /// # Errors
    ///
    /// [`Refused`] if the heap cannot hold two short strings, which is a heap
    /// that was full before anything ran.
    pub fn new(objects: &mut Objects) -> Result<Self, Refused> {
        let value_of = objects.key(&units("valueOf"))?;
        let to_string = objects.key(&units("toString"))?;
        let held = [value_of, to_string]
            .iter()
            .filter_map(|key| key.reference())
            .map(|held| objects.heap_mut().root(held))
            .collect();
        Ok(Self {
            value_of,
            to_string,
            _held: held,
        })
    }
}

/// A string's code units, for the names this file spells in Rust.
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `ToBoolean`, which throws nothing and calls nothing.
///
/// The one case that needs the heap is a string, because an empty one is false
/// and every other one is true — including `"0"` and `"false"`.
pub fn to_boolean(objects: &Objects, value: Value) -> bool {
    match value {
        Value::Undefined | Value::Null => false,
        Value::Bool(is) => is,
        Value::Number(number) => !(number == 0.0 || number.is_nan()),
        Value::Text(held) => objects.units(held).is_some_and(|units| !units.is_empty()),
        Value::Symbol(_) | Value::Object(_) => true,
    }
}

/// `ToPrimitive`.
///
/// # Errors
///
/// A `TypeError` for an object with nothing to call, and [`Missing::ACall`]
/// where there *is* something and calling it is queue item 209.
pub fn to_primitive(
    objects: &Objects,
    names: &Names,
    value: Value,
    hint: Hint,
    at: usize,
) -> Result<Value, Escape> {
    let Value::Object(object) = value else {
        return Ok(value);
    };
    // `Symbol.toPrimitive` comes first in the specification and is a well-known
    // symbol, which arrives with the builtins (queue item 73). Until then no
    // object can have one: a script cannot spell the symbol.
    let order = match hint {
        Hint::String => [names.to_string, names.value_of],
        Hint::Default | Hint::Number => [names.value_of, names.to_string],
    };
    for key in order {
        match objects.get(object, key)? {
            // Something is there. Whether it is callable is a question this
            // engine cannot answer yet — nothing is callable — so it says which
            // item answers it rather than skipping a method the object has.
            Found::Value(Value::Object(_)) | Found::Getter(_) => {
                return Err(Escape::NotBuiltYet(Missing::ACall));
            }
            // Not there, or there and a primitive: the specification's
            // `IsCallable` check skips both rather than throwing, and tries the
            // other name.
            Found::Missing | Found::Value(_) => {}
        }
    }
    Err(Escape::type_error(
        "this object has no valueOf or toString, so it cannot become a primitive value",
        at,
    ))
}

/// `ToNumber`.
///
/// # Errors
///
/// A `TypeError` for a symbol, and whatever [`to_primitive`] refuses.
pub fn to_number(objects: &Objects, names: &Names, value: Value, at: usize) -> Result<f64, Escape> {
    match value {
        Value::Undefined => Ok(f64::NAN),
        Value::Null => Ok(0.0),
        Value::Bool(is) => Ok(if is { 1.0 } else { 0.0 }),
        Value::Number(number) => Ok(number),
        Value::Text(held) => {
            let units = objects
                .units(held)
                .ok_or(Escape::fault(crate::object::Fault::NotAnObject))?;
            Ok(numeric::number_of(units))
        }
        Value::Symbol(_) => Err(Escape::type_error(
            "a symbol cannot be turned into a number",
            at,
        )),
        Value::Object(_) => {
            let primitive = to_primitive(objects, names, value, Hint::Number, at)?;
            to_number(objects, names, primitive, at)
        }
    }
}

/// `ToString`, as the code units it produces.
///
/// Code units rather than a heap string, because most callers are about to join
/// two of them and would otherwise allocate a cell nobody keeps.
///
/// # Errors
///
/// A `TypeError` for a symbol, which is the one value the language refuses to
/// spell implicitly, and whatever [`to_primitive`] refuses.
pub fn to_units(
    objects: &Objects,
    names: &Names,
    value: Value,
    at: usize,
) -> Result<Vec<u16>, Escape> {
    match value {
        Value::Undefined => Ok(units("undefined")),
        Value::Null => Ok(units("null")),
        Value::Bool(true) => Ok(units("true")),
        Value::Bool(false) => Ok(units("false")),
        Value::Number(number) => Ok(units(&numeric::text_of(number))),
        Value::Text(held) => objects
            .units(held)
            .map(<[u16]>::to_vec)
            .ok_or_else(|| Escape::fault(crate::object::Fault::NotAnObject)),
        Value::Symbol(_) => Err(Escape::type_error(
            "a symbol cannot be turned into a string; use String(symbol) to say so on purpose",
            at,
        )),
        Value::Object(_) => {
            let primitive = to_primitive(objects, names, value, Hint::String, at)?;
            to_units(objects, names, primitive, at)
        }
    }
}

/// `ToString`, as a string in the heap.
///
/// **This allocates**, so it is a safepoint: everything the caller means to keep
/// must be on the interpreter's stack or in a scope.
///
/// # Errors
///
/// Whatever [`to_units`] refuses, and a heap that cannot hold the result.
pub fn to_text(
    objects: &mut Objects,
    names: &Names,
    value: Value,
    at: usize,
) -> Result<Value, Escape> {
    if matches!(value, Value::Text(_)) {
        return Ok(value);
    }
    let units = to_units(objects, names, value, at)?;
    let held = objects
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Value::Text(held))
}

/// `ToPropertyKey`.
///
/// **This allocates** when the key is a string that has not been interned.
///
/// # Errors
///
/// Whatever [`to_units`] refuses, and a heap that cannot hold the name.
pub fn to_property_key(
    objects: &mut Objects,
    names: &Names,
    value: Value,
    at: usize,
) -> Result<Key, Escape> {
    if let Value::Symbol(held) = value {
        return objects.symbol_key(held).map_err(Escape::fault);
    }
    let units = to_units(objects, names, value, at)?;
    objects.key(&units).map_err(|why| Escape::refused(why, at))
}

/// `typeof`.
///
/// `"function"` is absent for the reason everything else about calling is:
/// nothing in this heap has a `[[Call]]` yet (queue item 209), so no value can
/// honestly answer with it.
pub fn type_of(value: Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        // The oldest wart in the language, and it is specified: `typeof null`
        // is `"object"`.
        Value::Null | Value::Object(_) => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::Text(_) => "string",
        Value::Symbol(_) => "symbol",
    }
}

/// `ToInt32`.
///
/// The modular arithmetic is the specification's and it is not a cast: `2**31`
/// is `-2147483648`, `NaN` and both infinities are zero, and the fraction is
/// discarded towards zero before the modulus rather than after it.
pub fn to_int32(number: f64) -> i32 {
    let unsigned = to_uint32(number);
    // Reinterpreting the low thirty-two bits is what `modulo 2**32` then
    // `- 2**32 if >= 2**31` comes to, and it is one operation rather than three
    // that could each be off by one. Through the bytes rather than through
    // `as`, because the two agree and only one of them says so.
    i32::from_ne_bytes(unsigned.to_ne_bytes())
}

/// `ToUint32`.
pub fn to_uint32(number: f64) -> u32 {
    if !number.is_finite() || number == 0.0 {
        return 0;
    }
    let truncated = number.trunc();
    let wrapped = truncated.rem_euclid(4_294_967_296.0);
    // `rem_euclid` answers in `0 .. 2**32`, which is exactly the range a
    // `u32` holds, so this conversion is exact rather than saturating — but it
    // is written as a fallible one because a value that is somehow outside it
    // must not become an arbitrary number.
    if (0.0..4_294_967_296.0).contains(&wrapped) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the range is checked on the line above, and the value is whole"
        )]
        {
            wrapped as u32
        }
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{to_boolean, to_int32, to_uint32, type_of};
    use crate::object::{Objects, Value};

    #[test]
    fn an_empty_string_is_the_only_false_string() {
        let mut objects = Objects::new();
        let Ok(empty) = objects.text(Vec::new()) else {
            panic!("an empty heap can hold an empty string");
        };
        let Ok(zero) = objects.text("0".encode_utf16().collect()) else {
            panic!("an empty heap can hold one character");
        };
        assert!(!to_boolean(&objects, Value::Text(empty)));
        assert!(to_boolean(&objects, Value::Text(zero)));
        assert!(!to_boolean(&objects, Value::Number(f64::NAN)));
        assert!(!to_boolean(&objects, Value::Number(-0.0)));
        assert!(to_boolean(&objects, Value::Number(-1.0)));
        assert!(!to_boolean(&objects, Value::Null));
    }

    #[test]
    fn the_integer_conversions_are_modular_rather_than_saturating() {
        assert_eq!(to_int32(2_147_483_648.0), -2_147_483_648);
        assert_eq!(to_int32(4_294_967_296.0), 0);
        assert_eq!(to_int32(-1.5), -1);
        assert_eq!(to_int32(f64::NAN), 0);
        assert_eq!(to_int32(f64::INFINITY), 0);
        assert_eq!(to_uint32(-1.0), 4_294_967_295);
        assert_eq!(to_uint32(4_294_967_297.0), 1);
    }

    #[test]
    fn typeof_null_is_object_because_the_language_says_so() {
        assert_eq!(type_of(Value::Null), "object");
        assert_eq!(type_of(Value::Undefined), "undefined");
        assert_eq!(type_of(Value::Number(1.0)), "number");
    }
}
