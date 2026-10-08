/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Promise.prototype.finally` (queue item 333): run a callback whatever the
//! outcome, and pass the outcome through.
//!
//! # Four closures, each a builtin made around one value
//!
//! `finally(onFinally)` calls `then` with two functions it makes:
//! `thenFinally` and `catchFinally`, each holding `onFinally`. Each of those,
//! when it runs, calls `onFinally` with nothing, waits for what it returned
//! (`Promise.resolve` of it), and then answers the original outcome — through
//! a `valueThunk` that returns the value, or a `thrower` that throws the
//! reason, each holding what it answers. So the callback's own result is
//! ignored, a callback that returns a promise delays the chain until it
//! settles, and a callback that throws replaces the outcome with its throw.
//!
//! Each is a builtin made around its one value
//! ([`Objects::native_holding`](crate::object::Objects::native_holding)), as a
//! promise's `resolve` is. `C` — which the specification also captures — is
//! always `Promise` here, because any other is refused by name when `finally`
//! reads it (see [`species`]).
//!
//! # What it is called on
//!
//! Any object: `finally` is generic, and calls whatever `then` its receiver
//! has. An `onFinally` that is not callable is handed to `then` as both
//! handlers unchanged, which `then` treats as none.

use crate::abrupt::{Escape, Internal, Thrown};
use crate::heap::Ref;
use crate::object::Value;
use crate::object::native::{Answer, Call, Native, Want};

use super::promise::key;
use super::promise_then::{Read, Species, judged, read, species, species_of};

/// `Promise.prototype.finally`, keeping the two functions it made.
pub(super) const FINALLY: Native = Native::new("finally", finally).keeping(2);

/// `thenFinally`, made around `onFinally`.
const THEN_FINALLY: Native = Native::new("thenFinally", then_finally).keeping(2);
/// `catchFinally`, made around `onFinally`.
const CATCH_FINALLY: Native = Native::new("catchFinally", catch_finally).keeping(2);
/// `valueThunk`, made around the value it returns.
const VALUE_THUNK: Native = Native::new("valueThunk", value_thunk);
/// `thrower`, made around the reason it throws.
const THROWER: Native = Native::new("thrower", thrower);

/// `finally`: the slot `thenFinally` is kept in.
const ON_FULFILLED: usize = 0;
/// `finally`: the slot `catchFinally` is kept in.
const ON_REJECTED: usize = 1;

/// `finally`: come back here with what a `constructor` getter answered.
const READ_CONSTRUCTOR: u32 = 1;
/// `finally`: come back here with what a `Symbol.species` getter answered.
const READ_SPECIES: u32 = 2;
/// Come back here with what a `then` getter answered.
const READ_THEN: u32 = 3;
/// Come back here with what `then` answered.
const CALLED_THEN: u32 = 4;

/// `thenFinally`/`catchFinally`: the slot the promise `onFinally` came to is
/// kept in.
const WAITED: usize = 0;
/// `thenFinally`/`catchFinally`: the slot the thunk is kept in.
const THUNK: usize = 1;

/// `thenFinally`/`catchFinally`: come back here with what `onFinally`
/// returned.
const CALLED_BACK: u32 = 1;
/// Come back here with `Promise.resolve` of it.
const WAITING: u32 = 2;
/// Come back here with what the promise's `then` answered. (`READ_THEN`
/// is shared with `finally`.)
const PASSED_ON: u32 = 4;

/// `Promise.prototype.finally(onFinally)`.
fn finally(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let Value::Object(object) = call.this() else {
        return Err(Escape::type_error(
            "Promise.prototype.finally was called on something that is not an object",
            call.at(),
        ));
    };
    match call.step() {
        0 => match species(call, object, READ_CONSTRUCTOR, READ_SPECIES)? {
            Species::Promise => prepare(call, object),
            Species::Asked(answer) => Ok(answer),
        },
        READ_CONSTRUCTOR => match species_of(call, call.answer()?, READ_SPECIES)? {
            Species::Promise => prepare(call, object),
            Species::Asked(answer) => Ok(answer),
        },
        READ_SPECIES => {
            judged(call, call.answer()?)?;
            prepare(call, object)
        }
        READ_THEN => handlers(call, object, call.answer()?),
        CALLED_THEN => Ok(Answer::Value(call.answer()?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Make `thenFinally` and `catchFinally` — or hand a callback that is not
/// callable to both unchanged — keep them, and read `then`.
fn prepare(call: &mut Call<'_>, object: Ref) -> Result<Answer, Escape> {
    let on_finally = call.argument(0);
    let callable = match on_finally {
        Value::Object(held) => call.seen().callable(held).is_some(),
        _ => false,
    };
    if callable {
        // Each a safepoint; `onFinally` is an argument, on the stack, and the
        // first function is kept before the second is made.
        let made = made_around(call, THEN_FINALLY, on_finally)?;
        call.keep(ON_FULFILLED, made)?;
        let made = made_around(call, CATCH_FINALLY, on_finally)?;
        call.keep(ON_REJECTED, made)?;
    } else {
        call.keep(ON_FULFILLED, on_finally)?;
        call.keep(ON_REJECTED, on_finally)?;
    }
    let key = key(call, "then")?;
    match read(call, object, key, READ_THEN, false)? {
        Read::Value(then) => handlers(call, object, then),
        Read::Asked(answer) => Ok(answer),
    }
}

/// Call `then` on `receiver` with `arguments`, coming back at `step`. A
/// `then` that is not callable is the `TypeError` any call of one is.
const fn invoke_then(receiver: Value, then: Value, arguments: Vec<Value>, step: u32) -> Answer {
    Answer::want(
        Want::Call {
            callee: then,
            receiver,
            arguments,
        },
        step,
    )
}

/// `finally`'s call of `then`: the two handlers it kept.
fn handlers(call: &Call<'_>, object: Ref, then: Value) -> Result<Answer, Escape> {
    let arguments = vec![call.kept(ON_FULFILLED)?, call.kept(ON_REJECTED)?];
    Ok(invoke_then(
        Value::Object(object),
        then,
        arguments,
        CALLED_THEN,
    ))
}

/// `thenFinally`'s and `catchFinally`'s call of `then`: the thunk it kept,
/// on the promise it waited for.
fn passed_on(call: &Call<'_>, then: Value) -> Result<Answer, Escape> {
    let arguments = vec![call.kept(THUNK)?];
    Ok(invoke_then(call.kept(WAITED)?, then, arguments, PASSED_ON))
}

/// A builtin made around `value`, inheriting from `Function.prototype`.
fn made_around(call: &mut Call<'_>, native: Native, value: Value) -> Result<Value, Escape> {
    let function_prototype = call.intrinsics()?.function_prototype(call.seen())?;
    let at = call.at();
    call.objects()
        .native_holding(native, Some(function_prototype), value)
        .map(Value::Object)
        .map_err(|why| Escape::refused(why, at))
}

/// `thenFinally(value)`.
fn then_finally(call: &mut Call<'_>) -> Result<Answer, Escape> {
    after_finally(call, VALUE_THUNK)
}

/// `catchFinally(reason)`.
fn catch_finally(call: &mut Call<'_>) -> Result<Answer, Escape> {
    after_finally(call, THROWER)
}

/// What `thenFinally` and `catchFinally` share: call `onFinally`, wait for
/// what it returned, then answer the outcome through `thunk` made around the
/// argument.
fn after_finally(call: &mut Call<'_>, thunk: Native) -> Result<Answer, Escape> {
    match call.step() {
        0 => Ok(Answer::want(
            Want::Call {
                callee: call.held(),
                receiver: Value::Undefined,
                arguments: Vec::new(),
            },
            CALLED_BACK,
        )),
        CALLED_BACK => {
            let intrinsics = call.intrinsics()?;
            let resolve = intrinsics.promise_resolve(call.seen())?;
            let promise = intrinsics.promise_constructor(call.seen())?;
            Ok(Answer::want(
                Want::Call {
                    callee: Value::Object(resolve),
                    receiver: Value::Object(promise),
                    arguments: vec![call.answer()?],
                },
                WAITING,
            ))
        }
        WAITING => {
            let waited = call.answer()?;
            let Value::Object(promise) = waited else {
                // `Promise.resolve` answers an object, always.
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            call.keep(WAITED, waited)?;
            // The value or reason is the argument, on the stack.
            let made = made_around(call, thunk, call.argument(0))?;
            call.keep(THUNK, made)?;
            let key = key(call, "then")?;
            match read(call, promise, key, READ_THEN, false)? {
                Read::Value(then) => passed_on(call, then),
                Read::Asked(answer) => Ok(answer),
            }
        }
        READ_THEN => passed_on(call, call.answer()?),
        PASSED_ON => Ok(Answer::Value(call.answer()?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `valueThunk()`: the value it was made around.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn value_thunk(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(call.held()))
}

/// `thrower()`: throw the reason it was made around.
fn thrower(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Err(Escape::Thrown(Thrown::Value {
        value: call.held(),
        at: call.at(),
    }))
}
