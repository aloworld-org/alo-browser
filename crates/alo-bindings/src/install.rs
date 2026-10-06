/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Giving a page's script its document.
//!
//! [`furnish`] makes the prototype of every interface in an engine's realm
//! and gives them to the document cell, which is where a native finds them
//! ([`crate::interface`]). [`install`] does that and puts the document on the
//! global object as `document`: the moment a page's script can reach it.
//!
//! It also puts the two interface objects a page constructs with on the
//! global object — `Event` and `CustomEvent` (ADR 0018 § 8), since a page
//! cannot make an event any other way. Each is a constructor whose
//! `prototype` is the interface's prototype the document cell holds, neither
//! writable nor configurable, with a `constructor` pointing back;
//! `CustomEvent` inherits from `Event`, and `Event` carries the four phase
//! constants as its prototype does. Called without `new`, each is a
//! `TypeError`.
//!
//! # `document` is a value, not yet a getter
//!
//! Web IDL makes `document` an unforgeable **accessor** on the window. Its
//! getter would be a native, and a native is handed its `this` — the global
//! object, an ordinary object here — and nothing else, so it would have
//! nowhere to find the document. Until the global object is a `Window` of its
//! own, `document` is a data property that is neither writable nor
//! configurable, which is what every member a script has today can observe of
//! the accessor: reading it answers the document, assigning to it does
//! nothing (or throws in strict code), and deleting it fails. Only a
//! property descriptor tells the two apart, and
//! `Object.getOwnPropertyDescriptor` is queue item 73's.

