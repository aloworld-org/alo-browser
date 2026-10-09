/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Window` (ADR 0037 § 4, queue item 362): the page's global object.
//!
//! `Window` is Web IDL's `[Global]`, so its regular members are **on the one
//! instance**, not on `Window.prototype`, which holds nothing of its own.
//! `EventTarget` is not `[Global]`, so `addEventListener` stays on
//! `EventTarget.prototype` and the window inherits it.
//!
//! - `window` — `[LegacyUnforgeable]`: on the unforgeables object, copied
//!   onto the instance by [`crate::install`]. An accessor, enumerable, not
//!   configurable, with no setter, answering the window itself.
//! - `self` — `[Replaceable]`: an accessor, enumerable and configurable,
//!   defined on the instance by [`members`]. Its getter answers the window;
//!   its setter defines an own data property `self` holding the value
//!   assigned, so a page's `self = 1` replaces it, as Web IDL says.
//! - `location` — `[LegacyUnforgeable]`, `[PutForwards=href]`: an accessor
//!   answering the page's one `Location` ([`crate::location`]), read
//!   through the window's document. Assigning to it navigates, and is
//!   refused by name until item 85.
//!
//! **`window`, `self`, `globalThis` and the top-level `this` are all the
//! global object.** HTML answers a `WindowProxy` for each, which exists so a
//! reference survives its browsing context navigating to a new global
//! object; with every document in a new realm and no frames, no script can
//! hold another realm's window, so a proxy would forward to the one object
//! it could ever point at (ADR 0037 § 4).
//!
//! Each member's brand check is Web IDL's for a `[Global]` interface: a
//! `this` that is `undefined` or `null` is the realm's global object —
//! found through the realm's host, the document cell, and its window — and
//! anything else that is not a `Window` is a `TypeError`.
//!
//! `document` stays a data property on the global object until item 251
//! makes it an unforgeable accessor here ([`crate::install`]).

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Property, Value};
use alo_js::{Escape, Fault};

use crate::define;
use crate::document_cell::DocumentCell;
use crate::location;
use crate::window::Window;

/// `Window`'s unforgeable members, on its unforgeables object.
pub(super) fn unforgeables(
    objects: &mut Objects,
    unforgeables: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::unforgeable_attribute(
        objects,
        unforgeables,
        function_prototype,
        "window",
        (window, None),
    )?;
    define::unforgeable_attribute(
        objects,
        unforgeables,
        function_prototype,
        "location",
        (get_location, Some(set_location)),
    )
}

/// `Window`'s regular members, on the one instance, `global`, since
/// `Window` is `[Global]`: `self`.
///
/// **A safepoint.** `global` is the realm's global object, which the realm
/// roots.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold them, and a fault for a
/// reference this engine has lost.
pub(crate) fn members(
    objects: &mut Objects,
    global: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        global,
        function_prototype,
        "self",
        window,
        Some(set_self),
    )
}

/// The brand check: `this` as a `Window` — the realm's own when `this` is
/// `undefined` or `null`, as for every `[Global]` interface.
///
/// # Errors
///
/// A `TypeError` naming `member` for anything else.
fn this(call: &Call<'_>, member: &'static str) -> Result<Ref, Escape> {
    let this = match call.this() {
        Value::Undefined | Value::Null => call
            .host_defined()
            .and_then(|cell| call.seen().embedded::<DocumentCell>(cell))
            .and_then(DocumentCell::window),
        Value::Object(held) if call.seen().embedded::<Window>(held).is_some() => Some(held),
        _ => None,
    };
    this.ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a Window"),
            call.at(),
        )
    })
}

/// `get window` and `get self`: the window itself.
fn window(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "window").map(|window| Answer::Value(Value::Object(window)))
}

/// `set self`: `[Replaceable]`, an own data property in its place.
fn set_self(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let window = this(call, "self")?;
    let value = call.argument(0);
    let at = call.at();
    let name: Vec<u16> = "self".encode_utf16().collect();
    match call
        .objects()
        .define_named(window, &name, Property::data(value, true, true, true))
    {
        Ok(true) => Ok(Answer::Value(Value::Undefined)),
        // `CreateDataPropertyOrThrow`: a window made non-extensible after
        // `self` was deleted, which nothing a page can call today does.
        Ok(false) => Err(Escape::type_error("'self' cannot be replaced", at)),
        Err(named) => Err(Escape::named(named, at)),
    }
}

/// `get location`: the page's `Location`, through the window's document.
fn get_location(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let window = this(call, "location")?;
    // A window is given its document before any script runs.
    let document = call
        .seen()
        .embedded::<Window>(window)
        .and_then(Window::document)
        .ok_or(Escape::fault(Fault::Gone))?;
    location::of(call.seen(), document).map(Answer::Value)
}

/// `set location`: `[PutForwards=href]`, a navigation, refused by name.
fn set_location(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "location")?;
    Err(location::refused())
}
