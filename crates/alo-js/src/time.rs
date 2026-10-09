/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ECMA-262's arithmetic on time values (§ 21.4.1): from milliseconds since
//! the epoch to a year, a month and an hour, and back (queue item 356).
//!
//! `Date` looks like arithmetic on a number, and almost all of it is: the
//! specification writes every step, and this file is those steps, named as
//! the specification names them, on `f64`s. That last part is the point of
//! the file as much as the names are. ADR 0036 § 4 and ADR 0013 § 4: a year
//! of `+275760`, or a month of `1e300`, is a page's to choose, so nothing here
//! is an integer a hostile value can overflow. Every operation is IEEE 754's,
//! which is what ECMA-262 says it is, and anything not finite is `NaN` out.
//!
//! # A time value is in range or it is `NaN`
//!
//! An ECMAScript time value is an integral number of milliseconds within
//! ±8.64 × 10¹⁵ of the epoch — a hundred million days either side — or `NaN`,
//! an Invalid Date. [`time_clip`] is the one door into that range, and every
//! time value this engine stores has come through it.
//!
//! # Local time is UTC
//!
//! ADR 0036 § 3: a page's local time zone is UTC until the person chooses one
//! in the browser's settings (queue item 358), and the renderer never reads
//! the machine's. So [`local_time`] and [`utc`] are written as the
//! specification writes them, with `LocalTZA` zero ([`LOCAL_TZA`]), and when
//! a zone can be chosen it arrives in those two functions and nowhere else.

/// Milliseconds in a day: `msPerDay`.
pub const MS_PER_DAY: f64 = 86_400_000.0;
/// Milliseconds in an hour: `msPerHour`.
pub const MS_PER_HOUR: f64 = 3_600_000.0;
/// Milliseconds in a minute: `msPerMinute`.
pub const MS_PER_MINUTE: f64 = 60_000.0;
/// Milliseconds in a second: `msPerSecond`.
pub const MS_PER_SECOND: f64 = 1_000.0;

/// The farthest a time value may be from the epoch, either way: exactly
/// 100,000,000 days (§ 21.4.1.1).
pub const FARTHEST: f64 = 8.64e15;

/// `LocalTZA`, the local zone's offset from UTC in milliseconds: zero, because
/// a page's local zone is UTC until the person chooses one (ADR 0036 § 3,
/// queue item 358).
pub const LOCAL_TZA: f64 = 0.0;

/// The farthest year [`make_day`] counts days to.
///
/// `MakeDay` returns `NaN` where *finding a time value for the first of that
/// month is not possible*, and this is where it stops being possible in
/// `f64`: up to here, [`day_from_year`] is an integer below 2⁵³ and exact, so
/// a date argument that brings a far year back into range lands on the right
/// day. Past it, the count of days would be rounded, and an answer that might
/// be a day out is refused as `NaN` rather than given (ADR 0013 § 3). No date
/// that far out is in range by itself: a time value's range is under 300,000
/// years either side.
const FARTHEST_YEAR: f64 = 1e13;

/// A number's integral part, as `ToIntegerOrInfinity` makes it for a finite
/// number: towards zero, and never `-0`.
pub fn to_integer(number: f64) -> f64 {
    if number.is_nan() {
        return 0.0;
    }
    // Adding zero turns `-0` into `+0`, which the specification's
    // mathematical integer never is.
    number.trunc() + 0.0
}

/// `a modulo b` as the specification means it: the sign of `b`, so never
/// negative here, and never `-0`.
fn modulo(a: f64, b: f64) -> f64 {
    a.rem_euclid(b) + 0.0
}

/// `Day(t)`: which day since the epoch `t` is in.
pub fn day(t: f64) -> f64 {
    (t / MS_PER_DAY).floor()
}

/// `TimeWithinDay(t)`: how far into its day `t` is.
pub fn time_within_day(t: f64) -> f64 {
    modulo(t, MS_PER_DAY)
}

/// `DaysInYear(y)`: 366 in a leap year of the proleptic Gregorian calendar,
/// and 365 otherwise.
fn days_in_year(y: f64) -> f64 {
    if modulo(y, 4.0) != 0.0 {
        365.0
    } else if modulo(y, 100.0) != 0.0 {
        366.0
    } else if modulo(y, 400.0) != 0.0 {
        365.0
    } else {
        366.0
    }
}

