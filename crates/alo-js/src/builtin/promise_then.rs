/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Promise.prototype.then` and `catch` (queue item 333): reacting to a
//! promise, and what a reaction asks of the promise's constructor.
//!
//! # `then` asks what to make before it makes it
//!
//! `then` answers a new promise made by `SpeciesConstructor(promise,
//! %Promise%)`: it reads `promise.constructor`, and that constructor's
//! `Symbol.species`, and either may be a getter a page wrote. Both reads are
//! made as the specification makes them, each getter asked for and called. An
//! answer that is not `Promise` itself is refused by name
//! ([`Missing::APromiseOfAnotherConstructor`], queue item 337) rather than
//! quietly answered with a `Promise`; one that is no constructor at all is
//! the `TypeError` the specification gives. `finally` reads the same way, and
//! uses [`species`] for it.
//!
//! # A `Promise` made for `Promise` needs no functions to resolve it
//!
//! `NewPromiseCapability(%Promise%)` makes a promise and a pair of resolving
//! functions nobody but the reaction will ever call, once. So the reaction
//! holds the derived promise itself, and its job resolves it through
//! `%ResolvePromise%` exactly as that pair would have — the same steps, the
//! same order, one allocation instead of four.
//!
//! # `catch` is `then`, looked up
//!
//! `promise.catch(f)` is `Invoke(promise, "then", «undefined, f»)`, which
//! reads `then` off the receiver every time — so a page that replaced `then`
//! on its own promise has its `catch` call the replacement. A receiver that is
//! a primitive needs its wrapper's prototype to look `then` up on, which is
//! item 73's, and is refused by name as `forEach` refuses one.

use crate::abrupt::{Escape, Internal, Missing};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native, Want};
use crate::object::promise::State;
use crate::object::symbol::WellKnown;
use crate::object::{Found, Key, Value};

use super::promise::{is_the_promise_constructor, key};

/// `Promise.prototype.then`, keeping the promise it made.
pub(super) const THEN: Native = Native::new("then", then).keeping(1);

/// `Promise.prototype.catch`.
pub(super) const CATCH: Native = Native::new("catch", catch);

/// The slot `then` keeps the promise it made in.
const DERIVED: usize = 0;

/// Come back here with what a `constructor` getter answered.
const READ_CONSTRUCTOR: u32 = 1;
/// Come back here with what a `Symbol.species` getter answered.
const READ_SPECIES: u32 = 2;
/// Come back here once the reaction's job is queued.
const QUEUED: u32 = 3;

/// `catch`: come back here with what a `then` getter answered.
const READ_THEN: u32 = 1;
/// `catch`: come back here with what `then` answered.
const CALLED: u32 = 2;

