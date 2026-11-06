/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel sliding over the conversation in a narrow window, and
//! the shade behind it (queue item 776, approved by the owner on 6 November
//! 2026).
//!
//! Below [`crate::conversation_fold::FOLD_WIDTH`] the panel is the rail, and
//! *Show the panel* brings the whole panel over the conversation as a
//! sheet, the first screen staying laid out beside the rail. This is how
//! far out the sheet is at an instant — time and nothing else, so it is
//! tested as state at given instants rather than by waiting.
//!
//! # Decided here
//!
//! - **It slides out from the rail**: from [`HIDDEN_AT`], where its right
//!   64 stand over the rail's room, to the window's left edge, so the rail
//!   is never seen bare as it comes. In [`SLIDE`], 200 ms — a sheet's
//!   time on macOS — eased out (`1 − (1 − t)³`), so it arrives gently.
//! - **It slides back the same way**, the same curve run backwards, which
//!   is eased in: a sheet closed half way out goes back from where it was.
//! - **The shade** under it is the ink, `#1b2433`, at most [`SHADE`] —
//!   0.32 — over the conversation and the rail, as far on as the sheet is.

use crate::conversation_place::{PANEL_WIDTH, RAIL_WIDTH};
use std::time::{Duration, Instant};

/// How long the sheet takes to slide out, or back.
pub const SLIDE: Duration = Duration::from_millis(200);

/// How dark the shade is, the sheet all the way out: the ink's alpha.
pub const SHADE: f32 = 0.32;

/// Where the sheet's left edge is when it is in, in CSS pixels from the
/// window's left: its right edge at the rail's.
#[expect(clippy::cast_precision_loss, reason = "176, exact in an f32")]
pub const HIDDEN_AT: f32 = -((PANEL_WIDTH - RAIL_WIDTH) as f32);

/// The sheet: whether it is going out or in, and from how far out, since
/// when.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Slide {
    /// Whether it is going out — or out — rather than in.
    out: bool,
    /// How far out it was, from `0` to `1`, when it last turned.
    from: f32,
    /// When it last turned, if it ever did.
    since: Option<Instant>,
}

impl Slide {
    /// It goes out from `now`, from wherever it is.
    pub fn open(&mut self, now: Instant) {
        self.turn(true, now);
    }

    /// It goes back in from `now`, from wherever it is.
    pub fn close(&mut self, now: Instant) {
        self.turn(false, now);
    }

    /// It is in, at once: the window was widened past the fold, and the
    /// panel stands whole beside the conversation.
    pub fn shut(&mut self) {
        *self = Self::default();
    }

    fn turn(&mut self, out: bool, now: Instant) {
        if self.out == out {
            return;
        }
        self.from = self.travelled(now);
        self.out = out;
        self.since = Some(now);
    }

    /// How far it has travelled out by `now`, from `0` in to `1` out, along
    /// the time and before easing.
    fn travelled(&self, now: Instant) -> f32 {
        let Some(since) = self.since else {
            return if self.out { 1.0 } else { 0.0 };
        };
        let along = now.saturating_duration_since(since).as_secs_f32() / SLIDE.as_secs_f32();
        if self.out {
            (self.from + along).min(1.0)
        } else {
            (self.from - along).max(0.0)
        }
    }

    /// How far out it is at `now`, eased: `0` in, `1` all the way out.
    pub fn out(&self, now: Instant) -> f32 {
        let travelled = self.travelled(now);
        1.0 - (1.0 - travelled).powi(3)
    }

    /// Whether any of it is seen at `now`.
    pub fn seen(&self, now: Instant) -> bool {
        self.travelled(now) > 0.0
    }

    /// Whether it is still moving at `now`: the window is then drawn again
    /// at each of the display's frames.
    pub fn moving(&self, now: Instant) -> bool {
        let travelled = self.travelled(now);
        if self.out {
            travelled < 1.0
        } else {
            travelled > 0.0
        }
    }

