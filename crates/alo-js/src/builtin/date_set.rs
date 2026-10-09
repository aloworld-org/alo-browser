/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Changing a date (queue item 356): `Date.prototype`'s setters, local and
//! UTC, and `setTime`.
//!
//! Every setter is the same four steps in the specification, and so one body
//! here ([`set`]), told which field it sets:
//!
//! 1. `thisTimeValue(this)` — the `TypeError` for anything that is not a date
//!    — read **before** any argument is converted, and kept ([`KEPT_TIME`]),
//!    because an argument's `valueOf` may change the date and the
//!    specification works from the value it read first;
//! 2. each argument converted with `ToNumber`, once, in order, and kept
//!    ([`date_numbers`](super::date_numbers), ADR 0031) — the first even when
//!    it is absent, which makes it `NaN`;
//! 3. an Invalid Date answers `NaN` and is **not** written, so a date an
//!    argument's `valueOf` set stays as it set it — except `setFullYear`,
//!    which starts an Invalid Date from `+0`;
//! 4. the fields not given are read from the date, in local time or UTC, the
//!    new time value is made, clipped and written, and answered.
//!
//! Local time is UTC (ADR 0036 § 3), so each local setter and its `UTC`
//! namesake agree — but each is written as the specification writes it, with
//! `LocalTime` and `UTC`, so that a zone chosen in item 358 changes
//! [`time`](crate::time) and nothing here. Annex B's `setYear` is absent
//! (ADR 0036 § 4).

use crate::abrupt::{Escape, Internal};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Native};
use crate::object::{Objects, Value};
use crate::time;

use super::date::date_of;
use super::date_numbers::{Numbers, given_or, numbers};

/// `setMilliseconds(ms)`.
const MILLISECONDS: u8 = 0;
/// `setSeconds(sec[, ms])`.
const SECONDS: u8 = 1;
/// `setMinutes(min[, sec[, ms]])`.
const MINUTES: u8 = 2;
/// `setHours(hour[, min[, sec[, ms]]])`.
const HOURS: u8 = 3;
/// `setDate(date)`.
const DATE: u8 = 4;
/// `setMonth(month[, date])`.
const MONTH: u8 = 5;
/// `setFullYear(year[, month[, date]])`.
const FULL_YEAR: u8 = 6;
/// `setTime(time)`.
const TIME: u8 = 7;

/// The slot the time value read at the start is kept in.
const KEPT_TIME: usize = 0;
/// The first slot an argument's number is kept in.
const FIRST_NUMBER: usize = 1;

/// A setter that reads up to `most` arguments: the time value and that many
/// numbers kept.
const fn setter(name: &'static str, body: crate::object::native::Body, most: usize) -> Native {
    Native::new(name, body).keeping(FIRST_NUMBER + most)
}

/// Every setter.
const SETTERS: [Native; 15] = [
    setter("setMilliseconds", set::<MILLISECONDS, true>, 1),
    setter("setSeconds", set::<SECONDS, true>, 2),
    setter("setMinutes", set::<MINUTES, true>, 3),
    setter("setHours", set::<HOURS, true>, 4),
    setter("setDate", set::<DATE, true>, 1),
    setter("setMonth", set::<MONTH, true>, 2),
    setter("setFullYear", set::<FULL_YEAR, true>, 3),
    setter("setUTCMilliseconds", set::<MILLISECONDS, false>, 1),
    setter("setUTCSeconds", set::<SECONDS, false>, 2),
    setter("setUTCMinutes", set::<MINUTES, false>, 3),
    setter("setUTCHours", set::<HOURS, false>, 4),
    setter("setUTCDate", set::<DATE, false>, 1),
    setter("setUTCMonth", set::<MONTH, false>, 2),
    setter("setUTCFullYear", set::<FULL_YEAR, false>, 3),
    setter("setTime", set::<TIME, false>, 1),
];

/// Put the setters on `Date.prototype`.
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
    for native in SETTERS {
        super::native_method(objects, prototype, function_prototype, native)?;
    }
    Ok(())
}

/// How many arguments setter `which` reads.
const fn most(which: u8) -> usize {
    match which {
        SECONDS | MONTH => 2,
        MINUTES | FULL_YEAR => 3,
        HOURS => 4,
        _ => 1,
    }
}

