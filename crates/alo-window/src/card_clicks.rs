/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How many presses of the main button came in quick succession at one
//! place of a card's page (queue item 740): what Chrome's `clickCount` is,
//! so a double click selects a word and a triple a paragraph.
//!
//! In Chrome's own window the window server counts them (`NSEvent`'s
//! `clickCount` on macOS); the conversation's window is told only that the
//! button went down, so the browser process counts them itself:
//!
//! - **Quick**: a press within [`QUICK`] of the one before — half a second,
//!   macOS's own interval until a person changes it in System Settings
//!   (`NSEvent.doubleClickInterval` with no setting), and Chromium's own off
//!   macOS (`kDoubleClickTimeMs`). A person's own interval is **not read
//!   yet** (queue item 800): this Mac's was measured at 0.15 s on 1 November
//!   2026, a setting its person made, and a card counts their presses more
//!   leniently than Chrome's own window would.
//! - **At one place**: within [`NEAR`] CSS pixels of it each way — four,
//!   Chromium's own off macOS (`kDoubleClickWidth`, `kDoubleClickHeight`),
//!   a hand's tremor and not a move.
//!
//! A press that is neither begins again at one. Counted in the page's CSS
//! pixels, so the same at every scale. The count goes on past three, as
//! the window server's does; the page decides what a fourth is.

use std::time::{Duration, Instant};

/// The most time between two presses counted as one succession.
pub const QUICK: Duration = Duration::from_millis(500);

/// The most CSS pixels, across or down, between two presses counted as
/// one succession.
pub const NEAR: f64 = 4.0;

/// The presses counted so far.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Clicks {
    /// The last press: when, where on the page, and its count.
    last: Option<(Instant, (f64, f64), u32)>,
}

impl Clicks {
    /// A press at `at`, in the page's CSS pixels, `now`: which of a quick
    /// succession at one place it is, counted from one.
    pub fn pressed(&mut self, now: Instant, at: (f64, f64)) -> u32 {
        let count = match self.last {
            Some((then, there, count))
                if now.saturating_duration_since(then) <= QUICK
                    && (at.0 - there.0).abs() <= NEAR
                    && (at.1 - there.1).abs() <= NEAR =>
            {
                count.saturating_add(1)
            }
            _ => 1,
        };
        self.last = Some((now, at, count));
        count
    }

    /// Begin again: the next press is a first — the card's page gone, or
    /// another opened.
    pub fn forget(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_in_quick_succession_at_one_place_are_counted() {
        let start = Instant::now();
        let mut clicks = Clicks::default();
        assert_eq!(clicks.pressed(start, (100.0, 50.0)), 1);
        assert_eq!(
            clicks.pressed(start + Duration::from_millis(200), (101.5, 48.0)),
            2,
            "a double click"
        );
        assert_eq!(
            clicks.pressed(start + Duration::from_millis(700), (104.0, 52.0)),
            3,
            "within half a second of the one before, not of the first"
        );
        assert_eq!(
            clicks.pressed(start + Duration::from_millis(1200), (100.0, 50.0)),
            4,
            "and on"
        );
    }

    #[test]
    fn a_slow_or_a_far_press_begins_again() {
        let start = Instant::now();
        let mut clicks = Clicks::default();
        assert_eq!(clicks.pressed(start, (100.0, 50.0)), 1);
        assert_eq!(clicks.pressed(start + QUICK, (100.0, 50.0)), 2, "at QUICK");
        assert_eq!(
            clicks.pressed(
                start + QUICK + QUICK + Duration::from_millis(1),
                (100.0, 50.0)
            ),
            1,
            "past it"
        );
        assert_eq!(
            clicks.pressed(
                start + QUICK + QUICK + Duration::from_millis(2),
                (104.5, 50.0)
            ),
            1,
            "more than four across"
        );
        assert_eq!(
            clicks.pressed(
                start + QUICK + QUICK + Duration::from_millis(3),
                (104.5, 45.5)
            ),
            1,
            "more than four down"
        );
    }

    #[test]
    fn forgotten_the_next_press_is_a_first() {
        let start = Instant::now();
        let mut clicks = Clicks::default();
        assert_eq!(clicks.pressed(start, (10.0, 10.0)), 1);
        clicks.forget();
        assert_eq!(
            clicks.pressed(start + Duration::from_millis(100), (10.0, 10.0)),
            1
        );
    }
}