    /// Where its left edge is at `now`, in CSS pixels from the window's
    /// left: [`HIDDEN_AT`] in, `0` out.
    pub fn left(&self, now: Instant) -> f32 {
        let out = self.out(now);
        if out >= 1.0 {
            0.0
        } else {
            HIDDEN_AT * (1.0 - out)
        }
    }

    /// How dark the shade under it is at `now`: the ink's alpha.
    pub fn shade(&self, now: Instant) -> f32 {
        SHADE * self.out(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `millis` after `at`.
    fn after(at: Instant, millis: u64) -> Instant {
        at + Duration::from_millis(millis)
    }

    /// A number to a hundredth, for comparing without comparing floats.
    fn hundredths(value: f32) -> String {
        format!("{value:.2}")
    }

    /// A number to a thousandth, as [`hundredths`].
    fn thousandths(value: f32) -> String {
        format!("{value:.3}")
    }

    #[test]
    fn in_it_is_hidden_behind_the_rail_and_shades_nothing() {
        let slide = Slide::default();
        let now = Instant::now();
        assert!(!slide.seen(now));
        assert!(!slide.moving(now));
        assert_eq!(hundredths(slide.left(now)), "-176.00");
        assert_eq!(thousandths(slide.shade(now)), "0.000");
    }

    #[test]
    fn it_slides_out_in_200_ms_eased_out() {
        let at = Instant::now();
        let mut slide = Slide::default();
        slide.open(at);
        // At each instant: where its left edge is, and the shade.
        for (millis, left, shade) in [
            (0, "-176.00", "0.000"),
            // A quarter of the time: 1 − 0.75³ = 0.578 out.
            (50, "-74.25", "0.185"),
            (100, "-22.00", "0.280"),
            (150, "-2.75", "0.315"),
            (200, "0.00", "0.320"),
            (1_000, "0.00", "0.320"),
        ] {
            let now = after(at, millis);
            assert_eq!(hundredths(slide.left(now)), left, "{millis} ms");
            assert_eq!(thousandths(slide.shade(now)), shade, "{millis} ms");
        }
        assert!(slide.seen(after(at, 1)));
        assert!(slide.moving(after(at, 199)));
        assert!(!slide.moving(after(at, 200)));
        assert!(slide.seen(after(at, 5_000)), "out until it is closed");
    }

    #[test]
    fn closed_it_slides_back_the_same_way_and_from_where_it_was() {
        let at = Instant::now();
        let mut slide = Slide::default();
        slide.open(at);
        slide.close(after(at, 400));
        // Back in 200 ms, the curve backwards: as far out at 50 ms back as
        // it was at 150 ms out.
        assert_eq!(hundredths(slide.left(after(at, 450))), "-2.75");
        assert_eq!(hundredths(slide.left(after(at, 550))), "-74.25");
        assert!(slide.moving(after(at, 599)));
        assert!(!slide.seen(after(at, 600)));
        assert!(!slide.moving(after(at, 600)));
        // Closed half way out, it goes back from there, not from all out.
        let mut slide = Slide::default();
        slide.open(at);
        slide.close(after(at, 100));
        assert!(!slide.seen(after(at, 200)), "100 ms out, 100 ms back");
        assert_eq!(hundredths(slide.left(after(at, 150))), "-74.25");
        // And opened again half way back, out from there.
        slide.open(after(at, 150));
        assert_eq!(hundredths(slide.left(after(at, 200))), "-22.00");
        // Closing what is closing changes nothing.
        let mut slide = Slide::default();
        slide.close(at);
        assert!(!slide.seen(after(at, 10)));
    }

    #[test]
    fn widened_past_the_fold_it_is_in_at_once() {
        let at = Instant::now();
        let mut slide = Slide::default();
        slide.open(at);
        slide.shut();
        assert!(!slide.seen(after(at, 100)));
        assert!(!slide.moving(after(at, 100)));
    }
}
