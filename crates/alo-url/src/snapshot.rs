//! How old the public suffix list compiled into this browser is.
//!
//! [`crate::site`] rents Mozilla's list from `psl`, which compiles a
//! **snapshot** of it in rather than fetching one. That is the right trade and
//! that file argues it: a security boundary arriving over the network would
//! exist only when the network did, and one that changed under a running
//! program would move where a cookie lives without anybody deciding.
//!
//! # What a snapshot costs, which is the whole reason this file exists
//!
//! A snapshot ages. Registries delegate new suffixes continually, and a suffix
//! delegated after ours was taken is read here as an ordinary registrable
//! domain — so every name under it is **one site**: one cookie jar (ADR 0007),
//! one cache partition (ADR 0011), one renderer process (ADR 0005). That is two
//! organisations sharing what one is given, which is the direction that costs
//! rather than the direction that annoys.
//!
//! Updating it is a version bump in the workspace `Cargo.toml`, which is a diff
//! somebody reads — but only if somebody is prompted to write it, and nothing
//! prompted anybody. An out-of-date boundary was a silence. It is a message
//! now: the test at the bottom of this file **fails** once the snapshot passes
//! [`STALE_AFTER_DAYS`], and says what to do about it.
//!
//! # Why the age is measured from the day we took it
//!
//! The list itself carries no date and `psl` publishes none, so the honest
//! recorded fact is the day the snapshot entered *this* repository — [`TAKEN`],
//! beside the [`VERSION`] it was taken at, which a test checks against
//! `Cargo.lock` so the two cannot drift apart.
//!
//! The list is therefore **at least** as old as this file says and possibly
//! older, because a crate version may have been published some weeks before we
//! took it. The error is in the direction of under-reporting age, which is why
//! the threshold is six months rather than the twelve somebody might argue for:
//! a few weeks of unknown slack has to fit inside it.
//!
//! Nothing here reads the clock except [`age_now`], for the reason the HTTP
//! cache gives: an answer that depends on the moment is asserted honestly only
//! when the moment is named.

use std::time::{SystemTime, UNIX_EPOCH};

/// The `psl` version whose snapshot of the list is compiled in.
///
/// Checked against `Cargo.lock` by a test in this file, because a record that
/// may quietly disagree with the code is a record nobody can act on.
pub const VERSION: &str = "2.1.223";

/// The day that snapshot was taken into this repository.
///
/// Set it, and [`VERSION`] with it, in the same change that bumps `psl`.
pub const TAKEN: Day = Day {
    year: 2026,
    month: 9,
    day: 3,
};

/// How old the snapshot may be before it is a message rather than a fact.
///
/// **Six months**, counted in days because the list does not care which months
/// they were. Long enough that bumping `psl` is a chore somebody does a couple
/// of times a year rather than a treadmill; short enough that a suffix
/// delegated today is honoured within a release or two of it happening.
pub const STALE_AFTER_DAYS: i64 = 183;

/// A day, as the record above writes one.
///
/// A calendar date rather than a moment: the snapshot was taken on a day, and
/// an hour of it would be a precision this cannot support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day {
    /// The year, as four digits.
    pub year: i64,
    /// The month, `1` to `12`.
    pub month: i64,
    /// The day of the month, `1` to `31`.
    pub day: i64,
}

impl std::fmt::Display for Day {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { year, month, day } = *self;
        write!(f, "{year:04}-{month:02}-{day:02}")
    }
}

impl Day {
    /// Days since 1970-01-01, negative before it.
    ///
    /// Howard Hinnant's `days_from_civil`, which is the same eight lines
    /// `alo-net` uses to read an HTTP date. They are not shared, and that is a
    /// decision rather than an oversight: `alo-net` depends on this crate and
    /// not the other way round, so sharing them would mean inverting a
    /// dependency for some arithmetic about the Gregorian calendar — and
    /// renting a calendar crate to hold eight lines is not a boundary worth
    /// ADR 0001's paperwork.
    #[must_use]
    pub const fn in_epoch_days(self) -> i64 {
        let Self { year, month, day } = self;
        let year = if month <= 2 { year - 1 } else { year };
        let era = (if year >= 0 { year } else { year - 399 }) / 400;
        let year_of_era = year - era * 400;
        let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}

/// How old the snapshot is on a named day, in days.
///
/// Never negative. A machine whose clock is set before [`TAKEN`] cannot tell us
/// the list is old — it can only tell us about itself — so the answer there is
/// zero rather than a number below it.
#[must_use]
pub fn age_on(today: Day) -> i64 {
    (today.in_epoch_days() - TAKEN.in_epoch_days()).max(0)
}

/// How old the snapshot is now, or [`None`] if this machine's clock is before
/// 1970 and so cannot date anything.
#[must_use]
pub fn age_now() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = i64::try_from(seconds / 86_400).ok()?;
    Some((days - TAKEN.in_epoch_days()).max(0))
}

