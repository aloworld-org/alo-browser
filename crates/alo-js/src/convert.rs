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
//! # Every conversion below takes a [`Primitive`], and that is a type rather
//! than a check
//!
//! Turning an **object** into a primitive means calling `valueOf` or `toString`
//! — a function the page wrote, which may do anything, including reading the
//! property again. Calling one is the interpreter's business (queue item 214),
//! because only the interpreter has a stack to put a frame on. So this file
//! does the half that is arithmetic and [`primitive_of`] does the half that is
//! a *search*: it says which method to call and where to carry on if what that
//! answers is not a primitive either.
//!
//! The two halves are kept apart by a type. [`Primitive`] wraps a value that is
//! not an object and there is no other way to make one, so a conversion cannot
//! be handed an object by mistake — there is nowhere to write the mistake down.
//! Before this, every one of these functions had an object arm that answered
//! *this is not built yet*, and the arm was reachable from a dozen operators.
//!
//! # The names the engine needs are interned once
//!
//! Interning a name allocates, and allocating is a safepoint (ADR 0014 § 2), so
//! a conversion that interned `"valueOf"` on every call would be a collection
//! in the middle of every arithmetic operation on an object. [`Names`] interns
//! the handful this engine asks for when the engine is made, and **roots
//! them**: a key holds a reference to its own string, and a key whose string
//! was collected would name nothing.

use crate::abrupt::Escape;
use crate::heap::{Ref, Root};
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

/// A value that is not an object.
///
/// The demand every conversion in this file makes, and the reason it is a type:
/// an object is the one kind of value that cannot be converted without running
/// a page's own code, so a conversion that accepted one would have to have an
/// answer for a case it cannot answer. [`Primitive::of`] is the only way to make
/// one and it refuses an object, so that case does not exist here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primitive(Value);

impl Primitive {
    /// The primitive this value is, or [`None`] if it is an object — which the
    /// caller turns into one with [`primitive_of`] and a call.
    pub const fn of(value: Value) -> Option<Self> {
        match value {
            Value::Object(_) => None,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Text(_)
            | Value::Symbol(_) => Some(Self(value)),
        }
    }

    /// The value it is.
    pub const fn value(self) -> Value {
        self.0
    }
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

    /// The two names `OrdinaryToPrimitive` tries, in the order this hint asks
    /// for.
    ///
    /// `Symbol.toPrimitive` comes before both in the specification and is a
    /// well-known symbol, which arrives with the builtins (queue item 73).
    /// Until then no object can have one, because a script cannot spell the
    /// symbol.
    const fn order(&self, hint: Hint) -> [Key; ORDER] {
        match hint {
            Hint::String => [self.to_string, self.value_of],
            Hint::Default | Hint::Number => [self.value_of, self.to_string],
        }
    }
}

/// How many names a conversion tries before it gives up.
const ORDER: usize = 2;

/// What turning an object into a primitive needs the interpreter to call.
///
/// Two shapes rather than one because the method may itself be behind an
/// accessor: `valueOf` is usually a value on a prototype, and it is allowed to
/// be a getter, in which case *finding* it is a call before *calling* it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    /// Call this method with the object as its receiver. If what it answers is
    /// not a primitive either, ask again from `next`.
    Call {
        /// The function to call.
        method: Ref,
        /// Where the search carries on if its answer is an object.
        next: usize,
    },
    /// Call this getter with the object as its receiver: whatever it answers
    /// *is* the method. If that is not callable, the search carries on from
    /// `next` rather than throwing, because `IsCallable` is a question rather
    /// than a demand.
    Fetch {
        /// The getter to call.
        getter: Ref,
        /// Where the search carries on if what it answers is not callable.
        next: usize,
    },
}

/// `OrdinaryToPrimitive`, as far as the next thing that has to be called.
///
/// `from` is where to start, which is `0` for a conversion that has not begun
/// and the `next` of a previous answer for one that has: a method that answered
/// with an object has not converted anything, and the specification's rule is
/// to try the *other* name rather than to throw or to ask the same one again.
///
/// # Errors
///
/// A `TypeError` when neither name is callable — which is the answer the
/// specification gives for `Object.create(null) + ""` — and a fault for a
/// prototype chain this engine has lost.
pub fn primitive_of(
    objects: &Objects,
    names: &Names,
    object: Ref,
    hint: Hint,
    from: usize,
    at: usize,
) -> Result<Wanted, Escape> {
    for (which, key) in names.order(hint).into_iter().enumerate().skip(from) {
        let next = which.saturating_add(1);
        match objects.get(object, key)? {
            // Not there — or there as an accessor with no getter, which reads
            // as `undefined` and is therefore not callable either.
            Found::Missing | Found::Getter(Value::Undefined) => {}
            // There and callable: this is the method. There and *not* callable
            // — a number, a plain object — is skipped rather than thrown at,
            // which is the specification's `IsCallable` check.
            Found::Value(value) => {
                if let Some(method) = function_of(objects, value) {
                    return Ok(Wanted::Call { method, next });
                }
            }
            Found::Getter(getter) => {
                let Some(getter) = function_of(objects, getter) else {
                    return Err(Escape::type_error(
                        format!("the getter for '{}' is not a function", name(objects, key)),
                        at,
                    ));
                };
                return Ok(Wanted::Fetch { getter, next });
            }
        }
    }
    Err(Escape::type_error(
        "this object has no valueOf or toString, so it cannot become a primitive value",
        at,
    ))
}

