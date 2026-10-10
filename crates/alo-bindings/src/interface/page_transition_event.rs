/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `PageTransitionEvent` (ADR 0039 §§ 2 and 5, queue item 373): what the
//! browser fires at the window as a page is shown — `pageshow`, the last
//! step of its load — and as it is left — `pagehide`, the first of its
//! leaving steps.
//!
//! Its one member, read-only:
//!
//! - `persisted`: `false`, on every one this engine makes. A page left is a
//!   page gone, because there is no back-forward cache to keep it (§ 5), so
//!   no page is ever shown again from one or left into one.
//!
//! **Absent** (ADR 0013 § 3): the constructor, which HTML gives it, as
//! [`super::ui_event`] says of its family — only the browser makes one here,
//! and a page that makes its own is the item that adds it.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::event::Event;

/// The name the brand check's `TypeError` uses.
const NAME: &str = "PageTransitionEvent";

/// `PageTransitionEvent.prototype`'s one member.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "persisted",
        persisted,
        None,
    )
}

/// `get persisted`: `false`, once `this` is shown to be one.
fn persisted(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let transition = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<Event>(held)
            .is_some_and(Event::is_page_transition),
        _ => false,
    };
    if transition {
        Ok(Answer::Value(Value::Bool(false)))
    } else {
        Err(Escape::type_error(
            format!("'persisted' was read from something that is not a {NAME}"),
            call.at(),
        ))
    }
}
