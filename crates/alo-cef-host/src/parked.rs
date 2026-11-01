/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a card's window is parked, and when it has been put somewhere a
//! person is to see it (queue item 771, ADR 0076 B4 as amended).
//!
//! A page shown as a card in alo's conversation is drawn from the frames
//! Chromium streams of it, and Chromium's own window is parked while it is
//! a card. ADR 0076 B4 parked it far left of every display; measured on a
//! Mac with three displays, the window server moves a shown window back
//! until a strip of it is on one — 153 points of any width on the leftmost
//! display there — so a parked window is still partly on the screen. A
//! window made transparent keeps streaming where one ordered out, hidden
//! with its application or put in a hidden target does not (measured), so
//! the host started for a card ([`crate::switches::card`]) shows its window
//! transparent, letting the pointer through (`unseen_mac.rs`) — and makes
//! it seen once alo has put it where a person is to see it: *Expand*, which
//! places it over the conversation's window.
//!
//! [`Stage`] follows the window from its first showing: unseen and not yet
//! parked, then parked, then seen for good once [`placed`] — where it was
//! shown and where alo first fits it to the card, before parking it, are
//! on a display and do not count; only a placing after the park does.
//!
//! [`placed`] says when a window is placed: when its centre is on a
//! display.
//! A window the window server has parked is on a display by its strip only,
//! far from its centre for any card the conversation draws (its page is at
//! least 470 points wide); a window placed over the conversation is where
//! the conversation's window is, whose centre a person sees. A window
//! placed half off every display stays unseen — the cost of the rule, said
//! by ADR 0076's amendment.
//!
//! Pure arithmetic on rectangles, in the screen's points with the main
//! display's top left corner at the origin, as CEF gives a window's bounds
//! and a display's.

/// A rectangle on the screen, in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// Its left edge.
    pub left: i32,
    /// Its top edge.
    pub top: i32,
    /// Its width.
    pub width: i32,
    /// Its height.
    pub height: i32,
}

impl Area {
    /// Whether the point `x`, `y` is inside it: its left and top edges are,
    /// its right and bottom ones are the next area's.
    fn holds(self, (x, y): (i64, i64)) -> bool {
        let (left, top) = (i64::from(self.left), i64::from(self.top));
        x >= left
            && y >= top
            && x < left + i64::from(self.width)
            && y < top + i64::from(self.height)
    }

    /// Its centre, rounded down to a whole point.
    fn centre(self) -> (i64, i64) {
        (
            i64::from(self.left) + i64::from(self.width) / 2,
            i64::from(self.top) + i64::from(self.height) / 2,
        )
    }
}

/// Where a card's window is in its life, as far as being seen goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Unseen, and not yet parked: shown where the window server chose and
    /// fitted to the card there by alo, both on a display.
    Opened,
    /// Unseen, and parked by alo: put where its centre is on no display.
    Parked,
    /// Seen, for good: placed after it was parked.
    Seen,
}

impl Stage {
    /// The stage after the window was put somewhere `placed` says of.
    pub fn after(self, placed: bool) -> Self {
        match (self, placed) {
            (Self::Opened, true) => Self::Opened,
            (Self::Opened | Self::Parked, false) => Self::Parked,
            (Self::Parked | Self::Seen, true) | (Self::Seen, false) => Self::Seen,
        }
    }
}

/// Whether a window at `window` has been put where a person is to see it,
/// among `displays`: when its centre is on one of them.
pub fn placed(window: Area, displays: &[Area]) -> bool {
    let centre = window.centre();
    displays.iter().any(|display| display.holds(centre))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Mac 771 was measured on: the built-in Retina display at the
    /// origin, a 1920 × 1080 display at one left of it and a portrait
    /// 1080 × 1920 one left of that, as CEF gives them.
    const THREE: [Area; 3] = [
        Area {
            left: 0,
            top: 0,
            width: 1710,
            height: 1107,
        },
        Area {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
        },
        Area {
            left: -3000,
            top: -80,
            width: 1080,
            height: 1920,
        },
    ];

    fn at(left: i32, top: i32, width: i32, height: i32) -> Area {
        Area {
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn a_window_the_window_server_parked_is_not_placed() {
        // Where it parked a card's window asked for at −20000, measured
        // through the machine's Chrome and alo's CEF host alike: 153 points
        // of it on the portrait display, at each width.
        assert!(!placed(at(-3725, 0, 878, 552), &THREE));
        assert!(!placed(at(-3447, 0, 600, 552), &THREE));
        // The narrowest card the conversation draws, about 470 wide.
        assert!(!placed(at(-3317, 0, 470, 520), &THREE));
        // Asked above every display as well.
        assert!(!placed(at(-3725, -50, 878, 552), &THREE));
        // Where alo asks for it (`chrome_window::PARKED`), had the window
        // server left it there.
        assert!(!placed(at(-20_000, 0, 878, 552), &THREE));
    }

    #[test]
    fn a_window_expanded_over_the_conversation_is_placed() {
        // The 717 test's *Expand*.
        assert!(placed(at(120, 80, 900, 600), &THREE));
        // Over a conversation on the display at one.
        assert!(placed(at(-1700, 40, 1200, 900), &THREE));
        // Over one straddling two displays.
        assert!(placed(at(-600, 100, 1000, 700), &THREE));
        // Over one partly off every display, its centre still on one.
        assert!(placed(at(1200, 300, 900, 600), &THREE));
    }

    #[test]
    fn a_window_whose_centre_is_off_every_display_is_not_placed() {
        // Right of the Retina display.
        assert!(!placed(at(1400, 300, 900, 600), &THREE));
        // Under the display at one, where nothing is.
        assert!(!placed(at(-1500, 1200, 878, 552), &THREE));
        // With no display at all.
        assert!(!placed(at(120, 80, 900, 600), &[]));
    }

    #[test]
    fn a_card_is_seen_once_placed_after_it_was_parked_and_then_for_good() {
        // As measured through the 717 test: shown where the window server
        // chose, fitted to the card there, parked, fitted narrower while
        // parked, then expanded — and then dragged half off.
        let seen = [true, true, false, false, true, false].iter().fold(
            vec![Stage::Opened],
            |mut stages, &placed| {
                let last = stages.last().copied().unwrap_or(Stage::Opened);
                stages.push(last.after(placed));
                stages
            },
        );
        assert_eq!(
            seen,
            [
                Stage::Opened,
                Stage::Opened,
                Stage::Opened,
                Stage::Parked,
                Stage::Parked,
                Stage::Seen,
                Stage::Seen,
            ]
        );
    }

    #[test]
    fn a_displays_right_and_bottom_edges_are_not_its_own() {
        let one = [at(0, 0, 100, 100)];
        assert!(placed(at(-50, -50, 100, 100), &one));
        assert!(placed(at(49, 49, 100, 100), &one));
        assert!(!placed(at(50, 0, 100, 100), &one));
        assert!(!placed(at(0, 50, 100, 100), &one));
    }
}
