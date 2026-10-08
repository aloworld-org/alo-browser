/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `%RegExp.prototype%`: `exec` and `test` (queue item 74, ADR 0029).
//!
//! `exec` is `RegExpBuiltinExec`: read `lastIndex`, search from it with the
//! [`matcher`](crate::regexp::matcher), move it if the pattern is global or
//! sticky, and answer an array of what matched. `test` is the
//! specification's generic method: it calls whatever `exec` the object has,
//! so a page that replaced `exec` is honoured, and uses the builtin search
//! only when the object has no `exec` to call.
//!
//! # The bound, as a page sees it
//!
//! A search that takes more steps than one match may is a `RangeError`, which
//! a page's `catch` catches — ADR 0029 § 3, and the same thing a runaway
//! recursion already is. A search the embedder stops is
//! [`Escape::Interrupted`], which nothing catches.
//!
//! # What runs script, and the one case refused
//!
//! The string argument may be an object, whose `toString` runs; `lastIndex`
//! may hold an object, whose `valueOf` runs; and `test` reads `exec`, which
//! may be a getter or a function the page wrote. A native keeps a step number
//! and nothing else across a call, so when the string came from a call the
//! converted string is in the one slot a **second** call's answer would be
//! written over. That case — an object for the string and then a getter for
//! `exec` or an object in `lastIndex` — is refused by name
//! ([`Missing::ATwoCallRegExpMethod`], queue item 221) rather than converted
//! twice. Every other order is the specification's, step for step.

use std::sync::Arc;

use crate::abrupt::{Escape, Internal, Missing};
use crate::bounds;
use crate::convert::{self, Hint, Primitive};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Want};
use crate::object::{Fault, Found, Key, Objects, Property, Set, Value};
use crate::regexp::flags::Flag;
use crate::regexp::{Found as Match, Halt, Program, search};

use super::Intrinsics;

/// The string argument was turned into a primitive by running script.
const STRING_CONVERTED: u32 = 1;
/// `lastIndex` was turned into a primitive by running script.
const LAST_INDEX_CONVERTED: u32 = 2;
/// `test` read `exec` through a getter.
const EXEC_READ: u32 = 3;
/// `test` called `exec`.
const EXEC_CALLED: u32 = 4;

/// Put `exec` and `test` on it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn furnish(objects: &mut Objects, intrinsics: &Intrinsics) -> Result<(), Escape> {
    let on = intrinsics.regexp_prototype(objects)?;
    let functions = intrinsics.function_prototype(objects)?;
    super::method(objects, on, functions, "exec", exec)?;
    super::method(objects, on, functions, "test", test)?;
    Ok(())
}