/// The function a value is, or [`None`] if it is not callable — the
/// specification's `IsCallable`.
fn function_of(objects: &Objects, value: Value) -> Option<Ref> {
    match value {
        Value::Object(held) if objects.callable(held).is_some() => Some(held),
        _ => None,
    }
}

/// A key's name, for a message a person reads.
fn name(objects: &Objects, key: Key) -> String {
    match key.as_text().and_then(|held| objects.units(held)) {
        Some(units) => String::from_utf16_lossy(units),
        None => "that name".to_owned(),
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

/// `ToNumber`.
///
/// # Errors
///
/// A `TypeError` for a symbol, which is the one primitive that refuses.
pub fn to_number(objects: &Objects, value: Primitive, at: usize) -> Result<f64, Escape> {
    match value.value() {
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
        // Unreachable by construction: a [`Primitive`] is never an object.
        Value::Object(_) => Err(Escape::fault(crate::object::Fault::NotAnObject)),
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
/// spell implicitly.
pub fn to_units(objects: &Objects, value: Primitive, at: usize) -> Result<Vec<u16>, Escape> {
    match value.value() {
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
        // Unreachable by construction, as in [`to_number`].
        Value::Object(_) => Err(Escape::fault(crate::object::Fault::NotAnObject)),
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
pub fn to_text(objects: &mut Objects, value: Primitive, at: usize) -> Result<Value, Escape> {
    if matches!(value.value(), Value::Text(_)) {
        return Ok(value.value());
    }
    let units = to_units(objects, value, at)?;
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
pub fn to_property_key(objects: &mut Objects, value: Primitive, at: usize) -> Result<Key, Escape> {
    if let Value::Symbol(held) = value.value() {
        return objects.symbol_key(held).map_err(Escape::fault);
    }
    let units = to_units(objects, value, at)?;
    objects.key(&units).map_err(|why| Escape::refused(why, at))
}

/// `typeof`.
///
/// It takes the heap because one of the eight answers is a question about the
/// *cell* rather than about the value: an object with a `[[Call]]` is
/// `"function"` and every other object is `"object"`, and a page's own feature
/// tests are written on exactly that difference.
pub fn type_of(objects: &Objects, value: Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        // The oldest wart in the language, and it is specified: `typeof null`
        // is `"object"`.
        Value::Null => "object",
        Value::Object(held) => {
            if objects.callable(held).is_some() {
                "function"
            } else {
                "object"
            }
        }
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
    use super::{
        Hint, Names, Primitive, Wanted, primitive_of, to_boolean, to_int32, to_uint32, type_of,
    };
    use crate::object::{Objects, Property, Value};

    #[test]
    fn an_object_is_the_one_value_that_is_not_a_primitive() {
        let mut objects = Objects::new();
        let Ok(held) = objects.object(None) else {
            panic!("an empty heap holds an object");
        };
        assert!(Primitive::of(Value::Object(held)).is_none());
        for value in [
            Value::Undefined,
            Value::Null,
            Value::Bool(false),
            Value::Number(0.0),
        ] {
            assert_eq!(Primitive::of(value).map(Primitive::value), Some(value));
        }
    }

    #[test]
    fn a_search_for_a_primitive_tries_each_name_once_and_then_gives_up() {
        let mut objects = Objects::new();
        let Ok(names) = Names::new(&mut objects) else {
            panic!("an empty heap holds two names");
        };
        let Ok(held) = objects.object(None) else {
            panic!("an empty heap holds an object");
        };
        // Nothing to call at all, which is a `TypeError` rather than a refusal
        // of ours — the answer a real engine gives for `Object.create(null)`.
        assert!(primitive_of(&objects, &names, held, Hint::Number, 0, 0).is_err());

        let unit = std::rc::Rc::new(crate::unit::Unit::new());
        let Ok(function) = objects.function(unit, 0, None, None) else {
            panic!("and a function");
        };
        let value_of: Vec<u16> = "valueOf".encode_utf16().collect();
        let Ok(true) =
            objects.define_named(held, &value_of, Property::plain(Value::Object(function)))
        else {
            panic!("the object takes a valueOf");
        };
        assert_eq!(
            primitive_of(&objects, &names, held, Hint::Number, 0, 0),
            Ok(Wanted::Call {
                method: function,
                next: 1
            })
        );
        // A hint decides the order, so the same object asked for a string looks
        // for `toString` first and finds `valueOf` second.
        assert_eq!(
            primitive_of(&objects, &names, held, Hint::String, 0, 0),
            Ok(Wanted::Call {
                method: function,
                next: 2
            })
        );
        // Carrying on past the last name is the same `TypeError`, which is what
        // stops a method that keeps answering with an object from looping.
        assert!(primitive_of(&objects, &names, held, Hint::Number, 2, 0).is_err());
    }

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
        let objects = Objects::new();
        assert_eq!(type_of(&objects, Value::Null), "object");
        assert_eq!(type_of(&objects, Value::Undefined), "undefined");
        assert_eq!(type_of(&objects, Value::Number(1.0)), "number");
    }

    #[test]
    fn typeof_a_function_is_the_one_answer_that_asks_about_the_cell() {
        let mut objects = Objects::new();
        let Ok(plain) = objects.object(None) else {
            panic!("an empty heap holds an object");
        };
        assert_eq!(type_of(&objects, Value::Object(plain)), "object");
        let unit = std::rc::Rc::new(crate::unit::Unit::new());
        let Ok(callable) = objects.function(unit, 0, None, None) else {
            panic!("and a function");
        };
        assert_eq!(type_of(&objects, Value::Object(callable)), "function");
    }
}
