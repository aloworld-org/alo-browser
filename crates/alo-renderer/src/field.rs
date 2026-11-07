/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the caret and the selection stand in one of alo's own one-line
//! fields — the one box and *Search chats* — as the browser process sends
//! it, and where the field's words were laid out, as its renderer answers
//! (queue item 861, `docs/design/shell.md`, *The one box as a text field*).
//!
//! The browser process holds what is typed and every edit of it
//! (`alo-window`'s `text_field`); a renderer is sent the words with an
//! [`Editing`] and draws them ([`crate::field_line`]): the caret, the
//! selection, an input method's composition, and the words scrolled so the
//! caret stays in view. It answers with a [`FieldLaidOut`] — where each
//! place the caret may stand is, the room the words had and how far they
//! were scrolled — which is what the browser process places a press's
//! caret by, keeps the scroll in, and puts an input method's candidates
//! under. Nothing here is a verb: an agent reads the field's value and its
//! selection as a range of characters, never a place (ADR 0002).
//!
//! # Characters
//!
//! Every place is counted in **characters** — Unicode scalar values — from
//! the field's start, as the browser process counts them. A renderer clamps
//! what it is sent to the words it holds, so no number from the wire indexes
//! past them.

/// The caret's width, in CSS pixels: one, in the ink — a point, as macOS
/// draws its own insertion point, one device pixel at a scale of one and two
/// at two, never a blurred column. (Two, tried first, was four device pixels
/// on a Retina display and read heavier than macOS's, by hand.)
pub const CARET_WIDTH: f32 = 1.0;

/// The most characters an input method's composition is drawn with: what
/// a person composes is a word or a phrase, and the field holds at most
/// [`crate::typed::LONGEST_TYPED`] in all.
pub const LONGEST_COMPOSING: usize = 256;

/// The furthest a field's words are ever scrolled, in CSS pixels: further
/// than [`crate::typed::LONGEST_TYPED`] characters of any face at the
/// field's size reach, so a number past it on the wire is refused.
pub const FURTHEST: f32 = 1_000_000.0;

/// How a field's caret, selection and composition stand, as the browser
/// process sends them with the field's words.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Editing {
    /// Where the caret is: the moving end of the selection, in characters.
    pub caret: usize,
    /// Where the selection began: the selection runs between it and
    /// [`Editing::caret`], and is nothing while the two are one place.
    pub anchor: usize,
    /// An input method's composition standing at the caret, if a person is
    /// composing (queue item 848).
    pub composing: Option<Composing>,
    /// Whether the caret is drawn now: the field has the keyboard, the
    /// window has it, nothing is selected, and the blink is in its shown
    /// half (`alo-window`'s `field_blink`).
    pub caret_shown: bool,
    /// How far the words were scrolled when the field was last laid out,
    /// in CSS pixels from their start: where the scroll is kept from.
    pub scrolled: f32,
    /// How wide the room the words had when the field was last laid out,
    /// if it has been: what the caret is kept inside.
    pub room: Option<f32>,
}

/// An input method's composition (queue item 848): the words it is
/// composing, drawn underlined at the caret, and where its own caret is
/// inside them, in characters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composing {
    /// What is being composed: `きょう`, before `今日` is chosen.
    pub text: String,
    /// Where the input method's caret is inside it, in characters.
    pub caret: usize,
}

/// A field's words as they are drawn: what is typed with any composition
/// put in at the caret, where the caret is in that, and which characters
/// are selected and which composed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    /// The words drawn.
    pub text: String,
    /// Where the caret is drawn, in characters of [`Drawn::text`].
    pub caret: usize,
    /// The characters selected, start before end, if any are.
    pub selected: Option<(usize, usize)>,
    /// The characters an input method is composing, if any.
    pub composed: Option<(usize, usize)>,
}

impl Editing {
    /// A caret at `at` with nothing selected, drawn.
    pub fn caret_at(at: usize) -> Self {
        Self {
            caret: at,
            anchor: at,
            caret_shown: true,
            ..Self::default()
        }
    }

