/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A fetch's answer, delivered: the task that settles the promise waiting
//! for it (ADR 0032 § 1, ADR 0016 § 2, queue item 335).
//!
//! The answer arrives as a message of its own, and its handling is **a task
//! of its own**: the renderer makes the [`crate::Response`] — or nothing,
//! for a network error — and queues one call of [`SETTLE`] with the ask's
//! number and it ([`answer`]). When the task runs, the promise is taken from
//! the document cell and settled: resolved with the response through the
//! realm's one resolve procedure, as Fetch's *resolve p with response*
//! says, or rejected with the one `TypeError` every failure rejects with
//! ([`crate::fetch::FAILED`]). The checkpoint after the task runs every
//! reaction, and what those reactions fetch is asked for in the answer to
//! the message that delivered it.
//!
//! [`SETTLE`] is made once per page ([`crate::fetch::offer`]) and held by the
//! document cell; no page can reach it.

use alo_js::abrupt::Internal;
use alo_js::builtin::error::Family;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::{Escape, Fault, Value};

use crate::document_cell::DocumentCell;
use crate::fetch::FAILED;
use crate::response::{self, Responded};

/// The function a delivery calls, keeping the promise it settles.
pub(crate) const SETTLE: Native = Native::new("a fetch's answer", settle).keeping(1);

/// Where it keeps the promise.
const PROMISE: usize = 0;
/// Come back here once the promise is resolved or rejected.
const SETTLED: u32 = 1;

/// What to queue to deliver an answer: one call of `callee` with
/// `arguments`, with nothing as `this`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Delivery {
    /// The function that settles the promise.
    pub callee: Value,
    /// The ask's number, and the response or `undefined`.
    pub arguments: [Value; 2],
}

/// The task that delivers the answer to ask `number` — `responded`, or a
/// network error for [`None`] — to the document `cell` holds, or [`None`]
/// when nothing there waits for it: an ask of a page that has gone, a
/// number the browser process sent that the page never chose, or a beacon,
/// which no promise waits for. A keep-alive ask's body is taken off the
/// document's count by [`crate::fetching::answered`], as the answer
/// arrives.
///
/// **A safepoint** when there is a response to make. `cell` must be rooted
/// by the caller. The response is in the answer's arguments, a Rust local:
/// the caller roots it until the task holds it — queueing it is an
/// allocation — and lets go once it is queued.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold the response, and a fault when
/// `cell` is not a document cell or the page was never offered `fetch`.
pub fn answer(
    objects: &mut Objects,
    cell: Ref,
    number: u64,
    responded: Option<Responded>,
) -> Result<Option<Delivery>, Escape> {
    let fetches = &objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .fetches;
    if !fetches.waits_for(number) {
        return Ok(None);
    }
    let response = match responded {
        Some(responded) => Value::Object(response::make(objects, cell, responded)?),
        None => Value::Undefined,
    };
    // Read after the response is made, which may have collected: the cell
    // holds the function, so it is still there, and now nothing allocates
    // before the caller roots what this answers.
    let callee = objects
        .embedded::<DocumentCell>(cell)
        .and_then(|held| held.fetches.settle())
        .ok_or(Escape::fault(Fault::Gone))?;
    Ok(Some(Delivery {
        callee: Value::Object(callee),
        arguments: [number_value(number), response],
    }))
}

/// An ask's number as a script value.
fn number_value(number: u64) -> Value {
    #[expect(
        clippy::cast_precision_loss,
        reason = "an ask's number is a count, far below 2⁵³"
    )]
    let number = number as f64;
    Value::Number(number)
}

/// An ask's number back from a script value: a whole number, not negative
/// and exactly representable — [`None`] for anything else.
pub(crate) fn number(value: Value) -> Option<u64> {
    let Value::Number(number) = value else {
        return None;
    };
    if !(number.is_finite()
        && number >= 0.0
        && number.fract() == 0.0
        && number < 9.007_199_254_740_992e15)
    {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked whole, not negative and below 2⁵³ just above"
    )]
    let whole = number as u64;
    Some(whole)
}

/// `(number, response)`: settle the promise waiting for ask `number` with
/// `response`, or reject it when `response` is `undefined`.
fn settle(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == SETTLED {
        return Ok(Answer::Value(Value::Undefined));
    }
    let number = number(call.argument(0)).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let owner = call
        .host_defined()
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let promise = call
        .objects()
        .write_embedded::<DocumentCell, _>(owner, |held, _| held.fetches.stop_waiting(number))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    // Nothing waits: this answer was queued twice, which the renderer does
    // not do. Answered rather than assumed.
    let Some(promise) = promise else {
        return Ok(Answer::Value(Value::Undefined));
    };
    // Out of the cell and into a slot before anything allocates.
    call.keep(PROMISE, Value::Object(promise))?;
    let response = call.argument(1);
    if response == Value::Undefined {
        let at = call.at();
        let error = call
            .intrinsics()?
            .error(call.objects(), Family::TypeError, FAILED, at)?;
        return Ok(Answer::want(
            Want::Settle {
                promise: Value::Object(promise),
                fulfilled: false,
                value: Value::Object(error),
            },
            SETTLED,
        ));
    }
    let resolve = call.intrinsics()?.resolve_promise(call.seen())?;
    Ok(Answer::want(
        Want::Call {
            callee: Value::Object(resolve),
            receiver: Value::Undefined,
            arguments: vec![Value::Object(promise), response],
        },
        SETTLED,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_whole_and_not_negative_or_it_is_none() {
        assert_eq!(number(Value::Number(0.0)), Some(0));
        assert_eq!(number(Value::Number(41.0)), Some(41));
        for not in [-1.0, 0.5, f64::NAN, f64::INFINITY, 1e300] {
            assert_eq!(number(Value::Number(not)), None, "{not}");
        }
        assert_eq!(number(Value::Undefined), None);
        assert_eq!(number(number_value(7)), Some(7));
    }
}
