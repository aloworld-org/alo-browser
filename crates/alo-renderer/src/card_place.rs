/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the card's page box is on the first screen (ADR 0076 B, ADR 0024
//! § 5, queue item 718): geometry the browser process reads from its own
//! composition, never from a renderer.
//!
//! A person's press, wheel and keys over the card's picture of the page
//! reach the page itself, in Chrome, as a person's input. Which point of
//! the page a point of the window is, is the browser process's to decide
//! (ADR 0024 § 5): a renderer that said where its page box was could send a
//! person's press anywhere on the page. So the box is computed here from
//! the first screen's viewport alone, as the sheet (`interface/start.css`)
//! lays it out with a card — the column [`column`] wide, centred; the
//! person's line, one line tall; under it the card, its border and its bar;
//! then the page box, [`page_size`] — and `alo-renderer`'s card test holds
//! this to the layout the engine computes, at more than one width, so the
//! sheet and this cannot drift apart unnoticed.
//!
//! # Narrow
//!
//! The column is the design's [`COLUMN`] where the room allows it and
//! [`GUTTER`] either side of it, and otherwise the room less a gutter
//! either side (queue item 774): never under the panel, never past the
//! window's right edge. So the card's page box is narrower in a narrow
//! window, and Chrome is asked to lay the page out at that narrower size
//! ([`page_size`]), one CSS pixel of the page still one of the first
//! screen's.
//!
//! # Scrolled
//!
//! The conversation scrolls (queue item 703, ADR 0085), and only the newest
//! turn is ever a live card, so the page box is where it would be with the
//! newest turn's top at the room's top, moved up by how far the browser
//! process scrolled it ([`crate::conversation_scroll`]): its own number, so
//! where a press lands on the page is still its own arithmetic. The
//! conversation is clipped to the room, from [`TOP`] down to the gap over
//! the box, which is pinned at the window's foot (queue item 750): what of
//! the page box is outside the room is not seen, and is not the page's to
//! be pressed ([`seen_page_box`]).
//!
//! # While a person types
//!
//! What is offered for what was typed stands just above the box while
//! conversing (queue item 750), and the room is shorter by it: the offers'
//! card, [`offers_height`] for how many are offered, and the 8 under it and
//! 24 over it (queue item 759). The browser process knows how many it
//! offered, so it knows how much shorter, without a renderer saying so; a
//! press on an offer drawn over the card's page is the offer's.

use crate::card::PAGE_SIZE;
use alo_layout::{Rect, Size};

/// How wide the column holding what the person said and the card is at
/// most: the design's 880.
pub const COLUMN: f32 = 880.0;

/// How far in from either side of the room the column always is: the 32
/// the conversation's clip is wider than the column either side, so the
/// card's shadow is not cut (queue items 750, 774).
pub const GUTTER: f32 = 32.0;

/// How wide the column is on a first screen laid out at `viewport`: the
/// design's [`COLUMN`], or the room less [`GUTTER`] either side when that is
/// less (queue item 774), and never less than nothing. The box at the
/// window's foot and the offers over it are as wide.
pub fn column(viewport: Size) -> f32 {
    (viewport.width - 2.0 * GUTTER).clamp(0.0, COLUMN)
}

/// Where the column's left edge is on a first screen laid out at
/// `viewport`: the column centred in the room.
pub fn column_left(viewport: Size) -> f32 {
    (viewport.width - column(viewport)) / 2.0
}

/// The size of the card's page box on a first screen laid out at
/// `viewport`, in CSS pixels: the column less the card's border on each
/// side, and the design's 520 of page — [`PAGE_SIZE`] wherever the room
/// holds the whole column. What Chrome lays the page out at while it is a
/// card, and what its frames are asked for at, times the window's scale
/// (ADR 0076 B, queue item 774). The first screen's viewport is a whole
/// number of CSS pixels, so a narrower column is one too; at least one
/// pixel across, since Chrome lays out no page of none.
pub fn page_size(viewport: Size) -> (u32, u32) {
    let inside = (column(viewport) - 2.0 * BORDER).floor().max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole, positive width of at most the design's 878, exact in a u32"
    )]
    let width = inside as u32;
    (width.min(PAGE_SIZE.0), PAGE_SIZE.1)
}

