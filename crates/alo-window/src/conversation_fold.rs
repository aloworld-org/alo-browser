/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether the chats panel stands whole or folded to its rail, and the
//! least window the conversation is laid out in (queue items 711, 775,
//! 819).
//!
//! The design (`docs/design/shell.md`, *The window*) folds the panel to its
//! 64 px rail on a narrow window and while the agent works. Three things
//! fold it, and they are kept apart, because only one of them is a
//! person's:
//!
//! - **A person hid it**, with the button at the mark's end: hidden until
//!   they press the rail's *Show the panel*, at any width.
//! - **The window is narrow**: narrower than [`FOLD_WIDTH`] CSS pixels, the
//!   panel folds on its own, and it unfolds on its own as the window is
//!   widened past it again — unless a person hid it meanwhile, which is
//!   remembered and kept.
//! - **alo's agent is at work** on the card's page (queue item 819): the
//!   rail for the run, at any width, so the split view has the room
//!   ([`Fold::side_at_work`]). Nothing a person chose is written over: a
//!   panel shown before the run is shown again once it ends, a panel hidden
//!   before it stays hidden, and *Hide* or *Show the panel* pressed during
//!   it is kept for after it — the window saying so, as for a narrow
//!   window ([`at_work`]).
//!
//! # Why 760
//!
//! The search's tab row, *All* to *News*, ends 412.75 from the column's
//! left — 444.75 from the room's, the column being 32 in
//! (`alo-renderer`'s `a_conversation_in_a_narrow_window.rs`) — and the
//! column is the room less 32 either side (queue item 774). Beside the
//! panel's 240 the row is just inside the column from a window 717 wide (a
//! room of 477); 760 is the first round width at which it is inside with a
//! gutter's 32 to spare (from 749: a column of 445), so a tab's words a
//! little wider in another face still fit — at 760 the room is 520 and the
//! column 456. At 759 the panel folds, and the same window has a column of
//! 631 beside the rail.
//!
//! # The least window
//!
//! Folded, the row is inside the column from a window of 541 (the rail's
//! 64 and the room's 477); the window is never made narrower than
//! [`LEAST_SIZE`]'s 600, a column of 472 with the row's end 59 inside it,
//! nor shorter than its 600, where the room the conversation is seen in
//! runs from 40 to 496 and the first screen's sites row (386 to 470 at the
//! design's width) is seen whole. The window server keeps it so
//! (`with_min_inner_size`), so the tab row never needs to scroll.
//!
//! # Shown in a narrow window
//!
//! *Show the panel* pressed while the window is narrow is a person's
//! choice, and kept: the panel comes back as soon as the window is
//! [`FOLD_WIDTH`] wide. It does not come back at once — the column beside
//! it would be narrower than the tab row — and the window says so in
//! words rather than doing nothing silently.

use crate::conversation_place::Side;

/// The window's width, in CSS pixels, from which the panel stands whole:
/// narrower, it folds to its rail.
pub const FOLD_WIDTH: u32 = 760;

/// The least window the conversation is laid out in, in CSS pixels, wide
/// and tall: what the window server is told is its least inner size.
pub const LEAST_SIZE: (u32, u32) = (600, 600);

/// What the window says when *Show the panel* is pressed while it is too
/// narrow for the panel: why it is still the rail, and when it is not.
pub fn too_narrow() -> String {
    format!("The window is too narrow for the panel: it comes back at {FOLD_WIDTH} wide")
}

/// What the window says when *Show the panel* is pressed while alo's agent
/// works on the card (queue item 819): why it is still the rail, and when
/// it is not.
pub fn at_work() -> String {
    "alo is working on this page: the panel comes back when it has finished".to_owned()
}

/// Whether the panel is folded, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fold {
    /// Whether a person hid it.
    hidden: bool,
    /// Whether the window is narrower than [`FOLD_WIDTH`].
    narrow: bool,
}

impl Fold {
    /// What stands down the window's left: the rail while a person hid the
    /// panel or the window is narrow, the panel otherwise.
    pub fn side(self) -> Side {
        if self.hidden || self.narrow {
            Side::Rail
        } else {
            Side::Panel
        }
    }

    /// What stands down the window's left while alo's agent is `working` on
    /// the card's page or not (queue item 819): the rail for the run, and
    /// otherwise [`Self::side`] — what a person chose and the window's width
    /// say, which the run never changed.
    pub fn side_at_work(self, working: bool) -> Side {
        if working { Side::Rail } else { self.side() }
    }

    /// Whether a person hid the panel, whatever the window's width.
    pub fn hidden(self) -> bool {
        self.hidden
    }

    /// Whether the window is too narrow for the panel.
    pub fn narrow(self) -> bool {
        self.narrow
    }

