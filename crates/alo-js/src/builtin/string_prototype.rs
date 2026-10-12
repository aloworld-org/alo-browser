/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `String.prototype` (ECMA-262 § 22.1.3, queue item 488): what a string's
//! properties are read from, with `toString`, `valueOf` and `slice` on it.
//!
//! Opened by alo Sites' analytics script on its contact form, whose last
//! loop writes `form.getAttribute("action").slice(3)` — the first property
//! of a string any frozen page reached. The read itself is the
//! interpreter's ([`interpret`](crate::interpret)'s `primitive_base`): a
//! string's own `length` and indices, and then this object.
//!
//! It is a `String` object of the empty string, as the specification makes
//! it, so its own `length` is `0` and `Object.prototype.toString` names it
//! `String`. Its methods read `this` as a string: `toString` and `valueOf`
//! through `thisStringValue`, and `slice` ([`string_slice`](super::string_slice))
//! through `ToString`, so `slice` works on anything that is not `null` or
//! `undefined`.
//!
//! # What is not here
//!
//! No `String` function, and so no `constructor` on this object: `String`
//! is a constructor, and a builtin with a `[[Construct]]` that makes a
//! `String` object is `ToObject` of a string, which is queue item 489 with
//! the `Number` and `Boolean` wrappers. Every other method is taken when a
//! page needs it (item 73), and the ones that take a regular expression are
//! item 323's. Each is absent rather than approximate, so `typeof
//! "".trim` is `"undefined"`, which is what a page's feature test reads.

use crate::abrupt::Escape;
use crate::heap::{Ref, Root};
use crate::object::native::{Answer, Call};
use crate::object::{Objects, Value};

use super::Intrinsics;
use super::string_create::string_create;

/// Make `String.prototype`, a `String` object of the empty string
/// inheriting from `Object.prototype`, and root it.
///
/// **This allocates.** `object_prototype` is the caller's to have rooted.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling.
pub(super) fn make(objects: &mut Objects, object_prototype: Ref) -> Result<Root, Escape> {
    let scope = objects.heap_mut().open();
    let outcome = made(objects, object_prototype);
    objects.heap_mut().close(scope);
    outcome
}

/// [`make`], with the scope already open.
fn made(objects: &mut Objects, object_prototype: Ref) -> Result<Root, Escape> {
    let empty = objects
        .text(Vec::new())
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(empty);
    let prototype = string_create(objects, Some(object_prototype), empty)?;
    Ok(objects.heap_mut().root(prototype))
}

/// Put the methods on `String.prototype`.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a root this
/// engine has lost.
pub(super) fn furnish(objects: &mut Objects, intrinsics: &Intrinsics) -> Result<(), Escape> {
    let on = intrinsics.string_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    super::method(objects, on, functions, "toString", to_string)?;
    super::method(objects, on, functions, "valueOf", value_of)?;
    super::native_method(objects, on, functions, super::string_slice::SLICE)?;
    Ok(())
}

/// `String.prototype.toString()`.
fn to_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Text(this_string_value(
        call, "toString",
    )?)))
}

/// `String.prototype.valueOf()`, which is the same question asked for a
/// conversion rather than for text.
fn value_of(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Text(this_string_value(
        call, "valueOf",
    )?)))
}

/// `thisStringValue(this)`: a string, or the string a `String` object holds.
///
/// # Errors
///
/// The `TypeError` the specification gives for anything else, naming the
/// method a person called.
fn this_string_value(call: &Call<'_>, method: &str) -> Result<Ref, Escape> {
    match call.this() {
        Value::Text(text) => Ok(text),
        Value::Object(held) => match call.seen().as_string_object(held) {
            Some(string) => Ok(string.data()),
            None => Err(not_a_string(call, method)),
        },
        _ => Err(not_a_string(call, method)),
    }
}

/// The `TypeError` for a `this` that is not a string.
fn not_a_string(call: &Call<'_>, method: &str) -> Escape {
    Escape::type_error(
        format!("String.prototype.{method} needs a string or a String object as this"),
        call.at(),
    )
}

#[cfg(test)]
mod tests {
    use crate::builtin::Intrinsics;
    use crate::object::{Found, Objects, Value};

    #[test]
    fn string_prototype_is_a_string_object_of_nothing_with_its_three_methods() {
        let mut objects = Objects::new();
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("the intrinsics are made");
        };
        let Ok(prototype) = intrinsics.string_prototype(&objects) else {
            panic!("it is rooted");
        };
        let data = objects
            .as_string_object(prototype)
            .map(crate::object::StringObject::data);
        assert_eq!(
            data.and_then(|data| objects.units(data)),
            Some([].as_slice())
        );
        assert_eq!(
            objects.prototype(prototype).ok().flatten(),
            intrinsics.object_prototype(&objects).ok()
        );
        for name in ["toString", "valueOf", "slice"] {
            let units: Vec<u16> = name.encode_utf16().collect();
            let Ok(key) = objects.key(&units) else {
                panic!("{name} is interned");
            };
            let Ok(Found::Value(Value::Object(method))) = objects.get(prototype, key) else {
                panic!("{name} is there");
            };
            assert!(objects.callable(method).is_some(), "{name} is a function");
        }
        let Ok(constructor) = objects.key(&"constructor".encode_utf16().collect::<Vec<_>>()) else {
            panic!("the name is interned");
        };
        assert!(
            matches!(objects.own_property(prototype, constructor), Ok(None)),
            "no String function yet (item 489)"
        );
    }
}
