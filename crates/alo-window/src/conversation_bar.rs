/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How much of the sign of more is seen (queue item 793, approved by the
//! owner on 6 November 2026): the conversation's thin bar fades out about
//! a second after scrolling stops, as macOS's own does, and comes back
//! while it scrolls or when the pointer nears it at the room's right.
//!
//! Time and nothing else, so it is tested as state at given instants: the
//! window says when the conversation moved and where the pointer is, and
//! asks how much of the bar is seen now ([`BarFade::seen`]) and when to be
//! woken to ask again ([`BarFade::wake_at`]).
//!
//! # Decided here
//!
//! - **Whole while it scrolls** — a wheel, a glide's every step, a key, a
//!   new turn put at the room's top: anything that moves the conversation —
//!   and for [`HOLD`] after it stops, a second; then it fades out in
//!   [`FADE`], 300 ms, straight, to nothing. It comes back whole at once,
//!   not faded in: macOS shows its bar the moment a person scrolls.
//! - **Near is the gutter's strip**: the pointer over the first screen,
//!   between the room's top and its foot, within [`NEAR`] — 24 CSS pixels
//!   — either side of the bar. While it is there the bar is whole; once it
//!   leaves, the second and the fade begin from then.
//! - **Until anything moves, it is not seen**: a chat opened at rest shows
//!   none until it scrolls, as a page in a macOS window does.
//! - Faded, the bar is still there to an agent, which reads what the
//!   interface is (`alo-renderer`'s `conversation_scroll::Bar::seen`).

use std::time::{Duration, Instant};

/// How long the bar stays whole after the conversation last moved.
pub const HOLD: Duration = Duration::from_secs(1);

/// How long it takes to fade out after that.
pub const FADE: Duration = Duration::from_millis(300);

/// How near the bar, either side of it, the pointer brings it back, in CSS
/// pixels.
pub const NEAR: f32 = 24.0;

/// How often a fading bar is drawn again: the display's sixtieth.
pub const FRAME: Duration = Duration::from_micros(16_667);

/// When the conversation last moved, and whether the pointer is near the
/// bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarFade {
    /// When it last moved, or the pointer last left the bar.
    stirred: Option<Instant>,
    /// Whether the pointer is near the bar now.
    near: bool,
}

impl BarFade {
    /// The conversation moved `now`: the bar is whole, and its second
    /// begins again.
    pub fn moved(&mut self, now: Instant) {
        self.stirred = Some(now);
    }

    /// The pointer is `near` the bar, or not, `now`: whether that changed
    /// it. Leaving, its second begins then.
    pub fn pointer(&mut self, near: bool, now: Instant) -> bool {
        if self.near == near {
            return false;
        }
        self.near = near;
        if !near {
            self.stirred = Some(now);
        }
        true
    }

    /// How much of the bar is seen at `now`, from `1` to `0`.
    pub fn seen(&self, now: Instant) -> f32 {
        if self.near {
            return 1.0;
        }
        let Some(stirred) = self.stirred else {
            return 0.0;
        };
        let since = now.saturating_duration_since(stirred);
        if since < HOLD {
            return 1.0;
        }
        let fading = since.saturating_sub(HOLD).as_secs_f32() / FADE.as_secs_f32();
        (1.0 - fading).clamp(0.0, 1.0)
    }

    /// When the window is next to be woken to draw the bar as it then is,
    /// after `now`: when its second ends, each frame while it fades, and
    /// never while it is whole by the pointer or gone.
    pub fn wake_at(&self, now: Instant) -> Option<Instant> {
        if self.near {
            return None;
        }
        let stirred = self.stirred?;
        let held = stirred + HOLD;
        if now < held {
            Some(held)
        } else if now < held + FADE {
            Some((now + FRAME).min(held + FADE))
        } else {
            None
        }
    }
}

