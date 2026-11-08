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

/// Where a page that scrolls as the conversation does is seen (queue item
/// 880): the conversation's room, or *Settings*' — the whole first screen,
/// which has no box under it — each from its top, so long.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Room {
    /// How far down the first screen its top is, in CSS pixels.
    pub top: f32,
    /// How tall it is, in CSS pixels.
    pub height: f32,
}

impl Room {
    /// The conversation's, on a first screen laid out at `viewport`: from
    /// [`TOP`], [`room`] tall.
    #[must_use]
    pub fn conversation(viewport: Size) -> Self {
        Self {
            top: TOP,
            height: room(viewport),
        }
    }

    /// *Settings*', on a first screen laid out at `viewport`: the whole of
    /// it, from its top to its foot.
    #[must_use]
    pub fn settings(viewport: Size) -> Self {
        Self {
            top: 0.0,
            height: if viewport.height.is_finite() {
                viewport.height.clamp(0.0, MOST)
            } else {
                0.0
            },
        }
    }
}

/// Where the thing the keyboard is on stands, down from the top of the page
/// that scrolls (queue item 880): its top and its foot, so that it can be
/// scrolled into view as the keyboard moves to it. A renderer's word, so
/// read as finite lengths inside [`MOST`], a foot never above its top; a
/// renderer that lies about it can scroll only as far as the reach lets.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Span {
    /// How far down the page its top is.
    pub top: f32,
    /// How far down the page its foot is.
    pub foot: f32,
}

impl Span {
    /// The same span read as finite lengths from `0` to [`MOST`], its foot
    /// never above its top.
    #[must_use]
    pub fn bounded(self) -> Self {
        let side = |length: f32| {
            if length.is_finite() {
                length.clamp(0.0, MOST)
            } else {
                0.0
            }
        };
        let top = side(self.top);
        Self {
            top,
            foot: side(self.foot).max(top),
        }
    }
}

/// How far the conversation may be scrolled on a first screen laid out at
/// `viewport` whose turns reach as `reach` says: back up until the first
/// turn's top stands at the room's top, and on down until the newest turn's
/// foot stands at the room's foot — or not at all, when it fits.
pub fn bounds(reach: Reach, viewport: Size) -> (f32, f32) {
    bounds_in(reach, room(viewport))
}

/// [`bounds`] in a room `room` tall (queue item 880).
pub fn bounds_in(reach: Reach, room: f32) -> (f32, f32) {
    let reach = reach.bounded();
    (-reach.earlier, (reach.newest - room).max(0.0))
}

/// `down`, kept inside [`bounds`].
pub fn clamped(down: f32, reach: Reach, viewport: Size) -> f32 {
    clamped_in(down, reach, room(viewport))
}

/// `down`, kept inside [`bounds_in`] a room `room` tall.
pub fn clamped_in(down: f32, reach: Reach, room: f32) -> f32 {
    let (least, most) = bounds_in(reach, room);
    if down.is_finite() {
        down.clamp(least, most)
    } else {
        0.0
    }
}

/// How far a page in a room `room` tall, scrolled `down` and reaching as
/// `reach` says, is to be scrolled for `span` to be seen whole with
/// `margin` about it: as little as that takes — not at all when it is seen
/// already — and, for a span taller than the room, its top at the room's
/// top. Kept inside [`bounds_in`].
pub fn into_view(down: f32, span: Span, margin: f32, reach: Reach, room: f32) -> f32 {
    let span = span.bounded();
    let (top, foot) = (span.top - margin, span.foot + margin);
    let wanted = if top < down || foot - top > room {
        top
    } else if foot > down + room {
        foot - room
    } else {
        down
    };
    clamped_in(wanted, reach, room)
}

/// The narrowest a sign of how far the conversation is scrolled is drawn
/// down the room, however long the conversation: a thumb a person can see.
pub const BAR_LEAST: f32 = 32.0;

/// The sign that the conversation goes on above or below the room (queue
/// item 755, ADR 0093 § 2): a bar down the room's right gutter, as long
/// against the room as the room is against the whole conversation, and as
/// far down it as the conversation is scrolled. Computed by the browser
/// process from its own numbers ([`bar`]), and sent to be drawn; never a
/// renderer's word.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// How far below the room's top it starts, in CSS pixels.
    pub top: f32,
    /// How long it is, in CSS pixels.
    pub length: f32,
    /// Whether the conversation goes on above the room: it is scrolled on
    /// from the first turn's top.
    pub above: bool,
    /// Whether it goes on below: it is scrolled back from the newest turn's
    /// foot.
    pub below: bool,
    /// How much of it is seen, from `1` to `0` (queue item 793): whole
    /// while the conversation scrolls and for a second after, then faded
    /// out, and whole again while the pointer is near it — the browser
    /// process's to say (`alo-window`'s `conversation_bar`). Faded, it is still there to an agent.
    pub seen: f32,
}

