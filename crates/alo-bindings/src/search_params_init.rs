/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `new URLSearchParams(init)`'s argument, converted (queue item 472).
//!
//! `init` is a Web IDL union, `(sequence<sequence<USVString>> or
//! record<USVString, USVString> or USVString)`, defaulting to `""`, and the
//! union decides by what `init` is:
//!
//! - **An object with a `Symbol.iterator` method** is a sequence of pairs —
//!   the contact form's `new FormData(form)` is one, and so is
//!   `[['a', 'b']]`. It is walked by the iterator protocol, and each item is
//!   itself walked as a sequence of strings, which must hold exactly two:
//!   anything else is Chrome 154's `TypeError`, *Sequence initializer must
//!   only contain pair elements*.
//! - **Any other object** is a record: its own enumerable properties, in
//!   order, each name and value a string. A second name that converts to
//!   one already there sets that one's value, as a record's conversion does.
//!   A symbol among the names is a `TypeError`, as Chrome's is.
//! - **Anything else** is a string — `null` is `"null"` — read by the URL
//!   Standard's parser ([`alo_url::form::parse`]) after one leading `?` is
//!   cut off. `undefined` is the default, and nothing.
//!
//! # One step at a time
//!
//! Walking a sequence calls the page's script at every turn: its
//! `[Symbol.iterator]`, its `next`, each result's `done` and `value` (any of
//! which may be a getter), and each item's `toString`. A native asks the
//! interpreter for each of those and is run again at the step it named
//! (ADR 0031), so this is the protocol written as a list of stages, each a
//! place to come back to. What is in hand across a call — the two
//! iterators, their `next` methods, the last result and the pair's two
//! strings — is in the constructor's slots, and every pair finished is in
//! the instance's list. The step says the stage, and how many strings of the
//! current pair have been converted, counting no further than three: a pair
//! of three is refused once its sequence ends, and every one of its items is
//! still converted first, as Web IDL converts them.
//!
//! Web IDL's sequence conversion does not close an iterator it leaves by a
//! throw, so neither does this.
//!
//! # Refused by name
//!
//! A record's value that is a getter, or an object whose `toString` must
//! run: each would need a call per property, as a fetch's headers would, and
//! waits for a page that does it.

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Found, Key};
use alo_js::{Escape, Value};

use crate::idl;
use crate::search_params::SearchParams;

/// The outer iterator.
const OUTER: usize = 0;
/// The outer iterator's `next`.
const OUTER_NEXT: usize = 1;
/// The inner iterable, and then its iterator.
const INNER: usize = 2;
/// The inner iterator's `next`.
const INNER_NEXT: usize = 3;
/// The last iterator result, outer or inner.
const RESULT: usize = 4;
/// The pair's first string.
const NAME: usize = 5;
/// The pair's second string.
const VALUE: usize = 6;
/// How many slots the constructor keeps.
pub(crate) const SLOTS: usize = 7;

/// A place in the protocol to come back to. Each comes back with what was
/// asked for as the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// `init[Symbol.iterator]`'s getter.
    OuterMethod = 1,
    /// `init[Symbol.iterator]()`: the outer iterator.
    OuterIterator,
    /// The outer iterator's `next` getter.
    OuterNext,
    /// The outer `next()`: a result.
    OuterResult,
    /// Its `done` getter.
    OuterDone,
    /// Its `value` getter: the item, a sequence.
    OuterValue,
    /// The item's `[Symbol.iterator]` getter.
    InnerMethod,
    /// The item's `[Symbol.iterator]()`: the inner iterator.
    InnerIterator,
    /// The inner iterator's `next` getter.
    InnerNext,
    /// The inner `next()`: a result.
    InnerResult,
    /// Its `done` getter.
    InnerDone,
    /// Its `value` getter: a string to convert.
    InnerValue,
    /// That string's `toString`.
    InnerConverted,
}

/// Every stage, in order of its number.
const STAGES: [Stage; 13] = [
    Stage::OuterMethod,
    Stage::OuterIterator,
    Stage::OuterNext,
    Stage::OuterResult,
    Stage::OuterDone,
    Stage::OuterValue,
    Stage::InnerMethod,
    Stage::InnerIterator,
    Stage::InnerNext,
    Stage::InnerResult,
    Stage::InnerDone,
    Stage::InnerValue,
    Stage::InnerConverted,
];

/// The most a pair's count is kept as: three means *more than two*.
const MORE_THAN_A_PAIR: u32 = 3;

/// The step that comes back at `stage` with `count` strings of the pair
/// converted.
const fn step(stage: Stage, count: u32) -> u32 {
    stage as u32 * 4 + count
}

/// The stage and count a step names.
fn place(step: u32) -> Result<(Stage, u32), Escape> {
    let stage = usize::try_from(step / 4)
        .ok()
        .and_then(|number| number.checked_sub(1))
        .and_then(|index| STAGES.get(index))
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok((*stage, step % 4))
}

