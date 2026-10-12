/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `URLSearchParams` (queue item 472): names and values a page builds,
//! opened by the contact form's script, which sends
//! `new URLSearchParams(new FormData(form))` as a fetch's body.
//!
//! - `new URLSearchParams(init)`: `init` converted by Web IDL's union
//!   ([`crate::search_params_init`]), one step at a time, into the
//!   instance's list. Called without `new` it is a `TypeError`.
//! - `toString()`: the list as `application/x-www-form-urlencoded`, which
//!   is also what it is as a body ([`crate::body_init`]).
//!
//! **Absent** until a page needs them: `size`, `append`, `delete`, `get`,
//! `getAll`, `has`, `set`, `sort` and the iteration members. With none of
//! them a page cannot change a list once made, so what it sends is what it
//! built.

use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Instance, Native};
use alo_js::{Escape, Value};

use crate::define;
use crate::idl;
use crate::search_params::{SearchParams, make_search_params};
use crate::search_params_init;

/// The constructor, keeping what it has in hand while it walks `init`.
pub(crate) const CONSTRUCTOR: Native = Native::constructor(
    "URLSearchParams",
    construct,
    Instance::Made(make_search_params),
)
.keeping(search_params_init::SLOTS);

/// `URLSearchParams.prototype`'s member.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(
        objects,
        prototype,
        function_prototype,
        "toString",
        to_string,
    )
}

/// `new URLSearchParams(init)`.
fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if !call.constructing() {
        return Err(Escape::type_error(
            "'URLSearchParams' is a constructor, and needs 'new'",
            call.at(),
        ));
    }
    let Value::Object(instance) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    if call.seen().embedded::<SearchParams>(instance).is_none() {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    match search_params_init::read(call, instance)? {
        Some(asked) => Ok(asked),
        None => Ok(Answer::Value(Value::Object(instance))),
    }
}

/// `toString()`.
fn to_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let written = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<SearchParams>(held)
            .map(SearchParams::serialized),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            "'toString' was used on something that is not a URLSearchParams",
            call.at(),
        )
    })?;
    idl::answer_text(call, Some(written))
}
