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
//! The first screen does not scroll yet. When a conversation holds more than
//! one card (703), where each is moves with the conversation, and this is
//! where that is decided. Until then, the conversation is clipped above the
//! box, which is pinned at the window's foot (queue item 750): what of the
//! page box is under that edge is not seen, and is not the page's to be
//! pressed ([`seen_page_box`]).

use crate::card::PAGE_SIZE;
use alo_layout::{Rect, Size};

/// How wide the column holding what the person said and the card is: the
/// design's 880.
pub const COLUMN: f32 = 880.0;

/// How far down the column begins: `.start.conversing`'s padding.
const TOP: f32 = 40.0;

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

/// The gap between the conversation and the box: the box's `margin-top`.
const ABOVE_THE_BOX: f32 = 24.0;

/// Where the box's top is on a first screen laid out at `viewport` that holds
/// a conversation (queue item 750): [`FOOT`] and [`BOX_HEIGHT`] above its
/// foot.
pub fn box_top(viewport: Size) -> f32 {
    viewport.height - FOOT - BOX_HEIGHT
}

/// How far down the conversation is seen on a first screen laid out at
/// `viewport`: what is lower is clipped, above the box and the gap over it.
pub fn room_bottom(viewport: Size) -> f32 {
    box_top(viewport) - ABOVE_THE_BOX
}

/// The part of the card's page box that is seen on a first screen laid out
/// at `viewport` — all of it when the window is tall enough, its top part
/// when the conversation is clipped above the box — or [`None`] when none
/// of it is.
pub fn seen_page_box(viewport: Size) -> Option<Rect> {
    let page = page_box(viewport);
    let seen = (room_bottom(viewport) - page.top()).min(page.size.height);
    (seen > 0.0).then(|| Rect::new(page.left(), page.top(), page.size.width, seen))
}

/// The card's page box on a first screen laid out at `viewport`, in its CSS
/// pixels: [`PAGE_SIZE`], inside the card's border, under its bar.
pub fn page_box(viewport: Size) -> Rect {
    let (width, height) = PAGE_SIZE;
    Rect::new(
        (viewport.width - COLUMN) / 2.0 + BORDER,
        TOP + SAID + GAP + BORDER + BAR,
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
            sides(page_box(Size::new(1040.0, 820.0))),
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
            seen_page_box(Size::new(1040.0, 820.0)).map(sides),
            Some(sides(Rect::new(81.0, 145.0, 878.0, 520.0)))
        );
        // At 700 it ends at 596: the page's lowest 69 are clipped.
        assert_eq!(
            seen_page_box(Size::new(1040.0, 700.0)).map(sides),
            Some(sides(Rect::new(81.0, 145.0, 878.0, 451.0)))
        );
        // At 240 none of it is seen.
        assert_eq!(seen_page_box(Size::new(1040.0, 240.0)), None);
    }

    #[test]
    fn it_is_centred_whatever_the_width_and_as_far_down() {
        assert_eq!(
            sides(page_box(Size::new(1400.0, 900.0))),
            sides(Rect::new(261.0, 145.0, 878.0, 520.0))
        );
        assert_eq!(
            sides(page_box(Size::new(1041.0, 820.0))),
            sides(Rect::new(81.5, 145.0, 878.0, 520.0))
        );
    }
}
