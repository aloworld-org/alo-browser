/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the card's page box is while alo's agent works on it: the split
//! view (queue item 819, `docs/design/shell.md`, *The agent at work*) —
//! geometry the browser process reads from its own composition, never from
//! a renderer, as [`crate::card_place`] is for the card in the
//! conversation.
//!
//! # What splits
//!
//! While the agent works on the live card — acting, or asking the person —
//! the first screen is no longer the conversation: **the page large on the
//! left** and **the agent's column, [`COLUMN`] wide, on the right**, the
//! whole height of the first screen. The page stands in its card, its bar
//! and border kept, [`INSET`] in from the first screen's top, left and
//! bottom and from the column; the column holds what was asked, the steps,
//! the question with *Allow* and *Not now*, and *Stop*, and at its foot the
//! box (`interface/start.css`, *The split view*). The panel is the rail for
//! the run (`alo-window`'s `conversation_fold`). When the run ends — done,
//! stuck, stopped — the first screen is the conversation again, and what
//! the run ended with is drawn under the page as before (queue item 704).
//!
//! # Its least width
//!
//! The page box in the split view is the first screen less the column, the
//! insets and the card's border: [`page_size`]. It splits only when that is
//! at least [`LEAST_PAGE`], 720 CSS pixels — far past a phone's width (430
//! at most), so a site lays the page out as for a tablet or a desktop
//! while the agent works on it, and the split never makes the page more
//! than 158 narrower than the conversation's 878: a first screen of
//! [`LEAST_ROOM`], 1138, a window of 1202 beside the rail. Narrower, the
//! agent's column stands under the page inside the card, as queue item 704
//! built it, the panel still the rail.
//!
//! # What the split costs the page
//!
//! In the design's window, 1280 × 820 at one, the conversation's page box
//! is 878 × 520 beside the panel; split beside the rail it is
//! **798 × 740** — 80 narrower and 220 taller — and Chrome is told that
//! size for the run, then 878 × 520 again (`alo-window`'s
//! `ConversationWindow::restream`, the road queue item 774 built).

use crate::card::Card;
use crate::card_place;
use alo_layout::{Rect, Size};

/// How wide the agent's column is, its border included: the design's 384.
pub const COLUMN: f32 = 384.0;

/// How far the card is in from the first screen's top, left and bottom
/// edges and from the column.
pub const INSET: f32 = 16.0;

/// The narrowest page box the split view lays a page out at, in CSS
/// pixels: narrower, the first screen stays the conversation.
pub const LEAST_PAGE: u32 = 720;

/// The narrowest first screen the split view is drawn in: [`LEAST_PAGE`]
/// and everything else across.
pub const LEAST_ROOM: f32 = 1138.0;

/// The card's border.
const BORDER: f32 = 1.0;

/// The card's bar, its line under it included.
const BAR: f32 = 46.0;

/// Where the card stands on the first screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stands {
    /// In the conversation's newest turn ([`crate::card_place`]).
    #[default]
    InTheConversation,
    /// In the split view, beside the agent's column.
    Split,
}

impl Stands {
    /// Where the live card `card` stands on a first screen laid out at
    /// `viewport`: split while the agent works on it — acting or asking —
    /// and the first screen is at least [`LEAST_ROOM`] wide; in the
    /// conversation otherwise.
    pub fn of(viewport: Size, card: Option<&Card>) -> Self {
        let working = card
            .and_then(|card| card.agent.as_ref())
            .is_some_and(crate::agent_work::AtWork::is_working);
        if working && viewport.width >= LEAST_ROOM {
            Self::Split
        } else {
            Self::InTheConversation
        }
    }

    /// The size of the card's page box, in CSS pixels: what Chrome lays the
    /// page out at.
    pub fn page_size(self, viewport: Size) -> (u32, u32) {
        match self {
            Self::InTheConversation => card_place::page_size(viewport),
            Self::Split => page_size(viewport),
        }
    }

    /// The card's page box, the conversation scrolled `down` — which the
    /// split view, holding no conversation, is not.
    pub fn page_box(self, viewport: Size, down: f32) -> Rect {
        match self {
            Self::InTheConversation => card_place::page_box(viewport, down),
            Self::Split => page_box(viewport),
        }
    }

    /// The part of the card's page box that is seen, scrolled `down` with
    /// `offered` offers over the box: in the split view, all of it, since
    /// nothing scrolls over it and the offers stand in the column.
    pub fn seen_page_box(self, viewport: Size, down: f32, offered: usize) -> Option<Rect> {
        match self {
            Self::InTheConversation => card_place::seen_page_box(viewport, down, offered),
            Self::Split => {
                let page = page_box(viewport);
                (page.size.width > 0.0 && page.size.height > 0.0).then_some(page)
            }
        }
    }
}

/// The size of the card's page box in the split view on a first screen laid
/// out at `viewport`, in CSS pixels: the first screen less the column, the
/// insets either side and the card's border; and its height less the insets
/// above and below, the card's border and its bar. At least one pixel each
/// way, since Chrome lays out no page of none.
pub fn page_size(viewport: Size) -> (u32, u32) {
    let wide = viewport.width - COLUMN - 2.0 * INSET - 2.0 * BORDER;
    let tall = viewport.height - 2.0 * INSET - 2.0 * BORDER - BAR;
    (whole(wide), whole(tall))
}

/// The card's page box in the split view on a first screen laid out at
/// `viewport`: [`page_size`], inside the card's border, under its bar.
pub fn page_box(viewport: Size) -> Rect {
    let (width, height) = page_size(viewport);
    Rect::new(
        INSET + BORDER,
        INSET + BORDER + BAR,
        length(width),
        length(height),
    )
}