/// Whether the pointer at `x`, `y` on the first screen, in its CSS pixels,
/// is near a bar whose left edge is at `bar_left` in a room from `top` to
/// `bottom`: within [`NEAR`] either side of its 6 across, and between the
/// room's top and its foot.
pub fn near(x: f32, y: f32, bar_left: f32, (top, bottom): (f32, f32)) -> bool {
    let width = alo_renderer::conversation_scroll::BAR_WIDTH;
    (bar_left - NEAR..=bar_left + width + NEAR).contains(&x) && (top..=bottom).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(at: Instant, millis: u64) -> Instant {
        at + Duration::from_millis(millis)
    }

    /// A fraction to a hundredth, for comparing without comparing floats.
    fn hundredths(value: f32) -> String {
        format!("{value:.2}")
    }

    #[test]
    fn at_rest_from_the_start_it_is_not_seen() {
        let fade = BarFade::default();
        let now = Instant::now();
        assert_eq!(hundredths(fade.seen(now)), "0.00");
        assert_eq!(fade.wake_at(now), None);
    }

    #[test]
    fn whole_for_a_second_after_it_moved_then_faded_out_in_300_ms() {
        let at = Instant::now();
        let mut fade = BarFade::default();
        fade.moved(at);
        for (millis, seen) in [
            (0, "1.00"),
            (500, "1.00"),
            (999, "1.00"),
            (1_000, "1.00"),
            (1_075, "0.75"),
            (1_150, "0.50"),
            (1_225, "0.25"),
            (1_300, "0.00"),
            (5_000, "0.00"),
        ] {
            assert_eq!(
                hundredths(fade.seen(after(at, millis))),
                seen,
                "{millis} ms"
            );
        }
        // Woken as its second ends, each frame while it fades, and never
        // once it is gone.
        assert_eq!(fade.wake_at(after(at, 10)), Some(after(at, 1_000)));
        assert_eq!(
            fade.wake_at(after(at, 1_100)),
            Some(after(at, 1_100) + FRAME)
        );
        assert_eq!(fade.wake_at(after(at, 1_295)), Some(after(at, 1_300)));
        assert_eq!(fade.wake_at(after(at, 1_300)), None);
    }

    #[test]
    fn scrolling_again_brings_it_back_whole_at_once() {
        let at = Instant::now();
        let mut fade = BarFade::default();
        fade.moved(at);
        assert_eq!(hundredths(fade.seen(after(at, 1_150))), "0.50");
        fade.moved(after(at, 1_150));
        assert_eq!(hundredths(fade.seen(after(at, 1_150))), "1.00");
        assert_eq!(hundredths(fade.seen(after(at, 2_149))), "1.00");
        assert_eq!(hundredths(fade.seen(after(at, 2_300))), "0.50");
    }

    #[test]
    fn the_pointer_near_it_keeps_it_whole_and_leaving_starts_its_second() {
        let at = Instant::now();
        let mut fade = BarFade::default();
        // Never scrolled, the pointer brings it.
        assert!(fade.pointer(true, at));
        assert!(!fade.pointer(true, at), "already near");
        assert_eq!(hundredths(fade.seen(after(at, 10_000))), "1.00");
        assert_eq!(fade.wake_at(after(at, 10_000)), None, "nothing to fade");
        assert!(fade.pointer(false, after(at, 10_000)));
        assert_eq!(hundredths(fade.seen(after(at, 10_999))), "1.00");
        assert_eq!(hundredths(fade.seen(after(at, 11_150))), "0.50");
        assert_eq!(hundredths(fade.seen(after(at, 11_300))), "0.00");
    }

    #[test]
    fn near_is_24_either_side_of_the_bar_within_the_room() {
        // The bar at 973 to 979 at the design's width; the room 40 to 716.
        let room = (40.0, 716.0);
        for (x, y) in [(949.0, 300.0), (976.0, 40.0), (1003.0, 716.0)] {
            assert!(near(x, y, 973.0, room), "({x}, {y}) is near");
        }
        for (x, y) in [
            (948.0, 300.0),
            (1004.0, 300.0),
            (976.0, 39.0),
            (976.0, 717.0),
        ] {
            assert!(!near(x, y, 973.0, room), "({x}, {y}) is not");
        }
    }
}