/// How far in from the clip's right edge the bar stands, in CSS pixels:
/// centred in the clip's gutter of [`crate::card_place::GUTTER`].
pub const BAR_INSET: f32 = 13.0;

/// How wide the bar is, in CSS pixels.
pub const BAR_WIDTH: f32 = 6.0;

/// Where the bar's left edge is on a first screen laid out at `viewport`,
/// in CSS pixels from its left: the clip is the column and a gutter either
/// side, and the bar [`BAR_INSET`] in from its right edge.
pub fn bar_left(viewport: Size) -> f32 {
    crate::card_place::column_left(viewport)
        + crate::card_place::column(viewport)
        + crate::card_place::GUTTER
        - BAR_INSET
        - BAR_WIDTH
}

impl Bar {
    /// How far down the conversation it is scrolled, from `0` at the first
    /// turn's top to `100` at the newest turn's foot, as a whole number.
    pub fn percent(self, room: f32) -> u32 {
        let travel = room - self.length;
        if travel <= 0.0 || !travel.is_finite() {
            return 0;
        }
        let along = (self.top / travel).clamp(0.0, 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a fraction clamped to 0..=1, times 100, rounded: 0 to 100"
        )]
        let percent = (along * 100.0).round() as u32;
        percent
    }

    /// What an agent reads of it: which way the conversation goes on, and
    /// how far down it is scrolled.
    pub fn said(self, room: f32) -> String {
        self.said_of("The conversation", room)
    }

    /// What an agent reads of it on `what` — the conversation, or *Settings*
    /// (queue item 880) — in a room `room` tall.
    pub fn said_of(self, what: &str, room: f32) -> String {
        let goes = match (self.above, self.below) {
            (true, true) => "above and below",
            (true, false) => "above",
            (false, true) => "below",
            (false, false) => "nowhere else",
        };
        format!(
            "{what} goes on {goes}; scrolled {}% of the way down",
            self.percent(room)
        )
    }
}

/// The bar for a conversation scrolled `down` on a first screen laid out at
/// `viewport` on a screen of `scale`, whose turns reach as `reach` says —
/// its top and length each on a device pixel, as Chrome puts a scroll bar's
/// thumb — or none when the turns fit the room, and there is nowhere else
/// to go.
pub fn bar(down: f32, reach: Reach, viewport: Size, scale: f32) -> Option<Bar> {
    bar_in(down, reach, room(viewport), scale)
}

/// [`bar`] in a room `room` tall (queue item 880), its top down from the
/// room's top.
pub fn bar_in(down: f32, reach: Reach, room: f32, scale: f32) -> Option<Bar> {
    let scale = alo_value::device_scale(scale);
    let on_a_pixel = |length: f32| (length * scale).round() / scale;
    let (least, most) = bounds_in(reach, room);
    let travel = most - least;
    if room <= 0.0 || travel < 1.0 || !travel.is_finite() {
        return None;
    }
    let down = clamped_in(down, reach, room);
    let length = (room * room / (room + travel)).clamp(BAR_LEAST.min(room), room);
    let along = (down - least) / travel;
    Some(Bar {
        top: on_a_pixel(along * (room - length)),
        length: on_a_pixel(length),
        above: down > least,
        below: down < most,
        seen: 1.0,
    })
}

/// How far the turns of `document`, drawn as `drawing`, reach: the
/// earlier turns' box, with the gap under it, and the newest turn's —
/// nothing for a first screen with no turns.
pub fn reach_of(drawing: &Drawing, document: &Document) -> Reach {
    let height = |id: &str| {
        let node = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "id") == Some(id))?;
        laid_out(drawing, node).map(|(_, height)| height)
    };
    Reach {
        earlier: height(EARLIER).map_or(0.0, |earlier| earlier + BETWEEN_TURNS),
        newest: height(NEWEST).unwrap_or(0.0),
    }
    .bounded()
}

/// The class a thing the keyboard is on carries, ringed (queue item 856).
pub const KEYED: &str = "keyed";

/// Where the thing the keyboard is on — the element of the class [`KEYED`]
/// — stands in `document`, drawn as `drawing`, down from the top of the
/// page that scrolls, the element [`NEWEST`] (queue item 880): none when
/// nothing is keyed.
pub fn keyed_of(drawing: &Drawing, document: &Document) -> Option<Span> {
    let of = |wanted: &dyn Fn(alo_dom::NodeId) -> bool| {
        document
            .descendants(document.root())
            .find(|&node| wanted(node))
            .and_then(|node| laid_out(drawing, node))
    };
    let page = of(&|node| document.attribute_by_name(node, "id") == Some(NEWEST))?;
    let keyed = of(&|node| {
        document
            .attribute_by_name(node, "class")
            .is_some_and(|class| class.split_ascii_whitespace().any(|each| each == KEYED))
    })?;
    Some(
        Span {
            top: keyed.0 - page.0,
            foot: keyed.0 + keyed.1 - page.0,
        }
        .bounded(),
    )
}