/// Whether a snapshot of this age is one nobody should still be deciding a site
/// boundary with.
#[must_use]
pub fn is_stale(age_in_days: i64) -> bool {
    age_in_days > STALE_AFTER_DAYS
}

/// What a person is told when it is: the age, the version, what it costs, and
/// the two lines of work that discharge it.
///
/// Written out even when nothing is stale, so that a caller which wants to
/// *show* the snapshot's age — a security surface, say — has the sentence
/// rather than the numbers.
#[must_use]
pub fn how_it_reads(age_in_days: i64) -> String {
    let state = if is_stale(age_in_days) {
        format!("older than the {STALE_AFTER_DAYS} days (six months) this browser allows it to be")
    } else {
        format!("within the {STALE_AFTER_DAYS} days (six months) this browser allows it")
    };
    format!(
        "The public suffix list compiled in is psl {VERSION}, taken on {TAKEN}, \
         and is {age_in_days} days old — {state}.\n\
         A suffix delegated since then is read as an ordinary registrable domain, \
         which puts two organisations into one site: one cookie jar, one cache \
         partition, one renderer process.\n\
         To bring it up to date: bump `psl` in the workspace `Cargo.toml`, \
         `cargo update -p psl`, and set VERSION and TAKEN in \
         `crates/alo-url/src/snapshot.rs` to what came out and the day it did."
    )
}

#[cfg(test)]
mod tests {
    use super::{Day, STALE_AFTER_DAYS, TAKEN, VERSION, age_now, age_on, how_it_reads, is_stale};

    /// The item's closing condition: an out-of-date boundary is a message
    /// rather than a silence.
    ///
    /// **This test failing is not a fault in whatever change is being tested.**
    /// It is the message, and `how_it_reads` says what discharges it.
    #[test]
    fn the_list_this_browser_decides_sites_with_is_not_six_months_old() {
        // A clock before 1970 dates nothing, and failing the build on it would
        // be a message about the machine rather than about the list.
        if let Some(age) = age_now() {
            assert!(!is_stale(age), "{}", how_it_reads(age));
        }
    }

