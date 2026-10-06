/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `InputEvent` (ADR 0018 §§ 5 and 8, queue item 257): what an agent's
//! `PutText` fires — a `beforeinput`, and an `input` once the text is in.
//!
//! Its members, each read-only, with what replacing a field's text carries
//! in them — the only `InputEvent` this engine makes
//! ([`Shape::Input`](crate::event::Shape)):
//!
//! - `inputType`: `"insertReplacementText"`.
//! - `data`: the text that replaced the field's.
//! - `isComposing`: `false`, since no composition is in progress — an agent
//!   puts its text in whole, and no input method stands between.
//!
//! **Absent** (ADR 0013 § 3): `dataTransfer`, a `DataTransfer`, an
//! interface that does not exist yet — and for a text field Input Events
//! gives it `null`, an answer nobody has asked this engine for;
//! `getTargetRanges()`, whose `StaticRange`s are a contenteditable's; and
//! the constructor, as [`super::ui_event`] says of its family.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::event::Event;

/// The name the brand check's `TypeError` uses.
const NAME: &str = "InputEvent";

/// `InputEvent.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("data", data as fn(&mut Call<'_>) -> _),
        ("isComposing", is_composing),
        ("inputType", input_type),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    Ok(())
}

/// Web IDL's brand check for a member of `InputEvent`: `this` must be one,
/// or the member's `TypeError`. Answers the event.
fn input<'c>(call: &'c Call<'_>, member: &'static str) -> Result<&'c Event, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Event>(held),
        _ => None,
    }
    .filter(|event| event.is_input())
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was read from something that is not an {NAME}"),
            call.at(),
        )
    })
}

/// `units` as a string the page can hold.
fn text(call: &mut Call<'_>, units: Vec<u16>) -> Result<Answer, Escape> {
    let at = call.at();
    let made = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(made)))
}

/// `get data`: the text, or `null`.
fn data(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = input(call, "data")?.data().map(<[u16]>::to_vec);
    match units {
        Some(units) => text(call, units),
        None => Ok(Answer::Value(Value::Null)),
    }
}

/// `get isComposing`: `false`.
fn is_composing(call: &mut Call<'_>) -> Result<Answer, Escape> {
    input(call, "isComposing")?;
    Ok(Answer::Value(Value::Bool(false)))
}

/// `get inputType`.
fn input_type(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = input(call, "inputType")?.input_type().to_vec();
    text(call, units)
}
