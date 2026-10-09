/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Reading a date (queue item 356): `Date.prototype`'s getters, `getTime`,
//! `valueOf`, `getTimezoneOffset` and `toISOString`.
//!
//! Each begins with `thisTimeValue(this)` — the `TypeError` for anything that
//! is not a date — answers `NaN` for an Invalid Date, and otherwise reads one
//! field of the time value with ECMA-262's arithmetic
//! ([`time`](crate::time)). A getter without `UTC` in its name reads **local
//! time**, which is UTC (ADR 0036 § 3), so `getHours()` and `getUTCHours()`
//! agree and `getTimezoneOffset()` is `0` until the person chooses a zone
//! (queue item 358).
//!
//! # The text forms are item 357's
//!
//! `toISOString` is here because it is arithmetic and nothing else: the one
//! form whose zone is always UTC and whose spelling has no names in it.
//! `toString`, `toDateString`, `toTimeString` and `toUTCString` are refused
//! by name ([`Missing::ADateAsText`]) after the same `TypeError` for a
//! `this` that is not a date, because that error is the language's. Annex B's
//! `getYear` and `toGMTString` are absent (ADR 0036 § 4), and so are
//! `toLocaleDateString` and `toLocaleTimeString`, which wait for `Intl`.

use crate::abrupt::{Escape, Missing};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native};
use crate::object::{Objects, Value};
use crate::time;

use super::date::time_value_of;

/// The year of the calendar.
const YEAR: u8 = 0;
/// The month, from zero for January.
const MONTH: u8 = 1;
/// The day of the month, from one.
const DATE: u8 = 2;
/// The day of the week, from zero for Sunday.
const DAY: u8 = 3;
/// The hour, from 0 to 23.
const HOURS: u8 = 4;
/// The minute, from 0 to 59.
const MINUTES: u8 = 5;
/// The second, from 0 to 59.
const SECONDS: u8 = 6;
/// The millisecond, from 0 to 999.
const MILLISECONDS: u8 = 7;

/// Every getter: its name, and its body, which is one field in local time or
/// in UTC.
const GETTERS: [Native; 16] = [
    Native::new("getFullYear", get::<YEAR, true>),
    Native::new("getMonth", get::<MONTH, true>),
    Native::new("getDate", get::<DATE, true>),
    Native::new("getDay", get::<DAY, true>),
    Native::new("getHours", get::<HOURS, true>),
    Native::new("getMinutes", get::<MINUTES, true>),
    Native::new("getSeconds", get::<SECONDS, true>),
    Native::new("getMilliseconds", get::<MILLISECONDS, true>),
    Native::new("getUTCFullYear", get::<YEAR, false>),
    Native::new("getUTCMonth", get::<MONTH, false>),
    Native::new("getUTCDate", get::<DATE, false>),
    Native::new("getUTCDay", get::<DAY, false>),
    Native::new("getUTCHours", get::<HOURS, false>),
    Native::new("getUTCMinutes", get::<MINUTES, false>),
    Native::new("getUTCSeconds", get::<SECONDS, false>),
    Native::new("getUTCMilliseconds", get::<MILLISECONDS, false>),
];

/// The rest of what reads a date.
const OTHERS: [Native; 8] = [
    Native::new("getTime", get_time),
    Native::new("valueOf", value_of),
    Native::new("getTimezoneOffset", get_timezone_offset),
    Native::new("toISOString", to_iso_string),
    Native::new("toString", as_text),
    Native::new("toDateString", as_text),
    Native::new("toTimeString", as_text),
    Native::new("toUTCString", as_text),
];

/// Put the methods that read a date on `Date.prototype`.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference
/// this engine has lost.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    for native in GETTERS.into_iter().chain(OTHERS) {
        super::native_method(objects, prototype, function_prototype, native)?;
    }
    Ok(())
}

/// One field of a time value.
fn field(which: u8, t: f64) -> f64 {
    match which {
        YEAR => time::year_from_time(t),
        MONTH => time::month_from_time(t),
        DATE => time::date_from_time(t),
        DAY => time::week_day(t),
        HOURS => time::hour_from_time(t),
        MINUTES => time::min_from_time(t),
        SECONDS => time::sec_from_time(t),
        MILLISECONDS => time::ms_from_time(t),
        _ => f64::NAN,
    }
}

/// The name a getter is called by, for a message.
const fn name(which: u8, local: bool) -> &'static str {
    match (which, local) {
        (YEAR, true) => "getFullYear",
        (MONTH, true) => "getMonth",
        (DATE, true) => "getDate",
        (DAY, true) => "getDay",
        (HOURS, true) => "getHours",
        (MINUTES, true) => "getMinutes",
        (SECONDS, true) => "getSeconds",
        (MILLISECONDS, true) => "getMilliseconds",
        (YEAR, false) => "getUTCFullYear",
        (MONTH, false) => "getUTCMonth",
        (DATE, false) => "getUTCDate",
        (DAY, false) => "getUTCDay",
        (HOURS, false) => "getUTCHours",
        (MINUTES, false) => "getUTCMinutes",
        (SECONDS, false) => "getUTCSeconds",
        _ => "getUTCMilliseconds",
    }
}

/// A getter: field `FIELD` of `this`'s time value, in local time when
/// `LOCAL` is true and in UTC otherwise, or `NaN` for an Invalid Date.
fn get<const FIELD: u8, const LOCAL: bool>(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let t = time_value_of(call, name(FIELD, LOCAL))?;
    if t.is_nan() {
        return Ok(Answer::Value(Value::Number(f64::NAN)));
    }
    let t = if LOCAL { time::local_time(t) } else { t };
    Ok(Answer::Value(Value::Number(field(FIELD, t))))
}

/// `getTime`: the time value itself.
fn get_time(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let t = time_value_of(call, "getTime")?;
    Ok(Answer::Value(Value::Number(t)))
}

/// `valueOf`: the time value itself, as `getTime` answers it, under its own
/// name in a message.
fn value_of(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let t = time_value_of(call, "valueOf")?;
    Ok(Answer::Value(Value::Number(t)))
}

/// `getTimezoneOffset`: how many minutes local time is behind UTC, which is
/// zero while a page's zone is UTC (ADR 0036 § 3).
fn get_timezone_offset(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let t = time_value_of(call, "getTimezoneOffset")?;
    if t.is_nan() {
        return Ok(Answer::Value(Value::Number(f64::NAN)));
    }
    let offset = (t - time::local_time(t)) / time::MS_PER_MINUTE;
    Ok(Answer::Value(Value::Number(offset)))
}

/// `toISOString`: `YYYY-MM-DDTHH:mm:ss.sssZ`, always in UTC, or the
/// `RangeError` the specification gives for an Invalid Date.
fn to_iso_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let t = time_value_of(call, "toISOString")?;
    let Some(text) = time::iso_string(t) else {
        return Err(Escape::range_error(
            "an Invalid Date has no ISO string",
            call.at(),
        ));
    };
    let at = call.at();
    let held = call
        .objects()
        .text(text.encode_utf16().collect())
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(held)))
}

/// `toString`, `toDateString`, `toTimeString` and `toUTCString`: the
/// `TypeError` for a `this` that is not a date, and otherwise item 357's.
fn as_text(call: &mut Call<'_>) -> Result<Answer, Escape> {
    time_value_of(call, "toString")?;
    Err(Escape::NotBuiltYet(Missing::ADateAsText))
}
