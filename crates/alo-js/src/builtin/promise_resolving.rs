/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Resolving a promise (queue item 333): the `resolve` and `reject` an
//! executor is handed, and the one procedure every resolution runs.
//!
//! # A pair of closures over one record
//!
//! `CreateResolvingFunctions` makes two functions that share the promise and
//! an `[[AlreadyResolved]]` flag, so that whichever runs first wins and the
//! other does nothing. The record is a [`Slots`](crate::object::Slots) cell of
//! two values — the promise and the flag — and each function is a builtin made
//! around it ([`Objects::native_holding`]), which the interpreter hands back
//! with every call ([`Call::held`]). A promise may have several pairs over its
//! life (a thenable's job makes a fresh one), which is why the flag is the
//! pair's and not the promise's.
//!
//! # One procedure, called rather than copied
//!
//! A promise is resolved with a value from four places: its `resolve`, a
//! reaction whose handler returned, `Promise.resolve`, and a pass-through
//! `then`. Each asks for a call of the realm's `%ResolvePromise%` — the
//! steps of the specification's resolve function after its flag — rather than
//! spelling them again:
//!
//! 1. resolving a promise with itself rejects it with a `TypeError`;
//! 2. anything but an object fulfils it;
//! 3. `then` is read off the object, and a getter that throws rejects it;
//! 4. a `then` that is callable queues a `NewPromiseResolveThenableJob` — so
//!    a thenable is adopted one job later than a plain value — and anything
//!    else fulfils it with the object.
//!
//! `%ResolvePromise%` is never reachable from a page: no property holds it.
//!
//! [`Objects::native_holding`]: crate::object::Objects::native_holding

use crate::abrupt::{Escape, Internal};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native, Want};
use crate::object::{Held, Objects, Value};

use super::error::{self, Family};
use super::promise::key;
use super::promise_then::{Read, read};

/// The `resolve` a pair is made of.
const RESOLVE_FUNCTION: Native = Native::new("resolve", resolve_function);
/// The `reject` a pair is made of.
const REJECT_FUNCTION: Native = Native::new("reject", reject_function);
/// `%ResolvePromise%(promise, resolution)`.
pub(super) const RESOLVE_PROMISE: Native = Native::new("ResolvePromise", resolve_promise);

/// Where the record keeps the promise.
const PROMISE: usize = 0;
/// Where the record keeps `[[AlreadyResolved]]`.
const ALREADY: usize = 1;

/// A resolving function: come back here once what it asked for is done.
const DONE: u32 = 1;

/// `%ResolvePromise%`: come back here with what a `then` getter answered or
/// threw.
const READ_THEN: u32 = 1;
/// `%ResolvePromise%`: come back here once the promise is settled or the job
/// is queued.
const RESOLVED: u32 = 2;

/// `CreateResolvingFunctions(promise)`: a `resolve` and a `reject` over one
/// fresh record.
///
/// **Allocates three times.** The promise is the caller's to have rooted; the
/// record and the two functions are held in a scope until this answers, and
/// are then in Rust locals the caller keeps before anything else allocates.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling.
pub(super) fn resolving_functions(call: &mut Call<'_>, promise: Ref) -> Result<(Ref, Ref), Escape> {
    let function_prototype = call.intrinsics()?.function_prototype(call.seen())?;
    let at = call.at();
    let objects = call.objects();
    let scope = objects.heap_mut().open();
    let made = pair(objects, function_prototype, promise, at);
    objects.heap_mut().close(scope);
    made
}

/// [`resolving_functions`], with the scope already open.
fn pair(
    objects: &mut Objects,
    function_prototype: Ref,
    promise: Ref,
    at: usize,
) -> Result<(Ref, Ref), Escape> {
    let record = objects.slots().map_err(|why| Escape::refused(why, at))?;
    objects.heap_mut().hold(record);
    objects
        .with_slots(record, |slots, _| {
            slots.push(Value::Object(promise));
            slots.push(Value::Bool(false));
        })
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let resolve = objects
        .native_holding(
            RESOLVE_FUNCTION,
            Some(function_prototype),
            Value::Object(record),
        )
        .map_err(|why| Escape::refused(why, at))?;
    objects.heap_mut().hold(resolve);
    let reject = objects
        .native_holding(
            REJECT_FUNCTION,
            Some(function_prototype),
            Value::Object(record),
        )
        .map_err(|why| Escape::refused(why, at))?;
    objects.heap_mut().hold(reject);
    Ok((resolve, reject))
}

