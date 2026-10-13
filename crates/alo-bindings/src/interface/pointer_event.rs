/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `PointerEvent` (ADR 0018 §§ 5 and 8, queue item 256): the interface a
//! `click` is, and so the one an agent's `Activate` fires.
//!
//! - `pointerId` and `pointerType`, read-only, from the event's device: `-1`
//!   and `""` for a click that no pointing device caused, what Pointer Events
//!   gives one, and `1` and `"mouse"` for a person's mouse (queue item 535),
//!   as Chrome 154 numbers it.
//!
//! **Absent** (ADR 0013 § 3): `width`, `height`, `pressure`,
//! `tangentialPressure`, `tiltX`, `tiltY`, `twist`, `altitudeAngle`,
//! `azimuthAngle`, `isPrimary`, `getCoalescedEvents` and
//! `getPredictedEvents` — a device's, wanted by no page yet (queue item
//! 546); and the constructor, as [`super::ui_event`] says.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::ui_event::pointer;
use crate::define;

/// `PointerEvent.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "pointerId",
        pointer_id,
        None,
    )?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "pointerType",
        pointer_type,
        None,
    )
}

/// `get pointerId`: the device's id.
fn pointer_id(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let pointing = pointer(call, "pointerId", "PointerEvent")?;
    Ok(Answer::Value(Value::Number(f64::from(
        pointing.device.id(),
    ))))
}

/// `get pointerType`: what kind of device it is, or the empty string.
fn pointer_type(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let pointing = pointer(call, "pointerType", "PointerEvent")?;
    let at = call.at();
    let kind = call
        .objects()
        .text(pointing.device.kind().encode_utf16().collect())
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(kind)))
}