    /// The same with `anchor` where the selection began.
    #[must_use]
    pub fn selecting_from(mut self, anchor: usize) -> Self {
        self.anchor = anchor;
        self
    }

    /// What `typed` is drawn as with this editing: the caret and the anchor
    /// clamped to it, a composition replacing what is selected at the caret
    /// — as macOS's text system puts one in — and nothing selected while a
    /// person composes.
    pub fn drawn(&self, typed: &str) -> Drawn {
        let count = typed.chars().count();
        let caret = self.caret.min(count);
        let anchor = self.anchor.min(count);
        let (start, end) = (caret.min(anchor), caret.max(anchor));
        match &self.composing {
            Some(composing) if !composing.text.is_empty() => {
                let composed: String = composing.text.chars().take(LONGEST_COMPOSING).collect();
                let length = composed.chars().count();
                let mut text: String = typed.chars().take(start).collect();
                text.push_str(&composed);
                text.extend(typed.chars().skip(end));
                Drawn {
                    text,
                    caret: start + composing.caret.min(length),
                    selected: None,
                    composed: Some((start, start + length)),
                }
            }
            _ => Drawn {
                text: typed.to_owned(),
                caret,
                selected: (start != end).then_some((start, end)),
                composed: None,
            },
        }
    }
}

/// Where a field's words were laid out, as its renderer answers the
/// browser process after drawing it, in the part's CSS pixels.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldLaidOut {
    /// The left of the room the words have: where their start stands while
    /// they are not scrolled.
    pub left: f32,
    /// The top of the field.
    pub top: f32,
    /// How tall the field is.
    pub height: f32,
    /// How wide the room the words have is.
    pub room: f32,
    /// How far the words are scrolled: how far left of [`FieldLaidOut::left`]
    /// their start stands.
    pub scrolled: f32,
    /// Each place the caret may stand, in order — a character and how far
    /// from the words' start it is, unscrolled — the first the start and the
    /// last the end. A letter with its marks is one place to stand either
    /// side of, never inside.
    pub stops: Vec<(usize, f32)>,
}

