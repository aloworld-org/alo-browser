/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Function.prototype`: what every function in the language inherits (218).
//!
//! It is itself a function, which is the oddity worth knowing about it: calling
//! it is legal, takes any arguments and answers `undefined`. That is the
//! specification's own description and it is what makes an empty function
//! object a usable default.
//!
//! # It has one method, and that method refuses
//!
//! `Function.prototype.toString` answers the text a function was written as,
//! and a [`Unit`](crate::unit::Unit) keeps no source text — the compiler is
//! given a tree and the tree is dropped. So it is
//! [`Missing::AFunctionsSourceText`] and queue item 220.
//!
//! **Defining it in order to refuse is the point.** Without it, `f + ''` would
//! find `Object.prototype.toString` and answer `"[object Function]"` — a
//! sentence no engine produces, handed to a page as though it were right. ADR
//! 0013 § 3's *absent beats approximate* is exactly about that trade, and a
//! refusal a person reads is the honest half of it.
//!
//! `call`, `apply` and `bind` are not here for a different reason: each one
//! makes a call, and a builtin cannot re-enter the script until queue item 219.

use crate::abrupt::{Escape, Missing};
use crate::object::Value;
use crate::object::native::Call;

use super::Intrinsics;

/// Put the methods on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(
    objects: &mut crate::object::Objects,
    intrinsics: &Intrinsics,
) -> Result<(), Escape> {
    let on = intrinsics.function_prototype(objects)?;
    super::method(objects, on, on, "toString", to_string)?;
    Ok(())
}

/// `Function.prototype` itself: it answers `undefined` for anything.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `native::Body`, which every builtin shares"
)]
pub(super) fn nothing(_: &mut Call<'_>) -> Result<Value, Escape> {
    Ok(Value::Undefined)
}

/// `Function.prototype.toString`, which needs the source text nothing keeps.
fn to_string(_: &mut Call<'_>) -> Result<Value, Escape> {
    Err(Escape::NotBuiltYet(Missing::AFunctionsSourceText))
}
