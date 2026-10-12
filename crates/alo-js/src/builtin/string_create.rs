/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `StringCreate` (ECMA-262 § 10.4.3.4, queue item 488): a `String` object
//! of a string, with its `length` and one property per code unit.
//!
//! The cell is [`StringObject`](crate::object::StringObject), which says why
//! the properties are stored rather than worked out. Storing them means
//! allocating — one string per code unit — and a cell may not allocate, so
//! the making is here. Today only `String.prototype` is made, of the empty
//! string; `ToObject` of a string, which would make one per string a page
//! wraps, is queue item 489's, and it decides whether making every index at
//! once is affordable before anything calls this with a long string.

use crate::abrupt::Escape;
use crate::heap::Ref;
use crate::object::{Key, Objects, Property, Value};

/// Make a `String` object of `data`, inheriting from `prototype`.
///
/// **This allocates repeatedly.** Everything made is held in a scope until
/// the object owns it; `data` and `prototype` are the caller's to have
/// rooted, and the object comes back held by nothing.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a `data` that
/// is not a string.
pub(super) fn string_create(
    objects: &mut Objects,
    prototype: Option<Ref>,
    data: Ref,
) -> Result<Ref, Escape> {
    let scope = objects.heap_mut().open();
    let outcome = created(objects, prototype, data);
    objects.heap_mut().close(scope);
    outcome
}

/// [`string_create`], with the scope already open.
fn created(objects: &mut Objects, prototype: Option<Ref>, data: Ref) -> Result<Ref, Escape> {
    let units = objects
        .units(data)
        .map(<[u16]>::to_vec)
        .ok_or_else(|| Escape::fault(crate::object::Fault::NotAnObject))?;
    let name: Vec<u16> = "length".encode_utf16().collect();
    let length = objects.key(&name).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = length.reference() {
        objects.heap_mut().hold(held);
    }
    let object = objects
        .string_object(prototype, data)
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(object);

    // A string is at most `LONGEST_STRING` units, far inside a `u32` and
    // exactly representable as a number.
    let count =
        u32::try_from(units.len()).map_err(|_| Escape::fault(crate::object::Fault::NotAnObject))?;
    objects.define(
        object,
        length,
        Property::data(Value::Number(f64::from(count)), false, false, false),
    )?;
    for (at, unit) in (0..count).zip(units) {
        let Some(key) = Key::index(at) else {
            break;
        };
        // Owned by the object the moment it is defined, before anything else
        // is allocated.
        let one = objects
            .text(vec![unit])
            .map_err(|why| Escape::refused(why, 0))?;
        objects.define(
            object,
            key,
            Property::data(Value::Text(one), false, true, false),
        )?;
    }
    Ok(object)
}

#[cfg(test)]
mod tests {
    use super::string_create;
    use crate::object::{Key, Objects, Value};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_string_object_has_its_length_and_a_fixed_property_per_unit() {
        for stress in [false, true] {
            let mut objects = Objects::new();
            objects.heap_mut().stress(stress);
            let Ok(data) = objects.text(vec![0x61, 0xD83D]) else {
                panic!("a string is made");
            };
            let data_root = objects.heap_mut().root(data);
            let Ok(object) = string_create(&mut objects, None, data) else {
                panic!("the object is made");
            };
            let object_root = objects.heap_mut().root(object);
            let Ok(length) = objects.key(&units("length")) else {
                panic!("the name is interned");
            };
            let Ok(Some(property)) = objects.own_property(object, length) else {
                panic!("length is its own");
            };
            assert_eq!(property.value(), Some(Value::Number(2.0)));
            assert!(!property.is_writable() && !property.is_enumerable());
            assert!(!property.is_configurable());
            let Some(second) = Key::index(1) else {
                panic!("one is an index");
            };
            let Ok(Some(property)) = objects.own_property(object, second) else {
                panic!("the second unit is a property");
            };
            assert!(!property.is_writable() && property.is_enumerable());
            assert!(!property.is_configurable());
            let Some(Value::Text(unit)) = property.value() else {
                panic!("its value is a string");
            };
            assert_eq!(
                objects.units(unit),
                Some([0xD83D].as_slice()),
                "a lone surrogate is a unit of its own"
            );
            assert_eq!(
                objects.own_keys(object).ok(),
                Key::index(0)
                    .zip(Some(second))
                    .map(|(first, second)| vec![first, second, length]),
                "two indices, then length"
            );
            assert_eq!(
                objects
                    .as_string_object(object)
                    .map(crate::object::StringObject::data),
                Some(data)
            );
            assert_eq!(
                objects.delete(object, length),
                Ok(false),
                "nothing it was made with goes"
            );
            objects.heap_mut().release(object_root);
            objects.heap_mut().release(data_root);
        }
    }
}