/// `DayFromYear(y)`: the day the first of January of year `y` is.
pub fn day_from_year(y: f64) -> f64 {
    365.0 * (y - 1970.0) + ((y - 1969.0) / 4.0).floor() - ((y - 1901.0) / 100.0).floor()
        + ((y - 1601.0) / 400.0).floor()
}

/// `TimeFromYear(y)`: the time value at the start of year `y`.
fn time_from_year(y: f64) -> f64 {
    MS_PER_DAY * day_from_year(y)
}

/// `YearFromTime(t)`: the largest integral year whose start is not after
/// `t`.
///
/// Worked out from the mean length of a Gregorian year, which is within a
/// year of the answer for any time value, and then corrected — at most a few
/// steps, and the loop is bounded so that nothing a page passes can make it
/// spin. `NaN` for a `t` that is not finite.
pub fn year_from_time(t: f64) -> f64 {
    if !t.is_finite() {
        return f64::NAN;
    }
    let mut y = (t / (MS_PER_DAY * 365.2425)).floor() + 1970.0;
    for _ in 0..4 {
        if time_from_year(y) > t {
            y -= 1.0;
        } else if time_from_year(y + 1.0) <= t {
            y += 1.0;
        } else {
            break;
        }
    }
    y
}

/// `InLeapYear(t)`, as the extra day it gives February: one or zero.
fn leap_day(t: f64) -> f64 {
    days_in_year(year_from_time(t)) - 365.0
}

/// `DayWithinYear(t)`: which day of its year `t` is, from zero.
fn day_within_year(t: f64) -> f64 {
    day(t) - day_from_year(year_from_time(t))
}

/// Each month's number from zero, and the day of the year it starts on in a
/// year that is not a leap year; March onwards start a day later in one that
/// is.
const MONTHS: [(f64, f64); 12] = [
    (0.0, 0.0),
    (1.0, 31.0),
    (2.0, 59.0),
    (3.0, 90.0),
    (4.0, 120.0),
    (5.0, 151.0),
    (6.0, 181.0),
    (7.0, 212.0),
    (8.0, 243.0),
    (9.0, 273.0),
    (10.0, 304.0),
    (11.0, 334.0),
];

/// Each month's number and the day of the year it starts on, given the leap
/// day of its year.
fn months(leap: f64) -> impl Iterator<Item = (f64, f64)> {
    MONTHS
        .into_iter()
        .map(move |(number, start)| (number, if number >= 2.0 { start + leap } else { start }))
}

/// The month a day of the year (from zero) is in, and the day it starts on:
/// the last month that starts on or before it.
fn month_of(within: f64, leap: f64) -> (f64, f64) {
    months(leap)
        .take_while(|&(_, start)| start <= within)
        .last()
        .unwrap_or((f64::NAN, f64::NAN))
}

/// Which month (from zero) and which day of it (from one) `t` is in.
fn month_and_date(t: f64) -> (f64, f64) {
    if !t.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    let within = day_within_year(t);
    let (month, start) = month_of(within, leap_day(t));
    (month, within - start + 1.0)
}

/// `MonthFromTime(t)`: the month `t` is in, from zero for January.
pub fn month_from_time(t: f64) -> f64 {
    month_and_date(t).0
}

/// `DateFromTime(t)`: the day of its month `t` is, from one.
pub fn date_from_time(t: f64) -> f64 {
    month_and_date(t).1
}

/// `WeekDay(t)`: the day of the week `t` is, from zero for Sunday. The epoch
/// was a Thursday.
pub fn week_day(t: f64) -> f64 {
    modulo(day(t) + 4.0, 7.0)
}

/// `HourFromTime(t)`.
pub fn hour_from_time(t: f64) -> f64 {
    modulo((t / MS_PER_HOUR).floor(), 24.0)
}

/// `MinFromTime(t)`.
pub fn min_from_time(t: f64) -> f64 {
    modulo((t / MS_PER_MINUTE).floor(), 60.0)
}

/// `SecFromTime(t)`.
pub fn sec_from_time(t: f64) -> f64 {
    modulo((t / MS_PER_SECOND).floor(), 60.0)
}

