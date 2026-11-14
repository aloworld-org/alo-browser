/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A moment a manifest names: `2026-11-28T09:00:00Z` (ADR 0116 § 3,
//! `published` and `expires`).
//!
//! # One spelling
//!
//! RFC 3339's form in UTC alone: four digits of year, two each of month,
//! day, hour, minute and second, the `T` and the `Z` — no fraction, no
//! offset, no lower-case letters, no leap second. A date that is not on
//! the calendar (`2026-02-29`) is not read. The year is from 1970: an
//! update signed before the epoch is no update. What is kept is the
//! seconds since the epoch, compared as numbers.

use core::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A moment, in whole seconds since 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(u64);

impl Instant {
    /// The moment this many seconds after the epoch.
    pub const fn at(seconds: u64) -> Self {
        Self(seconds)
    }

    /// The seconds since the epoch.
    pub const fn seconds(self) -> u64 {
        self.0
    }

    /// This clock's moment now; the epoch if the clock is set before it.
    pub fn now() -> Self {
        Self::of(SystemTime::now())
    }

    /// The moment `time` is, in whole seconds; the epoch if before it.
    pub fn of(time: SystemTime) -> Self {
        Self(
            time.duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        )
    }

    /// This moment as the system's time.
    pub fn as_system_time(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.0)
    }

    /// The moment `text` spells, if it spells one as this file reads them.
    pub fn read(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 20 {
            return None;
        }
        let at = |index: usize| bytes.get(index).copied();
        for (index, byte) in [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ] {
            if at(index) != Some(byte) {
                return None;
            }
        }
        let digits = |from: usize, to: usize| -> Option<u64> {
            let piece = text.get(from..to)?;
            if !piece.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            piece.parse().ok()
        };
        let year = digits(0, 4)?;
        let month = digits(5, 7)?;
        let day = digits(8, 10)?;
        let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
        if year < 1970 || !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        if day < 1 || day > days_in(year, month) {
            return None;
        }
        let days = days_since_epoch(year, month, day);
        Some(Self(days * 86_400 + hour * 3_600 + minute * 60 + second))
    }
}

/// How many days `month` of `year` has.
fn days_in(year: u64, month: u64) -> u64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if leap(year) => 29,
        2 => 28,
        _ => 31,
    }
}

/// Whether `year` has a 29 February.
fn leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// The days from 1970-01-01 to this date, a date on the calendar from 1970.
fn days_since_epoch(year: u64, month: u64, day: u64) -> u64 {
    let whole_years: u64 = (1970..year)
        .map(|each| if leap(each) { 366 } else { 365 })
        .sum();
    let whole_months: u64 = (1..month).map(|each| days_in(year, each)).sum();
    whole_years + whole_months + day - 1
}

impl fmt::Display for Instant {
    /// As a manifest writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut days = self.0 / 86_400;
        let rest = self.0 % 86_400;
        let mut year = 1970;
        loop {
            let length = if leap(year) { 366 } else { 365 };
            if days < length {
                break;
            }
            days -= length;
            year += 1;
        }
        let mut month = 1;
        while days >= days_in(year, month) {
            days -= days_in(year, month);
            month += 1;
        }
        write!(
            f,
            "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
            days + 1,
            rest / 3_600,
            rest % 3_600 / 60,
            rest % 60
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifests_moments_read_as_seconds_since_the_epoch() {
        assert_eq!(Instant::read("1970-01-01T00:00:00Z"), Some(Instant::at(0)));
        // `date -u -j -f %Y-%m-%dT%H:%M:%SZ 2026-11-14T09:00:00Z +%s`.
        assert_eq!(
            Instant::read("2026-11-14T09:00:00Z"),
            Some(Instant::at(1_794_646_800))
        );
        assert_eq!(
            Instant::read("2024-02-29T23:59:59Z"),
            Some(Instant::at(1_709_251_199))
        );
        assert_eq!(
            Instant::read("2000-03-01T00:00:00Z"),
            Some(Instant::at(951_868_800))
        );
    }

    #[test]
    fn written_back_as_read() {
        for text in [
            "1970-01-01T00:00:00Z",
            "2026-11-14T09:00:00Z",
            "2026-11-28T09:00:00Z",
            "2024-02-29T23:59:59Z",
            "2100-12-31T12:30:05Z",
        ] {
            assert_eq!(
                Instant::read(text).map(|at| at.to_string()).as_deref(),
                Some(text)
            );
        }
    }

    #[test]
    fn anything_but_the_one_spelling_is_not_read() {
        for odd in [
            "2026-11-14T09:00:00",
            "2026-11-14T09:00:00z",
            "2026-11-14t09:00:00Z",
            "2026-11-14 09:00:00Z",
            "2026-11-14T09:00:00.5Z",
            "2026-11-14T09:00:00+00:00",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-11-31T00:00:00Z",
            "2026-11-00T00:00:00Z",
            "2026-11-14T24:00:00Z",
            "2026-11-14T09:60:00Z",
            "2026-11-14T09:00:60Z",
            "1969-12-31T23:59:59Z",
            "+026-11-14T09:00:00Z",
            "2026-1-114T09:00:00Z",
            "",
        ] {
            assert_eq!(Instant::read(odd), None, "{odd:?}");
        }
    }
}
