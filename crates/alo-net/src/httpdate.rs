//! The three date formats HTTP has, and the one it is allowed to write.
//!
//! A cache is arithmetic on times, so a cache that cannot read a `Date` header
//! is a cache that treats every response as having no age at all — which is not
//! a small error in the direction of caution. It is how a response gets served
//! an hour after it expired.
//!
//! # Why three formats
//!
//! One is current: `Sun, 06 Nov 1994 08:49:37 GMT`, and it is the only one
//! anything may *send*. Two are obsolete and must still be read, because the
//! specification says a recipient must accept them and because servers written
//! in 1996 are still answering. Refusing them would mean treating a real
//! `Expires` as unparseable, and an unparseable `Expires` means *already
//! stale* — so being strict here makes a browser slower, never safer.
//!
//! # Why not a crate
//!
//! ADR 0001 rents the physics. A date is not physics; it is sixty lines of
//! integer arithmetic with no platform behaviour in it, and the calendar has
//! not changed since 1582.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Read an HTTP date, in any of the three formats.
///
/// Returns [`None`] for anything else, and the caller decides what that means.
/// For a cache the answer is always "treat it as expired" — a header nobody can
/// read must never be read as permission.
pub fn parse(text: &str) -> Option<SystemTime> {
    let text = text.trim();
    // `Sun, 06 Nov 1994 08:49:37 GMT` — the only one anything may send.
    if let Some(rest) = text.split_once(", ").map(|(_, rest)| rest) {
        if let Some(at) = imf_fixdate(rest).or_else(|| rfc850(rest)) {
            return Some(at);
        }
    }
    // `Sun Nov  6 08:49:37 1994` — C's `asctime`, no comma anywhere.
    asctime(text)
}

/// Write an HTTP date, in the one format anything may send.
///
/// Used for `If-Modified-Since`, which is a date this engine chose rather than
/// one it read — so it is written correctly rather than echoed.
pub fn format(at: SystemTime) -> String {
    let seconds = match at.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        // Before 1970. Nothing in HTTP is, but the type allows it.
        Err(_) => 0,
    };
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    // 1970-01-01 was a Thursday, which is day 4 of a week starting at Sunday.
    let weekday = (days + 4).rem_euclid(7);
    format!(
        "{}, {day:02} {} {year:04} {:02}:{:02}:{:02} GMT",
        DAY_NAMES
            .get(usize::try_from(weekday).unwrap_or(0))
            .unwrap_or(&"Thu"),
        MONTH_NAMES
            .get(usize::try_from(month).unwrap_or(1).saturating_sub(1))
            .unwrap_or(&"Jan"),
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `06 Nov 1994 08:49:37 GMT`
fn imf_fixdate(rest: &str) -> Option<SystemTime> {
    let mut parts = rest.split(' ').filter(|part| !part.is_empty());
    let day = parts.next()?.parse::<i64>().ok()?;
    let month = month_of(parts.next()?)?;
    let year = parts.next()?.parse::<i64>().ok()?;
    let time = parts.next()?;
    if parts.next()? != "GMT" {
        return None;
    }
    at(year, month, day, time)
}

/// `06-Nov-94 08:49:37 GMT` — obsolete, and the two-digit year is why.
fn rfc850(rest: &str) -> Option<SystemTime> {
    let mut parts = rest.split(' ').filter(|part| !part.is_empty());
    let date = parts.next()?;
    let time = parts.next()?;
    if parts.next()? != "GMT" {
        return None;
    }
    let mut fields = date.split('-');
    let day = fields.next()?.parse::<i64>().ok()?;
    let month = month_of(fields.next()?)?;
    let year = fields.next()?.parse::<i64>().ok()?;
    // A two-digit year more than fifty years ahead is a year in the past. This
    // is the specification's own rule, and it is the reason this format is
    // obsolete rather than merely old.
    let year = if year < 100 {
        if year < 70 { 2000 + year } else { 1900 + year }
    } else {
        year
    };
    at(year, month, day, time)
}

/// `Nov  6 08:49:37 1994` — C's `asctime`, where a single-digit day is padded
/// with a space rather than a zero.
fn asctime(text: &str) -> Option<SystemTime> {
    let mut parts = text.split(' ').filter(|part| !part.is_empty());
    let _weekday = parts.next()?;
    let month = month_of(parts.next()?)?;
    let day = parts.next()?.parse::<i64>().ok()?;
    let time = parts.next()?;
    let year = parts.next()?.parse::<i64>().ok()?;
    at(year, month, day, time)
}

fn month_of(name: &str) -> Option<i64> {
    MONTH_NAMES
        .iter()
        .position(|known| known.eq_ignore_ascii_case(name))
        .and_then(|index| i64::try_from(index + 1).ok())
}

/// A calendar date and a `hh:mm:ss`, as a moment.
fn at(year: i64, month: i64, day: i64, time: &str) -> Option<SystemTime> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut clock = time.split(':');
    let hour = clock.next()?.parse::<i64>().ok()?;
    let minute = clock.next()?.parse::<i64>().ok()?;
    // A leap second is 60, and HTTP dates carrying one exist.
    let second = clock.next()?.parse::<i64>().ok()?;
    if clock.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let seconds = days_from_civil(year, month, day)
        .checked_mul(86_400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    if seconds < 0 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_secs(u64::try_from(seconds).ok()?))
}