/// The record a resolving function was made around, the promise in it, and
/// whether either of its pair has run.
fn record(call: &Call<'_>) -> Result<(Ref, Value, bool), Escape> {
    let Value::Object(record) = call.held() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let objects = call.seen();
    match (objects.slot(record, PROMISE), objects.slot(record, ALREADY)) {
        (Some(Held::Value(promise)), Some(Held::Value(Value::Bool(already)))) => {
            Ok((record, promise, already))
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Set `[[AlreadyResolved]]`, so that the other of the pair does nothing.
fn mark(call: &mut Call<'_>, record: Ref) -> Result<(), Escape> {
    call.objects()
        .with_slots(record, |slots, barrier| {
            slots.set(barrier, ALREADY, Value::Bool(true))
        })
        .filter(|wrote| *wrote)
        .map(|_| ())
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// A promise's `resolve(resolution)`.
fn resolve_function(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == DONE {
        return Ok(Answer::Value(Value::Undefined));
    }
    let (record, promise, already) = record(call)?;
    if already {
        return Ok(Answer::Value(Value::Undefined));
    }
    mark(call, record)?;
    let resolve = call.intrinsics()?.resolve_promise(call.seen())?;
    Ok(Answer::want(
        Want::Call {
            callee: Value::Object(resolve),
            receiver: Value::Undefined,
            arguments: vec![promise, call.argument(0)],
        },
        DONE,
    ))
}

/// A promise's `reject(reason)`.
fn reject_function(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == DONE {
        return Ok(Answer::Value(Value::Undefined));
    }
    let (record, promise, already) = record(call)?;
    if already {
        return Ok(Answer::Value(Value::Undefined));
    }
    mark(call, record)?;
    Ok(Answer::want(
        Want::Settle {
            promise,
            fulfilled: false,
            value: call.argument(0),
        },
        DONE,
    ))
}

/// `%ResolvePromise%(promise, resolution)`: the resolve function's steps
/// after its flag. It never throws: what a `then` getter throws rejects the
/// promise.
fn resolve_promise(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let promise = call.argument(0);
    let resolution = call.argument(1);
    match call.step() {
        0 => {
            if resolution.same_value(promise) {
                let at = call.at();
                let intrinsics = call.intrinsics()?;
                let error = error::made(
                    call.objects(),
                    intrinsics,
                    Family::TypeError,
                    "a promise cannot be resolved with itself",
                    at,
                )?;
                return Ok(settle(promise, false, Value::Object(error)));
            }
            let Value::Object(thenable) = resolution else {
                return Ok(settle(promise, true, resolution));
            };
            // Interning may allocate; both are arguments, on the stack.
            let key = key(call, "then")?;
            match read(call, thenable, key, READ_THEN, true)? {
                Read::Value(then) => adopt(call, then),
                Read::Asked(answer) => Ok(answer),
            }
        }
        READ_THEN if call.threw() => Ok(settle(promise, false, call.answer()?)),
        READ_THEN => adopt(call, call.answer()?),
        RESOLVED => Ok(Answer::Value(Value::Undefined)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// With `then` read: queue the thenable's job if it is callable, and fulfil
/// with the object if it is not.
fn adopt(call: &Call<'_>, then: Value) -> Result<Answer, Escape> {
    let promise = call.argument(0);
    let resolution = call.argument(1);
    let callable = match then {
        Value::Object(held) => call.seen().callable(held).is_some(),
        _ => false,
    };
    if !callable {
        return Ok(settle(promise, true, resolution));
    }
    let job = call.intrinsics()?.thenable_job(call.seen())?;
    Ok(Answer::want(
        Want::Job {
            callee: Value::Object(job),
            arguments: vec![promise, resolution, then],
        },
        RESOLVED,
    ))
}

/// Ask for `promise` to be settled, coming back once it is.
const fn settle(promise: Value, fulfilled: bool, value: Value) -> Answer {
    Answer::want(
        Want::Settle {
            promise,
            fulfilled,
            value,
        },
        RESOLVED,
    )
}
