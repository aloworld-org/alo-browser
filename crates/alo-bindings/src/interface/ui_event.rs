/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `UIEvent` (ADR 0018 §§ 5 and 8, queue item 256).
//!
//! Between `Event` and `MouseEvent` in a click's chain. Its one member here
//! is `detail`, read-only, which for a click is how many times the pointer
//! was pressed in quick succession — `0` for a click keyboard activation
//! fires, which is the only `UIEvent` this engine makes
//! ([`Shape::Pointer`](crate::event::Shape)).
//!
//! **Absent**: `view`, a `Window`, which the global object is not until
//! item 251; `which`, law 1's; and the constructor, since a page makes no
//! event of this family yet that a test or a page has asked for — each of
//! the three would be its own item, not a member here answering wrongly.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::event::Event;

/// `UIEvent.prototype`'s one member.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "detail",
        detail,
        None,
    )
}

/// Web IDL's brand check for a member of `UIEvent`, `MouseEvent` or
/// `PointerEvent`, whose instances are all [`Shape::Pointer`] events:
/// `this` must be one, or the member's `TypeError`.
///
/// [`Shape::Pointer`]: crate::event::Shape::Pointer
pub(super) fn pointer(
    call: &Call<'_>,
    member: &'static str,
    interface: &'static str,
) -> Result<(), Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Event>(held),
        _ => None,
    }
    .filter(|event| event.is_pointer())
    .map(drop)
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was read from something that is not a {interface}"),
            call.at(),
        )
    })
}

/// `get detail`: `0`, for a click nothing pressed.
fn detail(call: &mut Call<'_>) -> Result<Answer, Escape> {
    pointer(call, "detail", "UIEvent")?;
    Ok(Answer::Value(Value::Number(0.0)))
}
