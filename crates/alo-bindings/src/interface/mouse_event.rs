/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `MouseEvent` (ADR 0018 §§ 5 and 8, queue item 256).
//!
//! The members a click carries, each read-only, with what a click keyboard
//! activation fires carries in them — the only `MouseEvent` this engine
//! makes ([`Shape::Pointer`](crate::event::Shape)):
//!
//! - `screenX`, `screenY`, `clientX` and `clientY`: `0`. Not a position
//!   somebody pressed — nothing pointed — and not one made up (ADR 0002).
//! - `ctrlKey`, `shiftKey`, `altKey` and `metaKey`: `false`.
//! - `button` and `buttons`: `0`.
//! - `relatedTarget`: `null`.
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

use super::ui_event::pointer;
use crate::define;

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
    pointer(call, member, NAME)?;
    Ok(Answer::Value(value))
}

/// `get screenX`.
fn screen_x(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "screenX", Value::Number(0.0))
}

/// `get screenY`.
fn screen_y(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "screenY", Value::Number(0.0))
}

/// `get clientX`.
fn client_x(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "clientX", Value::Number(0.0))
}

/// `get clientY`.
fn client_y(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "clientY", Value::Number(0.0))
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
    answer(call, "button", Value::Number(0.0))
}

/// `get buttons`.
fn buttons(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "buttons", Value::Number(0.0))
}

/// `get relatedTarget`.
fn related_target(call: &mut Call<'_>) -> Result<Answer, Escape> {
    answer(call, "relatedTarget", Value::Null)
}