/// The name setter `which` is called by, for a message.
const fn name(which: u8, local: bool) -> &'static str {
    match (which, local) {
        (MILLISECONDS, true) => "setMilliseconds",
        (SECONDS, true) => "setSeconds",
        (MINUTES, true) => "setMinutes",
        (HOURS, true) => "setHours",
        (DATE, true) => "setDate",
        (MONTH, true) => "setMonth",
        (FULL_YEAR, true) => "setFullYear",
        (MILLISECONDS, false) => "setUTCMilliseconds",
        (SECONDS, false) => "setUTCSeconds",
        (MINUTES, false) => "setUTCMinutes",
        (HOURS, false) => "setUTCHours",
        (DATE, false) => "setUTCDate",
        (MONTH, false) => "setUTCMonth",
        (FULL_YEAR, false) => "setUTCFullYear",
        _ => "setTime",
    }
}

/// A setter: field `WHICH`, in local time when `LOCAL` is true and in UTC
/// otherwise.
fn set<const WHICH: u8, const LOCAL: bool>(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let date = date_of(call, name(WHICH, LOCAL))?;
    if call.step() == 0 {
        let t = value_of(call, date)?;
        call.keep(KEPT_TIME, Value::Number(t))?;
    }
    let count = call.count().clamp(1, most(WHICH));
    if let Numbers::Asked(answer) = numbers(call, count, FIRST_NUMBER)? {
        return Ok(answer);
    }
    let t = call.kept_number(KEPT_TIME)?;
    let u = if WHICH == TIME {
        time::time_clip(call.kept_number(FIRST_NUMBER)?)
    } else {
        if t.is_nan() && WHICH != FULL_YEAR {
            return Ok(Answer::Value(Value::Number(f64::NAN)));
        }
        changed(call, WHICH, LOCAL, t, count)?
    };
    call.objects()
        .set_date(date, u)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Number(u)))
}

/// A date's time value, read through its cell.
fn value_of(call: &Call<'_>, date: Ref) -> Result<f64, Escape> {
    call.seen()
        .as_date(date)
        .map(crate::object::Date::value)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// The time value setter `which` makes of `t` and its kept arguments,
/// clipped.
fn changed(call: &Call<'_>, which: u8, local: bool, t: f64, count: usize) -> Result<f64, Escape> {
    let t = if t.is_nan() {
        // Only `setFullYear` gets here with an Invalid Date, and it starts
        // from the epoch.
        0.0
    } else if local {
        time::local_time(t)
    } else {
        t
    };
    let given =
        |which: usize, otherwise: f64| given_or(call, FIRST_NUMBER, count, which, otherwise);
    let made = match which {
        MILLISECONDS => time::make_date(
            time::day(t),
            time::make_time(
                time::hour_from_time(t),
                time::min_from_time(t),
                time::sec_from_time(t),
                given(0, f64::NAN)?,
            ),
        ),
        SECONDS => time::make_date(
            time::day(t),
            time::make_time(
                time::hour_from_time(t),
                time::min_from_time(t),
                given(0, f64::NAN)?,
                given(1, time::ms_from_time(t))?,
            ),
        ),
        MINUTES => time::make_date(
            time::day(t),
            time::make_time(
                time::hour_from_time(t),
                given(0, f64::NAN)?,
                given(1, time::sec_from_time(t))?,
                given(2, time::ms_from_time(t))?,
            ),
        ),
        HOURS => time::make_date(
            time::day(t),
            time::make_time(
                given(0, f64::NAN)?,
                given(1, time::min_from_time(t))?,
                given(2, time::sec_from_time(t))?,
                given(3, time::ms_from_time(t))?,
            ),
        ),
        DATE => time::make_date(
            time::make_day(
                time::year_from_time(t),
                time::month_from_time(t),
                given(0, f64::NAN)?,
            ),
            time::time_within_day(t),
        ),
        MONTH => time::make_date(
            time::make_day(
                time::year_from_time(t),
                given(0, f64::NAN)?,
                given(1, time::date_from_time(t))?,
            ),
            time::time_within_day(t),
        ),
        FULL_YEAR => time::make_date(
            time::make_day(
                given(0, f64::NAN)?,
                given(1, time::month_from_time(t))?,
                given(2, time::date_from_time(t))?,
            ),
            time::time_within_day(t),
        ),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    Ok(time::time_clip(if local { time::utc(made) } else { made }))
}
