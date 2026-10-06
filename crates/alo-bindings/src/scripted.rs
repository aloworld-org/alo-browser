/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The dispatch's driver **from script** (ADR 0018 § 3): a native that
//! suspends once per listener.
//!
//! The stepper ([`crate::dispatch`]) says which listener is next; this asks
//! the interpreter to call it — as a call whose throw is **reported** rather
//! than propagated, so a listener's throw never leaves the native — and,
//! when the native is run again, tells the stepper the call finished. No
//! microtask checkpoint runs between listeners, because script is still
//! running: the difference between a person's click and a script's.
//!
//! Two natives drive a dispatch this way, `dispatchEvent` and `click()`
//! (queue item 261), and so it is written once, here, as the renderer's
//! loop is written once for the browser's dispatches. Each native names a
//! **base** step, and this uses the two after it: `base + RETURNED` when a
//! listener has returned or thrown, `base + HANDLE_EVENT` when a callback
//! object's `handleEvent` getter has answered.

use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Value};

use crate::dispatch::{self, Invoke, Next};
use crate::event::Event;

/// A listener returned or was reported.
pub(crate) const RETURNED: u32 = 1;

/// A callback object's `handleEvent` getter answered.
pub(crate) const HANDLE_EVENT: u32 = 2;

/// How many steps past its base a native's dispatch uses; a native driving
/// several dispatches gives each a base this far apart.
pub(crate) const STEPS: u32 = 4;

/// What driving the dispatch came to.
#[derive(Debug)]
pub(crate) enum Driven {
    /// A listener is to be called: answer this.
    Asked(Answer),
    /// The dispatch is over; `canceled` is whether a listener cancelled it.
    Done {
        /// `defaultPrevented`.
        canceled: bool,
    },
}

/// The native has been run again at `base + RETURNED` or
/// `base + HANDLE_EVENT`, during `event`'s dispatch: finish what it asked
/// for, and drive on.
///
/// # Errors
///
/// [`Internal::BuiltinIsWrong`] for any other step, and as [`drive`].
pub(crate) fn resume(call: &mut Call<'_>, event: Ref, base: u32) -> Result<Driven, Escape> {
    match call.step().checked_sub(base) {
        Some(RETURNED) => dispatch::returned(call.objects(), event),
        Some(HANDLE_EVENT) if call.reported() => dispatch::returned(call.objects(), event),
        Some(HANDLE_EVENT) => {
            // The getter answered `handleEvent`: call it on the object.
            let receiver = call
                .seen()
                .embedded::<Event>(event)
                .and_then(Event::progress)
                .and_then(dispatch::Progress::invoking)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            return Ok(Driven::Asked(Answer::want(
                Want::Report {
                    callee: call.answer()?,
                    receiver: Value::Object(receiver),
                    arguments: vec![Value::Object(event)],
                },
                base.saturating_add(RETURNED),
            )));
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
    drive(call, event, base)
}

/// Ask the stepper what is next in `event`'s dispatch, and ask the
/// interpreter for it, coming back at a step after `base`.
///
/// Nothing allocates between the stepper's answer and the call being asked
/// for: the callback is held by the event while it runs, and its `this` by
/// the document cell.
///
/// # Errors
///
/// A fault if `event` is not being dispatched, which the native began.
pub(crate) fn drive(call: &mut Call<'_>, event: Ref, base: u32) -> Result<Driven, Escape> {
    let (callback, this) = match dispatch::next(call.objects(), event)? {
        Next::Done { canceled } => return Ok(Driven::Done { canceled }),
        Next::Call { callback, this } => (callback, this),
    };
    let (callee, receiver, step, arguments) = match dispatch::invoke(call.seen(), callback, this)? {
        Invoke::Call { callee, this } => (callee, this, RETURNED, vec![Value::Object(event)]),
        // A getter is a call of its own, and its throw is reported like the
        // listener's would be.
        Invoke::Get { getter, this } => (getter, Value::Object(this), HANDLE_EVENT, Vec::new()),
    };
    Ok(Driven::Asked(Answer::want(
        Want::Report {
            callee,
            receiver,
            arguments,
        },
        base.saturating_add(step),
    )))
}
