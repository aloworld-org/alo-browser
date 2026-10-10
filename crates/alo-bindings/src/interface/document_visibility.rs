/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HTML's page visibility members of `Document` (ADR 0039 § 1, queue item
//! 364): `visibilityState` and `hidden`, each a read-only attribute on
//! `Document.prototype`.
//!
//! - `visibilityState`: `"visible"` or `"hidden"`, the document's
//!   [`Visibility`] as the renderer last stated it, read from the document
//!   cell at each read.
//! - `hidden`: `true` exactly when `visibilityState` is `"hidden"`.
//!
//! A document no window was associated with is `"hidden"` and `true`, and
//! never changes ([`crate::visibility`]).
//!
//! # Not here
//!
//! `onvisibilitychange`, an event handler property, which is item 259's.
//!
//! [`Visibility`]: crate::visibility::Visibility

use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Objects, Value};
use alo_js::{Escape, Fault};

use crate::define;
use crate::idl::{self, Brand};
use crate::visibility::{self, Visibility};

/// `Document.prototype`'s two members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "visibilityState",
        visibility_state,
        None,
    )?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "hidden",
        hidden,
        None,
    )
}

/// `get visibilityState`.
fn visibility_state(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let state = state(call, "visibilityState")?;
    idl::answer_text(call, Some(state.as_str().to_owned()))
}

/// `get hidden`.
fn hidden(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let state = state(call, "hidden")?;
    Ok(Answer::Value(Value::Bool(state == Visibility::Hidden)))
}

/// The visibility state of the document `call` was made on.
fn state(call: &Call<'_>, name: &'static str) -> Result<Visibility, Escape> {
    let this = idl::this(call, Brand::Document, name)?;
    // Not [`None`]: the brand check has said `owner` is a document cell.
    visibility::of(call.seen(), this.owner).ok_or(Escape::fault(Fault::NotAnObject))
}