use alo_js::builtin::error::Family;
use alo_js::heap::Ref;
use alo_js::interpret::Engine;
use alo_js::object::native::{Body, Instance, Make, Native};
use alo_js::object::{Found, Objects, Property, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::embed::{self, Wrapping};
use crate::event::{make_custom_event, make_event};
use crate::interface::{Inherits, Interface, custom_event, event};

/// Make every interface's prototype in `engine`'s realm and give them to the
/// document `cell` holds.
///
/// **A safepoint.** `cell` must be rooted by the caller.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold them; a fault when `cell` is
/// not a document cell or the realm has lost an intrinsic.
pub fn furnish(engine: &mut Engine, cell: Ref) -> Result<(), Escape> {
    let (intrinsics, objects) = engine.intrinsics();
    let object_prototype = intrinsics.object_prototype(objects)?;
    let function_prototype = intrinsics.function_prototype(objects)?;
    let error = intrinsics.error_constructor(objects, Family::Error)?;
    let error_prototype = objects
        .existing_key(&"prototype".encode_utf16().collect::<Vec<_>>())
        .map(|key| objects.get(error, key))
        .transpose()?;
    let Some(Found::Value(Value::Object(error_prototype))) = error_prototype else {
        return Err(Escape::fault(Fault::Gone));
    };

    let objects = engine.objects();
    if objects.embedded::<DocumentCell>(cell).is_none() {
        return Err(Escape::fault(Fault::NotAnObject));
    }
    for interface in Interface::ALL {
        let above = match interface.inherits() {
            Inherits::Object => Some(object_prototype),
            Inherits::Error => Some(error_prototype),
            Inherits::Interface(parent) => objects
                .embedded::<DocumentCell>(cell)
                .and_then(|held| held.interfaces().prototype(parent)),
        };
        let prototype = objects
            .object(above)
            .map_err(|why| Escape::refused(why, 0))?;
        // Held by the cell before its members are made, so every allocation
        // they cause finds it reachable.
        objects.write_embedded::<DocumentCell, _>(cell, |held, barrier| {
            held.interfaces.set(barrier, interface, prototype);
        });
        interface.furnish(objects, prototype, function_prototype)?;
    }
    Ok(())
}

/// [`furnish`], then put the document on `engine`'s global object as
/// `document`, answering the document node's wrapper.
///
/// **A safepoint.** `cell` must be rooted by the caller.
///
/// # Errors
///
/// As [`furnish`], and [`Escape::Full`] when the heap cannot hold the
/// wrapper.
pub fn install(engine: &mut Engine, cell: Ref) -> Result<Ref, Escape> {
    furnish(engine, cell)?;
    constructors(engine, cell)?;
    let global = engine.global()?;
    let objects = engine.objects();
    let root = embed::document(objects, cell)
        .map(alo_dom::Document::root)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let prototype = crate::interface::prototype_of(objects, cell, root);
    let wrapper = match embed::wrap(objects, cell, root, prototype) {
        Ok(wrapper) => wrapper,
        Err(Wrapping::Refused(refused)) => return Err(Escape::refused(refused, 0)),
        Err(Wrapping::NotADocument | Wrapping::NoSuchNode(_)) => {
            return Err(Escape::fault(Fault::NotAnObject));
        }
    };
    // The document node's wrapper is held by the cell, which the caller
    // roots, so interning the name below cannot take it.
    let name: Vec<u16> = "document".encode_utf16().collect();
    let property = Property::data(Value::Object(wrapper), false, true, false);
    match objects.define_named(global, &name, property) {
        Ok(true) => Ok(wrapper),
        // A page has had no chance to run, so the global object refusing is
        // an embedder installing twice: the first `document` stands.
        Ok(false) => Err(Escape::type_error("this realm already has a document", 0)),
        Err(named) => Err(Escape::named(named, 0)),
    }
}

/// Put `Event` and `CustomEvent` on `engine`'s global object.
///
/// **A safepoint.** Each constructor is held in a scope until the global
/// object owns it, and each prototype is held by the document cell, which the
/// caller roots.
fn constructors(engine: &mut Engine, cell: Ref) -> Result<(), Escape> {
    let global = engine.global()?;
    let (intrinsics, objects) = engine.intrinsics();
    let mut above = intrinsics.function_prototype(objects)?;
    let made: [(Interface, Body, Make); 2] = [
        (Interface::Event, event::construct, make_event),
        (
            Interface::CustomEvent,
            custom_event::construct,
            make_custom_event,
        ),
    ];
    for (interface, body, make) in made {
        let objects = engine.objects();
        let prototype = objects
            .embedded::<DocumentCell>(cell)
            .and_then(|held| held.interfaces().prototype(interface))
            .ok_or(Escape::fault(Fault::Gone))?;
        let scope = objects.heap_mut().open();
        let outcome = constructor(objects, global, prototype, above, interface, body, make);
        objects.heap_mut().close(scope);
        // `CustomEvent` inherits from `Event`, which the global object holds.
        above = outcome?;
    }
    Ok(())
}

/// One interface object, with the scope open, answering it.
fn constructor(
    objects: &mut Objects,
    global: Ref,
    prototype: Ref,
    above: Ref,
    interface: Interface,
    body: Body,
    make: Make,
) -> Result<Ref, Escape> {
    let name = interface.name();
    let function = objects
        .native(
            Native::constructor(name, body, Instance::Made(make)),
            Some(above),
        )
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(function);
    let named = |text: &str| -> Vec<u16> { text.encode_utf16().collect() };
    let properties = [
        (
            function,
            named("prototype"),
            Property::data(Value::Object(prototype), false, false, false),
        ),
        (
            prototype,
            named("constructor"),
            Property::data(Value::Object(function), true, false, true),
        ),
        (
            global,
            named(name),
            Property::data(Value::Object(function), true, false, true),
        ),
    ];
    for (object, units, property) in properties {
        match objects.define_named(object, &units, property) {
            Ok(true) => {}
            // A page has had no chance to run: the global refusing is an
            // embedder installing twice.
            Ok(false) => {
                return Err(Escape::type_error(
                    format!("this realm already has '{name}'"),
                    0,
                ));
            }
            Err(named) => return Err(Escape::named(named, 0)),
        }
    }
    if interface == Interface::Event {
        event::constants(objects, function)?;
    }
    Ok(function)
}