/// `msFromTime(t)`.
pub fn ms_from_time(t: f64) -> f64 {
    modulo(t, MS_PER_SECOND)
}

/// `MakeTime(hour, min, sec, ms)`: a time within a day, which may run past
/// either end of it — `setHours(25)` is tomorrow — or `NaN` if any part is
/// not finite.
pub fn make_time(hour: f64, min: f64, sec: f64, ms: f64) -> f64 {
    if !(hour.is_finite() && min.is_finite() && sec.is_finite() && ms.is_finite()) {
        return f64::NAN;
    }
    to_integer(hour) * MS_PER_HOUR
        + to_integer(min) * MS_PER_MINUTE
        + to_integer(sec) * MS_PER_SECOND
        + to_integer(ms)
}

/// `MakeDay(year, month, date)`: the day that is `date` (from one) of month
/// `month` (from zero, and allowed to run past either end of the year) of
/// `year`, or `NaN`.
pub fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !(year.is_finite() && month.is_finite() && date.is_finite()) {
        return f64::NAN;
    }
    let y = to_integer(year);
    let m = to_integer(month);
    let dt = to_integer(date);
    let ym = y + (m / 12.0).floor();
    if !ym.is_finite() || ym.abs() > FARTHEST_YEAR {
        return f64::NAN;
    }
    // A whole number from 0 to 11, whose month is the last that does not
    // come after it.
    let mn = modulo(m, 12.0);
    let leap = days_in_year(ym) - 365.0;
    let (_, start) = months(leap)
        .take_while(|&(number, _)| number <= mn)
        .last()
        .unwrap_or((f64::NAN, f64::NAN));
    day_from_year(ym) + start + dt - 1.0
}

/// `MakeDate(day, time)`: the time value `time` into day `day`, or `NaN`.
pub fn make_date(day: f64, time: f64) -> f64 {
    if !(day.is_finite() && time.is_finite()) {
        return f64::NAN;
    }
    let tv = day * MS_PER_DAY + time;
    if tv.is_finite() { tv } else { f64::NAN }
}

/// `TimeClip(time)`: a time value, or `NaN` if `time` is not finite or is
/// farther than [`FARTHEST`] from the epoch.
pub fn time_clip(time: f64) -> f64 {
    if !time.is_finite() || time.abs() > FARTHEST {
        return f64::NAN;
    }
    to_integer(time)
}

/// `LocalTime(t)`: `t` in the page's local zone, which is UTC (ADR 0036 § 3).
pub fn local_time(t: f64) -> f64 {
    t + LOCAL_TZA
}

/// `UTC(t)`: a local time `t` as a time value in UTC, or `NaN` if it is not
/// finite.
pub fn utc(t: f64) -> f64 {
    if !t.is_finite() {
        return f64::NAN;
    }
    t - LOCAL_TZA
}

/// The year written as `toISOString` writes it: four digits from 0 to 9999,
/// and otherwise a sign and six.
fn iso_year(year: f64) -> String {
    if (0.0..=9999.0).contains(&year) {
        format!("{year:04}")
    } else if year < 0.0 {
        format!("-{:06}", -year)
    } else {
        format!("+{year:06}")
    }
}