    /// The record and the code cannot drift apart: bumping `psl` without
    /// re-dating the snapshot fails here, which is the only thing that makes
    /// [`TAKEN`] worth measuring from.
    #[test]
    fn the_version_recorded_here_is_the_one_actually_compiled_in() {
        let lock = include_str!("../../../Cargo.lock");
        let resolved = lock
            .split("[[package]]")
            .find(|package| package.lines().any(|line| line.trim() == r#"name = "psl""#))
            .and_then(|package| {
                package
                    .lines()
                    .map(str::trim)
                    .find_map(|line| line.strip_prefix("version = "))
            })
            .map(|version| version.trim_matches('"'));
        assert_eq!(
            resolved,
            Some(VERSION),
            "Cargo.lock resolves psl to a different version than \
             crates/alo-url/src/snapshot.rs records. Set VERSION and TAKEN to \
             the version that is now compiled in and the day it was taken."
        );
    }

    fn day(year: i64, month: i64, day: i64) -> Day {
        Day { year, month, day }
    }

    /// A day some number of days from [`TAKEN`], counted from the constant
    /// rather than written out as a date somebody read off it — because the
    /// whole point of that constant is that it moves, and a test needing to be
    /// re-dated with it is friction on the one chore this file exists to ask
    /// for.
    ///
    /// It counts by pushing the day of the month past the end of its month,
    /// which is arithmetic rather than a date: `in_epoch_days` is affine in
    /// that field — every extra day is exactly one more day since the epoch,
    /// whatever the month's length — so `2026-09-186` is a real answer and it
    /// is the answer for 183 days after the third of September.
    fn days_after_it_was_taken(days: i64) -> Day {
        Day {
            day: TAKEN.day + days,
            ..TAKEN
        }
    }

    /// The pair either side of the threshold, which is the only place the
    /// answer changes and therefore the only place worth asserting.
    #[test]
    fn six_months_and_a_day_is_stale_and_six_months_is_not() {
        assert_eq!(
            age_on(days_after_it_was_taken(STALE_AFTER_DAYS)),
            STALE_AFTER_DAYS
        );
        assert!(!is_stale(age_on(days_after_it_was_taken(STALE_AFTER_DAYS))));
        assert!(is_stale(age_on(days_after_it_was_taken(
            STALE_AFTER_DAYS + 1
        ))));
    }

    #[test]
    fn the_day_it_was_taken_is_no_age_at_all() {
        assert_eq!(age_on(TAKEN), 0);
        assert!(!is_stale(age_on(TAKEN)));
    }

    /// A machine whose clock is wrong in the past must not report the list as
    /// fresh *or* as ancient — it must report that it cannot say, which here is
    /// an age of zero.
    #[test]
    fn a_clock_set_before_the_snapshot_is_not_an_age_below_zero() {
        assert_eq!(age_on(days_after_it_was_taken(-1)), 0);
        assert_eq!(age_on(days_after_it_was_taken(-400)), 0);
        assert_eq!(age_on(day(1970, 1, 1)), 0);
        assert_eq!(age_on(day(1969, 12, 31)), 0);
    }

    /// The arithmetic is a calendar's, so the two places a calendar is
    /// irregular are named: a leap day, and a year boundary.
    #[test]
    fn the_calendar_counts_leap_days_and_year_ends() {
        assert_eq!(
            day(2026, 9, 3).in_epoch_days() + 1,
            day(2026, 9, 4).in_epoch_days()
        );
        assert_eq!(
            day(2026, 12, 31).in_epoch_days() + 1,
            day(2027, 1, 1).in_epoch_days()
        );
        // 2028 is a leap year, so February has an extra day and 2024-02-28 is
        // two days from March.
        assert_eq!(
            day(2028, 2, 28).in_epoch_days() + 2,
            day(2028, 3, 1).in_epoch_days()
        );
        assert_eq!(
            day(2027, 2, 28).in_epoch_days() + 1,
            day(2027, 3, 1).in_epoch_days()
        );
        // A century that is not a leap year, and one that is.
        assert_eq!(
            day(2100, 2, 28).in_epoch_days() + 1,
            day(2100, 3, 1).in_epoch_days()
        );
        assert_eq!(
            day(2000, 2, 28).in_epoch_days() + 2,
            day(2000, 3, 1).in_epoch_days()
        );
        assert_eq!(day(1970, 1, 1).in_epoch_days(), 0);
    }

    /// The property `days_after_it_was_taken` counts with, asserted rather than
    /// claimed: a day of the month past the end of its month is a day count,
    /// and it lands where a calendar says it does.
    #[test]
    fn a_day_of_the_month_past_the_end_of_it_is_the_day_it_counts_to() {
        assert_eq!(
            day(2026, 9, 3 + 183).in_epoch_days(),
            day(2027, 3, 5).in_epoch_days(),
            "183 days after the third of September 2026 is the fifth of March"
        );
        for count in [-400, -1, 0, 1, 183, 184, 4000] {
            assert_eq!(
                day(2026, 9, 3 + count).in_epoch_days() - day(2026, 9, 3).in_epoch_days(),
                count
            );
        }
    }

    /// The message is what somebody acts on, so it says the four things they
    /// need: which list, how old, what it costs, and what to do.
    #[test]
    fn the_message_names_the_version_the_day_and_the_work() {
        let stale = how_it_reads(STALE_AFTER_DAYS + 1);
        assert!(stale.contains(VERSION), "{stale}");
        assert!(stale.contains(&TAKEN.to_string()), "{stale}");
        assert!(stale.contains("older than"), "{stale}");
        assert!(stale.contains("cargo update -p psl"), "{stale}");
        assert!(how_it_reads(0).contains("within"));
    }
}
