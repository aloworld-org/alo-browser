/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Fetch's *extract a body* for the objects a page can make that are a
//! `BodyInit` (queue item 472): what a fetch's `body` and a beacon's `data`
//! become when they are one of this engine's objects rather than a string.
//!
//! - A **`URLSearchParams`** is its list written as
//!   `application/x-www-form-urlencoded` ([`SearchParams::serialized`]),
//!   with `Content-Type: application/x-www-form-urlencoded;charset=UTF-8`,
//!   which is what Chrome 154 sent for the contact form's listener
//!   (measured). That type is CORS-safelisted, so a beacon carrying one is
//!   still `no-cors`.
//! - A **`FormData`** is a `multipart/form-data` body whose boundary the
//!   renderer chooses (ADR 0051 § 7), and is refused by name: it is queue
//!   item 492.
//!
//! Anything else is not this module's, and its caller converts it as it did
//! before: a string for a fetch, `ToString` for a beacon. Nothing here runs
//! the page's script, so it is answered at once, wherever its caller is in
//! its own steps.

use alo_js::abrupt::Missing;
use alo_js::object::Objects;
use alo_js::{Escape, Value};

use crate::form_data::FormData;
use crate::search_params::SearchParams;

/// The `Content-Type` a `URLSearchParams` body brings.
pub(crate) const URLENCODED: &str = "application/x-www-form-urlencoded;charset=UTF-8";

/// The `Content-Type` a string body brings.
pub(crate) const TEXT_PLAIN: &str = "text/plain;charset=UTF-8";

/// A body extracted from one of this engine's objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Extracted {
    /// The body, as text: every type here writes UTF-8.
    pub text: String,
    /// The `Content-Type` it brings, unless the page set one.
    pub content_type: &'static str,
}

/// `value` as a body, if it is one of this engine's `BodyInit` objects:
/// [`None`] for anything else.
///
/// # Errors
///
/// A refusal by name for a `FormData`.
pub(crate) fn extracted(objects: &Objects, value: Value) -> Result<Option<Extracted>, Escape> {
    let Value::Object(held) = value else {
        return Ok(None);
    };
    if let Some(params) = objects.embedded::<SearchParams>(held) {
        return Ok(Some(Extracted {
            text: params.serialized(),
            content_type: URLENCODED,
        }));
    }
    if objects.embedded::<FormData>(held).is_some() {
        return Err(Escape::NotBuiltYet(Missing::InTheEmbedder(
            "a FormData as a body is multipart/form-data with a boundary the renderer chooses, \
             and is queue item 492",
        )));
    }
    Ok(None)
}

/// Whether `value` is one of this engine's `BodyInit` objects, which a
/// caller must not convert with `ToString`.
pub(crate) fn is_one(objects: &Objects, value: Value) -> bool {
    match value {
        Value::Object(held) => {
            objects.embedded::<SearchParams>(held).is_some()
                || objects.embedded::<FormData>(held).is_some()
        }
        _ => false,
    }
}