/// Chrome 154's words for an item that is not a pair.
const NOT_A_PAIR: &str = "Sequence initializer must only contain pair elements";
/// Chrome 154's words for an item that is not a sequence at all.
const NOT_A_SEQUENCE: &str = "The provided value cannot be converted to a sequence.";

/// Convert `init` into `instance`'s list, as far as it can be without
/// asking for something: what to ask for, or [`None`] once it is all in.
///
/// # Errors
///
/// A `TypeError` wherever Web IDL throws one, whatever the page's script
/// threw on the way, a refusal by name for a record this engine cannot
/// read, and this crate's bug at a step it never named.
pub(crate) fn read(call: &mut Call<'_>, instance: Ref) -> Result<Option<Answer>, Escape> {
    if call.step() == 0 {
        return start(call, instance);
    }
    let (stage, count) = place(call.step())?;
    let answer = call.answer()?;
    match stage {
        Stage::OuterMethod => outer_method(call, instance, answer),
        Stage::OuterIterator => {
            let iterator = an_object(call, answer, "the iterator is not an object")?;
            call.keep(OUTER, Value::Object(iterator))?;
            match got(call, iterator, Name::Next, step(Stage::OuterNext, 0))? {
                Ok(next) => outer_next(call, next),
                Err(asked) => Ok(Some(asked)),
            }
        }
        Stage::OuterNext => outer_next(call, answer),
        Stage::OuterResult => {
            let result = an_object(call, answer, "an iterator result is not an object")?;
            call.keep(RESULT, Value::Object(result))?;
            match got(call, result, Name::Done, step(Stage::OuterDone, 0))? {
                Ok(done) => outer_done(call, done),
                Err(asked) => Ok(Some(asked)),
            }
        }
        Stage::OuterDone => outer_done(call, answer),
        Stage::OuterValue => inner_start(call, answer),
        Stage::InnerMethod => inner_method(call, answer),
        Stage::InnerIterator => {
            let iterator = an_object(call, answer, "the iterator is not an object")?;
            call.keep(INNER, Value::Object(iterator))?;
            match got(call, iterator, Name::Next, step(Stage::InnerNext, 0))? {
                Ok(next) => inner_next(call, next),
                Err(asked) => Ok(Some(asked)),
            }
        }
        Stage::InnerNext => inner_next(call, answer),
        Stage::InnerResult => {
            let result = an_object(call, answer, "an iterator result is not an object")?;
            call.keep(RESULT, Value::Object(result))?;
            match got(call, result, Name::Done, step(Stage::InnerDone, count))? {
                Ok(done) => inner_done(call, instance, done, count),
                Err(asked) => Ok(Some(asked)),
            }
        }
        Stage::InnerDone => inner_done(call, instance, answer, count),
        Stage::InnerValue => item(call, answer, count),
        Stage::InnerConverted => {
            let text = idl::answered_string(call)?;
            stored(call, &text, count)
        }
    }
}

/// Step 0: `init` as the union decides it.
fn start(call: &mut Call<'_>, instance: Ref) -> Result<Option<Answer>, Escape> {
    match call.argument(0) {
        Value::Undefined => Ok(None),
        Value::Object(init) => {
            let iterator = call
                .intrinsics()?
                .well_known_key(call.seen(), WellKnown::Iterator)?;
            match got_key(call, init, iterator, step(Stage::OuterMethod, 0))? {
                Ok(method) => outer_method(call, instance, method),
                Err(asked) => Ok(Some(asked)),
            }
        }
        other => {
            let primitive = Primitive::of(other).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let units = convert::to_units(call.seen(), primitive, call.at())?;
            let text = String::from_utf16_lossy(&units);
            let query = text.strip_prefix('?').unwrap_or(&text);
            let pairs = alo_url::form::parse(query.as_bytes());
            add(call, instance, |params| {
                for (name, value) in pairs {
                    params.append(name, value);
                }
            })?;
            Ok(None)
        }
    }
}

/// `init` has `method` as its `[Symbol.iterator]`: none makes it a record,
/// and anything else is called.
fn outer_method(
    call: &mut Call<'_>,
    instance: Ref,
    method: Value,
) -> Result<Option<Answer>, Escape> {
    let Value::Object(init) = call.argument(0) else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    if matches!(method, Value::Undefined | Value::Null) {
        record(call, instance, init)?;
        return Ok(None);
    }
    Ok(Some(Answer::want(
        Want::Call {
            callee: method,
            receiver: Value::Object(init),
            arguments: Vec::new(),
        },
        step(Stage::OuterIterator, 0),
    )))
}

