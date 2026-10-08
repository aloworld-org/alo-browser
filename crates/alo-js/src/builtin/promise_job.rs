/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The two jobs a promise queues (queue item 333): a reaction running, and a
//! thenable being adopted.
//!
//! The specification writes each as an abstract closure handed to
//! `HostEnqueuePromiseJob`. This engine's job queue holds a callee and its
//! arguments (ADR 0016 § 1, item 232), so each is a builtin of the realm's —
//! `%PromiseReactionJob%` and `%PromiseResolveThenableJob%` — and what the
//! closure captured are the job's arguments. Neither is reachable from a page.
//!
//! # A reaction never throws
//!
//! The handler is called with [`Want::Catch`]: what it returns resolves the
//! promise `then` made, and what it throws rejects it. Nothing a handler does
//! reaches the checkpoint's report — which is how a rejection is passed down
//! a chain to the `catch` at the end of it rather than reported at the first
//! link. A missing handler passes the outcome through unchanged.
//!
//! # A thenable is asked once, with a fresh pair
//!
//! Adopting a thenable calls its `then` with a new `resolve` and `reject` for
//! the promise, made when the job runs. A `then` that throws after calling
//! either has already settled the promise, so the throw is handed to the
//! pair's `reject`, whose flag makes it nothing — the specification's
//! behaviour, which a page that calls `resolve` and then throws relies on.

use crate::abrupt::{Escape, Internal};
use crate::object::Value;
use crate::object::native::{Answer, Call, Native, Want};

use super::promise_resolving::resolving_functions;

/// `%PromiseReactionJob%(derived, handler, fulfilled, argument)`.
pub(super) const REACTION_JOB: Native = Native::new("PromiseReactionJob", reaction_job);

/// `%PromiseResolveThenableJob%(promise, thenable, then)`, keeping the pair
/// it made.
pub(super) const THENABLE_JOB: Native =
    Native::new("PromiseResolveThenableJob", thenable_job).keeping(2);

/// Come back here with what the handler returned or threw.
const HANDLED: u32 = 1;
/// Come back here once the derived promise is resolved or settled, or the
/// thenable's `then` has been answered for.
const DONE: u32 = 2;

/// The thenable job: come back here with what `then` returned or threw.
const CALLED: u32 = 1;

/// Where the thenable job keeps its `resolve`.
const RESOLVE: usize = 0;
/// Where it keeps its `reject`.
const REJECT: usize = 1;

/// `NewPromiseReactionJob`'s closure.
fn reaction_job(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let derived = call.argument(0);
    let handler = call.argument(1);
    let Value::Bool(fulfilled) = call.argument(2) else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    match call.step() {
        0 if handler == Value::Undefined => {
            let argument = call.argument(3);
            if fulfilled {
                resolve(call, derived, argument)
            } else {
                Ok(reject(derived, argument))
            }
        }
        0 => Ok(Answer::want(
            Want::Catch {
                callee: handler,
                receiver: Value::Undefined,
                arguments: vec![call.argument(3)],
            },
            HANDLED,
        )),
        HANDLED if call.threw() => Ok(reject(derived, call.answer()?)),
        HANDLED => resolve(call, derived, call.answer()?),
        DONE => Ok(Answer::Value(Value::Undefined)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Resolve the derived promise with `value`, through `%ResolvePromise%`.
fn resolve(call: &Call<'_>, derived: Value, value: Value) -> Result<Answer, Escape> {
    let resolve = call.intrinsics()?.resolve_promise(call.seen())?;
    Ok(Answer::want(
        Want::Call {
            callee: Value::Object(resolve),
            receiver: Value::Undefined,
            arguments: vec![derived, value],
        },
        DONE,
    ))
}

/// Reject the derived promise with `reason`.
const fn reject(derived: Value, reason: Value) -> Answer {
    Answer::want(
        Want::Settle {
            promise: derived,
            fulfilled: false,
            value: reason,
        },
        DONE,
    )
}

/// `NewPromiseResolveThenableJob`'s closure.
fn thenable_job(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match call.step() {
        0 => {
            let Value::Object(promise) = call.argument(0) else {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            // The promise, the thenable and `then` are arguments, on the
            // stack, across the three allocations.
            let (resolve, reject) = resolving_functions(call, promise)?;
            call.keep(RESOLVE, Value::Object(resolve))?;
            call.keep(REJECT, Value::Object(reject))?;
            Ok(Answer::want(
                Want::Catch {
                    callee: call.argument(2),
                    receiver: call.argument(1),
                    arguments: vec![Value::Object(resolve), Value::Object(reject)],
                },
                CALLED,
            ))
        }
        CALLED if call.threw() => Ok(Answer::want(
            Want::Call {
                callee: call.kept(REJECT)?,
                receiver: Value::Undefined,
                arguments: vec![call.answer()?],
            },
            DONE,
        )),
        CALLED | DONE => Ok(Answer::Value(Value::Undefined)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}