/// The top and the height of the first box `node` made, as `drawing` laid
/// it out.
fn laid_out(drawing: &Drawing, node: alo_dom::NodeId) -> Option<(f32, f32)> {
    let boxes = &drawing.boxes;
    boxes
        .ids()
        .filter(|&made| boxes.get(made).and_then(|held| held.kind.node()) == Some(node))
        .find_map(|made| drawing.layout.get(made))
        .map(|geometry| (geometry.border_box.top(), geometry.border_box.size.height))
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
    fn the_bar_is_as_long_as_the_room_is_against_the_whole_and_as_far_down_as_it_is_scrolled() {
        let viewport = Size::new(1040.0, 820.0);
        // Room 676; the earlier turns 900 above, the newest 1000 tall: it
        // scrolls from -900 to 324, 1224 in all, the whole 1900.
        let reach = Reach {
            earlier: 900.0,
            newest: 1000.0,
        };
        // 676 × 676 / 1900 is 240.51 long; at the newest turn's top, 900 of
        // 1224 down: 320.21 of the 435.49 it travels. Each on a device pixel.
        let at = |down: f32| bar(down, reach, viewport, 1.0).expect("a bar");
        let newest = at(0.0);
        assert_eq!(
            (newest.top.to_bits(), newest.length.to_bits()),
            (320.0_f32.to_bits(), 241.0_f32.to_bits()),
            "{newest:?}"
        );
        assert!(newest.above && newest.below);
        let at_two = bar(0.0, reach, viewport, 2.0).expect("a bar");
        assert_eq!(
            (at_two.top.to_bits(), at_two.length.to_bits()),
            (320.0_f32.to_bits(), 240.5_f32.to_bits()),
            "{at_two:?}"
        );
        let first = at(-900.0);
        assert_eq!(
            first.top.to_bits(),
            0.0_f32.to_bits(),
            "the first turn's top"
        );
        assert!(!first.above && first.below);
        let last = at(324.0);
        assert_eq!(
            (last.top + last.length).to_bits(),
            676.0_f32.to_bits(),
            "at the room's foot"
        );
        assert!(last.above && !last.below);
        // Past either end, kept at it.
        assert_eq!(at(-5000.0), first);
        assert_eq!(first.percent(676.0), 0);
        assert_eq!(last.percent(676.0), 100);
        assert_eq!(newest.percent(676.0), 74);
        assert_eq!(
            newest.said(676.0),
            "The conversation goes on above and below; scrolled 74% of the way down"
        );
        assert_eq!(
            first.said(676.0),
            "The conversation goes on below; scrolled 0% of the way down"
        );
        // Turns that fit draw none; a long one is never shorter than 32.
        let fits = Reach {
            earlier: 0.0,
            newest: 600.0,
        };
        assert_eq!(bar(0.0, fits, viewport, 1.0), None);
        let long = Reach {
            earlier: 100_000.0,
            newest: 600.0,
        };
        assert_eq!(
            bar(0.0, long, viewport, 1.0).map(|bar| bar.length.to_bits()),
            Some(BAR_LEAST.to_bits())
        );
        assert_eq!(
            bar(0.0, reach, Size::new(1040.0, 100.0), 1.0),
            None,
            "no room"
        );
    }

    #[test]
    fn settings_room_is_the_whole_screen_and_the_conversations_from_40_to_the_box() {
        let viewport = Size::new(536.0, 600.0);
        assert_eq!(
            Room::settings(viewport),
            Room {
                top: 0.0,
                height: 600.0
            }
        );
        assert_eq!(
            Room::conversation(Size::new(1040.0, 820.0)),
            Room {
                top: 40.0,
                height: 676.0
            }
        );
        assert_eq!(
            Room::settings(Size::new(536.0, f32::INFINITY))
                .height
                .to_bits(),
            0.0_f32.to_bits()
        );
        // A page 838 tall in it scrolls 238; a bar says where on Settings.
        let reach = Reach {
            earlier: 0.0,
            newest: 838.0,
        };
        assert_eq!(bits(bounds_in(reach, 600.0)), bits((-0.0, 238.0)));
        let bar = bar_in(238.0, reach, 600.0, 1.0).expect("a bar");
        assert_eq!(
            bar.said_of("Settings", 600.0),
            "Settings goes on above; scrolled 100% of the way down"
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