/// Keep the outer `next`, and call it.
fn outer_next(call: &mut Call<'_>, next: Value) -> Result<Option<Answer>, Escape> {
    call.keep(OUTER_NEXT, next)?;
    ask_next(call, OUTER, OUTER_NEXT, step(Stage::OuterResult, 0))
}

/// The outer result says `done`: finished, or its `value` is the next item.
fn outer_done(call: &mut Call<'_>, done: Value) -> Result<Option<Answer>, Escape> {
    if convert::to_boolean(call.seen(), done) {
        return Ok(None);
    }
    let result = kept_object(call, RESULT)?;
    match got(call, result, Name::Value, step(Stage::OuterValue, 0))? {
        Ok(value) => inner_start(call, value),
        Err(asked) => Ok(Some(asked)),
    }
}

/// An item of the outer sequence, to be walked as a sequence of strings.
fn inner_start(call: &mut Call<'_>, item: Value) -> Result<Option<Answer>, Escape> {
    let Value::Object(iterable) = item else {
        return Err(Escape::type_error(NOT_A_SEQUENCE, call.at()));
    };
    call.keep(INNER, item)?;
    let iterator = call
        .intrinsics()?
        .well_known_key(call.seen(), WellKnown::Iterator)?;
    match got_key(call, iterable, iterator, step(Stage::InnerMethod, 0))? {
        Ok(method) => inner_method(call, method),
        Err(asked) => Ok(Some(asked)),
    }
}

/// The item has `method` as its `[Symbol.iterator]`: none is not a
/// sequence, and anything else is called.
fn inner_method(call: &mut Call<'_>, method: Value) -> Result<Option<Answer>, Escape> {
    if matches!(method, Value::Undefined | Value::Null) {
        return Err(Escape::type_error(NOT_A_SEQUENCE, call.at()));
    }
    let iterable = call.kept(INNER)?;
    Ok(Some(Answer::want(
        Want::Call {
            callee: method,
            receiver: iterable,
            arguments: Vec::new(),
        },
        step(Stage::InnerIterator, 0),
    )))
}

/// Keep the inner `next`, and call it for the pair's first string.
fn inner_next(call: &mut Call<'_>, next: Value) -> Result<Option<Answer>, Escape> {
    call.keep(INNER_NEXT, next)?;
    ask_next(call, INNER, INNER_NEXT, step(Stage::InnerResult, 0))
}

/// The inner result says `done`: the pair is finished — two strings, or
/// refused — or its `value` is the next string.
fn inner_done(
    call: &mut Call<'_>,
    instance: Ref,
    done: Value,
    count: u32,
) -> Result<Option<Answer>, Escape> {
    if convert::to_boolean(call.seen(), done) {
        if count != 2 {
            return Err(Escape::type_error(NOT_A_PAIR, call.at()));
        }
        let name = kept_string(call, NAME)?;
        let value = kept_string(call, VALUE)?;
        add(call, instance, |params| params.append(name, value))?;
        return ask_next(call, OUTER, OUTER_NEXT, step(Stage::OuterResult, 0));
    }
    let result = kept_object(call, RESULT)?;
    match got(call, result, Name::Value, step(Stage::InnerValue, count))? {
        Ok(value) => item(call, value, count),
        Err(asked) => Ok(Some(asked)),
    }
}

/// One string of a pair: converted now, or its `toString` asked for.
fn item(call: &mut Call<'_>, value: Value, count: u32) -> Result<Option<Answer>, Escape> {
    match Primitive::of(value) {
        Some(primitive) => {
            let units = convert::to_units(call.seen(), primitive, call.at())?;
            stored(call, &String::from_utf16_lossy(&units), count)
        }
        None => Ok(Some(Answer::want(
            Want::Primitive {
                of: value,
                hint: Hint::String,
            },
            step(Stage::InnerConverted, count),
        ))),
    }
}

/// Keep the pair's `count`th string, if it is one of the two, and ask the
/// inner iterator for the next.
fn stored(call: &mut Call<'_>, text: &str, count: u32) -> Result<Option<Answer>, Escape> {
    let slot = match count {
        0 => Some(NAME),
        1 => Some(VALUE),
        _ => None,
    };
    if let Some(slot) = slot {
        let at = call.at();
        let made = call
            .objects()
            .text(text.encode_utf16().collect())
            .map_err(|why| Escape::refused(why, at))?;
        call.keep(slot, Value::Text(made))?;
    }
    let count = count.saturating_add(1).min(MORE_THAN_A_PAIR);
    ask_next(call, INNER, INNER_NEXT, step(Stage::InnerResult, count))
}

/// Ask for `next` of the iterator in slot `iterator`, kept in slot `next`,
/// coming back at `at`.
fn ask_next(
    call: &Call<'_>,
    iterator: usize,
    next: usize,
    at: u32,
) -> Result<Option<Answer>, Escape> {
    let receiver = call.kept(iterator)?;
    let callee = call.kept(next)?;
    Ok(Some(Answer::want(
        Want::Call {
            callee,
            receiver,
            arguments: Vec::new(),
        },
        at,
    )))
}

