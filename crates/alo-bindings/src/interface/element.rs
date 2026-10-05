/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Element`: an element's attributes, by name (queue item 249, item 80's
//! members only).
//!
//! `getAttribute`, `setAttribute` and `removeAttribute`, each `alo-dom`'s
//! operation *by name* ([`alo_dom::by_name`]): a qualified name, lowercased
//! on an HTML element, the first attribute with it, and a name no attribute
//! can have refused with `InvalidCharacterError`. And `remove()`, which is
//! the `ChildNode` mixin's ([`super::child_node`]).
//!
//! # `setAttribute` with two objects
//!
//! Both arguments are `DOMString`s, so an object for either runs the page's
//! `toString`, and the interpreter does that for a native that asks — but a
//! native keeps a step number and nothing else across what it asked for,
//! and the first argument's string is in the slot the second's answer is
//! written to. So when **both** are objects the call is refused by name
//! ([`Missing::ASecondArgumentBehindACall`], queue item 221) rather than
//! converting the first one twice, which a page could count. One object and
//! one primitive is converted in full: a primitive's string needs no script
//! and is read again at no cost.
//!
//! **Not here** (ADR 0017 § 8): `id`, `className`, `classList`, `attributes`
//! (a live map), `tagName`, `hasAttribute`, `toggleAttribute`, the
//! namespaced forms, `innerHTML` and every query. Each is absent until a page
//! or an item needs it.

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::Primitive;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::{child_node, dom_exception};
use crate::define;
use crate::idl::{self, Brand, Converted};

/// `Element.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let operations = [
        ("getAttribute", get_attribute as fn(&mut Call<'_>) -> _),
        ("setAttribute", set_attribute),
        ("removeAttribute", remove_attribute),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    child_node::furnish(objects, prototype, function_prototype)
}

/// `getAttribute(qualifiedName)`: the value, or `null`.
fn get_attribute(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Element, "getAttribute")?;
    idl::needs(call, 1, "getAttribute")?;
    let name = match idl::only_string(call)? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    let value = idl::read(call, this.owner)?
        .attribute_by_name(this.node, &name)
        .map(str::to_owned);
    idl::answer_text(call, value)
}

/// `setAttribute(qualifiedName, value)`.
///
/// Step 1 has the name an object made, with the value still to read; step 2
/// has the value an object made, with the name a primitive to read again.
fn set_attribute(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Element, "setAttribute")?;
    idl::needs(call, 2, "setAttribute")?;
    let (name, value) = match call.step() {
        0 => {
            let name = match idl::string(call, call.argument(0), 1)? {
                Converted::Ready(name) => name,
                Converted::Asked(asked) => return Ok(asked),
            };
            match idl::string(call, call.argument(1), 2)? {
                Converted::Ready(value) => (name, value),
                Converted::Asked(asked) => return Ok(asked),
            }
        }
        1 => {
            let name = idl::answered_string(call)?;
            if Primitive::of(call.argument(1)).is_none() {
                return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
            }
            match idl::string(call, call.argument(1), 2)? {
                Converted::Ready(value) => (name, value),
                Converted::Asked(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
            }
        }
        2 => {
            let value = idl::answered_string(call)?;
            match idl::string(call, call.argument(0), 1)? {
                Converted::Ready(name) => (name, value),
                // Step 2 is reached only with a primitive name.
                Converted::Asked(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
            }
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let changed = idl::change(call, this.owner, |document| {
        document.set_attribute_by_name(this.node, &name, &value)
    })?;
    match changed {
        Ok(_) => Ok(Answer::Value(Value::Undefined)),
        Err(refusal) => Err(dom_exception::thrown(call, this.owner, refusal)),
    }
}

/// `removeAttribute(qualifiedName)`.
fn remove_attribute(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Element, "removeAttribute")?;
    idl::needs(call, 1, "removeAttribute")?;
    let name = match idl::only_string(call)? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    idl::change(call, this.owner, |document| {
        document.remove_attribute_by_name(this.node, &name)
    })?;
    Ok(Answer::Value(Value::Undefined))
}