impl FieldLaidOut {
    /// The place the caret stands at nearest `x`, in the part's CSS pixels:
    /// where a press there puts it, before the start the start and past the
    /// end the end.
    pub fn index_at(&self, x: f32) -> usize {
        let along = x - self.left + self.scrolled;
        self.stops
            .iter()
            .min_by(|(_, one), (_, other)| {
                (one - along)
                    .abs()
                    .partial_cmp(&(other - along).abs())
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            .map_or(0, |(at, _)| *at)
    }

    /// How far from the room's left the caret at `at` stands, scrolled: the
    /// stop at it, or the nearest before it.
    pub fn x_of(&self, at: usize) -> f32 {
        let along = self
            .stops
            .iter()
            .take_while(|(stop, _)| *stop <= at)
            .last()
            .map_or(0.0, |(_, x)| *x);
        along - self.scrolled
    }
}

/// How far a field's words are scrolled with the caret at `caret` (CSS
/// pixels from their start), the words `width` wide in a room `room` wide,
/// last scrolled `previous`: as macOS's own fields keep the caret in view.
///
/// - The words are never scrolled while they fit, and never so far that
///   room is left empty after their end (and the caret there).
/// - A caret past the room's right edge brings it in to stand at that edge;
///   one before its left edge, to stand at the left edge; one already in
///   view moves nothing.
///
/// Whole device pixels at `scale`, so the words never shimmer between two.
pub fn kept_in_view(previous: f32, caret: f32, width: f32, room: f32, scale: f32) -> f32 {
    let furthest = (width + CARET_WIDTH - room).max(0.0);
    let mut scrolled = previous.clamp(0.0, furthest);
    if caret + CARET_WIDTH - scrolled > room {
        scrolled = caret + CARET_WIDTH - room;
    }
    if caret < scrolled {
        scrolled = caret;
    }
    let scrolled = scrolled.clamp(0.0, furthest);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    ((scrolled * scale).round() / scale).clamp(0.0, FURTHEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(value: f32) -> u32 {
        value.to_bits()
    }

    #[test]
    fn a_selection_is_between_the_anchor_and_the_caret_either_way() {
        let drawn = Editing::caret_at(1).selecting_from(4).drawn("kampala");
        assert_eq!(drawn.selected, Some((1, 4)));
        assert_eq!(drawn.caret, 1);
        let drawn = Editing::caret_at(4).selecting_from(1).drawn("kampala");
        assert_eq!(drawn.selected, Some((1, 4)));
        assert_eq!(drawn.caret, 4);
        assert_eq!(Editing::caret_at(3).drawn("kampala").selected, None);
    }

    #[test]
    fn places_past_the_words_are_their_end() {
        let drawn = Editing::caret_at(90).selecting_from(70).drawn("abc");
        assert_eq!(drawn.caret, 3);
        assert_eq!(drawn.selected, None);
        assert_eq!(Editing::caret_at(2).drawn("€é").caret, 2);
    }

    #[test]
    fn a_composition_stands_in_place_of_the_selection_at_the_caret() {
        let editing = Editing {
            composing: Some(Composing {
                text: "きょう".to_owned(),
                caret: 3,
            }),
            ..Editing::caret_at(5).selecting_from(2)
        };
        let drawn = editing.drawn("Kampala");
        assert_eq!(drawn.text, "Kaきょうla");
        assert_eq!(drawn.composed, Some((2, 5)));
        assert_eq!(drawn.caret, 5);
        assert_eq!(drawn.selected, None);
        // An empty composition is none.
        let ended = Editing {
            composing: Some(Composing::default()),
            ..Editing::caret_at(1)
        };
        assert_eq!(ended.drawn("ab").text, "ab");
        assert_eq!(ended.drawn("ab").composed, None);
    }

    #[test]
    fn the_caret_is_kept_in_view_as_macos_keeps_it() {
        // Words that fit never scroll.
        assert_eq!(bits(kept_in_view(0.0, 80.0, 100.0, 300.0, 1.0)), bits(0.0));
        assert_eq!(bits(kept_in_view(40.0, 80.0, 100.0, 300.0, 1.0)), bits(0.0));
        // Typing on past the right edge brings the caret to it.
        assert_eq!(
            bits(kept_in_view(0.0, 500.0, 500.0, 300.0, 1.0)),
            bits(201.0)
        );
        // Moving back within view moves nothing.
        assert_eq!(
            bits(kept_in_view(201.0, 260.0, 500.0, 300.0, 1.0)),
            bits(201.0)
        );
        // Past the left edge, the caret stands at it.
        assert_eq!(
            bits(kept_in_view(201.0, 120.0, 500.0, 300.0, 1.0)),
            bits(120.0)
        );
        // Words shortened leave no room empty after their end.
        assert_eq!(
            bits(kept_in_view(201.0, 350.0, 350.0, 300.0, 1.0)),
            bits(51.0)
        );
        // Whole device pixels.
        assert_eq!(
            bits(kept_in_view(0.0, 400.3, 400.3, 300.0, 2.0)),
            bits(101.5)
        );
    }

    #[test]
    fn a_press_places_the_caret_at_the_nearest_stop() {
        let laid = FieldLaidOut {
            left: 100.0,
            top: 0.0,
            height: 40.0,
            room: 300.0,
            scrolled: 20.0,
            stops: vec![(0, 0.0), (1, 10.0), (3, 30.0), (4, 38.0)],
        };
        assert_eq!(laid.index_at(50.0), 0, "before the start");
        assert_eq!(laid.index_at(88.0), 1, "nearer 10 than 0 or 30");
        assert_eq!(laid.index_at(106.0), 3, "two characters are one stop");
        assert_eq!(laid.index_at(900.0), 4, "past the end");
        assert_eq!(bits(laid.x_of(2)), bits(-10.0));
        assert_eq!(bits(laid.x_of(4)), bits(18.0));
    }
}