    /// A person hid the panel: whether that changed what stands down the
    /// left.
    pub fn hide(&mut self) -> bool {
        self.chosen(true)
    }

    /// A person showed the panel: whether that changed what stands down the
    /// left — not while the window is narrow, though the choice is kept.
    pub fn show(&mut self) -> bool {
        self.chosen(false)
    }

    /// The window is `width` CSS pixels wide now: whether that folded or
    /// unfolded the panel.
    pub fn fit(&mut self, width: u32) -> bool {
        let before = self.side();
        self.narrow = width < FOLD_WIDTH;
        self.side() != before
    }

    /// A person's choice, `hidden` or not: whether it changed the side.
    fn chosen(&mut self, hidden: bool) -> bool {
        let before = self.side();
        self.hidden = hidden;
        self.side() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_folds_under_760_and_unfolds_from_it() {
        let mut fold = Fold::default();
        assert_eq!(fold.side(), Side::Panel);
        assert!(!fold.fit(1280), "the design's window");
        assert!(!fold.fit(760), "760 holds the panel");
        assert_eq!(fold.side(), Side::Panel);
        assert!(fold.fit(759), "759 folds it");
        assert_eq!(fold.side(), Side::Rail);
        assert!(fold.narrow());
        assert!(!fold.hidden(), "nobody hid it");
        assert!(!fold.fit(600), "narrower still is still the rail");
        assert!(fold.fit(760), "widened to 760 it unfolds");
        assert_eq!(fold.side(), Side::Panel);
        assert!(!fold.narrow());
    }

    #[test]
    fn a_panel_a_person_hid_stays_hidden_as_the_window_widens() {
        let mut fold = Fold::default();
        assert!(fold.hide());
        assert!(!fold.hide(), "already hidden");
        assert!(!fold.fit(700), "hidden, narrowing changes nothing seen");
        assert!(!fold.fit(1280), "nor widening");
        assert_eq!(fold.side(), Side::Rail);
        assert!(fold.hidden());
        assert!(fold.show());
        assert_eq!(fold.side(), Side::Panel);
    }

    #[test]
    fn hidden_wide_then_narrowed_and_widened_it_is_still_hidden() {
        let mut fold = Fold::default();
        assert!(fold.hide());
        assert!(!fold.fit(700));
        assert!(fold.narrow() && fold.hidden(), "both, and kept apart");
        assert!(!fold.fit(900), "widened, still hidden");
        assert_eq!(fold.side(), Side::Rail);
    }

    #[test]
    fn shown_in_a_narrow_window_it_comes_back_once_the_window_is_wide_enough() {
        let mut fold = Fold::default();
        assert!(fold.hide());
        let _ = fold.fit(700);
        assert!(!fold.show(), "the window is too narrow for it yet");
        assert_eq!(fold.side(), Side::Rail);
        assert!(!fold.hidden(), "but the choice is kept");
        assert!(fold.fit(760), "and it comes back at 760");
        assert_eq!(fold.side(), Side::Panel);
    }

    /// Queue item 819: the rail while the agent works, and back to what the
    /// person chose and the width say once it ends — whatever was chosen
    /// before or during the run.
    #[test]
    fn the_agent_at_work_folds_the_panel_and_keeps_what_a_person_chose() {
        // Shown before the run: the rail during it, the panel after it.
        let mut fold = Fold::default();
        assert_eq!(fold.side_at_work(true), Side::Rail);
        assert_eq!(fold.side_at_work(false), Side::Panel);
        assert!(!fold.hidden(), "the run chose nothing for the person");
        // Hidden during the run: still the rail after it.
        assert!(fold.hide());
        assert_eq!(fold.side_at_work(true), Side::Rail);
        assert_eq!(fold.side_at_work(false), Side::Rail);
        assert!(fold.hidden());
        // Hidden before, shown during: the panel after it.
        assert!(fold.show());
        assert_eq!(fold.side_at_work(true), Side::Rail);
        assert_eq!(fold.side_at_work(false), Side::Panel);
        // Narrow: the rail after it too, unless widened.
        let _ = fold.fit(700);
        assert_eq!(fold.side_at_work(false), Side::Rail);
        let _ = fold.fit(1280);
        assert_eq!(fold.side_at_work(false), Side::Panel);
    }

    #[test]
    fn the_least_window_is_folded() {
        // Its 600 beside the rail is a room of 536, the column 472: the
        // tab row inside it is asserted where it is laid out
        // (`alo-renderer`'s `a_conversation_in_a_narrow_window.rs`).
        assert_eq!(LEAST_SIZE, (600, 600));
        let mut fold = Fold::default();
        assert!(fold.fit(LEAST_SIZE.0));
        assert_eq!(fold.side(), Side::Rail);
    }
}