/// A record's own enumerable properties, in order, into `instance`.
fn record(call: &mut Call<'_>, instance: Ref, init: Ref) -> Result<(), Escape> {
    let at = call.at();
    let mut pairs = Vec::new();
    for key in call.seen().own_keys(init)? {
        let Some(property) = call.seen().own_property(init, key)? else {
            continue;
        };
        if !property.is_enumerable() {
            continue;
        }
        let name = key_text(call, key)?;
        let value = property.value().ok_or_else(|| {
            not_built(
                "a URLSearchParams record whose value is behind a getter is not built: it needs \
                 a call per property, and waits for a page that does it",
            )
        })?;
        let primitive = Primitive::of(value).ok_or_else(|| {
            not_built(
                "a URLSearchParams record whose value is an object is not built: it needs a \
                 call per property, and waits for a page that does it",
            )
        })?;
        let value = String::from_utf16_lossy(&convert::to_units(call.seen(), primitive, at)?);
        pairs.push((name, value));
    }
    add(call, instance, |params| {
        for (name, value) in pairs {
            params.set_first(name, value);
        }
    })
}

/// A property key as a `USVString`: a symbol is a `TypeError`.
fn key_text(call: &Call<'_>, key: Key) -> Result<String, Escape> {
    if let Some(index) = key.as_index() {
        return Ok(index.to_string());
    }
    key.as_text()
        .and_then(|held| call.seen().units(held))
        .map(String::from_utf16_lossy)
        .ok_or_else(|| Escape::type_error("Cannot convert a Symbol value to a string", call.at()))
}

/// A refusal by name of `what`, which this engine has not built.
fn not_built(what: &'static str) -> Escape {
    Escape::NotBuiltYet(Missing::InTheEmbedder(what))
}

/// A name the protocol reads.
#[derive(Debug, Clone, Copy)]
enum Name {
    Next,
    Done,
    Value,
}

impl Name {
    const fn text(self) -> &'static str {
        match self {
            Name::Next => "next",
            Name::Done => "done",
            Name::Value => "value",
        }
    }
}

/// `Get(object, name)`: the value, or the getter call to ask for, coming
/// back at `at`. A name never interned is one no object has.
fn got(call: &Call<'_>, object: Ref, name: Name, at: u32) -> Result<Result<Value, Answer>, Escape> {
    let units: Vec<u16> = name.text().encode_utf16().collect();
    match call.seen().existing_key(&units) {
        Some(key) => got_key(call, object, key, at),
        None => Ok(Ok(Value::Undefined)),
    }
}

/// `Get(object, key)`, as [`got`].
fn got_key(
    call: &Call<'_>,
    object: Ref,
    key: Key,
    at: u32,
) -> Result<Result<Value, Answer>, Escape> {
    Ok(match call.seen().get(object, key)? {
        Found::Missing | Found::Getter(Value::Undefined) => Ok(Value::Undefined),
        Found::Value(value) => Ok(value),
        Found::Getter(getter) => Err(Answer::want(
            Want::Call {
                callee: getter,
                receiver: Value::Object(object),
                arguments: Vec::new(),
            },
            at,
        )),
    })
}

/// `value` as an object, or the `TypeError` `why`.
fn an_object(call: &Call<'_>, value: Value, why: &'static str) -> Result<Ref, Escape> {
    match value {
        Value::Object(held) => Ok(held),
        _ => Err(Escape::type_error(why, call.at())),
    }
}

/// The object kept in slot `which`.
fn kept_object(call: &Call<'_>, which: usize) -> Result<Ref, Escape> {
    match call.kept(which)? {
        Value::Object(held) => Ok(held),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// The string kept in slot `which`.
fn kept_string(call: &Call<'_>, which: usize) -> Result<String, Escape> {
    match call.kept(which)? {
        Value::Text(held) => call
            .seen()
            .units(held)
            .map(String::from_utf16_lossy)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Change `instance`'s list with `change`.
fn add(
    call: &mut Call<'_>,
    instance: Ref,
    change: impl FnOnce(&mut SearchParams),
) -> Result<(), Escape> {
    call.objects()
        .write_embedded::<SearchParams, _>(instance, |params, _| change(params))
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_names_its_own_place() {
        for (index, stage) in STAGES.iter().enumerate() {
            assert_eq!(*stage as usize, index + 1, "{stage:?} is in order");
            for count in 0..=MORE_THAN_A_PAIR {
                assert_eq!(place(step(*stage, count)).ok(), Some((*stage, count)));
            }
        }
        for broken in [1, 2, 3, step(Stage::InnerConverted, 3) + 1] {
            assert!(place(broken).is_err(), "{broken}");
        }
    }
}
