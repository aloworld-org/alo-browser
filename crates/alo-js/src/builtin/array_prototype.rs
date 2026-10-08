/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Array.prototype`'s methods: the three that make an iterator,
//! `Symbol.iterator` (queue item 230), and `forEach` (queue item 331).
//!
//! `for (const x of list)` begins with `list[Symbol.iterator]()`, and for an
//! array that is this file. `[Symbol.iterator]` is **the same function** as
//! `values` rather than a second one that does the same thing — the
//! specification says so, and a page can see it with `===`.
//!
//! `forEach` keeps state across the calls it asks for, so its body is a
//! file of its own ([`super::for_each`]); this one puts it here.
//!
//! # Every other method is absent
//!
//! `push`, `map`, `concat`, `join` and the rest are item 73's, each with
//! its own closing condition; `[].push` is `undefined`, which a page's own
//! feature test reads correctly.
//!
//! # `this` is anything with a `length`
//!
//! All four are generic: `Array.prototype.values.call(arrayLike)` iterates an
//! object that is not an array. `ToObject(this)` comes first, so `null` and
//! `undefined` are the `TypeError` the language specifies and a string — which
//! is iterable through a wrapper this engine has not built — is
//! [`Missing::AWrapperObject`].

use crate::abrupt::{Escape, Missing};
use crate::heap::Ref;
use crate::object::Objects;
use crate::object::Value;
use crate::object::array_iterator::Kind;
use crate::object::native::{Answer, Call};
use crate::object::symbol::WellKnown;

use super::Intrinsics;

/// Put the methods on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(objects: &mut Objects, intrinsics: &Intrinsics) -> Result<(), Escape> {
    let on = intrinsics.array_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    super::method(objects, on, functions, "keys", keys)?;
    super::method(objects, on, functions, "values", values)?;
    super::method(objects, on, functions, "entries", entries)?;
    super::native_method(objects, on, functions, super::for_each::FOR_EACH)?;
    let key = intrinsics.well_known_key(objects, WellKnown::Iterator)?;
    super::alias(objects, on, "values", key)
}

/// `Array.prototype.keys`.
fn keys(call: &mut Call<'_>) -> Result<Answer, Escape> {
    iterate(call, Kind::Keys, "keys")
}

/// `Array.prototype.values`, which is also `Array.prototype[Symbol.iterator]`.
fn values(call: &mut Call<'_>) -> Result<Answer, Escape> {
    iterate(call, Kind::Values, "values")
}

/// `Array.prototype.entries`.
fn entries(call: &mut Call<'_>) -> Result<Answer, Escape> {
    iterate(call, Kind::Entries, "entries")
}

/// `CreateArrayIterator(ToObject(this), kind)`.
///
/// The object iterated is `this`, which is still on the interpreter's stack
/// while the iterator is allocated, and the prototype is an intrinsic and
/// rooted — so the one allocation here has nothing in a Rust local to lose.
fn iterate(call: &mut Call<'_>, kind: Kind, name: &str) -> Result<Answer, Escape> {
    let iterated = object_of(call, name)?;
    let prototype = call.intrinsics()?.array_iterator_prototype(call.seen())?;
    let at = call.at();
    let iterator = call
        .objects()
        .array_iterator(Some(prototype), iterated, kind)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Object(iterator)))
}

/// `ToObject(this)`, with its three answers, for the method called `name`.
pub(super) fn object_of(call: &Call<'_>, name: &str) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) => Ok(held),
        Value::Undefined | Value::Null => Err(Escape::type_error(
            format!("Array.prototype.{name} needs an object to walk, and was given nothing"),
            call.at(),
        )),
        Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
            Err(Escape::NotBuiltYet(Missing::AWrapperObject))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builtin::Intrinsics;
    use crate::object::symbol::WellKnown;
    use crate::object::{Found, Objects, Value};

    #[test]
    fn the_iteration_intrinsics_are_built_whole_with_the_collector_at_every_allocation() {
        let mut objects = Objects::new();
        objects.heap_mut().stress(true);
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("an empty heap holds the intrinsics however often it collects");
        };
        objects.heap_mut().stress(false);
        assert_eq!(objects.heap().scoped(), 0, "every scope was closed");

        let (Ok(arrays), Ok(iterators), Ok(array_iterators), Ok(objects_prototype)) = (
            intrinsics.array_prototype(&objects),
            intrinsics.iterator_prototype(&objects),
            intrinsics.array_iterator_prototype(&objects),
            intrinsics.object_prototype(&objects),
        ) else {
            panic!("each is rooted");
        };
        assert_eq!(objects.prototype(array_iterators), Ok(Some(iterators)));
        assert_eq!(objects.prototype(iterators), Ok(Some(objects_prototype)));

        // `[Symbol.iterator]` is the very function `values` is.
        let (Ok(iterator), Ok(tag)) = (
            intrinsics.well_known_key(&objects, WellKnown::Iterator),
            intrinsics.well_known_key(&objects, WellKnown::ToStringTag),
        ) else {
            panic!("both symbols are rooted");
        };
        let values: Vec<u16> = "values".encode_utf16().collect();
        let Some(named) = objects.existing_key(&values) else {
            panic!("values was interned when it was defined");
        };
        let (Ok(Found::Value(by_symbol)), Ok(Found::Value(by_name))) =
            (objects.get(arrays, iterator), objects.get(arrays, named))
        else {
            panic!("both are data properties");
        };
        assert_eq!(by_symbol, by_name, "one function under two keys");
        let Ok(Some(property)) = objects.own_property(arrays, iterator) else {
            panic!("it is Array.prototype's own");
        };
        assert!(property.is_writable() && property.is_configurable() && !property.is_enumerable());

        // `%IteratorPrototype%[Symbol.iterator]` is a function too.
        let Ok(Found::Value(Value::Object(itself))) = objects.get(iterators, iterator) else {
            panic!("%IteratorPrototype% has a [Symbol.iterator]");
        };
        assert!(objects.callable(itself).is_some());

        // The tag: a string, and neither writable nor enumerable.
        let Ok(Some(property)) = objects.own_property(array_iterators, tag) else {
            panic!("%ArrayIteratorPrototype% has its own tag");
        };
        assert!(!property.is_writable() && !property.is_enumerable() && property.is_configurable());
        let Some(Value::Text(text)) = property.value() else {
            panic!("the tag is a string");
        };
        assert_eq!(
            objects.units(text).map(String::from_utf16_lossy),
            Some("Array Iterator".to_owned())
        );

        // And each symbol is described as the specification spells it.
        for which in WellKnown::ALL {
            let Ok(held) = intrinsics.well_known(&objects, which) else {
                panic!("{which:?} is rooted");
            };
            let description = objects
                .heap()
                .get(held)
                .and_then(crate::object::Cell::symbol)
                .and_then(crate::object::Symbol::description)
                .and_then(|text| objects.units(text))
                .map(String::from_utf16_lossy);
            assert_eq!(description.as_deref(), Some(which.description()));
        }
    }
}
