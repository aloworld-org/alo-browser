/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the frame of ours around Chrome's window is told, as Chrome's
//! window moves, is hidden and closes (ADR 0062, queue item 642).
//!
//! [`crate::chrome_frame`] decides where the frame stands from one reading
//! of Chrome's word and the window server's. This is what the thread
//! holding Chrome does with those readings: it reads again every
//! [`FOLLOW_EVERY`] while it has nothing else to do, turns each reading
//! into a [`Follow`] — the only thing the frame's window is ever told — and
//! tells the window only when that changes ([`Following`]), so a window
//! standing still costs the event loop nothing.
//!
//! # A reading that failed stands aside
//!
//! A reading that could not be made — the window server's listing refused,
//! Chrome declining to say where its window is while still holding the
//! page — cannot say that Chrome's window is on the screen. ADR 0062 § 3
//! refuses a frame floating over somebody else's work with nothing of ours
//! under it, so a frame that does not know stands aside, and the next
//! reading puts it back.
//!
//! # How often
//!
//! Each reading is one message to Chrome and one `osascript`, measured by
//! iteration 382 at 80 ms of wall time and 20–30 ms of processor time. A
//! person's drag was read about a third of a second apart and followed to
//! the point (ADR 0062's addendum), so [`FOLLOW_EVERY`] waits a fifth of a
//! second between readings: a frame that trails a drag by less than half a
//! second, for about a tenth of a core. No claim is made about how that
//! feels to a person; that is measured on a screen, by a person, or not made.

use crate::chrome_frame::{Reading, Stance, Unread};
use crate::chrome_protocol::Bounds;
use std::time::Duration;

/// How long the thread holding Chrome waits between one reading of where
/// the frame stands and the next, when nothing else arrives.
pub const FOLLOW_EVERY: Duration = Duration::from_millis(200);

/// What the frame's window is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follow {
    /// Stand at these bounds, in the screen's points, and be seen.
    At(Bounds),
    /// Be hidden, until told where to stand.
    Aside,
    /// Chrome's window is gone, or was closed by this browser: close too.
    Closed,
}

impl Follow {
    /// What a reading of where the frame stands tells the frame.
    pub fn of(read: &Result<Reading, Unread>) -> Self {
        match read {
            Ok(Reading {
                stance: Stance::Beside { frame },
                ..
            }) => Self::At(*frame),
            Ok(Reading {
                stance: Stance::Closed(_),
                ..
            }) => Self::Closed,
            Ok(Reading {
                stance: Stance::Aside(_),
                ..
            })
            | Err(_) => Self::Aside,
        }
    }
}

/// What the frame was last told, so it is told only what changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Following {
    told: Option<Follow>,
}

impl Following {
    /// A reading came to `now`: what to tell the frame, if it is not what
    /// it was told last.
    pub fn heard(&mut self, now: Follow) -> Option<Follow> {
        if self.told == Some(now) {
            return None;
        }
        self.told = Some(now);
        Some(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome_frame::{Aside, Closed};
    use crate::window_server::Unlisted;

    fn at(left: i64, top: i64, width: i64, height: i64) -> Bounds {
        Bounds {
            left,
            top,
            width,
            height,
        }
    }

    fn reading(stance: Stance) -> Reading {
        Reading {
            stance,
            placed: None,
            listed: Vec::new(),
        }
    }

    #[test]
    fn a_frame_beside_stands_where_the_reading_puts_it() {
        let frame = at(300, 224, 700, 36);
        assert_eq!(
            Follow::of(&Ok(reading(Stance::Beside { frame }))),
            Follow::At(frame)
        );
    }

    #[test]
    fn a_frame_aside_or_unread_is_hidden() {
        assert_eq!(
            Follow::of(&Ok(reading(Stance::Aside(Aside::NotOnTheScreen)))),
            Follow::Aside
        );
        assert_eq!(
            Follow::of(&Err(Unread::Unlisted(Unlisted::Elsewhere))),
            Follow::Aside,
            "a frame that cannot say Chrome's window is on the screen stands aside"
        );
        assert_eq!(Follow::of(&Err(Unread::Headless)), Follow::Aside);
    }

    #[test]
    fn a_window_gone_either_way_closes_the_frame() {
        for how in [Closed::Page, Closed::Chrome] {
            assert_eq!(
                Follow::of(&Ok(reading(Stance::Closed(how)))),
                Follow::Closed
            );
        }
    }

    #[test]
    fn the_frame_is_told_only_what_changed() {
        let mut following = Following::default();
        let first = Follow::At(at(120, 44, 1200, 36));
        let moved = Follow::At(at(300, 224, 1200, 36));
        assert_eq!(following.heard(Follow::Aside), Some(Follow::Aside));
        assert_eq!(following.heard(first), Some(first));
        assert_eq!(following.heard(first), None, "standing still is not news");
        assert_eq!(following.heard(moved), Some(moved));
        assert_eq!(following.heard(Follow::Aside), Some(Follow::Aside));
        assert_eq!(following.heard(Follow::Aside), None);
        assert_eq!(following.heard(moved), Some(moved), "back from the strip");
        assert_eq!(following.heard(Follow::Closed), Some(Follow::Closed));
    }
}
