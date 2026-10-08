/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Headers` (ADR 0032 § 4, queue item 335): a response's headers, read.
//!
//! `get(name)` answers every value under `name` joined by `", "`, or `null`;
//! `has(name)` whether there is one. A name that is not a header name is the
//! `TypeError` Fetch throws for one. Both take a `ByteString`, so a name
//! with a code unit past `0xFF` is a `TypeError` too.
//!
//! **Absent**: `append`, `set`, `delete`, `getSetCookie` (a page never sees
//! a `Set-Cookie`, ADR 0032 § 4), iteration and the constructor — a
//! `Headers` a page builds or walks is its own item.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::headers::Headers;
use crate::idl::{self, Converted};

/// `Headers.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "get", get)?;
    define::operation(objects, prototype, function_prototype, "has", has)
}

/// Web IDL's brand check: `this` must be a `Headers`, or the member's
/// `TypeError`. Answers it.
fn this<'c>(call: &'c Call<'_>, member: &'static str) -> Result<&'c Headers, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Headers>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a Headers"),
            call.at(),
        )
    })
}

/// The member's one argument as a header name — or what to ask for first.
///
/// # Errors
///
/// A `TypeError` for too few arguments, for a code unit past `0xFF`, and for
/// a name that is not a header name.
fn name(call: &Call<'_>, member: &'static str) -> Result<Result<String, Answer>, Escape> {
    this(call, member)?;
    idl::needs(call, 1, member)?;
    let name = match idl::only_string(call)? {
        Converted::Ready(name) => name,
        Converted::Asked(answer) => return Ok(Err(answer)),
    };
    if name.chars().any(|unit| u32::from(unit) > 0xFF) {
        return Err(Escape::type_error(
            format!("argument 1 to '{member}' is not a ByteString"),
            call.at(),
        ));
    }
    if !alo_net::forbidden::is_a_header_name(&name) {
        return Err(Escape::type_error(
            format!("{name:?} is not a header name"),
            call.at(),
        ));
    }
    Ok(Ok(name))
}

/// `get(name)`.
fn get(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let name = match name(call, "get")? {
        Ok(name) => name,
        Err(answer) => return Ok(answer),
    };
    let value = this(call, "get")?.get(&name);
    idl::answer_text(call, value)
}

/// `has(name)`.
fn has(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let name = match name(call, "has")? {
        Ok(name) => name,
        Err(answer) => return Ok(answer),
    };
    Ok(Answer::Value(Value::Bool(this(call, "has")?.has(&name))))
}