/// `Promise.prototype.then(onFulfilled, onRejected)`.
fn then(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let promise = this_promise(call)?;
    match call.step() {
        0 => match species(call, promise, READ_CONSTRUCTOR, READ_SPECIES)? {
            Species::Promise => perform(call, promise),
            Species::Asked(answer) => Ok(answer),
        },
        READ_CONSTRUCTOR => match species_of(call, call.answer()?, READ_SPECIES)? {
            Species::Promise => perform(call, promise),
            Species::Asked(answer) => Ok(answer),
        },
        READ_SPECIES => {
            judged(call, call.answer()?)?;
            perform(call, promise)
        }
        QUEUED => Ok(Answer::Value(call.kept(DERIVED)?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `PerformPromiseThen` with a new `Promise` as the capability: wait on a
/// pending promise, or queue the job at once for a settled one.
fn perform(call: &mut Call<'_>, promise: Ref) -> Result<Answer, Escape> {
    let intrinsics = call.intrinsics()?;
    let prototype = intrinsics.promise_prototype(call.seen())?;
    let at = call.at();
    // A safepoint. The promise is `this` and the handlers are arguments, all
    // on the interpreter's stack.
    let derived = call
        .objects()
        .promise(Some(prototype))
        .map_err(|why| Escape::refused(why, at))?;
    call.keep(DERIVED, Value::Object(derived))?;
    let on_fulfilled = callable_or_undefined(call, call.argument(0));
    let on_rejected = callable_or_undefined(call, call.argument(1));
    let job = intrinsics.reaction_job(call.seen())?;
    let (state, result) = call
        .objects()
        .with_promise(promise, |promise, barrier| {
            if promise.state() == State::Pending {
                promise.react(barrier, Value::Object(derived), on_fulfilled, on_rejected);
            }
            // `HostPromiseRejectionTracker(promise, "handle")` for a rejected
            // one: a rejection waiting to be reported no longer is.
            promise.handle();
            (promise.state(), promise.result())
        })
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let (handler, fulfilled) = match state {
        State::Pending => return Ok(Answer::Value(Value::Object(derived))),
        State::Fulfilled => (on_fulfilled, true),
        State::Rejected => (on_rejected, false),
    };
    Ok(Answer::want(
        Want::Job {
            callee: Value::Object(job),
            arguments: vec![
                Value::Object(derived),
                handler,
                Value::Bool(fulfilled),
                result,
            ],
        },
        QUEUED,
    ))
}

/// `Promise.prototype.catch(onRejected)`.
fn catch(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match call.step() {
        0 => {
            let object = receiver(call)?;
            let key = key(call, "then")?;
            match read(call, object, key, READ_THEN, false)? {
                Read::Value(then) => Ok(invoke_then(call, then)),
                Read::Asked(answer) => Ok(answer),
            }
        }
        READ_THEN => Ok(invoke_then(call, call.answer()?)),
        CALLED => Ok(Answer::Value(call.answer()?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `catch`'s call of `then`, with `undefined` and its own argument. A `then`
/// that is not callable is the `TypeError` any call of one is.
fn invoke_then(call: &Call<'_>, then: Value) -> Answer {
    Answer::want(
        Want::Call {
            callee: then,
            receiver: call.this(),
            arguments: vec![Value::Undefined, call.argument(0)],
        },
        CALLED,
    )
}

/// `catch`'s receiver as the object `then` is read from.
fn receiver(call: &Call<'_>) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) => Ok(held),
        Value::Undefined | Value::Null => Err(Escape::type_error(
            "Promise.prototype.catch was called on null or undefined",
            call.at(),
        )),
        Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
            Err(Escape::NotBuiltYet(Missing::AWrapperObject))
        }
    }
}

/// The promise `then` was called on: `IsPromise`, or the `TypeError` the
/// specification gives.
fn this_promise(call: &Call<'_>) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) if call.seen().as_promise(held).is_some() => Ok(held),
        _ => Err(Escape::type_error(
            "Promise.prototype.then was called on something that is not a promise",
            call.at(),
        )),
    }
}

/// A handler as `then` keeps it: itself if it is callable, and `undefined`,
/// which passes the outcome through, if it is anything else.
pub(super) fn callable_or_undefined(call: &Call<'_>, value: Value) -> Value {
    match value {
        Value::Object(held) if call.seen().callable(held).is_some() => value,
        _ => Value::Undefined,
    }
}

/// What reading a property came to.
pub(super) enum Read {
    /// Its value, which needed nothing run.
    Value(Value),
    /// Its getter, asked for: the answer arrives at the step named.
    Asked(Answer),
}

/// `Get(object, key)`: a data property's value, or its getter asked for at
/// `step` — caught, if `caught`, so that a getter that throws is the
/// builtin's answer rather than its caller's ([`Want::Catch`]).
///
/// # Errors
///
/// A fault for a reference that names nothing, which is this engine's bug.
pub(super) fn read(
    call: &Call<'_>,
    object: Ref,
    key: Key,
    step: u32,
    caught: bool,
) -> Result<Read, Escape> {
    match call.seen().get(object, key)? {
        Found::Value(value) => Ok(Read::Value(value)),
        Found::Missing | Found::Getter(Value::Undefined) => Ok(Read::Value(Value::Undefined)),
        Found::Getter(getter) => {
            let (callee, receiver, arguments) = (getter, Value::Object(object), Vec::new());
            let want = if caught {
                Want::Catch {
                    callee,
                    receiver,
                    arguments,
                }
            } else {
                Want::Call {
                    callee,
                    receiver,
                    arguments,
                }
            };
            Ok(Read::Asked(Answer::want(want, step)))
        }
    }
}

/// Where `SpeciesConstructor(O, %Promise%)` has got to.
pub(super) enum Species {
    /// It came to `Promise`, the only constructor this engine makes promises
    /// with.
    Promise,
    /// A getter is asked for, and the answer arrives at the step named.
    Asked(Answer),
}

/// `SpeciesConstructor(object, %Promise%)` from the beginning: read
/// `object.constructor`, asking for its getter at `constructor_step`, and go
/// on to [`species_of`] with what it is.
///
/// # Errors
///
/// What [`species_of`] and [`judged`] refuse, and a full heap interning the
/// name.
pub(super) fn species(
    call: &mut Call<'_>,
    object: Ref,
    constructor_step: u32,
    species_step: u32,
) -> Result<Species, Escape> {
    let key = key(call, "constructor")?;
    match read(call, object, key, constructor_step, false)? {
        Read::Value(constructor) => species_of(call, constructor, species_step),
        Read::Asked(answer) => Ok(Species::Asked(answer)),
    }
}

/// `SpeciesConstructor` with `C` read: `undefined` is `Promise`; anything
/// else must be an object, whose `Symbol.species` is read — its getter asked
/// for at `step` — and [`judged`].
///
/// # Errors
///
/// The `TypeError` for a `constructor` that is neither `undefined` nor an
/// object, and what [`judged`] refuses.
pub(super) fn species_of(
    call: &Call<'_>,
    constructor: Value,
    step: u32,
) -> Result<Species, Escape> {
    let held = match constructor {
        Value::Undefined => return Ok(Species::Promise),
        Value::Object(held) => held,
        _ => {
            return Err(Escape::type_error(
                "a promise's constructor is neither undefined nor an object",
                call.at(),
            ));
        }
    };
    let key = call
        .intrinsics()?
        .well_known_key(call.seen(), WellKnown::Species)?;
    match read(call, held, key, step, false)? {
        Read::Value(species) => judged(call, species).map(|()| Species::Promise),
        Read::Asked(answer) => Ok(Species::Asked(answer)),
    }
}

/// What `C[Symbol.species]` answered: `undefined`, `null` and `Promise` are
/// `Promise`; another constructor is refused by name; anything else is the
/// `TypeError` the specification gives.
///
/// # Errors
///
/// [`Missing::APromiseOfAnotherConstructor`], or the `TypeError`.
pub(super) fn judged(call: &Call<'_>, species: Value) -> Result<(), Escape> {
    match species {
        Value::Undefined | Value::Null => Ok(()),
        _ if is_the_promise_constructor(call, species)? => Ok(()),
        Value::Object(held)
            if call
                .seen()
                .callable(held)
                .is_some_and(crate::object::Function::is_constructor) =>
        {
            Err(Escape::NotBuiltYet(Missing::APromiseOfAnotherConstructor))
        }
        _ => Err(Escape::type_error(
            "a promise constructor's Symbol.species is not a constructor",
            call.at(),
        )),
    }
}
