//! Windows: how much may be sent before the other end says more.
//!
//! # Why this is arithmetic worth its own file
//!
//! A window is a signed number that two ends change independently, and every
//! way of getting it wrong is a bug that only appears under load:
//!
//! - Adding an increase without checking overflows past the protocol's ceiling,
//!   and a peer can send increases forever.
//! - Subtracting what arrived without checking lets a peer send more than it
//!   was allowed, which is the whole thing a window exists to prevent.
//! - And a window **may legitimately be negative**, which is the one that
//!   surprises people. Lowering `SETTINGS_INITIAL_WINDOW_SIZE` applies to every
//!   stream that already exists, retroactively — so data already in flight can
//!   leave a window below zero, and treating that as an error would break a
//!   peer that did nothing wrong.
//!
//! So this is `i64` arithmetic with named refusals, rather than a `u32` and
//! some hope.

use super::ErrorCode;
use super::frame::Broken;

/// What both ends start with, before any `SETTINGS`.
pub const AT_FIRST: i64 = 65_535;

/// The most a window may ever hold.
///
/// Two to the thirty-first, less one. A window pushed past this is a peer
/// sending increases that cannot be meant, and the protocol says to refuse it
/// rather than to saturate — saturating would leave the two ends disagreeing
/// about how much may be sent, which is worse than stopping.
pub const CEILING: i64 = (1 << 31) - 1;

/// One direction of one thing — a stream, or the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    room: i64,
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

impl Window {
    /// A window at the protocol's starting size.
    pub fn new() -> Self {
        Self { room: AT_FIRST }
    }

    /// A window of exactly this much.
    pub fn of(room: i64) -> Self {
        Self { room }
    }

    /// How much room there is, which may be negative.
    pub fn room(self) -> i64 {
        self.room
    }

    /// Whether anything may be sent at all.
    pub fn is_open(self) -> bool {
        self.room > 0
    }

    /// Take room for something being sent.
    ///
    /// Returns how much may actually go now, which is the smaller of what was
    /// wanted and what there is. A sender that ignored the answer would be the
    /// peer this file is written to defend against.
    pub fn take(&mut self, wanted: usize) -> usize {
        let can = self
            .room
            .max(0)
            .min(i64::try_from(wanted).unwrap_or(i64::MAX));
        self.room -= can;
        usize::try_from(can).unwrap_or(0)
    }

    /// Account for something that arrived.
    ///
    /// # Errors
    ///
    /// [`Broken`] with [`ErrorCode::FlowControlError`] when more arrived than
    /// was allowed. That is the peer ignoring the window, and there is nothing
    /// to do but stop: the bytes are already held.
    pub fn arrived(&mut self, how_much: usize, fatal: bool) -> Result<(), Broken> {
        let how_much = i64::try_from(how_much).unwrap_or(i64::MAX);
        if how_much > self.room {
            return Err(Broken {
                why: format!(
                    "{how_much} bytes arrived against a window of {}, which is more than was allowed",
                    self.room
                ),
                error: ErrorCode::FlowControlError,
                fatal,
            });
        }
        self.room -= how_much;
        Ok(())
    }

    /// Widen it, because the other end said there is more room.
    ///
    /// # Errors
    ///
    /// [`Broken`] when the increase would push it past [`CEILING`].
    pub fn widen(&mut self, by: u32, fatal: bool) -> Result<(), Broken> {
        let widened = self.room.saturating_add(i64::from(by));
        if widened > CEILING {
            return Err(Broken {
                why: format!(
                    "a window widened to {widened}, past the {CEILING} this protocol allows"
                ),
                error: ErrorCode::FlowControlError,
                fatal,
            });
        }
        self.room = widened;
        Ok(())
    }

    /// Move it by the difference a new `SETTINGS_INITIAL_WINDOW_SIZE` makes.
    ///
    /// **This is where a window legitimately goes negative.** The change applies
    /// to every stream that already exists, and data already in flight was sent
    /// against the old size. A peer that lowers the initial size after sending
    /// has done nothing wrong, and refusing the negative result would break it.
    ///
    /// # Errors
    ///
    /// [`Broken`] only when the *increase* would pass the ceiling. Going below
    /// zero is not an error here — it is an error only if the peer then sends
    /// something, which [`Window::arrived`] catches on its own.
    pub fn resettle(&mut self, by: i64) -> Result<(), Broken> {
        let moved = self.room.saturating_add(by);
        if moved > CEILING {
            return Err(Broken {
                why: format!("a new initial window size would put a stream at {moved}"),
                error: ErrorCode::FlowControlError,
                fatal: true,
            });
        }
        self.room = moved;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_starts_where_the_protocol_says() {
        assert_eq!(Window::new().room(), 65_535);
    }

    #[test]
    fn taking_is_bounded_by_what_is_there() {
        let mut window = Window::of(100);
        assert_eq!(window.take(30), 30);
        assert_eq!(window.take(1000), 70, "it handed out room it did not have");
        assert_eq!(window.room(), 0);
        assert_eq!(window.take(1), 0);
    }

    #[test]
    fn more_arriving_than_was_allowed_is_refused() {
        let mut window = Window::of(10);
        assert!(window.arrived(10, true).is_ok());
        let mut window = Window::of(10);
        let refused = window.arrived(11, true);
        assert!(
            refused.is_err(),
            "a peer overran its window and was believed"
        );
        assert_eq!(
            refused.err().map(|why| why.error),
            Some(ErrorCode::FlowControlError)
        );
    }

    #[test]
    fn a_window_widened_past_the_ceiling_is_refused_rather_than_saturated() {
        let mut window = Window::of(CEILING - 1);
        assert!(window.widen(1, true).is_ok());
        assert_eq!(window.room(), CEILING);
        assert!(
            window.widen(1, true).is_err(),
            "saturating would leave the two ends disagreeing about how much may be sent"
        );
    }

    /// The one that surprises people. Lowering the initial size applies to
    /// streams that already exist, and data already in flight was sent against
    /// the old size.
    #[test]
    fn a_window_may_legitimately_be_negative() {
        let mut window = Window::of(1000);
        // The peer lowers the initial window size by 5000 after 1000 was left.
        assert!(
            window.resettle(-5000).is_ok(),
            "a negative window was refused"
        );
        assert_eq!(window.room(), -4000);
        assert!(!window.is_open());
        // Nothing may be sent until it climbs back above zero.
        assert_eq!(window.take(1), 0);
        assert!(window.widen(4001, true).is_ok());
        assert!(window.is_open());
    }

    #[test]
    fn resettling_past_the_ceiling_is_still_refused() {
        let mut window = Window::of(CEILING);
        assert!(window.resettle(1).is_err());
    }
}
