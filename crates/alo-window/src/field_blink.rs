/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether the caret of alo's field is drawn at an instant: it blinks
//! (queue item 861, `docs/design/shell.md`, *The one box as a text field*).
//!
//! **Shown for half a second, then hidden for half a second**, over and
//! over, from the moment it last moved — an edit, an arrow, a press, the
//! field given the keyboard — so a caret a person is moving or typing with
//! is always there to be seen, and never vanishes under their hands. Half a
//! second is Chromium's default caret interval. A Mac's own setting
//! (`NSTextInsertionPointBlinkPeriodOn` and `…Off`) is not read, and the
//! rate was not measured against macOS's own fields.
//!
//! It is a function of the instant and nothing else, so a test asks it at
//! given instants of an injected clock and never sleeps; the window wakes
//! at [`Blink::next_change`] to draw the change.

use std::time::{Duration, Instant};

/// How long the caret is shown, and then how long hidden.
pub const HALF: Duration = Duration::from_millis(500);

/// When the caret last moved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Blink {
    /// The instant it last moved, if it has.
    since: Option<Instant>,
}

impl Blink {
    /// The caret moved `now`: shown from now, for half a second.
    pub fn restart(&mut self, now: Instant) {
        self.since = Some(now);
    }

    /// How many whole halves have passed since it moved, at `now`.
    fn halves(self, now: Instant) -> u128 {
        let since = self.since.unwrap_or(now);
        now.saturating_duration_since(since).as_millis() / HALF.as_millis()
    }

    /// Whether the caret is shown at `now`: in the first half of each
    /// second since it moved. One that never moved is shown.
    pub fn shown(self, now: Instant) -> bool {
        self.halves(now).is_multiple_of(2)
    }

    /// When it next goes from shown to hidden or back, after `now`.
    pub fn next_change(self, now: Instant) -> Instant {
        let since = self.since.unwrap_or(now);
        let halves = u32::try_from(self.halves(now) + 1).unwrap_or(u32::MAX);
        since + HALF.saturating_mul(halves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_for_half_a_second_then_hidden_for_half_from_its_last_move() {
        let start = Instant::now();
        let at = |millis: u64| start + Duration::from_millis(millis);
        let mut blink = Blink::default();
        assert!(blink.shown(start), "never moved, shown");
        blink.restart(start);
        assert!(blink.shown(at(0)));
        assert!(blink.shown(at(499)));
        assert!(!blink.shown(at(500)));
        assert!(!blink.shown(at(999)));
        assert!(blink.shown(at(1_000)));
        assert!(!blink.shown(at(2_750)));
        assert_eq!(blink.next_change(at(0)), at(500));
        assert_eq!(blink.next_change(at(620)), at(1_000));
        // Moved again while hidden: shown at once, for a whole half.
        blink.restart(at(700));
        assert!(blink.shown(at(700)));
        assert!(blink.shown(at(1_199)));
        assert!(!blink.shown(at(1_200)));
        assert_eq!(blink.next_change(at(700)), at(1_200));
    }
}
