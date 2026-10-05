/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `DOMException`: what a refused change throws (ADR 0017 § 5).
//!
//! `alo-dom` refuses a change with a [`Refusal`] that names the standard's
//! exception — `HierarchyRequestError`, `NotFoundError`,
//! `InvalidCharacterError` — and this is that refusal as the object a page's
//! `catch` receives: `name` and `message` as the standard gives them, and
//! **`Error.prototype` on its chain**, so `Error.prototype.isPrototypeOf(e)`
//! holds and `e.toString` is the language's own.
//!
//! As Web IDL defines it, `name` and `message` are **getters on
//! `DOMException.prototype`** reading the exception's own slots, not own
//! properties of it — so the object is an embedder cell holding the two, with
//! an ordinary part for whatever a page hangs off it. `e.toString()` reaches
//! `Error.prototype.toString`, which reads `name` through its getter and then
//! refuses a `message` behind one by name (queue item 228); that is the
//! engine's limit, met honestly, rather than a reason to make `message` an
//! own property the standard says it is not.
//!
//! **Not here:** the `DOMException` constructor on the global object, the
//! legacy `code` attribute and its constants (law 1), and the other
//! exception names. A page cannot make one; it can only be thrown one.

use alo_dom::Refusal;
use alo_js::abrupt::Thrown;
use alo_js::heap::{Barrier, Ref, Trace, Tracer};
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property};
use alo_js::{Escape, Value};

use super::Interface;
use crate::define;
use crate::idl;

/// A `DOMException`: its name, its message, and an ordinary object's part.
#[derive(Debug)]
pub struct DomException {
    name: &'static str,
    message: &'static str,
    own: Ordinary,
}

impl DomException {
    /// The exception `refusal` is, inheriting from `prototype`.
    pub fn new(refusal: Refusal, prototype: Option<Ref>) -> Self {
        Self {
            name: refusal.name(),
            message: refusal.message(),
            own: Ordinary::with_prototype(prototype),
        }
    }

    /// Its name, as the standard spells it.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Which rule refused, in words.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl Internal for DomException {
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

impl Trace for DomException {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for DomException {
    fn describe(&self) -> &'static str {
        "a DOMException"
    }
}

/// The escape a member answers with when `alo-dom` refused its change:
/// `refusal` as a `DOMException`, thrown, inheriting from the prototype the
/// document cell holds.
///
/// **A safepoint**, and the last thing a member does: the exception is in a
/// Rust local from here until the interpreter lands it, so nothing may
/// allocate in between (`interpret/catch.rs`).
pub(crate) fn thrown(call: &mut Call<'_>, owner: Ref, refusal: Refusal) -> Escape {
    let at = call.at();
    let prototype = match idl::owner_cell(call, owner) {
        Ok(held) => held.interfaces.prototype(Interface::DomException),
        Err(broken) => return broken,
    };
    match call
        .objects()
        .foreign(Box::new(DomException::new(refusal, prototype)))
    {
        Ok(exception) => Escape::Thrown(Thrown::Value {
            value: Value::Object(exception),
            at,
        }),
        Err(refused) => Escape::refused(refused, at),
    }
}

/// `DOMException.prototype`'s two attributes.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(objects, prototype, function_prototype, "name", name, None)?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "message",
        message,
        None,
    )
}

/// The exception `this` is.
fn this<'a>(call: &'a Call<'_>, member: &'static str) -> Result<&'a DomException, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<DomException>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was read from something that is not a DOMException"),
            call.at(),
        )
    })
}

/// `get name`.
fn name(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let name = this(call, "name")?.name();
    idl::answer_text(call, Some(name.to_owned()))
}

/// `get message`.
fn message(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let message = this(call, "message")?.message();
    idl::answer_text(call, Some(message.to_owned()))
}
