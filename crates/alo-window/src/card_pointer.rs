/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's pointer over the card's page sends the page (ADR 0076 B,
//! ADR 0024 § 5, queue items 718 and 740): the conversation's window asks
//! this of each move, press and release of the main button, and the cursor
//! leaving it, and sends what it answers ([`crate::card_input`]).
//!
//! - **A move with no button held** over the page box is a hover at that
//!   point, so the page's `:hover` and `mouseover` follow the pointer; the
//!   first move off the page box — onto the card's bar, the box, the panel
//!   — is one more hover, where the pointer then is, outside the page, so
//!   the page's hover ends; and so is the cursor leaving the window, at a
//!   point just outside the page's corner. Then nothing until the pointer
//!   is over the page again.
//! - **A press** over the page box is the button going down there, counted
//!   with the presses before it ([`crate::card_clicks`]); **the moves while
//!   it is held** are drags, wherever the pointer is; **the release** comes
//!   up wherever it is, as the press's count.
//!
//! Each point is the person's pointer, aimed by [`crate::card_aim`] from
//! the window's own composition; nothing here is reached by an agent (ADR
//! 0002).

use crate::card_clicks::Clicks;
use crate::card_input::{CardInput, Went};
use std::time::Instant;

/// Where the cursor leaving the window is told to the page to be: just
/// outside its top left corner, where nothing of the page is hovered.
pub const OUTSIDE: (f64, f64) = (-1.0, -1.0);

/// The pointer over the card's page, as far as the page has been told.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CardPointer {
    /// The count of the press held on the page, while it is.
    held: Option<u32>,
    /// Whether the page was last told the pointer is over it.
    hovering: bool,
    /// The presses counted so far.
    clicks: Clicks,
}

/// Where a point of the window is, as the card's page has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnThePage {
    /// The point, in the page's CSS pixels — outside the page as well.
    pub at: (f64, f64),
    /// Whether it is over the page box, seen in the first screen's room,
    /// rather than over something else of the window.
    pub over: bool,
}

impl CardPointer {
    /// Whether a press on the page is held.
    pub fn holds(&self) -> bool {
        self.held.is_some()
    }

    /// The cursor moved to `on`, or — with [`None`] — to where no card's
    /// page is shown: what the page is sent, if anything.
    pub fn moved(&mut self, on: Option<OnThePage>) -> Option<CardInput> {
        let Some(on) = on else {
            // The card went, with whatever was held or hovered.
            self.held = None;
            self.hovering = false;
            return None;
        };
        let went = if self.held.is_some() {
            Went::Dragged
        } else if on.over || std::mem::replace(&mut self.hovering, false) {
            self.hovering = on.over;
            Went::Hovered
        } else {
            return None;
        };
        Some(CardInput::Pointer {
            went,
            x: on.at.0,
            y: on.at.1,
        })
    }

    /// The main button went down `now` over the page at `at`, in its CSS
    /// pixels: the press to send, counted.
    pub fn pressed(&mut self, now: Instant, at: (f64, f64)) -> CardInput {
        let clicks = self.clicks.pressed(now, at);
        self.held = Some(clicks);
        self.hovering = true;
        CardInput::Pointer {
            went: Went::Pressed { clicks },
            x: at.0,
            y: at.1,
        }
    }

    /// The main button came up with the cursor at `at` on the page — or
    /// with no card's page shown — : the release to send, if a press on
    /// the page was held.
    pub fn released(&mut self, at: Option<(f64, f64)>) -> Option<CardInput> {
        let clicks = self.held.take()?;
        let (x, y) = at?;
        Some(CardInput::Pointer {
            went: Went::Released { clicks },
            x,
            y,
        })
    }

    /// The cursor left the window: the hover that ends the page's, if it
    /// was over the page and no press is held — a held press goes on being
    /// dragged, as the window server goes on telling where it is.
    pub fn left(&mut self) -> Option<CardInput> {
        if self.held.is_some() || !std::mem::replace(&mut self.hovering, false) {
            return None;
        }
        Some(CardInput::Pointer {
            went: Went::Hovered,
            x: OUTSIDE.0,
            y: OUTSIDE.1,
        })
    }