/// The *Date Time String Format* in UTC, `YYYY-MM-DDTHH:mm:ss.sssZ`, as
/// `toISOString` writes it — or [`None`] for an Invalid Date, which
/// `toISOString` refuses with a `RangeError`.
pub fn iso_string(tv: f64) -> Option<String> {
    if !tv.is_finite() {
        return None;
    }
    Some(format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        iso_year(year_from_time(tv)),
        month_from_time(tv) + 1.0,
        date_from_time(tv),
        hour_from_time(tv),
        min_from_time(tv),
        sec_from_time(tv),
        ms_from_time(tv),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        FARTHEST, date_from_time, day, hour_from_time, iso_string, make_date, make_day, make_time,
        min_from_time, month_from_time, ms_from_time, sec_from_time, time_clip, time_within_day,
        to_integer, week_day, year_from_time,
    };

    /// Two numbers are the same number, `-0` and `NaN` included.
    fn same(left: f64, right: f64) -> bool {
        left.to_bits() == right.to_bits() || (left.is_nan() && right.is_nan())
    }

    /// The fields a time value is made of, in the order a person writes them.
    fn fields(t: f64) -> [f64; 7] {
        [
            year_from_time(t),
            month_from_time(t),
            date_from_time(t),
            hour_from_time(t),
            min_from_time(t),
            sec_from_time(t),
            ms_from_time(t),
        ]
    }

    #[test]
    fn the_epoch_is_the_first_of_january_1970_a_thursday() {
        assert_eq!(
            fields(0.0).map(f64::to_bits),
            [1970.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0].map(f64::to_bits)
        );
        assert!(same(week_day(0.0), 4.0));
        assert_eq!(iso_string(0.0).as_deref(), Some("1970-01-01T00:00:00.000Z"));
    }

    #[test]
    fn a_millisecond_before_the_epoch_is_the_last_of_1969() {
        assert_eq!(
            fields(-1.0).map(f64::to_bits),
            [1969.0, 11.0, 31.0, 23.0, 59.0, 59.0, 999.0].map(f64::to_bits)
        );
        assert!(same(week_day(-1.0), 3.0), "a Wednesday");
        assert!(same(day(-1.0), -1.0));
        assert!(same(time_within_day(-1.0), 86_399_999.0));
        assert_eq!(
            iso_string(-1.0).as_deref(),
            Some("1969-12-31T23:59:59.999Z")
        );
    }

    #[test]
    fn the_ends_of_the_range_are_the_specifications_worked_values() {
        // § 21.4.1.1: "the exact moment of midnight at the beginning of
        // 13 September 275760" and "20 April 271822 BCE".
        assert_eq!(
            fields(FARTHEST).map(f64::to_bits),
            [275_760.0, 8.0, 13.0, 0.0, 0.0, 0.0, 0.0].map(f64::to_bits)
        );
        assert!(same(week_day(FARTHEST), 6.0), "a Saturday");
        assert_eq!(
            iso_string(FARTHEST).as_deref(),
            Some("+275760-09-13T00:00:00.000Z")
        );
        assert_eq!(
            fields(-FARTHEST).map(f64::to_bits),
            [-271_821.0, 3.0, 20.0, 0.0, 0.0, 0.0, 0.0].map(f64::to_bits)
        );
        assert!(same(week_day(-FARTHEST), 2.0), "a Tuesday");
        assert_eq!(
            iso_string(-FARTHEST).as_deref(),
            Some("-271821-04-20T00:00:00.000Z")
        );
        // Made back from its fields, each end is itself.
        assert!(same(
            time_clip(make_date(make_day(275_760.0, 8.0, 13.0), 0.0)),
            FARTHEST
        ));
        assert!(same(
            time_clip(make_date(make_day(-271_821.0, 3.0, 20.0), 0.0)),
            -FARTHEST
        ));
    }

    #[test]
    fn one_past_either_end_is_an_invalid_date() {
        assert!(time_clip(FARTHEST + 1.0).is_nan());
        assert!(time_clip(-FARTHEST - 1.0).is_nan());
        assert!(time_clip(make_date(make_day(275_760.0, 8.0, 13.0), 1.0)).is_nan());
        assert!(time_clip(make_date(make_day(-271_821.0, 3.0, 19.0), MS_LAST)).is_nan());
        assert_eq!(iso_string(f64::NAN), None);
    }

    /// The last millisecond of a day.
    const MS_LAST: f64 = 86_399_999.0;

    #[test]
    fn time_clip_truncates_towards_zero_and_never_answers_minus_zero() {
        assert!(same(time_clip(1.9), 1.0));
        assert!(same(time_clip(-1.9), -1.0));
        assert!(same(time_clip(-0.0), 0.0));
        assert!(same(time_clip(-0.5), 0.0));
        assert!(time_clip(f64::INFINITY).is_nan());
        assert!(time_clip(f64::NAN).is_nan());
        assert!(same(to_integer(f64::NAN), 0.0));
    }

    #[test]
    fn a_leap_year_is_every_fourth_but_not_every_hundredth_but_every_four_hundredth() {
        let leap_day_2000 = make_date(make_day(2000.0, 1.0, 29.0), 0.0);
        assert!(same(leap_day_2000, 951_782_400_000.0));
        assert_eq!(
            fields(leap_day_2000).map(f64::to_bits),
            [2000.0, 1.0, 29.0, 0.0, 0.0, 0.0, 0.0].map(f64::to_bits)
        );
        // 1900 was not a leap year: the 29th of February is the 1st of March.
        assert!(same(
            make_day(1900.0, 1.0, 29.0),
            make_day(1900.0, 2.0, 1.0)
        ));
        assert!(same(
            make_day(2024.0, 2.0, 0.0),
            make_day(2024.0, 1.0, 29.0)
        ));
    }

    #[test]
    fn a_month_or_a_day_out_of_its_range_carries_into_the_next() {
        assert!(same(
            make_day(2026.0, 13.0, 1.0),
            make_day(2027.0, 1.0, 1.0)
        ));
        assert!(same(
            make_day(2026.0, -1.0, 1.0),
            make_day(2025.0, 11.0, 1.0)
        ));
        assert!(same(
            make_day(2026.0, 0.0, 0.0),
            make_day(2025.0, 11.0, 31.0)
        ));
        assert!(same(
            make_day(2026.0, 0.0, 366.0),
            make_day(2027.0, 0.0, 1.0)
        ));
        // An hour past a day is the next day.
        assert!(same(
            make_date(make_day(2026.0, 9.0, 9.0), make_time(25.0, 0.0, 0.0, 0.0)),
            make_date(make_day(2026.0, 9.0, 10.0), make_time(1.0, 0.0, 0.0, 0.0))
        ));
    }

    #[test]
    fn today_is_a_friday() {
        let today = make_date(make_day(2026.0, 9.0, 9.0), 0.0);
        assert!(same(today, 1_791_504_000_000.0));
        assert!(same(week_day(today), 5.0));
        assert_eq!(
            iso_string(today).as_deref(),
            Some("2026-10-09T00:00:00.000Z")
        );
    }

    #[test]
    fn a_year_outside_four_digits_is_written_with_a_sign_and_six() {
        let year_zero = make_date(make_day(0.0, 0.0, 1.0), 0.0);
        assert_eq!(
            iso_string(year_zero).as_deref(),
            Some("0000-01-01T00:00:00.000Z")
        );
        let before = make_date(make_day(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(
            iso_string(before).as_deref(),
            Some("-000001-01-01T00:00:00.000Z")
        );
        let after = make_date(make_day(10_000.0, 0.0, 1.0), 0.0);
        assert_eq!(
            iso_string(after).as_deref(),
            Some("+010000-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn hostile_numbers_are_nan_and_never_a_panic_or_an_overflow() {
        for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(make_time(hostile, 0.0, 0.0, 0.0).is_nan());
            assert!(make_day(2026.0, hostile, 1.0).is_nan());
            assert!(make_date(hostile, 0.0).is_nan());
            assert!(year_from_time(hostile).is_nan());
            assert!(month_from_time(hostile).is_nan());
            assert!(date_from_time(hostile).is_nan());
        }
        assert!(make_day(f64::MAX, f64::MAX, 1.0).is_nan());
        assert!(make_day(1e300, 0.0, 1.0).is_nan());
        assert!(make_day(0.0, 1e300, 1.0).is_nan());
        // A product past `f64`'s range is `NaN`, and one inside it is still
        // clipped.
        assert!(make_date(1e305, 0.0).is_nan());
        assert!(time_clip(make_date(1e300, 1e300)).is_nan());
        // A day far out but finite still answers, and a time far out is
        // clipped rather than wrapped.
        assert!(make_day(2026.0, 0.0, 1e300).is_finite());
        assert!(time_clip(make_date(make_day(2026.0, 0.0, 1e300), 0.0)).is_nan());
        // The year loop is bounded whatever it is handed.
        assert!(year_from_time(f64::MAX).is_finite());
        assert!(year_from_time(-f64::MAX).is_finite());
    }

    #[test]
    fn a_far_year_brought_back_by_its_date_lands_on_the_right_day() {
        // Up to the farthest year counted, the days are exact.
        let back = make_day(1e9, 0.0, 1.0 - super::day_from_year(1e9));
        assert!(same(back, 0.0));
        assert!(make_day(1e14, 0.0, 1.0).is_nan());
    }
}
