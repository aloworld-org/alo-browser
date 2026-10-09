/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Date` (queue item 356, ADR 0036): the constructor, `Date.now`, `Date.UTC`,
//! and what its prototype is made of.
//!
//! Opened by alo Sites' analytics script, which every page alo Sites publishes
//! carries and which stopped at its third line, `var since = Date.now();`.
//! Everything from a time value to a year is ECMA-262's arithmetic, in
//! [`time`](crate::time); the instant is the embedder's ([`Call::now`],
//! ADR 0036 § 1), and a realm given no clock answers `Date.now()`,
//! `new Date()` and `Date()` with the `TypeError` that says so, while
//! `new Date(0)`, `Date.UTC(…)` and every method on a date the page made
//! still work.
//!
//! What a date's prototype does is in three siblings: reading one
//! ([`date_prototype`](super::date_prototype)), changing one
//! ([`date_set`](super::date_set)), and turning one into a primitive or into
//! JSON ([`date_convert`](super::date_convert)). The arguments the
//! constructor, `Date.UTC` and the setters convert are converted once each,
//! in order, and kept ([`date_numbers`](super::date_numbers), ADR 0031).
//!
//! # A date as text is item 357's
//!
//! `new Date(string)`, `Date.parse`, `Date()` called as a function and the
//! `toString` family are refused by name ([`Missing::ADateAsText`]): ADR 0036
//! § 4 decides what they read and write, and item 357 builds them when a
//! frozen page needs them. `Date()` asks the clock first, as the
//! specification does, so a realm with none still answers it with the
//! `TypeError`.

use crate::abrupt::{Escape, Internal, Missing};
use crate::convert::{self, Hint, Primitive};
use crate::heap::{Ref, Root};
use crate::object::native::{Answer, Call, Instance, Native, Want};
use crate::object::{Key, Objects, Property, Value};
use crate::time;

use super::date_numbers::{MOST, Numbers, given_or, numbers};

/// The constructor, keeping up to seven converted arguments.
const DATE: Native = Native::constructor("Date", construct, Instance::Date).keeping(MOST);
/// `Date.now`.
const NOW: Native = Native::new("now", now);
/// `Date.UTC`, keeping up to seven converted arguments.
const UTC: Native = Native::new("UTC", utc).keeping(MOST);
/// `Date.parse`, which is item 357's.
const PARSE: Native = Native::new("parse", parse);

/// The constructor with one argument: come back here with what it converted
/// to, with no hint.
const ONE_CONVERTED: u32 = 1;

/// What a realm's `Date` is made of, each rooted.
#[derive(Debug)]
pub(super) struct Made {
    /// `Date`.
    pub(super) constructor: Root,
    /// `Date.prototype`.
    pub(super) prototype: Root,
}

/// Make `Date`, its prototype and their methods.
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
    to_primitive: Key,
) -> Result<Made, Escape> {
    let scope = objects.heap_mut().open();
    let outcome = made(objects, object_prototype, function_prototype, to_primitive);
    objects.heap_mut().close(scope);
    outcome
}

/// [`make`], with the scope already open.
fn made(
    objects: &mut Objects,
    object_prototype: Ref,
    function_prototype: Ref,
    to_primitive: Key,
) -> Result<Made, Escape> {
    // `Date.prototype` is an ordinary object, not a date, as the
    // specification has made it since ES2015.
    let prototype = objects
        .object(Some(object_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(prototype);
    let constructor = objects
        .native(DATE, Some(function_prototype))
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
    for native in [NOW, UTC, PARSE] {
        super::native_method(objects, constructor, function_prototype, native)?;
    }
    super::date_prototype::furnish(objects, prototype, function_prototype)?;
    super::date_set::furnish(objects, prototype, function_prototype)?;
    super::date_convert::furnish(objects, prototype, function_prototype, to_primitive)?;

    let heap = objects.heap_mut();
    Ok(Made {
        constructor: heap.root(constructor),
        prototype: heap.root(prototype),
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

/// `RequireInternalSlot(this, [[DateValue]])`: the date `this` is.
///
/// # Errors
///
/// The `TypeError` the specification gives for anything that is not a date,
/// naming the method a person called.
pub(super) fn date_of(call: &Call<'_>, method: &str) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) if call.seen().as_date(held).is_some() => Ok(held),
        _ => Err(Escape::type_error(
            format!("Date.prototype.{method} was called on something that is not a Date"),
            call.at(),
        )),
    }
}

/// `thisTimeValue(this)`: the time value of the date `this` is.
///
/// # Errors
///
/// The same as [`date_of`].
pub(super) fn time_value_of(call: &Call<'_>, method: &str) -> Result<f64, Escape> {
    let held = date_of(call, method)?;
    call.seen()
        .as_date(held)
        .map(crate::object::Date::value)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// `new Date(…)`, and `Date()`.
fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if !call.constructing() {
        // `Date()` reads the clock and answers it as text. The clock is read
        // first, so a realm with none is the `TypeError` it is everywhere.
        call.now()?;
        return Err(Escape::NotBuiltYet(Missing::ADateAsText));
    }
    let Value::Object(date) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    match call.count() {
        0 => {
            let now = call.now()?;
            finish(call, date, now)
        }
        1 => from_one(call, date),
        given => from_fields(call, date, given.min(MOST)),
    }
}

/// `new Date(value)`: another date's time value, or a number — a string is
/// item 357's.
fn from_one(call: &mut Call<'_>, date: Ref) -> Result<Answer, Escape> {
    let primitive = if call.step() == ONE_CONVERTED {
        Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?
    } else {
        let value = call.argument(0);
        if let Value::Object(held) = value
            && let Some(other) = call.seen().as_date(held)
        {
            let tv = other.value();
            return finish(call, date, tv);
        }
        match Primitive::of(value) {
            Some(primitive) => primitive,
            None => {
                return Ok(Answer::want(
                    Want::Primitive {
                        of: value,
                        hint: Hint::Default,
                    },
                    ONE_CONVERTED,
                ));
            }
        }
    };
    if matches!(primitive.value(), Value::Text(_)) {
        return Err(Escape::NotBuiltYet(Missing::ADateAsText));
    }
    let tv = convert::to_number(call.seen(), primitive, call.at())?;
    finish(call, date, tv)
}

/// `new Date(year, month[, date[, hours[, minutes[, seconds[, ms]]]]])`, in
/// local time.
fn from_fields(call: &mut Call<'_>, date: Ref, count: usize) -> Result<Answer, Escape> {
    if let Numbers::Asked(answer) = numbers(call, count, 0)? {
        return Ok(answer);
    }
    let made = from_kept(call, count)?;
    finish(call, date, time::utc(made))
}

/// `Date.UTC(year[, month[, date[, hours[, minutes[, seconds[, ms]]]]]])`.
fn utc(call: &mut Call<'_>) -> Result<Answer, Escape> {
    // The year is converted even when it is absent, which makes it `NaN`.
    let count = call.count().clamp(1, MOST);
    if let Numbers::Asked(answer) = numbers(call, count, 0)? {
        return Ok(answer);
    }
    let made = from_kept(call, count)?;
    Ok(Answer::Value(Value::Number(time::time_clip(made))))
}

/// The date the kept fields make, before `UTC` or `TimeClip`: a year from 0
/// to 99 is in the twentieth century, a month that is absent is January, a
/// day that is absent the first, and a time that is absent midnight.
fn from_kept(call: &Call<'_>, count: usize) -> Result<f64, Escape> {
    let year = call.kept_number(0)?;
    let month = given_or(call, 0, count, 1, 0.0)?;
    let day = given_or(call, 0, count, 2, 1.0)?;
    let hours = given_or(call, 0, count, 3, 0.0)?;
    let minutes = given_or(call, 0, count, 4, 0.0)?;
    let seconds = given_or(call, 0, count, 5, 0.0)?;
    let ms = given_or(call, 0, count, 6, 0.0)?;
    Ok(time::make_date(
        time::make_day(full_year(year), month, day),
        time::make_time(hours, minutes, seconds, ms),
    ))
}

/// A year as `new Date` and `Date.UTC` read it: 0 to 99 is 1900 to 1999.
fn full_year(year: f64) -> f64 {
    if year.is_nan() {
        return year;
    }
    let whole = time::to_integer(year);
    if (0.0..=99.0).contains(&whole) {
        1900.0 + whole
    } else {
        year
    }
}

/// Give the new date its time value, clipped, and answer it.
fn finish(call: &mut Call<'_>, date: Ref, tv: f64) -> Result<Answer, Escape> {
    call.objects()
        .set_date(date, tv)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(call.this()))
}

/// `Date.now()`: the realm's clock, or the `TypeError` a realm with none
/// answers.
fn now(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Number(call.now()?)))
}

/// `Date.parse(string)`, which is item 357's.
fn parse(_: &mut Call<'_>) -> Result<Answer, Escape> {
    Err(Escape::NotBuiltYet(Missing::ADateAsText))
}
