/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ChildNode`: the mixin `Element`, `CharacterData` and `DocumentType`
//! include, and so a member on each of their prototypes rather than on
//! `Node`'s — a document has no `remove()` (queue item 249).
//!
//! Its one member here is `remove()`, which takes the node out of whatever
//! holds it and does nothing to a node nothing holds, as the standard says.
//! **Not here:** `before`, `after` and `replaceWith`, absent until a page or
//! an item needs them.

use alo_js::Escape;
use alo_js::Value;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};

use crate::define;
use crate::idl::{self, Brand};

/// The mixin's members, on the prototype of an interface that includes it.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "remove", remove)
}

/// `remove()`.
fn remove(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::ChildNode, "remove")?;
    idl::change(call, this.owner, |document| document.remove(this.node))?;
    Ok(Answer::Value(Value::Undefined))
}