/// The agent's column in the split view on a first screen laid out at
/// `viewport`: [`COLUMN`] wide at its right edge, its whole height.
pub fn column(viewport: Size) -> Rect {
    Rect::new(
        (viewport.width - COLUMN).max(0.0),
        0.0,
        COLUMN.min(viewport.width),
        viewport.height,
    )
}

/// A length as whole CSS pixels, at least one.
fn whole(length: f32) -> u32 {
    let floor = length.floor().max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole, positive length of a first screen's, at most a window's, exact in a u32"
    )]
    let pixels = floor as u32;
    pixels
}

/// A page box's side as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "the page box's sides are in the thousands at most, exact in an f32"
)]
fn length(pixels: u32) -> f32 {
    pixels as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_work::{AtWork, Now};

    fn sides(rect: Rect) -> [u32; 4] {
        [rect.left(), rect.top(), rect.size.width, rect.size.height].map(f32::to_bits)
    }

    fn card(now: Option<Now>) -> Card {
        let mut card = Card::new("a", alo_url::parse("https://example.com/").unwrap());
        card.agent = now.map(|now| AtWork {
            now,
            ..AtWork::starting("x")
        });
        card
    }

    #[test]
    fn in_the_designs_window_beside_the_rail_the_page_is_798_by_740() {
        // 1280 × 820 at one, the rail's 64: a first screen of 1216.
        let viewport = Size::new(1216.0, 820.0);
        assert_eq!(page_size(viewport), (798, 740));
        assert_eq!(
            sides(page_box(viewport)),
            sides(Rect::new(17.0, 63.0, 798.0, 740.0))
        );
        assert_eq!(
            sides(column(viewport)),
            sides(Rect::new(832.0, 0.0, 384.0, 820.0))
        );
        // The page's right edge, its border and the inset meet the column.
        assert_eq!(
            (17.0 + 798.0 + BORDER + INSET).to_bits(),
            column(viewport).left().to_bits()
        );
        // Its foot, its border and the inset meet the screen's foot.
        assert_eq!(
            (63.0 + 740.0 + BORDER + INSET).to_bits(),
            820.0_f32.to_bits()
        );
    }

    #[test]
    fn in_a_wide_window_the_page_takes_the_width_and_the_column_stays_384() {
        // 1680 × 1050 beside the rail: 1616.
        let viewport = Size::new(1616.0, 1050.0);
        assert_eq!(page_size(viewport), (1198, 970));
        assert_eq!(
            sides(column(viewport)),
            sides(Rect::new(1232.0, 0.0, 384.0, 1050.0))
        );
    }

    #[test]
    fn it_splits_from_a_page_720_wide_and_not_one_narrower() {
        let at_work = card(Some(Now::Acting));
        let least = Size::new(LEAST_ROOM, 820.0);
        assert_eq!(page_size(least), (LEAST_PAGE, 740));
        assert_eq!(Stands::of(least, Some(&at_work)), Stands::Split);
        let narrower = Size::new(LEAST_ROOM - 1.0, 820.0);
        assert_eq!(page_size(narrower).0, LEAST_PAGE - 1);
        assert_eq!(
            Stands::of(narrower, Some(&at_work)),
            Stands::InTheConversation
        );
        // There the page is the conversation's: the column's 880 less the
        // card's border.
        assert_eq!(
            Stands::InTheConversation.page_size(narrower),
            crate::card::PAGE_SIZE
        );
    }

    #[test]
    fn it_splits_only_while_the_agent_acts_or_asks() {
        let wide = Size::new(1216.0, 820.0);
        for (now, stands) in [
            (Some(Now::Acting), Stands::Split),
            (
                Some(Now::Asking {
                    what: "Press Send?".to_owned(),
                    paying: None,
                }),
                Stands::Split,
            ),
            (
                Some(Now::Done {
                    said: "Done.".to_owned(),
                }),
                Stands::InTheConversation,
            ),
            (
                Some(Now::Stuck {
                    why: "No.".to_owned(),
                    choices: vec!["Try".to_owned()],
                }),
                Stands::InTheConversation,
            ),
            (
                Some(Now::Stopped {
                    why: "You stopped it.".to_owned(),
                }),
                Stands::InTheConversation,
            ),
            (None, Stands::InTheConversation),
        ] {
            assert_eq!(
                Stands::of(wide, Some(&card(now.clone()))),
                stands,
                "{now:?}"
            );
        }
        assert_eq!(Stands::of(wide, None), Stands::InTheConversation);
    }

    #[test]
    fn split_the_page_box_is_seen_whole_whatever_was_scrolled_or_offered() {
        let viewport = Size::new(1216.0, 820.0);
        let whole = Some(sides(Rect::new(17.0, 63.0, 798.0, 740.0)));
        assert_eq!(
            Stands::Split.seen_page_box(viewport, 0.0, 0).map(sides),
            whole
        );
        assert_eq!(
            Stands::Split.seen_page_box(viewport, 300.0, 3).map(sides),
            whole
        );
        assert_eq!(
            sides(Stands::Split.page_box(viewport, -200.0)),
            sides(Rect::new(17.0, 63.0, 798.0, 740.0))
        );
        // In the conversation, as card_place has it.
        assert_eq!(
            sides(Stands::InTheConversation.page_box(Size::new(1040.0, 820.0), 0.0)),
            sides(Rect::new(81.0, 145.0, 878.0, 520.0))
        );
    }

    #[test]
    fn a_screen_too_small_for_it_has_a_page_one_pixel_across() {
        assert_eq!(page_size(Size::new(100.0, 50.0)), (1, 1));
    }
}