    /// Begin again, for a card that went or another opened: nothing held,
    /// hovered or counted.
    pub fn forget(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn over(x: f64, y: f64) -> OnThePage {
        OnThePage {
            at: (x, y),
            over: true,
        }
    }

    fn off(x: f64, y: f64) -> OnThePage {
        OnThePage {
            at: (x, y),
            over: false,
        }
    }

    fn sent(went: Went, x: f64, y: f64) -> CardInput {
        CardInput::Pointer { went, x, y }
    }

    #[test]
    fn a_hover_follows_the_pointer_over_the_page_and_ends_once_off_it() {
        let mut pointer = CardPointer::default();
        assert_eq!(pointer.moved(Some(off(-20.0, 30.0))), None, "never over it");
        assert_eq!(
            pointer.moved(Some(over(10.0, 20.0))),
            Some(sent(Went::Hovered, 10.0, 20.0))
        );
        assert_eq!(
            pointer.moved(Some(over(11.0, 20.0))),
            Some(sent(Went::Hovered, 11.0, 20.0))
        );
        assert_eq!(
            pointer.moved(Some(off(10.0, -3.0))),
            Some(sent(Went::Hovered, 10.0, -3.0)),
            "once off it, where it is, so the page's hover ends"
        );
        assert_eq!(
            pointer.moved(Some(off(10.0, -9.0))),
            None,
            "and then nothing"
        );
        assert_eq!(
            pointer.moved(Some(over(10.0, 1.0))),
            Some(sent(Went::Hovered, 10.0, 1.0))
        );
        assert_eq!(
            pointer.left(),
            Some(sent(Went::Hovered, -1.0, -1.0)),
            "leaving the window ends it too"
        );
        assert_eq!(pointer.left(), None);
        assert_eq!(pointer.moved(Some(off(10.0, -9.0))), None);
    }

    #[test]
    fn a_press_is_counted_dragged_wherever_and_released_as_its_count() {
        let start = Instant::now();
        let mut pointer = CardPointer::default();
        assert_eq!(
            pointer.pressed(start, (40.0, 50.0)),
            CardInput::Pointer {
                went: Went::Pressed { clicks: 1 },
                x: 40.0,
                y: 50.0
            }
        );
        assert!(pointer.holds());
        assert_eq!(
            pointer.moved(Some(off(-5.0, 900.0))),
            Some(sent(Went::Dragged, -5.0, 900.0))
        );
        assert_eq!(pointer.left(), None, "a held press is not a hover");
        assert_eq!(
            pointer.released(Some((-5.0, 900.0))),
            Some(sent(Went::Released { clicks: 1 }, -5.0, 900.0))
        );
        assert!(!pointer.holds());
        assert_eq!(pointer.released(Some((1.0, 1.0))), None, "nothing held");
        // A second press soon after at the same place is a double click,
        // released as one.
        let second = pointer.pressed(start + Duration::from_millis(150), (41.0, 51.0));
        assert_eq!(
            second,
            CardInput::Pointer {
                went: Went::Pressed { clicks: 2 },
                x: 41.0,
                y: 51.0
            }
        );
        assert_eq!(
            pointer.released(Some((41.0, 51.0))),
            Some(sent(Went::Released { clicks: 2 }, 41.0, 51.0))
        );
        // After it, the pointer is over the page: a move off ends the hover.
        assert_eq!(
            pointer.moved(Some(off(41.0, -2.0))),
            Some(sent(Went::Hovered, 41.0, -2.0))
        );
    }

    #[test]
    fn the_card_gone_ends_what_was_held_or_hovered() {
        let mut pointer = CardPointer::default();
        let _ = pointer.pressed(Instant::now(), (1.0, 1.0));
        assert_eq!(pointer.moved(None), None);
        assert!(!pointer.holds());
        assert_eq!(pointer.released(Some((1.0, 1.0))), None);
        assert_eq!(pointer.left(), None, "nothing hovered either");
        // A release with no card shown sends nothing, and holds nothing.
        let _ = pointer.pressed(Instant::now(), (1.0, 1.0));
        assert_eq!(pointer.released(None), None);
        assert!(!pointer.holds());
        // Forgotten, the next press is a first.
        let now = Instant::now();
        let _ = pointer.pressed(now, (1.0, 1.0));
        pointer.forget();
        assert_eq!(
            pointer.pressed(now, (1.0, 1.0)),
            CardInput::Pointer {
                went: Went::Pressed { clicks: 1 },
                x: 1.0,
                y: 1.0
            }
        );
    }
}
