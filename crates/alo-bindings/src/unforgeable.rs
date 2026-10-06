/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Copying an interface's unforgeable members onto a new instance (ADR 0019
//! § 3).
//!
//! Web IDL's *internally create a new object implementing the interface*:
//! for the interface and every interface it inherits from, each property of
//! that interface's `[[Unforgeables]]` object is defined on the instance as
//! it is. The **property** is copied, not remade, so every event's
//! `isTrusted` holds the one getter its realm made — which a page can see —
//! and the property stays not configurable, so a page cannot delete or
//! redefine it.
//!
//! One function, because two callers make an event — the constructors, at
//! their first step, before any page script can run inside them, and the
//! browser's [`crate::event::create`] — and they must not disagree about what
//! an instance gets.
//!
//! It allocates nothing: the keys are already interned, held by the
//! unforgeables object, and defining an own property is a write to the
//! instance's own table. So it is **not a safepoint**, and an instance
//! nothing holds yet survives it.

use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::{Objects, Property, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::{Inherits, Interface};

/// Define on `instance` the unforgeable members of `interface` and of every
/// interface it inherits from, as the document `cell` holds them.
///
/// # Errors
///
/// A fault when `cell` is not a document cell, or an interface that has
/// unforgeable members has no unforgeables object — its interfaces were
/// never made — and [`Internal::BuiltinIsWrong`] when `instance` refuses one,
/// which a fresh instance never does.
pub(crate) fn copy(
    objects: &mut Objects,
    cell: Ref,
    instance: Ref,
    interface: Interface,
) -> Result<(), Escape> {
    let mut at = Some(interface);
    while let Some(interface) = at {
        if interface.has_unforgeables() {
            let unforgeables = objects
                .embedded::<DocumentCell>(cell)
                .ok_or(Escape::fault(Fault::NotAnObject))?
                .interfaces()
                .unforgeables(interface)
                .ok_or(Escape::fault(Fault::Gone))?;
            copy_from(objects, unforgeables, instance)?;
        }
        at = match interface.inherits() {
            Inherits::Interface(parent) => Some(parent),
            Inherits::Object | Inherits::Error => None,
        };
    }
    Ok(())
}

/// Define each own property of `unforgeables` on `instance`, as it is.
fn copy_from(objects: &mut Objects, unforgeables: Ref, instance: Ref) -> Result<(), Escape> {
    for key in objects.own_keys(unforgeables)? {
        let Some(property) = objects.own_property(unforgeables, key)? else {
            continue;
        };
        let copied = match property.value() {
            Some(value) => Property::data(
                value,
                property.is_writable(),
                property.is_enumerable(),
                property.is_configurable(),
            ),
            None => Property::accessor(
                property.getter().unwrap_or(Value::Undefined),
                property.setter().unwrap_or(Value::Undefined),
                property.is_enumerable(),
                property.is_configurable(),
            ),
        };
        if !objects.define(instance, key, copied)? {
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        }
    }
    Ok(())
}
