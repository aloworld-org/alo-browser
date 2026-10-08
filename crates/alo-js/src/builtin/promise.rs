/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Promise` (ADR 0032 § 5, queue item 333): the constructor and its
//! executor, `Promise.resolve`, `Promise.reject`, and what they are made of.
//!
//! Cut from item 75 because `fetch` answers one (items 334 and 335), and a
//! promise needs only the job queue (item 232) and the checkpoint after every
//! task (item 235), both built. What is here and in its four siblings:
//!
//! - the constructor and its statics, here;
//! - `then` and `catch`, and what `then` reads to learn what to make
//!   ([`promise_then`](super::promise_then));
//! - `finally` and its closures ([`promise_finally`](super::promise_finally));
//! - the `resolve` and `reject` an executor is handed, and the one resolve
//!   procedure ([`promise_resolving`](super::promise_resolving));
//! - the reaction job and the thenable job
//!   ([`promise_job`](super::promise_job)).
//!
//! The state is a cell of its own ([`Promise`](crate::object::Promise)), and
//! settling it is the engine's ([`Want::Settle`]), because what follows —
//! a job per reaction, and a rejection nobody handled reported at the end of
//! the checkpoint — is the engine's.
//!
//! # The executor runs now, and its throw is a rejection
//!
//! `new Promise(executor)` calls the executor before it answers, with the
//! promise's `resolve` and `reject`, and a throw from it rejects the promise
//! — through `reject`, so a throw after `resolve` has run changes nothing.
//! `Promise()` without `new` is the `TypeError` the specification gives, and
//! so is an executor that is not callable.
//!
//! # What is not here
//!
//! The combinators — `all`, `allSettled`, `race`, `any` — and `async`/`await`
//! stay in item 75. A promise made by a constructor other than `Promise` — a
//! subclass, or a `Symbol.species` naming another — is refused by name
//! ([`Missing::APromiseOfAnotherConstructor`], item 337). `Promise.try` and
//! `Promise.withResolvers` are not here either: no page has asked for them.

use crate::abrupt::{Escape, Internal, Missing};
use crate::heap::{Ref, Root};
use crate::object::native::{Answer, Call, Instance, Native, Want};
use crate::object::{Fault, Found, Key, Objects, Property, Value};

use super::promise_finally::FINALLY;
use super::promise_job::{REACTION_JOB, THENABLE_JOB};
use super::promise_resolving::{RESOLVE_PROMISE, resolving_functions};
use super::promise_then::{CATCH, Read, THEN, read};

/// The constructor, keeping the pair it hands its executor.
const PROMISE: Native = Native::constructor("Promise", construct, Instance::Promise).keeping(2);
/// `Promise.resolve`, keeping the promise it made.
const RESOLVE: Native = Native::new("resolve", resolve).keeping(1);
/// `Promise.reject`, keeping the promise it made.
const REJECT: Native = Native::new("reject", reject).keeping(1);
/// `get Promise[Symbol.species]`.
const SPECIES: Native = Native::new("get [Symbol.species]", species);

/// The constructor: where it keeps `resolve`.
const RESOLVE_SLOT: usize = 0;
/// The constructor: where it keeps `reject`.
const REJECT_SLOT: usize = 1;
/// `Promise.resolve` and `Promise.reject`: where they keep what they made.
const MADE: usize = 0;

/// The constructor: come back here with what the executor returned or threw.
const EXECUTED: u32 = 1;
/// The constructor: come back here once `reject` has run.
const REJECTED: u32 = 2;
/// `Promise.resolve`: come back here with what a `constructor` getter
/// answered.
const READ_CONSTRUCTOR: u32 = 1;
/// `Promise.resolve` and `Promise.reject`: come back here once the promise
/// they made is resolved or rejected.
const SETTLED: u32 = 2;

/// What a realm's `Promise` is made of, each rooted.
#[derive(Debug)]
pub(super) struct Made {
    /// `Promise`.
    pub(super) constructor: Root,
    /// `Promise.prototype`.
    pub(super) prototype: Root,
    /// The function `Promise.resolve` is, which `finally` calls whatever a
    /// page has since done to the property.
    pub(super) resolve: Root,
    /// `%PromiseReactionJob%`.
    pub(super) reaction_job: Root,
    /// `%PromiseResolveThenableJob%`.
    pub(super) thenable_job: Root,
    /// `%ResolvePromise%`.
    pub(super) resolve_promise: Root,
}

/// The keys `Promise` is furnished under that are well-known symbols.
#[derive(Debug, Clone, Copy)]
pub(super) struct Symbols {
    /// `Symbol.species`.
    pub(super) species: Key,
    /// `Symbol.toStringTag`.
    pub(super) to_string_tag: Key,
}

/// Make `Promise`, its prototype, their methods and the realm's three
/// internal promise functions.
///
/// **This allocates repeatedly.** Everything made is held in a scope until a
/// property or a root owns it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference
/// this engine has lost.
pub(super) fn make(
    objects: &mut Objects,
    object_prototype: Ref,
    function_prototype: Ref,
    symbols: Symbols,
) -> Result<Made, Escape> {
    let scope = objects.heap_mut().open();
    let outcome = made(objects, object_prototype, function_prototype, symbols);
    objects.heap_mut().close(scope);
    outcome
}

/// [`make`], with the scope already open.
fn made(
    objects: &mut Objects,
    object_prototype: Ref,
    function_prototype: Ref,
    symbols: Symbols,
) -> Result<Made, Escape> {
    let prototype = objects
        .object(Some(object_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(prototype);
    let constructor = objects
        .native(PROMISE, Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(constructor);

    // `prototype` on a builtin constructor is fixed, which is what lets the
    // interpreter treat anything else there as its own bug when it makes an
    // instance.
    let key = held_key(objects, "prototype")?;
    objects.define(
        constructor,
        key,
        Property::data(Value::Object(prototype), false, false, false),
    )?;
    let key = held_key(objects, "constructor")?;
    objects.define(
        prototype,
        key,
        Property::data(Value::Object(constructor), true, false, true),
    )?;
    // `"[object Promise]"`, which `Object.prototype.toString` reads here.
    let tag = objects
        .text("Promise".encode_utf16().collect())
        .map_err(|why| Escape::refused(why, 0))?;
    objects.define(
        prototype,
        symbols.to_string_tag,
        Property::data(Value::Text(tag), false, false, true),
    )?;
    for native in [THEN, CATCH, FINALLY] {
        super::native_method(objects, prototype, function_prototype, native)?;
    }
    for native in [RESOLVE, REJECT] {
        super::native_method(objects, constructor, function_prototype, native)?;
    }
    let getter = objects
        .native(SPECIES, Some(function_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(getter);
    objects.define(
        constructor,
        symbols.species,
        Property::accessor(Value::Object(getter), Value::Undefined, false, true),
    )?;

    let key = held_key(objects, "resolve")?;
    let Found::Value(Value::Object(resolve)) = objects.get(constructor, key)? else {
        return Err(Escape::fault(Fault::Gone));
    };
    let mut internal = Vec::with_capacity(3);
    for native in [REACTION_JOB, THENABLE_JOB, RESOLVE_PROMISE] {
        let function = objects
            .native(native, Some(function_prototype))
            .map_err(|why| Escape::refused(why, 0))?;
        objects.heap_mut().hold(function);
        internal.push(function);
    }
    let [reaction_job, thenable_job, resolve_promise] = internal[..] else {
        return Err(Escape::fault(Fault::Gone));
    };
    let heap = objects.heap_mut();
    Ok(Made {
        constructor: heap.root(constructor),
        prototype: heap.root(prototype),
        resolve: heap.root(resolve),
        reaction_job: heap.root(reaction_job),
        thenable_job: heap.root(thenable_job),
        resolve_promise: heap.root(resolve_promise),
    })
}

/// Intern a name and hold the string that spells it in the open scope.
fn held_key(objects: &mut Objects, name: &str) -> Result<Key, Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = objects.key(&units).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = key.reference() {
        objects.heap_mut().hold(held);
    }
    Ok(key)
}

/// The key `name` is, for a builtin about to read it.
///
/// **May allocate**, interning the spelling; whatever the caller means to keep
/// must be on the stack or in a slot first.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling.
pub(super) fn key(call: &mut Call<'_>, name: &str) -> Result<Key, Escape> {
    let at = call.at();
    let units: Vec<u16> = name.encode_utf16().collect();
    call.objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))
}

/// Whether `value` is this realm's `Promise`.
///
/// # Errors
///
/// A fault if the realm has lost it, which is this engine's own bug.
pub(super) fn is_the_promise_constructor(call: &Call<'_>, value: Value) -> Result<bool, Escape> {
    let promise = call.intrinsics()?.promise_constructor(call.seen())?;
    Ok(value == Value::Object(promise))
}

/// `new Promise(executor)`.
fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match call.step() {
        0 => {
            if !call.constructing() {
                return Err(Escape::type_error(
                    "Promise is a constructor, and is called with new",
                    call.at(),
                ));
            }
            let executor = call.argument(0);
            let callable = match executor {
                Value::Object(held) => call.seen().callable(held).is_some(),
                _ => false,
            };
            if !callable {
                return Err(Escape::type_error(
                    "a promise's executor is not a function",
                    call.at(),
                ));
            }
            let Value::Object(promise) = call.this() else {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            // The promise is `this` and the executor an argument, both on the
            // stack, across the three allocations.
            let (resolve, reject) = resolving_functions(call, promise)?;
            call.keep(RESOLVE_SLOT, Value::Object(resolve))?;
            call.keep(REJECT_SLOT, Value::Object(reject))?;
            Ok(Answer::want(
                Want::Catch {
                    callee: executor,
                    receiver: Value::Undefined,
                    arguments: vec![Value::Object(resolve), Value::Object(reject)],
                },
                EXECUTED,
            ))
        }
        EXECUTED if call.threw() => Ok(Answer::want(
            Want::Call {
                callee: call.kept(REJECT_SLOT)?,
                receiver: Value::Undefined,
                arguments: vec![call.answer()?],
            },
            REJECTED,
        )),
        EXECUTED | REJECTED => Ok(Answer::Value(call.this())),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `Promise.resolve(x)`: `x` itself when it is a promise `Promise` made, and
/// otherwise a new promise resolved with it.
///
/// `this` must be an object before anything else; whether it is a constructor
/// this engine makes promises with is asked only when one is made, after
/// `x.constructor` has been read, which is the specification's order.
fn resolve(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if !matches!(call.this(), Value::Object(_)) {
        return Err(Escape::type_error(
            "Promise.resolve was called on something that is not an object",
            call.at(),
        ));
    }
    let x = call.argument(0);
    match call.step() {
        0 => {
            let Value::Object(held) = x else {
                return resolved_anew(call);
            };
            if call.seen().as_promise(held).is_none() {
                return resolved_anew(call);
            }
            let key = key(call, "constructor")?;
            match read(call, held, key, READ_CONSTRUCTOR, false)? {
                Read::Value(constructor) => same_or_anew(call, constructor),
                Read::Asked(answer) => Ok(answer),
            }
        }
        READ_CONSTRUCTOR => same_or_anew(call, call.answer()?),
        SETTLED => Ok(Answer::Value(call.kept(MADE)?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `PromiseResolve`'s test: a promise whose `constructor` is the `this`
/// `Promise.resolve` was called on is answered as it is.
fn same_or_anew(call: &mut Call<'_>, constructor: Value) -> Result<Answer, Escape> {
    if constructor.same_value(call.this()) {
        return Ok(Answer::Value(call.argument(0)));
    }
    resolved_anew(call)
}

/// A new promise, kept, resolved with the argument through
/// `%ResolvePromise%`.
fn resolved_anew(call: &mut Call<'_>) -> Result<Answer, Escape> {
    made_by_promise(call, "resolve")?;
    let made = new_promise(call)?;
    let resolve = call.intrinsics()?.resolve_promise(call.seen())?;
    Ok(Answer::want(
        Want::Call {
            callee: Value::Object(resolve),
            receiver: Value::Undefined,
            arguments: vec![made, call.argument(0)],
        },
        SETTLED,
    ))
}

/// `Promise.reject(r)`: a new promise, rejected with `r`.
fn reject(call: &mut Call<'_>) -> Result<Answer, Escape> {
    made_by_promise(call, "reject")?;
    match call.step() {
        0 => {
            let made = new_promise(call)?;
            Ok(Answer::want(
                Want::Settle {
                    promise: made,
                    fulfilled: false,
                    value: call.argument(0),
                },
                SETTLED,
            ))
        }
        SETTLED => Ok(Answer::Value(call.kept(MADE)?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// A pending promise from `Promise.prototype`, kept in [`MADE`].
fn new_promise(call: &mut Call<'_>) -> Result<Value, Escape> {
    let prototype = call.intrinsics()?.promise_prototype(call.seen())?;
    let at = call.at();
    let made = call
        .objects()
        .promise(Some(prototype))
        .map(Value::Object)
        .map_err(|why| Escape::refused(why, at))?;
    call.keep(MADE, made)?;
    Ok(made)
}

/// `NewPromiseCapability(this)`'s first question, for the statics: `this`
/// must be a constructor, and the only one built is `Promise`.
fn made_by_promise(call: &Call<'_>, name: &str) -> Result<(), Escape> {
    let this = call.this();
    if is_the_promise_constructor(call, this)? {
        return Ok(());
    }
    match this {
        Value::Object(held)
            if call
                .seen()
                .callable(held)
                .is_some_and(crate::object::Function::is_constructor) =>
        {
            Err(Escape::NotBuiltYet(Missing::APromiseOfAnotherConstructor))
        }
        _ => Err(Escape::type_error(
            format!("Promise.{name} was called on something that is not a constructor"),
            call.at(),
        )),
    }
}

/// `get Promise[Symbol.species]`: its `this`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn species(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(call.this()))
}