/// How far down the column begins, and the room the conversation is seen
/// in with it: `.start.conversing`'s padding.
pub const TOP: f32 = 40.0;

/// How tall the person's line is: its 22 of one line, nowrap, and its 10
/// above and below.
const SAID: f32 = 42.0;

/// The gap between the person's line and the card.
const GAP: f32 = 16.0;

/// The card's border.
const BORDER: f32 = 1.0;

/// The card's bar, its line under it included.
const BAR: f32 = 46.0;

/// The box's height once the conversation holds something: the design's 56.
pub const BOX_HEIGHT: f32 = 56.0;

/// How far above the window's foot the box is: the design's 24.
pub const FOOT: f32 = 24.0;

/// The gap between the conversation and the box: the box's `margin-top`,
/// or the offers' when they stand between.
const ABOVE_THE_BOX: f32 = 24.0;

/// How tall one offer is: its two lines, 20 and 18, and 7 above and below.
const OFFER: f32 = 52.0;

/// The offers' card around them: its 6 inside and its 1 of border, above
/// and below.
const OFFERS_FRAME: f32 = 14.0;

/// The gap between the offers and the box under them.
const OVER_THE_BOX: f32 = 8.0;

/// How tall the offers' card is for `offered` offers, and nothing for none,
/// when none is drawn.
pub fn offers_height(offered: usize) -> f32 {
    if offered == 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a handful of offers, exact in an f32"
    )]
    let offered = offered as f32;
    OFFERS_FRAME + OFFER * offered
}

/// Where the box's top is on a first screen laid out at `viewport` that holds
/// a conversation (queue item 750): [`FOOT`] and [`BOX_HEIGHT`] above its
/// foot.
pub fn box_top(viewport: Size) -> f32 {
    viewport.height - FOOT - BOX_HEIGHT
}

/// How far down the conversation is seen on a first screen laid out at
/// `viewport`, `offered` offers standing over the box (queue item 759):
/// what is lower is clipped, above the box and the gap over it, or above
/// the offers, the gap under them and the gap over them.
pub fn room_bottom(viewport: Size, offered: usize) -> f32 {
    if offered == 0 {
        box_top(viewport) - ABOVE_THE_BOX
    } else {
        box_top(viewport) - OVER_THE_BOX - offers_height(offered) - ABOVE_THE_BOX
    }
}

/// The part of the card's page box that is seen on a first screen laid out
/// at `viewport`, scrolled `down` and with `offered` offers over the box —
/// all of it when it is inside the room, what is inside when the room clips
/// it above or below — or [`None`] when none of it is.
pub fn seen_page_box(viewport: Size, down: f32, offered: usize) -> Option<Rect> {
    let page = page_box(viewport, down);
    let top = page.top().max(TOP);
    let bottom = page.bottom().min(room_bottom(viewport, offered));
    (bottom > top).then(|| Rect::new(page.left(), top, page.size.width, bottom - top))
}

/// The card's page box on a first screen laid out at `viewport`, in its CSS
/// pixels, the conversation scrolled `down`: [`page_size`], inside the
/// card's border, under its bar, the card being the newest turn.
pub fn page_box(viewport: Size, down: f32) -> Rect {
    let (width, height) = page_size(viewport);
    Rect::new(
        column_left(viewport) + BORDER,
        TOP + SAID + GAP + BORDER + BAR - down,
        length(width),
        length(height),
    )
}