/// Days since 1970-01-01, from a civil date.
///
/// Howard Hinnant's algorithm, which is exact for every year the proleptic
/// Gregorian calendar has and needs no table of leap years. The shift by March
/// is what makes the leap day fall at the *end* of the era, where it stops
/// being a special case.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse, for writing one out.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification's own example, in all three of its spellings. They are
    /// the same moment, and a cache that read them differently would expire
    /// three identical responses at three different times.
    #[test]
    fn the_three_formats_are_the_same_moment() {
        let imf = parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("the current format");
        let obsolete = parse("Sunday, 06-Nov-94 08:49:37 GMT").expect("RFC 850");
        let ancient = parse("Sun Nov  6 08:49:37 1994").expect("asctime");
        assert_eq!(imf, obsolete);
        assert_eq!(imf, ancient);
        assert_eq!(
            imf.duration_since(UNIX_EPOCH)
                .expect("after 1970")
                .as_secs(),
            784_111_777
        );
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        for seconds in [
            0u64,
            1,
            784_111_777,
            951_782_400,
            1_800_000_000,
            4_102_444_800,
        ] {
            let at = UNIX_EPOCH + Duration::from_secs(seconds);
            let written = format(at);
            assert_eq!(parse(&written), Some(at), "{written} did not round-trip");
        }
    }

    #[test]
    fn the_epoch_is_a_thursday() {
        assert_eq!(format(UNIX_EPOCH), "Thu, 01 Jan 1970 00:00:00 GMT");
    }

    /// 2000 is a leap year and 1900 was not, which is the case a table of
    /// leap years gets wrong.
    #[test]
    fn the_leap_day_of_2000_exists() {
        let leap = parse("Tue, 29 Feb 2000 12:00:00 GMT").expect("2000 was a leap year");
        assert_eq!(format(leap), "Tue, 29 Feb 2000 12:00:00 GMT");
    }

    /// Being strict here would make a browser slower rather than safer, so
    /// nonsense is `None` and the caller decides — for a cache, "expired".
    #[test]
    fn nonsense_is_none_rather_than_a_guess() {
        for text in [
            "",
            "tomorrow",
            "Sun, 06 Nov 1994 08:49:37",     // no zone
            "Sun, 06 Nov 1994 08:49:37 PST", // a zone that is not GMT
            "Sun, 06 Xxx 1994 08:49:37 GMT", // not a month
            "Sun, 06 Nov 1994 25:00:00 GMT", // not an hour
            "Sun, 32 Nov 1994 08:49:37 GMT", // not a day
            "Sun, 06 Nov 1994 08:49:37:00 GMT",
        ] {
            assert_eq!(parse(text), None, "{text:?} parsed as a date");
        }
    }

    /// A leap second is a real value in a real header.
    #[test]
    fn a_leap_second_is_read_rather_than_refused() {
        assert!(parse("Sat, 31 Dec 2016 23:59:60 GMT").is_some());
    }
}
