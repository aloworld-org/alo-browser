/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `%IteratorPrototype%`: what every iterator the language makes inherits
//! (queue item 230).
//!
//! It has one method, and the method is the reason an iterator can be written
//! where an iterable is wanted: `[Symbol.iterator]()` answers `this`, so
//! `for (const x of [1, 2].values())` asks the iterator for an iterator and is
//! handed the iterator itself.
//!
//! # What is absent
//!
//! The iterator helpers — `map`, `filter`, `take`, `toArray` and the rest —
//! and the `Iterator` constructor they hang off, with its `Symbol.toStringTag`
//! accessor, are item 73's library rather than the protocol `for…of` reads.
//! `[].values().map` is `undefined`, which a page's own check reads correctly.

use crate::abrupt::Escape;
use crate::object::Objects;
use crate::object::native::{Answer, Call};
use crate::object::symbol::WellKnown;

use super::Intrinsics;

/// Put the method on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(objects: &mut Objects, intrinsics: &Intrinsics) -> Result<(), Escape> {
    let on = intrinsics.iterator_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    let key = intrinsics.well_known_key(objects, WellKnown::Iterator)?;
    super::symbol_method(objects, on, functions, key, "[Symbol.iterator]", itself)
}

/// `%IteratorPrototype%[Symbol.iterator]`, which answers its `this` unchanged —
/// a primitive included, since a builtin is strict code.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `native::Body`, which every builtin shares"
)]
fn itself(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(call.this()))
}
