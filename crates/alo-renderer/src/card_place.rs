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
//! lays it out with a card — the column [`COLUMN`] wide, centred; the
//! person's line, one line tall; under it the card, its border and its bar;
//! then the page box, [`PAGE_SIZE`] — and `alo-renderer`'s card test holds
//! this to the layout the engine computes, at more than one width, so the
//! sheet and this cannot drift apart unnoticed.
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

/// How wide the column holding what the person said and the card is: the
/// design's 880.
pub const COLUMN: f32 = 880.0;

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
/// pixels, the conversation scrolled `down`: [`PAGE_SIZE`], inside the
/// card's border, under its bar, the card being the newest turn.
pub fn page_box(viewport: Size, down: f32) -> Rect {
    let (width, height) = PAGE_SIZE;
    Rect::new(
        (viewport.width - COLUMN) / 2.0 + BORDER,
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
