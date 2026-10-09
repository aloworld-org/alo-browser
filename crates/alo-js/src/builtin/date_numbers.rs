/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ToNumber` on each of a date builtin's arguments, in order, with each kept
//! (queue item 356, ADR 0031).
//!
//! `new Date(2026, 9, 9)`, `Date.UTC(…)` and every setter convert up to seven
//! arguments before they do any arithmetic, and any of them may be an object
//! whose `valueOf` runs the page's script. The specification converts them
//! **once each, in order**, and a page can count the calls — so each number is
//! kept the moment it exists, in a slot the builtin declares (ADR 0031 § 1),
//! and the conversion that has to run script is asked for and comes back here
//! at a step that says which argument it was.
//!
//! Which argument rides in the step rather than in a slot: it is at most six,
//! a bound this file sets and no page can raise (ADR 0031 § 5).

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive};
use crate::object::Value;
use crate::object::native::{Answer, Call, Want};

/// The step a builtin comes back at with the primitive argument `which`
/// converted to — this, plus `which`. Every step below it is the builtin's
/// own.
pub(super) const CONVERTED: u32 = 16;

/// The most arguments any date builtin converts: `Date.UTC`'s seven.
pub(super) const MOST: usize = 7;

/// How far the arguments have got.
pub(super) enum Numbers {
    /// Every argument asked for is a number, kept.
    Done,
    /// One is an object whose conversion runs script, and this is the answer
    /// that asks for it.
    Asked(Answer),
}

/// Convert arguments `0..count` to numbers, in order, keeping argument `i`'s
/// in slot `first + i`.
///
/// Called at every step of a builtin that converts: at a step below
/// [`CONVERTED`] it begins at the first argument, and at one of its own it
/// keeps the number the answer converts to and carries on from the next. An
/// argument that is not there is `undefined`, which is `NaN` — the caller
/// decides which arguments must be read even when absent by the `count` it
/// passes.
///
/// # Errors
///
/// A `TypeError` for a symbol, which no number can be made of, and
/// [`Internal::BuiltinIsWrong`] for a count past [`MOST`] or a slot the
/// builtin never declared.
pub(super) fn numbers(call: &mut Call<'_>, count: usize, first: usize) -> Result<Numbers, Escape> {
    if count > MOST {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    let mut which = 0;
    if let Some(converted) = call.step().checked_sub(CONVERTED) {
        which = usize::try_from(converted).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
        // A conversion answers a primitive or throws, so anything else here is
        // this engine having resumed the wrong builtin.
        let primitive =
            Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        let number = convert::to_number(call.seen(), primitive, call.at())?;
        call.keep(first.saturating_add(which), Value::Number(number))?;
        which = which.saturating_add(1);
    }
    while which < count {
        let value = call.argument(which);
        let Some(primitive) = Primitive::of(value) else {
            let step = u32::try_from(which)
                .ok()
                .and_then(|which| CONVERTED.checked_add(which))
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            return Ok(Numbers::Asked(Answer::want(
                Want::Primitive {
                    of: value,
                    hint: Hint::Number,
                },
                step,
            )));
        };
        let number = convert::to_number(call.seen(), primitive, call.at())?;
        call.keep(first.saturating_add(which), Value::Number(number))?;
        which = which.saturating_add(1);
    }
    Ok(Numbers::Done)
}

/// The number argument `which` was converted to, if it was given, and
/// `otherwise` if it was not: the specification's *if `ms` is present*.
///
/// # Errors
///
/// [`Internal::BuiltinIsWrong`] for a slot that does not hold the number
/// [`numbers`] kept there.
pub(super) fn given_or(
    call: &Call<'_>,
    first: usize,
    count: usize,
    which: usize,
    otherwise: f64,
) -> Result<f64, Escape> {
    if which < count {
        call.kept_number(first.saturating_add(which))
    } else {
        Ok(otherwise)
    }
}
