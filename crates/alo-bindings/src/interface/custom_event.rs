/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `CustomEvent`: an event carrying a page's own `detail` (ADR 0018 § 8,
//! queue item 254).
//!
//! `new CustomEvent(type, { detail })` is `Event`'s constructor with one
//! more member in its dictionary, after the three it inherits; `detail` is
//! `null` unless given, and read back as the very value given. Its prototype
//! inherits from `Event.prototype`, and its constructor from `Event`.
//!
//! **Absent** (law 1): `initCustomEvent`, the legacy form of the
//! constructor.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::Interface;
use super::event::constructed;
use crate::define;
use crate::event::Event;

/// `CustomEvent.prototype`'s one member.
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

/// `new CustomEvent(type, eventInitDict)`.
pub(crate) fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constructed(call, Interface::CustomEvent)
}

/// `get detail`.
fn detail(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Event>(held),
        _ => None,
    }
    .filter(|event| event.is_custom())
    .map(|event| Answer::Value(event.detail()))
    .ok_or_else(|| {
        Escape::type_error(
            "'detail' was read from something that is not a CustomEvent",
            call.at(),
        )
    })
}
