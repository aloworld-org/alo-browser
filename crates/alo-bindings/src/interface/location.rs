/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Location` (queue item 360): where the page is.
//!
//! **Every member is `[LegacyUnforgeable]`**, so every one is on the
//! interface's unforgeables object, copied onto the page's one `Location`
//! as it is made ([`crate::location::make`]), and `Location.prototype` holds
//! nothing of its own.
//!
//! The reading members each answer the URL Standard's reading of the
//! document's URL, written once in `alo-url`'s `reading` and asked here at
//! the moment of the read:
//!
//! - `href`, and `toString()`, which is its stringifier;
//! - `origin`, `protocol`, `host`, `hostname`, `port`, `pathname`, `search`
//!   and `hash`.
//!
//! Every setter — `href`'s and each part's except `origin`, which has none —
//! and `assign()`, `replace()` and `reload()` navigate, and are refused by
//! name until item 85 ([`crate::location`] says why).
//!
//! Each member checks that its `this` is a `Location`, and is the `TypeError`
//! Web IDL's brand check gives otherwise.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};
use alo_url::{Url, reading};

use crate::define;
use crate::document_cell::DocumentCell;
use crate::location::{self, Location};

/// A member's body.
type Body = fn(&mut Call<'_>) -> Result<Answer, Escape>;

/// `Location`'s unforgeable members, on its unforgeables object.
pub(super) fn unforgeables(
    objects: &mut Objects,
    unforgeables: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let attributes: [(&'static str, Body, Option<Body>); 9] = [
        ("href", href, Some(navigate)),
        ("origin", origin, None),
        ("protocol", protocol, Some(navigate)),
        ("host", host, Some(navigate)),
        ("hostname", hostname, Some(navigate)),
        ("port", port, Some(navigate)),
        ("pathname", pathname, Some(navigate)),
        ("search", search, Some(navigate)),
        ("hash", hash, Some(navigate)),
    ];
    for (name, get, set) in attributes {
        define::unforgeable_attribute(objects, unforgeables, function_prototype, name, (get, set))?;
    }
    let operations: [(&'static str, Body); 4] = [
        ("assign", assign),
        ("replace", replace),
        ("reload", reload),
        ("toString", href_string),
    ];
    for (name, body) in operations {
        define::unforgeable_operation(objects, unforgeables, function_prototype, name, body)?;
    }
    Ok(())
}

/// Web IDL's brand check: `this` must be a `Location`, or the member's
/// `TypeError`. Answers the document cell it reads.
fn this(call: &Call<'_>, member: &'static str) -> Result<Ref, Escape> {
    let location = match call.this() {
        Value::Object(held) => call.seen().embedded::<Location>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a Location"),
            call.at(),
        )
    })?;
    // A `Location` is made holding its document, and holds it for life.
    location.document().ok_or(Escape::fault(Fault::Gone))
}

/// After the brand check, `read` of the document's URL, as a string the page
/// can hold.
fn reads(
    call: &mut Call<'_>,
    member: &'static str,
    read: fn(&Url) -> String,
) -> Result<Answer, Escape> {
    let document = this(call, member)?;
    let units: Vec<u16> = call
        .seen()
        .embedded::<DocumentCell>(document)
        .ok_or(Escape::fault(Fault::NotAnObject))
        .map(|held| read(held.url()))?
        .encode_utf16()
        .collect();
    let at = call.at();
    let made = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(made)))
}

/// `get href`.
fn href(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "href", reading::href)
}

/// `toString()`: the stringifier, which is `href`.
fn href_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "toString", reading::href)
}

/// `get origin`.
fn origin(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "origin", reading::origin)
}

/// `get protocol`.
fn protocol(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "protocol", reading::protocol)
}

/// `get host`.
fn host(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "host", reading::host)
}

/// `get hostname`.
fn hostname(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "hostname", reading::hostname)
}

/// `get port`.
fn port(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "port", reading::port)
}

/// `get pathname`.
fn pathname(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "pathname", reading::pathname)
}

/// `get search`.
fn search(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "search", reading::search)
}

/// `get hash`.
fn hash(call: &mut Call<'_>) -> Result<Answer, Escape> {
    reads(call, "hash", reading::hash)
}

/// Every setter: after the brand check, a navigation, refused by name.
fn navigate(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "a part's setter")?;
    Err(location::refused())
}

/// `assign(url)`: a navigation, refused by name.
fn assign(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "assign")?;
    Err(location::refused())
}

/// `replace(url)`: a navigation, refused by name.
fn replace(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "replace")?;
    Err(location::refused())
}

/// `reload()`: a navigation, refused by name.
fn reload(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "reload")?;
    Err(location::refused())
}
