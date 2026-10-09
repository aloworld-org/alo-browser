/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `location`: where the page is, as its script reads it (queue item 360).
//!
//! The page's one [`Location`] is an embedder cell holding an edge to the
//! document cell — its *relevant document* — and nothing else: every member
//! reads the document's URL **when it is asked**, so the answer is always
//! the address the browser process stated last ([`crate::navigating::locate`])
//! and never a copy taken when the object was made. A document nobody said
//! an address for is at `about:blank`, which is HTML's own answer for a
//! document with no other URL, so `location.href` is `"about:blank"` and
//! `location.origin` is `"null"` rather than anything invented.
//!
//! [`make`] is HTML's *Location object creation*: a new `Location` with its
//! interface's unforgeables copied onto it (every member of `Location` is
//! `[LegacyUnforgeable]`, [`crate::interface::location`]), then two own
//! properties neither writable, enumerable nor configurable — `valueOf`,
//! which is the realm's own `Object.prototype.valueOf`, and
//! `Symbol.toPrimitive`, which is `undefined`. So `"" + location` is
//! `location.href` by way of `toString`, as in every browser.
//!
//! It is reached from two places, each answering the same object: the
//! document's own `location` (`Document`'s unforgeable member,
//! [`crate::interface::document`]) and the global object's, which [`make`]
//! puts there.
//!
//! # The global object's `location` is an accessor on an ordinary object
//!
//! On a `Window`, `location` is a `[LegacyUnforgeable]`,
//! `[PutForwards=href]` attribute. Until the global object is a `Window`
//! (item 251) it is an accessor on the global object itself — enumerable,
//! not configurable — whose getter answers the realm's `Location`, found
//! through the realm's `[[HostDefined]]` (ADR 0019 § 2), and whose setter is
//! assigning to `location.href`. With no `Window` brand to check, the getter
//! answers whatever its `this`; that is the one thing it does that a
//! `Window`'s would not, and only a getter taken off the global object can
//! show it.
//!
//! # Everything that navigates is refused by name
//!
//! Assigning to `location`, to `location.href` or to any other part of it,
//! and `assign`, `replace` and `reload`, are each a navigation, which a page
//! asks for and the browser process decides (ADR 0020) and which is item
//! 85's to build through that ask. Until then each is refused by name —
//! ending the script with a sentence saying what is not built — rather than
//! doing nothing a page could mistake for having gone somewhere. The value
//! assigned is not converted first: a refusal ends the run either way.
//!
//! # What is not here
//!
//! `ancestorOrigins` is absent: with no frames (item 86) it is always an
//! empty list, and it waits for a page that reads it. `Location`'s exotic
//! internal methods — `[[PreventExtensions]]` refusing, an immutable
//! prototype, and the default properties' descriptors reported configurable
//! — are queue item 361's; until then a `Location` is an ordinary object
//! carrying unforgeable members, which every reading member is unaffected
//! by.

use alo_js::abrupt::Missing;
use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::interpret::Engine;
use alo_js::object::native::{Answer, Call, Native};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Exotic, Found, Internal, Key, Objects, Ordinary, Property, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::Interface;
use crate::unforgeable;

/// The `Location` a page's script holds.
#[derive(Debug)]
pub struct Location {
    own: Ordinary,
    /// The document cell whose URL it reads.
    document: Field,
}

impl Location {
    /// The document cell whose URL it reads.
    pub const fn document(&self) -> Option<Ref> {
        self.document.get()
    }
}

