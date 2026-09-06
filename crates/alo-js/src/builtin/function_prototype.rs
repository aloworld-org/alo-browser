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
//! # `call` is the shape queue item 219 is for
//!
//! `f.call(o, 1)` is a builtin whose whole job is to run something else, which
//! is what a native could not do until it could **ask**. It asks once and comes
//! back at step 1 with the answer, and that answer is its own — see
//! [`Answer::Want`](crate::object::native::Answer) for the mechanism and
//! [`call`] for why the asking is not skipped.
//!
//! # One method here still refuses
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
//! `apply` is queue item 221 and `bind` is queue item 220. `apply` takes its
//! arguments from an array-like, which means reading a `length` and then a
//! property per index — each of them possibly an accessor, so each of them
//! possibly a call — and a builtin that suspends in the middle of building a
//! list needs somewhere to keep the list. That is a design rather than a chore,
//! and it is the same one `Array.prototype.map` needs.

use crate::abrupt::{Escape, Missing};
use crate::object::Value;
use crate::object::native::{Answer, Call, Want};

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
    super::method(objects, on, on, "call", call)?;
    super::method(objects, on, on, "toString", to_string)?;
    Ok(())
}

/// `Function.prototype` itself: it answers `undefined` for anything.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `native::Body`, which every builtin shares"
)]
pub(super) fn nothing(_: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Undefined))
}

/// `Function.prototype.call`.
///
/// `func.call(thisArg, ...args)` is `Call(func, thisArg, args)` and nothing
/// else: no `ToObject` on the receiver, because the callee's own strictness
/// decides that and the interpreter applies it where every other call does.
/// A `func` that is not callable is the `TypeError` any other call on a
/// non-function is, produced in the one place that produces it rather than
/// spelled a second time here.
///
/// # It asks rather than answering the call's own answer directly
///
/// The interpreter could be told to give the asked-for call's answer straight
/// to whoever wanted this one — a proper tail call, one turn of the loop
/// shorter. Coming back at step 1 instead keeps the same continuation model
/// every builtin uses and counts this pending call toward the recursion bound.
/// A script that repeatedly calls itself through `call` therefore reaches the
/// same bound as one that calls itself directly.
fn call(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() != 0 {
        return Ok(Answer::Value(call.answer()?));
    }
    let callee = call.this();
    let receiver = call.argument(0);
    // Built last, out of values that are already on the interpreter's stack,
    // and nothing allocates between here and returning it.
    let arguments = (1..call.count())
        .map(|which| call.argument(which))
        .collect();
    Ok(Answer::want(
        Want::Call {
            callee,
            receiver,
            arguments,
        },
        1,
    ))
}

/// `Function.prototype.toString`, which needs the source text nothing keeps.
fn to_string(_: &mut Call<'_>) -> Result<Answer, Escape> {
    Err(Escape::NotBuiltYet(Missing::AFunctionsSourceText))
}