/// `RegExp.prototype.exec(string)`.
fn exec(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let regexp = match call.this() {
        Value::Object(held) if call.seen().as_regexp(held).is_some() => held,
        _ => {
            return Err(Escape::type_error(
                "RegExp.prototype.exec was called on something that is not a regular expression",
                at,
            ));
        }
    };
    let (string, ran, last_index) = match call.step() {
        0 => match Primitive::of(call.argument(0)) {
            Some(string) => (string, false, None),
            None => return Ok(to_string_first(call)),
        },
        STRING_CONVERTED => (primitive(call.answer()?)?, true, None),
        LAST_INDEX_CONVERTED => (
            primitive(call.argument(0))?,
            false,
            Some(primitive(call.answer()?)?),
        ),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    Ok(match builtin_exec(call, regexp, string, ran, last_index)? {
        Exec::Want(answer) => answer,
        Exec::Done(value) => Answer::Value(value),
    })
}

/// `RegExp.prototype.test(string)`.
fn test(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let Value::Object(object) = call.this() else {
        return Err(Escape::type_error(
            "RegExp.prototype.test was called on something that is not an object",
            at,
        ));
    };
    match call.step() {
        0 => match Primitive::of(call.argument(0)) {
            Some(string) => read_exec(call, object, string, false),
            None => Ok(to_string_first(call)),
        },
        STRING_CONVERTED => {
            let string = primitive(call.answer()?)?;
            read_exec(call, object, string, true)
        }
        EXEC_READ => {
            let string = primitive(call.argument(0))?;
            let found = call.answer()?;
            call_exec(call, object, string, false, found)
        }
        EXEC_CALLED => match call.answer()? {
            Value::Object(_) => Ok(Answer::Value(Value::Bool(true))),
            Value::Null => Ok(Answer::Value(Value::Bool(false))),
            _ => Err(Escape::type_error(
                "a regular expression's exec answered something that is neither an object nor null",
                at,
            )),
        },
        LAST_INDEX_CONVERTED => {
            let string = primitive(call.argument(0))?;
            let last_index = primitive(call.answer()?)?;
            Ok(matched(builtin_exec(
                call,
                object,
                string,
                false,
                Some(last_index),
            )?))
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Ask for the string argument to be made a primitive, coming back at
/// [`STRING_CONVERTED`].
fn to_string_first(call: &Call<'_>) -> Answer {
    Answer::want(
        Want::Primitive {
            of: call.argument(0),
            hint: Hint::String,
        },
        STRING_CONVERTED,
    )
}

/// `RegExpExec`'s `Get(R, "exec")`.
fn read_exec(
    call: &mut Call<'_>,
    object: Ref,
    string: Primitive,
    ran: bool,
) -> Result<Answer, Escape> {
    // The name is on `%RegExp.prototype%`, so it is interned and reading it
    // allocates nothing; if nothing anywhere has it, nothing has an `exec`.
    let units: Vec<u16> = "exec".encode_utf16().collect();
    let found = match call.seen().existing_key(&units) {
        Some(key) => call.seen().get(object, key)?,
        None => Found::Missing,
    };
    let found = match found {
        Found::Value(value) => value,
        Found::Missing | Found::Getter(Value::Undefined) => Value::Undefined,
        Found::Getter(getter) => {
            if ran {
                return Err(Escape::NotBuiltYet(Missing::ATwoCallRegExpMethod));
            }
            return Ok(Answer::want(
                Want::Call {
                    callee: getter,
                    receiver: Value::Object(object),
                    arguments: Vec::new(),
                },
                EXEC_READ,
            ));
        }
    };
    call_exec(call, object, string, ran, found)
}

/// The rest of `RegExpExec`: call `exec` if it is callable, and otherwise
/// search with the builtin.
fn call_exec(
    call: &mut Call<'_>,
    object: Ref,
    string: Primitive,
    ran: bool,
    found: Value,
) -> Result<Answer, Escape> {
    let at = call.at();
    let callable = matches!(found, Value::Object(held) if call.seen().callable(held).is_some());
    if callable {
        // The string is made last, so nothing allocates between it and the
        // interpreter putting it on its stack.
        let text = convert::to_text(call.objects(), string, at)?;
        return Ok(Answer::want(
            Want::Call {
                callee: found,
                receiver: Value::Object(object),
                arguments: vec![text],
            },
            EXEC_CALLED,
        ));
    }
    if call.seen().as_regexp(object).is_none() {
        return Err(Escape::type_error(
            "RegExp.prototype.test was called on an object with no exec that is not a regular expression",
            at,
        ));
    }
    Ok(matched(builtin_exec(call, object, string, ran, None)?))
}

/// `test`'s answer from the builtin search's.
fn matched(exec: Exec) -> Answer {
    match exec {
        Exec::Want(answer) => answer,
        Exec::Done(value) => Answer::Value(Value::Bool(!matches!(value, Value::Null))),
    }
}

/// What the builtin search came to.
enum Exec {
    /// It needs `lastIndex` converted first.
    Want(Answer),
    /// The match array, or `null`.
    Done(Value),
}

/// `RegExpBuiltinExec(R, S)`.
///
/// `ran` says the string came from running script, which is the case a
/// second call cannot be asked for in. `last_index` is the converted
/// `lastIndex` once the step that converts it has run.
fn builtin_exec(
    call: &mut Call<'_>,
    regexp: Ref,
    string: Primitive,
    ran: bool,
    last_index: Option<Primitive>,
) -> Result<Exec, Escape> {
    let at = call.at();
    let units = convert::to_units(call.seen(), string, at)?;
    let key = last_index_key(call.seen())?;
    let last_index = if let Some(converted) = last_index {
        converted
    } else {
        let value = match call.seen().get(regexp, key)? {
            Found::Value(value) => value,
            // `lastIndex` is an own data property that cannot be configured,
            // so it is never missing and never an accessor.
            Found::Missing | Found::Getter(_) => return Err(Escape::fault(Fault::Gone)),
        };
        match Primitive::of(value) {
            Some(converted) => converted,
            None if ran => return Err(Escape::NotBuiltYet(Missing::ATwoCallRegExpMethod)),
            None => {
                return Ok(Exec::Want(Answer::want(
                    Want::Primitive {
                        of: value,
                        hint: Hint::Number,
                    },
                    LAST_INDEX_CONVERTED,
                )));
            }
        }
    };
    let last_index = to_length(convert::to_number(call.seen(), last_index, at)?);

    let program: Arc<Program> = call
        .seen()
        .as_regexp(regexp)
        .map(|held| Arc::clone(held.program()))
        .ok_or(Escape::fault(Fault::Gone))?;
    let moves = program.flags().has(Flag::Global) || program.flags().has(Flag::Sticky);
    let start = if moves {
        index_within(last_index, units.len())
    } else {
        0
    };
    let found = {
        let stop = || call.stop_asked();
        search(&program, &units, start, &stop).map_err(|halt| halted(halt, at))?
    };
    let Some(found) = found else {
        if moves {
            set_last_index(call, regexp, key, 0.0)?;
        }
        return Ok(Exec::Done(Value::Null));
    };
    let (index, end) = found
        .captures
        .first()
        .copied()
        .flatten()
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    if moves {
        set_last_index(call, regexp, key, as_number(end))?;
    }
    let scope = call.objects().heap_mut().open();
    let array = result(call, &program, string, &units, &found, index);
    call.objects().heap_mut().close(scope);
    Ok(Exec::Done(array?))
}

/// The key `lastIndex` is, which every `RegExp` object has, so it is interned
/// and finding it allocates nothing.
fn last_index_key(objects: &Objects) -> Result<Key, Escape> {
    let units: Vec<u16> = "lastIndex".encode_utf16().collect();
    objects
        .existing_key(&units)
        .ok_or(Escape::fault(Fault::Gone))
}

/// `Set(R, "lastIndex", value, true)`.
fn set_last_index(call: &mut Call<'_>, regexp: Ref, key: Key, value: f64) -> Result<(), Escape> {
    match call.objects().set(regexp, key, Value::Number(value))? {
        Set::Done => Ok(()),
        Set::Refused => Err(Escape::type_error(
            "a regular expression's lastIndex is not writable",
            call.at(),
        )),
        // An own data property that cannot be configured is never an
        // accessor.
        Set::Setter(_) => Err(Escape::fault(Fault::Gone)),
    }
}

/// The match array, with a scope open: `index`, `input`, the match, `groups`
/// and each capture, in the specification's order.
fn result(
    call: &mut Call<'_>,
    program: &Program,
    string: Primitive,
    units: &[u16],
    found: &Match,
    index: usize,
) -> Result<Value, Escape> {
    let at = call.at();
    let keys = ["index", "input", "groups"]
        .into_iter()
        .map(|name| held_key(call, name))
        .collect::<Result<Vec<Key>, Escape>>()?;
    let [index_key, input_key, groups_key] = keys.as_slice() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let input = convert::to_text(call.objects(), string, at)?;
    if let Some(held) = input.reference() {
        call.objects().heap_mut().hold(held);
    }
    let above = call.intrinsics()?.array_prototype(call.seen())?;
    let groups = program.groups();
    let array = call
        .objects()
        .array(Some(above), groups.saturating_add(1))
        .map_err(|why| Escape::refused(why, at))?;
    call.objects().heap_mut().hold(array);
    define(call, array, *index_key, Value::Number(as_number(index)))?;
    define(call, array, *input_key, input)?;

    let whole = capture(call, units, found.captures.first().copied().flatten())?;
    define(call, array, Key::index(0).ok_or(Fault::Gone)?, whole)?;

    let named = if program.has_names() {
        let object = call
            .objects()
            .object(None)
            .map_err(|why| Escape::refused(why, at))?;
        call.objects().heap_mut().hold(object);
        Some(object)
    } else {
        None
    };
    define(
        call,
        array,
        *groups_key,
        named.map_or(Value::Undefined, Value::Object),
    )?;

    let mut matched_names: Vec<&[u16]> = Vec::new();
    for number in 1..=groups {
        let pair = found
            .captures
            .get(usize::try_from(number).unwrap_or(usize::MAX))
            .copied()
            .flatten();
        let value = capture(call, units, pair)?;
        define(call, array, Key::index(number).ok_or(Fault::Gone)?, value)?;
        let (Some(object), Some(name)) = (named, program.name(number)) else {
            continue;
        };
        // Two groups may share a name when they are in different
        // alternatives; the one that took part is the one the name holds.
        if matched_names.contains(&name) {
            continue;
        }
        if !matches!(value, Value::Undefined) {
            matched_names.push(name);
        }
        let key = call
            .objects()
            .key(name)
            .map_err(|why| Escape::refused(why, at))?;
        define(call, object, key, value)?;
    }
    Ok(Value::Object(array))
}

/// A key the result names, interned and held.
fn held_key(call: &mut Call<'_>, name: &str) -> Result<Key, Escape> {
    let at = call.at();
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = call
        .objects()
        .key(&units)
        .map_err(|why| Escape::refused(why, at))?;
    if let Some(held) = key.reference() {
        call.objects().heap_mut().hold(held);
    }
    Ok(key)
}

/// `CreateDataPropertyOrThrow` on an object this method made, which cannot
/// refuse.
fn define(call: &mut Call<'_>, on: Ref, key: Key, value: Value) -> Result<(), Escape> {
    if call.objects().define(on, key, Property::plain(value))? {
        return Ok(());
    }
    Err(Escape::Broken(Internal::BuiltinIsWrong))
}

/// What one capture is: the text it spans, or `undefined`. The string is
/// made and handed straight to [`define`], so nothing allocates while only a
/// Rust local holds it.
fn capture(
    call: &mut Call<'_>,
    units: &[u16],
    pair: Option<(usize, usize)>,
) -> Result<Value, Escape> {
    let Some((start, end)) = pair else {
        return Ok(Value::Undefined);
    };
    let at = call.at();
    let piece = units
        .get(start..end)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let held = call
        .objects()
        .text(piece.to_vec())
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Value::Text(held))
}

/// What a search that did not finish is, to a page.
fn halted(halt: Halt, at: usize) -> Escape {
    match halt {
        Halt::Steps => Escape::range_error(
            format!(
                "this regular expression did more than {} steps of work in one match, \
                 which is as much as one match may do",
                bounds::STEPS_IN_A_MATCH
            ),
            at,
        ),
        Halt::Places => Escape::range_error(
            format!(
                "this regular expression held more than {} places to come back to in one match, \
                 which is as many as one match may hold",
                bounds::PLACES_IN_A_MATCH
            ),
            at,
        ),
        Halt::Stopped { .. } => Escape::Interrupted,
        Halt::Broken => Escape::Broken(Internal::JumpIsWrong),
    }
}

/// A primitive the interpreter answered, or this engine's own mistake.
fn primitive(value: Value) -> Result<Primitive, Escape> {
    Primitive::of(value).ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// `ToLength`: a whole number from zero to 2⁵³−1, with `NaN` as zero.
fn to_length(number: f64) -> f64 {
    if number.is_nan() || number <= 0.0 {
        return 0.0;
    }
    number.trunc().min(9_007_199_254_740_991.0)
}

/// A length as the index a search starts at: past the end of the string is
/// one past it, which no search finds anything at.
fn index_within(length: f64, units: usize) -> usize {
    let limit = u32::try_from(units).map_or(f64::MAX, f64::from);
    if length > limit {
        return units.saturating_add(1);
    }
    // Whole, not negative and no more than a string's length, so exact.
    usize::try_from(convert::to_uint32(length)).unwrap_or(units.saturating_add(1))
}

/// An index into a string, as the number a page reads. A string is at most
/// [`bounds::LONGEST_STRING`] code units, so every index is exact.
fn as_number(index: usize) -> f64 {
    u32::try_from(index).map_or(f64::MAX, f64::from)
}
