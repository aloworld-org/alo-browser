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
//! where that is decided.

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