/// A page box's side as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "the page box's sides are in the hundreds, exact in an f32"
)]
fn length(pixels: u32) -> f32 {
    pixels as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rectangle's sides as their bits, compared without comparing
    /// floats.
    fn sides(rect: Rect) -> [u32; 4] {
        [rect.left(), rect.top(), rect.size.width, rect.size.height].map(f32::to_bits)
    }

    #[test]
    fn at_the_designs_width_the_page_box_is_at_81_145() {
        assert_eq!(
            sides(page_box(Size::new(1040.0, 820.0), 0.0)),
            sides(Rect::new(81.0, 145.0, 878.0, 520.0))
        );
    }

    #[test]
    fn the_page_box_is_seen_down_to_the_room_above_the_box() {
        // At the design's 820 the room ends at 716, under the page's 665.
        assert_eq!(
            box_top(Size::new(1040.0, 820.0)).to_bits(),
            740.0_f32.to_bits()
        );
        assert_eq!(
            seen_page_box(Size::new(1040.0, 820.0), 0.0, 0).map(sides),
            Some(sides(Rect::new(81.0, 145.0, 878.0, 520.0)))
        );
        // At 700 it ends at 596: the page's lowest 69 are clipped.
        assert_eq!(
            seen_page_box(Size::new(1040.0, 700.0), 0.0, 0).map(sides),
            Some(sides(Rect::new(81.0, 145.0, 878.0, 451.0)))
        );
        // At 240 none of it is seen.
        assert_eq!(seen_page_box(Size::new(1040.0, 240.0), 0.0, 0), None);
    }

    #[test]
    fn scrolled_the_page_box_moves_by_exactly_as_much_and_is_seen_inside_the_room() {
        let viewport = Size::new(1040.0, 820.0);
        // Scrolled 120 on: up by 120, its top 15 above the room's 40 not
        // seen.
        assert_eq!(
            sides(page_box(viewport, 120.0)),
            sides(Rect::new(81.0, 25.0, 878.0, 520.0))
        );
        assert_eq!(
            seen_page_box(viewport, 120.0, 0).map(sides),
            Some(sides(Rect::new(81.0, 40.0, 878.0, 505.0)))
        );
        // Scrolled 200 back: down by 200, its foot past the room's 716.
        assert_eq!(
            sides(page_box(viewport, -200.0)),
            sides(Rect::new(81.0, 345.0, 878.0, 520.0))
        );
        assert_eq!(
            seen_page_box(viewport, -200.0, 0).map(sides),
            Some(sides(Rect::new(81.0, 345.0, 878.0, 371.0)))
        );
        // Scrolled past it either way, none of it.
        assert_eq!(seen_page_box(viewport, 700.0, 0), None);
        assert_eq!(seen_page_box(viewport, -700.0, 0), None);
    }

    #[test]
    fn offers_over_the_box_shorten_the_room_by_their_card_and_its_gaps() {
        let viewport = Size::new(1040.0, 820.0);
        assert_eq!(offers_height(0).to_bits(), 0.0_f32.to_bits());
        assert_eq!(offers_height(1).to_bits(), 66.0_f32.to_bits());
        assert_eq!(offers_height(3).to_bits(), 170.0_f32.to_bits());
        assert_eq!(room_bottom(viewport, 0).to_bits(), 716.0_f32.to_bits());
        // Three offers: the box at 740, 8 over it the card's foot at 732,
        // its top 170 above at 562, and the room 24 above that, at 538.
        assert_eq!(room_bottom(viewport, 3).to_bits(), 538.0_f32.to_bits());
        // Two, as for words not an address: 104 + 14 = 118 tall, from 614.
        assert_eq!(room_bottom(viewport, 2).to_bits(), 590.0_f32.to_bits());
        // So the card's page box, 145 to 665 unscrolled, is seen down to
        // 538 only, and a press at 600 is on the offers, not the page.
        assert_eq!(
            seen_page_box(viewport, 0.0, 3).map(sides),
            Some(sides(Rect::new(81.0, 145.0, 878.0, 393.0)))
        );
        assert!(
            seen_page_box(viewport, 0.0, 3)
                .is_some_and(|seen| !seen.contains(alo_layout::Point::new(400.0, 600.0)))
        );
        assert!(
            seen_page_box(viewport, 0.0, 0)
                .is_some_and(|seen| seen.contains(alo_layout::Point::new(400.0, 600.0)))
        );
    }

    #[test]
    fn in_a_narrow_room_the_column_is_the_room_less_a_gutter_either_side() {
        // The window 967 wide beside the panel's 240: a room of 727, the
        // column 663 from 32, and the page box inside its border.
        let narrow = Size::new(727.0, 820.0);
        assert_eq!(column(narrow).to_bits(), 663.0_f32.to_bits());
        assert_eq!(column_left(narrow).to_bits(), 32.0_f32.to_bits());
        assert_eq!(page_size(narrow), (661, 520));
        assert_eq!(
            sides(page_box(narrow, 0.0)),
            sides(Rect::new(33.0, 145.0, 661.0, 520.0))
        );
        // Beside the rail's 64 the same window has 903: 839 from 32.
        let railed = Size::new(903.0, 820.0);
        assert_eq!(column(railed).to_bits(), 839.0_f32.to_bits());
        assert_eq!(page_size(railed), (837, 520));
        // 720 wide beside the panel: 480, the column 416.
        let smaller = Size::new(480.0, 820.0);
        assert_eq!(column(smaller).to_bits(), 416.0_f32.to_bits());
        assert_eq!(page_size(smaller), (414, 520));
        // Just wide enough for the design's column and its gutters, and
        // one short of it.
        assert_eq!(
            column(Size::new(944.0, 820.0)).to_bits(),
            880.0_f32.to_bits()
        );
        assert_eq!(page_size(Size::new(944.0, 820.0)), PAGE_SIZE);
        assert_eq!(
            column(Size::new(943.0, 820.0)).to_bits(),
            879.0_f32.to_bits()
        );
        assert_eq!(page_size(Size::new(943.0, 820.0)), (877, 520));
        // A room narrower than the gutters has no column, and a page one
        // pixel across.
        assert_eq!(column(Size::new(40.0, 820.0)).to_bits(), 0.0_f32.to_bits());
        assert_eq!(page_size(Size::new(40.0, 820.0)), (1, 520));
    }

    #[test]
    fn at_the_panels_fold_and_the_least_window_the_column_holds_the_tab_row() {
        // Queue item 775: 760 wide beside the panel's 240, a room of 520;
        // 759 beside the rail it folds to, 695; the least window, 600
        // beside the rail, 536. The tab row ends 444.75 from the room's
        // left, 412.75 from the column's, inside each.
        for (room, wide, page) in [
            (520.0, 456.0, 454),
            (695.0, 631.0, 629),
            (536.0, 472.0, 470),
        ] {
            let viewport = Size::new(room, 820.0);
            assert_eq!(column(viewport).to_bits(), f32::to_bits(wide), "{room}");
            assert_eq!(column_left(viewport).to_bits(), GUTTER.to_bits(), "{room}");
            assert_eq!(page_size(viewport), (page, 520), "{room}");
            assert_eq!(
                sides(page_box(viewport, 0.0)),
                sides(Rect::new(33.0, 145.0, length(page), 520.0)),
                "{room}"
            );
            assert!(column_left(viewport) + column(viewport) >= 444.75, "{room}");
        }
    }

    #[test]
    fn at_the_designs_width_and_wider_the_column_is_the_designs() {
        for width in [1040.0, 1041.0, 1216.0, 1400.0] {
            let viewport = Size::new(width, 820.0);
            assert_eq!(column(viewport).to_bits(), COLUMN.to_bits(), "{width}");
            assert_eq!(page_size(viewport), PAGE_SIZE, "{width}");
        }
    }

    #[test]
    fn it_is_centred_whatever_the_width_and_as_far_down() {
        assert_eq!(
            sides(page_box(Size::new(1400.0, 900.0), 0.0)),
            sides(Rect::new(261.0, 145.0, 878.0, 520.0))
        );
        assert_eq!(
            sides(page_box(Size::new(1041.0, 820.0), 0.0)),
            sides(Rect::new(81.5, 145.0, 878.0, 520.0))
        );
    }
}
