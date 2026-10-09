/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The time a page reads (ADR 0036 § 2): the machine's wall clock, in whole
//! milliseconds, floored.
//!
//! The engine has no clock (ADR 0013 § 5) and is handed one ([`alo_js::Clock`]);
//! this is the one every realm a renderer makes is handed. It is the one fact
//! about the outside the renderer reads without asking, because asking the
//! browser process for each reading would make `Date.now()` a round trip, and
//! it is in reach of the sandbox on every system we build for (ADR 0010).
//!
//! # A whole millisecond, and nothing finer
//!
//! ECMA-262's time values are integers of milliseconds, so the grain is not a
//! mitigation chosen here: it is the floor of what `Date` can express, and the
//! renderer does not hand the engine a fraction to round later. There is **no
//! jitter** — site isolation is the answer to a timer a Spectre gadget wants
//! (ADR 0005) — and **no promise of monotonicity**: `Date` is the wall clock,
//! and the wall clock moves when the machine's is set.
//!
//! # An instant the language cannot hold is `NaN`
//!
//! A machine whose clock reads before 1970 or past ECMA-262's 8.64 × 10¹⁵ ms
//! makes an Invalid Date, which a page can test for (ADR 0036 § 1), rather
//! than a panic or a wrong year. The arithmetic is on whole milliseconds as
//! integers until the answer is known to fit, so nothing here rounds or
//! overflows.
//!
//! # Not the zone
//!
//! This reads the instant and never the machine's time zone: a page's local
//! time is UTC until the person chooses a zone in settings (ADR 0036 § 3, queue
//! item 358), and that is the engine's arithmetic, not a reading.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The machine's wall clock, read in whole milliseconds since the epoch.
#[derive(Debug, Clone, Copy, Default)]
pub struct WallClock;

impl alo_js::Clock for WallClock {
    fn now(&self) -> f64 {
        time_value(SystemTime::now().duration_since(UNIX_EPOCH).ok())
    }
}

/// The farthest a time value may be from the epoch, in whole milliseconds:
/// ECMA-262's 8.64 × 10¹⁵ (§ 21.4.1.1).
const FARTHEST: u64 = 8_640_000_000_000_000;

/// How far past the epoch a reading is, as a time value: whole milliseconds,
/// floored, or `NaN` for a reading before the epoch ([`None`]) or past the
/// range a time value has.
fn time_value(since_epoch: Option<Duration>) -> f64 {
    let Some(since) = since_epoch else {
        return f64::NAN;
    };
    // `as_millis` floors: a reading of 1.999 ms is 1.
    let Ok(ms) = u64::try_from(since.as_millis()) else {
        return f64::NAN;
    };
    if ms > FARTHEST {
        return f64::NAN;
    }
    // Below 2⁵³, so exact as an `f64`; built from two halves because no
    // conversion from a `u64` to an `f64` is lossless in general.
    let high = u32::try_from(ms >> 32).unwrap_or(u32::MAX);
    let low = u32::try_from(ms & u64::from(u32::MAX)).unwrap_or(u32::MAX);
    f64::from(high) * 4_294_967_296.0 + f64::from(low)
}

#[cfg(test)]
mod tests {
    use super::{FARTHEST, WallClock, time_value};
    use alo_js::Clock;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// The machine's own reading, as a time value.
    fn machine() -> f64 {
        time_value(SystemTime::now().duration_since(UNIX_EPOCH).ok())
    }

    #[test]
    fn the_wall_clock_is_floored_and_within_the_machines_own_reading() {
        let before = machine();
        let read = WallClock.now();
        let after = machine();
        assert!(read.is_finite(), "this machine's clock is in range");
        assert_eq!(
            read.fract().to_bits(),
            0.0_f64.to_bits(),
            "a whole millisecond"
        );
        assert!(
            before <= read && read <= after,
            "{before} <= {read} <= {after}"
        );
    }

    #[test]
    fn a_reading_is_floored_to_a_whole_millisecond() {
        assert_eq!(
            time_value(Some(Duration::from_micros(1_999))).to_bits(),
            1.0_f64.to_bits()
        );
        assert_eq!(
            time_value(Some(Duration::from_nanos(999_999))).to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            time_value(Some(Duration::from_millis(1_791_551_655_678))).to_bits(),
            1_791_551_655_678.0_f64.to_bits()
        );
    }

    #[test]
    fn a_reading_the_language_cannot_hold_is_nan_and_never_a_panic() {
        assert!(time_value(None).is_nan(), "before 1970");
        assert_eq!(
            time_value(Some(Duration::from_millis(FARTHEST))).to_bits(),
            8.64e15_f64.to_bits(),
            "the last instant is in range"
        );
        assert!(time_value(Some(Duration::from_millis(FARTHEST + 1))).is_nan());
        assert!(time_value(Some(Duration::MAX)).is_nan());
    }
}