impl Internal for Location {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Location {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.document.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for Location {
    fn describe(&self) -> &'static str {
        "a Location"
    }
}

/// What every navigating member is refused as, until item 85.
pub(crate) const NAVIGATING: &str = "a script navigating by 'location' is queue item 85, through \
                                     the ask of ADR 0020";

/// The refusal by name of a navigation through `location`.
pub(crate) const fn refused() -> Escape {
    Escape::NotBuiltYet(Missing::InTheEmbedder(NAVIGATING))
}

/// Make the page's `Location` in the document `cell` holds, keep it there,
/// and put it on `engine`'s global object as `location`, answering it.
///
/// Called by [`crate::install`], after the interfaces are made.
///
/// **A safepoint.** `cell` must be rooted by the caller; the `Location` is
/// the cell's from the moment it is made.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold it; a fault when `cell` is not
/// a document cell, its interfaces were never made or the realm has lost an
/// intrinsic; a `TypeError` when the global object already has a
/// `location` — an embedder making it twice.
pub(crate) fn make(engine: &mut Engine, cell: Ref) -> Result<Ref, Escape> {
    let global = engine.global()?;
    let (intrinsics, objects) = engine.intrinsics();
    let object_prototype = intrinsics.object_prototype(objects)?;
    let function_prototype = intrinsics.function_prototype(objects)?;
    let to_primitive = intrinsics.well_known_key(objects, WellKnown::ToPrimitive)?;
    let objects = engine.objects();
    let value_of_key = objects
        .existing_key(&units("valueOf"))
        .ok_or(Escape::fault(Fault::Gone))?;
    let Found::Value(value_of @ Value::Object(_)) = objects.get(object_prototype, value_of_key)?
    else {
        return Err(Escape::fault(Fault::Gone));
    };

    let held = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    if held.location().is_some() {
        return Err(Escape::type_error("this realm already has a location", 0));
    }
    let prototype = held
        .interfaces()
        .prototype(Interface::Location)
        .ok_or(Escape::fault(Fault::Gone))?;
    let location = Location {
        own: Ordinary::with_prototype(Some(prototype)),
        document: Field::holding(cell),
    };
    let made = objects
        .foreign(Box::new(location))
        .map_err(|why| Escape::refused(why, 0))?;
    // Held by the cell before anything else allocates.
    objects.write_embedded::<DocumentCell, _>(cell, |held, barrier| {
        held.location.set(barrier, Some(made));
    });

    unforgeable::copy(objects, cell, made, Interface::Location)?;
    for (key, value) in [(value_of_key, value_of), (to_primitive, Value::Undefined)] {
        if !objects.define(made, key, Property::data(value, false, false, false))? {
            return Err(Escape::fault(Fault::NotAnObject));
        }
    }
    on_the_global(objects, global, function_prototype)?;
    Ok(made)
}

/// Put `location` on `global`: an accessor, enumerable and not
/// configurable, read by [`window_location`] and written by
/// [`set_window_location`].
///
/// **A safepoint.** Each function is held in a scope until the global object
/// owns it.
fn on_the_global(
    objects: &mut Objects,
    global: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let name = units("location");
    if let Some(key) = objects.existing_key(&name)
        && objects.own_property(global, key)?.is_some()
    {
        return Err(Escape::type_error("this realm already has a location", 0));
    }
    let scope = objects.heap_mut().open();
    let outcome = held_on_the_global(objects, global, function_prototype, &name);
    objects.heap_mut().close(scope);
    outcome
}

/// [`on_the_global`], with the scope open.
fn held_on_the_global(
    objects: &mut Objects,
    global: Ref,
    function_prototype: Ref,
    name: &[u16],
) -> Result<(), Escape> {
    let mut made = [Value::Undefined; 2];
    for (slot, body) in made.iter_mut().zip([
        window_location as fn(&mut Call<'_>) -> _,
        set_window_location,
    ]) {
        let function = objects
            .native(Native::new("location", body), Some(function_prototype))
            .map_err(|why| Escape::refused(why, 0))?;
        objects.heap_mut().hold(function);
        *slot = Value::Object(function);
    }
    let [getter, setter] = made;
    match objects.define_named(
        global,
        name,
        Property::accessor(getter, setter, true, false),
    ) {
        Ok(true) => Ok(()),
        // A global object refusing a new property is a reference to
        // something else: this crate's bug, not a page's.
        Ok(false) => Err(Escape::fault(Fault::NotAnObject)),
        Err(named) => Err(Escape::named(named, 0)),
    }
}

/// The `Location` of the document cell `cell`, or `null` for a document
/// with none — one that is no page's, made by [`crate::furnish`] alone.
pub(crate) fn of(objects: &Objects, cell: Ref) -> Result<Value, Escape> {
    let held = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    Ok(held.location().map_or(Value::Null, Value::Object))
}

/// The global object's `get location`: the realm's `Location`.
fn window_location(call: &mut Call<'_>) -> Result<Answer, Escape> {
    // A realm with no host is an embedder that never installed a document,
    // and could not have made this getter.
    let document = call.host_defined().ok_or(Escape::fault(Fault::Gone))?;
    of(call.seen(), document).map(Answer::Value)
}

/// The global object's `set location`: `[PutForwards=href]`, a navigation.
fn set_window_location(_call: &mut Call<'_>) -> Result<Answer, Escape> {
    Err(refused())
}

/// `text` as UTF-16 code units.
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
