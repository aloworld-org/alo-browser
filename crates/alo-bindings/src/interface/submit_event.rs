/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `SubmitEvent` (ADR 0051 § 4, queue item 485): what the browser fires at a
//! form as it is submitted — `submit`, after its controls were checked and
//! before anything is asked for. A listener that cancels it keeps the form
//! from being sent, which is how a page sends its form by script instead.
//!
//! Its one member, read-only:
//!
//! - `submitter`: the button the form was submitted from, as the same
//!   wrapper every other reach of that element answers
//!   ([`Shape::Submit`](crate::event::Shape)).
//!
//! **Absent** (ADR 0013 § 3): the constructor, which HTML gives it, as
//! [`super::ui_event`] says of its family — only the browser makes one
//! here, and a page that makes its own is the item that adds it.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::event::Event;

/// The name the brand check's `TypeError` uses.
const NAME: &str = "SubmitEvent";

/// `SubmitEvent.prototype`'s one member.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "submitter",
        submitter,
        None,
    )
}

/// `get submitter`: the button's wrapper, or `null`, once `this` is shown
/// to be one.
fn submitter(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let submitter = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<Event>(held)
            .filter(|event| event.is_submit())
            .map(Event::submitter),
        _ => None,
    };
    match submitter {
        Some(Some(button)) => Ok(Answer::Value(Value::Object(button))),
        Some(None) => Ok(Answer::Value(Value::Null)),
        None => Err(Escape::type_error(
            format!("'submitter' was read from something that is not a {NAME}"),
            call.at(),
        )),
    }
}
