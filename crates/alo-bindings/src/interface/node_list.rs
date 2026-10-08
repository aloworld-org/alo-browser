/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `NodeList` (queue item 329): the static list `querySelectorAll`
//! answers ([`crate::node_list`]).
//!
//! - `length`, how many nodes it holds.
//! - `item(index)`, the node at `index` or `null`; `index` is an `unsigned
//!   long`, so it is converted as Web IDL converts one — `ToNumber`, then
//!   whole and modulo 2³², with `NaN` and the infinities zero — and an
//!   object's own `valueOf` runs first.
//!
//! The indexed getter, `list[0]`, is the cell's own (`node_list.rs`).
//!
//! **Not here** (queue item 331): `forEach`, `keys`, `values`, `entries`
//! and `[Symbol.iterator]`. Web IDL makes each of them **the very function**
//! `Array.prototype` has under the same name, so a page can compare them
//! with `===`; this engine's `Array.prototype` has no `forEach` yet, and a
//! `NodeList` of its own would be the approximation ADR 0013 § 3 refuses.

use alo_js::abrupt::Internal;
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Value};

use crate::define;
use crate::idl;
use crate::node_list::NodeList;

/// `NodeList.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "length",
        length,
        None,
    )?;
    define::operation(objects, prototype, function_prototype, "item", item)
}

/// Web IDL's brand check: `this` must be a `NodeList`, or the member's
/// `TypeError`.
fn this<'a>(call: &'a Call<'_>, member: &'static str) -> Result<&'a NodeList, Escape> {
    let held = match call.this() {
        Value::Object(held) => call.seen().embedded::<NodeList>(held),
        _ => None,
    };
    held.ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a NodeList"),
            call.at(),
        )
    })
}

/// `get length`.
fn length(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let count = this(call, "length")?.len();
    // An `unsigned long`; a list is one wrapper per node of a document the
    // heap holds, far fewer than that.
    let count = u32::try_from(count).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Number(f64::from(count))))
}

/// `item(index)`.
fn item(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "item")?;
    idl::needs(call, 1, "item")?;
    let primitive = match call.step() {
        0 => match Primitive::of(call.argument(0)) {
            Some(primitive) => primitive,
            None => {
                return Ok(Answer::want(
                    Want::Primitive {
                        of: call.argument(0),
                        hint: Hint::Number,
                    },
                    1,
                ));
            }
        },
        1 => Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?,
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let index = convert::to_uint32(convert::to_number(call.seen(), primitive, call.at())?);
    // Read again after the conversion: the list is the same object, and
    // static, but a borrow does not live across a call into the page.
    let found = usize::try_from(index)
        .ok()
        .and_then(|index| this(call, "item").ok()?.get(index));
    Ok(Answer::Value(found.map_or(Value::Null, Value::Object)))
}
