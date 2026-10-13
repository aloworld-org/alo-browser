/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `MouseEvent` (ADR 0018 §§ 5 and 8, queue item 256).
//!
//! The members a click carries, each read-only, read from the event's
//! [`Pointing`](crate::pointing::Pointing) — a click keyboard activation
//! fires, or a person's mouse (queue item 535):
//!
//! - `clientX` and `clientY`: where the pointer was, in CSS pixels from the
//!   viewport's corner; `0` for a keyboard's click, which is not a position
//!   somebody pressed and not one made up (ADR 0002).
//! - `screenX` and `screenY`: the same numbers, the window as though it
//!   were at the screen's origin — where it is on the person's screen is
//!   beyond the window (ADR 0038 § 1).
//! - `button` and `buttons`: what changed and what is held.
//! - `ctrlKey`, `shiftKey`, `altKey` and `metaKey`: `false`. A person's
//!   click with a modifier held is not sent to a page yet (queue item 545).
//! - `relatedTarget`: the node a person's pointer crossed from or to, for
//!   the events it fires as it crosses (queue item 542); `null` on every
//!   other.
//!
//! **Absent** (ADR 0013 § 3): `getModifierState`, which converts its
//! argument and is wanted by no page yet; CSSOM View's `pageX`, `pageY`,
//! `offsetX`, `offsetY`, `x` and `y`, whose values for a keyboard's click
//! are relative to a box rather than `0`, and so are not this table's;
//! `movementX`/`movementY` (Pointer Lock); law 1's `initMouseEvent`; and
//! the constructor, as [`super::ui_event`] says.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::ui_event::mouse;
use crate::define;
use crate::event::Event;
use crate::pointing::Pointing;

/// The name the brand check's `TypeError` uses.
const NAME: &str = "MouseEvent";

/// `MouseEvent.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("screenX", screen_x as fn(&mut Call<'_>) -> _),
        ("screenY", screen_y),
        ("clientX", client_x),
        ("clientY", client_y),
        ("ctrlKey", ctrl_key),
        ("shiftKey", shift_key),
        ("altKey", alt_key),
        ("metaKey", meta_key),
        ("button", button),
        ("buttons", buttons),
        ("relatedTarget", related_target),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    Ok(())
}

/// `member`'s answer, `value`, after the brand check.
fn answer(call: &Call<'_>, member: &'static str, value: Value) -> Result<Answer, Escape> {
    mouse(call, member, NAME)?;
    Ok(Answer::Value(value))
}

/// `member`'s answer, a number `read` from the event's pointer, after the
/// brand check.
fn number(
    call: &Call<'_>,
    member: &'static str,
    read: fn(Pointing) -> f64,
) -> Result<Answer, Escape> {
    let pointing = mouse(call, member, NAME)?;
    Ok(Answer::Value(Value::Number(read(pointing))))
}

/// `get screenX`.
fn screen_x(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "screenX", |at| f64::from(at.x))
}

/// `get screenY`.
fn screen_y(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "screenY", |at| f64::from(at.y))
}

/// `get clientX`.
fn client_x(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "clientX", |at| f64::from(at.x))
}

/// `get clientY`.
fn client_y(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "clientY", |at| f64::from(at.y))
}

/// `get ctrlKey`.
fn ctrl_key(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "ctrlKey", Value::Bool(false))
}

/// `get shiftKey`.
fn shift_key(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "shiftKey", Value::Bool(false))
}

/// `get altKey`.
fn alt_key(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "altKey", Value::Bool(false))
}

/// `get metaKey`.
fn meta_key(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "metaKey", Value::Bool(false))
}

/// `get button`.
fn button(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "button", |at| f64::from(at.button))
}

/// `get buttons`.
fn buttons(call: &mut Call<'_>) -> Result<Answer, Escape> {
    number(call, "buttons", |at| f64::from(at.buttons))
}

/// `get relatedTarget`: the other node's wrapper, or `null`.
fn related_target(call: &mut Call<'_>) -> Result<Answer, Escape> {
    mouse(call, "relatedTarget", NAME)?;
    let related = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<Event>(held)
            .and_then(Event::related_target),
        _ => None,
    };
    Ok(Answer::Value(related.map_or(Value::Null, Value::Object)))
}
