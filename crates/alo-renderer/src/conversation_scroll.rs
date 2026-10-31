/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How far the conversation scrolls (ADR 0085 § 3, queue item 703).
//!
//! # Scrolled from the newest turn
//!
//! The browser process holds how far the conversation is scrolled as one
//! number, [`crate::start::Start::down`]: how far **the newest turn's top**
//! is above the top of the room the conversation is seen in. At `0` the
//! newest turn's top stands at the room's top, which is where a new turn
//! puts it; above `0` the person has scrolled on down into the newest turn;
//! below `0`, back up into the earlier ones, which stand above the newest.
//!
//! Measured from the newest turn and not from the first, because a live
//! card is only ever the newest turn: where its page box is, which decides
//! where on the page a person's press lands (ADR 0024 § 5), is then the
//! browser process's own arithmetic — [`crate::card_place::page_box`] moved
//! up by `down` — and never a renderer's word about how tall the turns
//! above it came out.
//!
//! # How far it may go: the renderer's reach
//!
//! How tall the earlier turns and the newest one are is text laid out, which
//! only the renderer does. So the first screen's renderer answers
//! [`crate::ToRenderer::ConversationReach`] with a [`Reach`] measured from
//! its own drawing ([`reach_of`]), and the browser process keeps a person's
//! wheel inside it ([`bounds`]). A renderer that lies about it can let the
//! conversation scroll onto empty paper, or stop it short: never move where
//! a press on the page goes. What it says is read as numbers, finite and
//! inside [`MOST`], and nothing else.

use crate::card_place::{TOP, room_bottom};
use crate::pipeline::Drawing;
use alo_dom::Document;
use alo_layout::Size;

/// The most the conversation is ever scrolled either way, and the most a
/// reach is read as: past any conversation a person keeps on one screen.
pub const MOST: f32 = 1_000_000.0;

/// The gap between two turns, the earlier turns' `margin-bottom` and
/// `row-gap` in the sheet (`interface/start.css`).
pub const BETWEEN_TURNS: f32 = 32.0;

/// The id of the box holding every turn but the newest.
pub const EARLIER: &str = "earlier";

/// The id of the newest turn.
pub const NEWEST: &str = "newest";

/// How far the conversation reaches either side of the newest turn's top,
/// in CSS pixels, as the renderer laid it out.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reach {
    /// How tall the earlier turns are, with the gap under the last of them:
    /// how far above the newest turn's top the first turn's top is. `0`
    /// with none.
    pub earlier: f32,
    /// How tall the newest turn is.
    pub newest: f32,
}

impl Reach {
    /// The same reach, each side a finite length from `0` to [`MOST`]: what
    /// is read of a renderer's answer.
    #[must_use]
    pub fn bounded(self) -> Self {
        let side = |length: f32| {
            if length.is_finite() {
                length.clamp(0.0, MOST)
            } else {
                0.0
            }
        };
        Self {
            earlier: side(self.earlier),
            newest: side(self.newest),
        }
    }
}

/// How tall the room the conversation is seen in is, on a first screen laid
/// out at `viewport`: from [`TOP`] down to the gap over the box. The wheel is
/// kept inside it as it is with nothing offered: offers over the box come
/// and go as a person types, and the conversation does not move for them.
pub fn room(viewport: Size) -> f32 {
    (room_bottom(viewport, 0) - TOP).max(0.0)
}

/// How far the conversation may be scrolled on a first screen laid out at
/// `viewport` whose turns reach as `reach` says: back up until the first
/// turn's top stands at the room's top, and on down until the newest turn's
/// foot stands at the room's foot — or not at all, when it fits.
pub fn bounds(reach: Reach, viewport: Size) -> (f32, f32) {
    let reach = reach.bounded();
    (-reach.earlier, (reach.newest - room(viewport)).max(0.0))
}

/// `down`, kept inside [`bounds`].
pub fn clamped(down: f32, reach: Reach, viewport: Size) -> f32 {
    let (least, most) = bounds(reach, viewport);
    if down.is_finite() {
        down.clamp(least, most)
    } else {
        0.0
    }
}

/// How far the turns of `document`, drawn as `drawing`, reach: the
/// earlier turns' box, with the gap under it, and the newest turn's —
/// nothing for a first screen with no turns.
pub fn reach_of(drawing: &Drawing, document: &Document) -> Reach {
    let height = |id: &str| {
        let node = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "id") == Some(id))?;
        let boxes = &drawing.boxes;
        boxes
            .ids()
            .filter(|&made| boxes.get(made).and_then(|held| held.kind.node()) == Some(node))
            .find_map(|made| drawing.layout.get(made))
            .map(|geometry| geometry.border_box.size.height)
    };
    Reach {
        earlier: height(EARLIER).map_or(0.0, |earlier| earlier + BETWEEN_TURNS),
        newest: height(NEWEST).unwrap_or(0.0),
    }
    .bounded()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits((least, most): (f32, f32)) -> (u32, u32) {
        (least.to_bits(), most.to_bits())
    }

    #[test]
    fn the_room_runs_from_40_to_the_gap_over_the_box() {
        // At 820: the box at 740, the room from 40 to 716.
        assert_eq!(
            room(Size::new(1040.0, 820.0)).to_bits(),
            676.0_f32.to_bits()
        );
        assert_eq!(room(Size::new(1040.0, 100.0)).to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn it_scrolls_back_to_the_first_turn_and_on_to_the_newest_ones_foot() {
        let viewport = Size::new(1040.0, 820.0);
        let reach = Reach {
            earlier: 300.0,
            newest: 626.0,
        };
        // The newest fits in 676: nothing further down.
        assert_eq!(bits(bounds(reach, viewport)), bits((-300.0, 0.0)));
        let tall = Reach {
            earlier: 0.0,
            newest: 1000.0,
        };
        assert_eq!(bits(bounds(tall, viewport)), bits((-0.0, 324.0)));
        assert_eq!(
            clamped(-500.0, reach, viewport).to_bits(),
            (-300.0_f32).to_bits()
        );
        assert_eq!(clamped(40.0, reach, viewport).to_bits(), 0.0_f32.to_bits());
        assert_eq!(
            clamped(-120.0, reach, viewport).to_bits(),
            (-120.0_f32).to_bits()
        );
        assert_eq!(
            clamped(f32::NAN, reach, viewport).to_bits(),
            0.0_f32.to_bits()
        );
    }

    #[test]
    fn a_reach_is_read_as_finite_lengths_inside_the_most() {
        let lied = Reach {
            earlier: f32::INFINITY,
            newest: -5.0,
        }
        .bounded();
        assert_eq!(lied, Reach::default());
        let large = Reach {
            earlier: 1e30,
            newest: f32::NAN,
        }
        .bounded();
        assert_eq!(large.earlier.to_bits(), MOST.to_bits());
        assert_eq!(large.newest.to_bits(), 0.0_f32.to_bits());
    }
}
