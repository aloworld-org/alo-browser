/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The live card's page, drawn beneath the first screen (queue item 869,
//! ADR 0100).
//!
//! The first screen is asked for with its live card's picture beneath it
//! once a frame has come ([`pictured_beneath`]), so a new frame of the page
//! is the same first screen and is not drawn again. Its renderer says where
//! it left the picture's place clear, just before the frame it left it in
//! ([`ConversationWindow::beneath_told`]); the window asks for the newest
//! frame drawn at that place's size ([`ConversationWindow::ask_pictured`])
//! — again for each frame Chrome streams, and again when the place's size
//! changes — and composes the picture under the first screen's frame
//! ([`ConversationWindow::beneath_scene`]).
//!
//! A picture is the card's it was asked for: a card replaced or put away
//! leaves no picture of its page in the next card's place
//! ([`ConversationWindow::card_changed`]).

use super::ConversationWindow;
use alo_renderer::Frame;
use alo_renderer::beneath::Beneath;
use alo_renderer::card::PictureAt;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;

/// What the window holds of the card's page beneath the first screen.
#[derive(Debug, Default)]
pub(super) struct CardBeneath {
    /// Where the first screen about to be told painted left its place
    /// clear: told just before its frame, and nothing told is no place.
    telling: Option<Beneath>,
    /// Where the first screen held left it clear, if it did.
    place: Option<Beneath>,
    /// The card's page drawn for it, if one has been.
    picture: Option<Frame>,
    /// The number of the newest picture the thread has said is drawn.
    pictured_through: u64,
    /// The first picture's number asked for the card on the screen now: a
    /// picture of a number before it is of a card that has gone.
    card_from: u64,
}

/// `start` asked for with its live card's page beneath it rather than in
/// it, once a frame of the page has come: the frame's bytes are not sent,
/// and a new frame leaves the first screen as it is.
pub(super) fn pictured_beneath(start: &mut Start) {
    if let Some(Turn::Page(card)) = start.turns.last_mut()
        && card.frame.take().is_some()
    {
        card.picture_at = PictureAt::Beneath;
    }
}

impl ConversationWindow {
    /// The first screen's drawing thread said where the frame it is about
    /// to tell left the card's place clear.
    pub(super) fn beneath_told(&mut self, place: Option<Beneath>) {
        self.beneath.telling = place;
    }

    /// The first screen's frame is held: the place it left clear with it
    /// — none drawing no picture, which is kept for when one is left again
    /// — and the card's page asked for at the place's size.
    pub(super) fn beneath_painted(&mut self) {
        self.beneath.place = self.beneath.telling.take();
        self.ask_pictured();
    }

    /// The card's newest frame asked to be drawn at the size of the place
    /// the first screen held leaves for it — unless that is what was last
    /// asked, which the thread says nothing more of.
    pub(super) fn ask_pictured(&mut self) {
        let Some(place) = self.beneath.place else {
            return;
        };
        let Some(png) = self
            .conversation
            .card()
            .and_then(|card| card.frame.as_ref())
        else {
            return;
        };
        if let Some(drawing) = self.drawing.as_mut() {
            drawing.picture(png, place);
        }
    }

    /// The thread drew picture `number`, or could not.
    pub(super) fn pictured(&mut self, frame: Option<Frame>, number: u64) {
        self.beneath.pictured_through = number;
        if let Some(frame) = frame
            && number >= self.beneath.card_from
        {
            self.beneath.picture = Some(frame);
        }
    }

    /// The live card replaced or put away: no picture of its page is drawn
    /// in the place of the next.
    pub(super) fn card_changed(&mut self) {
        self.beneath.picture = None;
        self.beneath.card_from = self.drawing.as_ref().map_or(
            0,
            crate::conversation_drawing::ConversationDrawing::pictured_through,
        ) + 1;
        // The next card's frame is drawn for it, even one the same as the
        // last card's.
        if let Some(drawing) = self.drawing.as_mut() {
            drawing.forget_picture();
        }
    }

    /// Whether the card's page held is the newest asked for, or none was.
    pub(super) fn pictured_as_asked(&self) -> bool {
        self.drawing
            .as_ref()
            .is_none_or(|drawing| drawing.pictured_through() == self.beneath.pictured_through)
    }

    /// The card's page and the place it is drawn in, under the first
    /// screen's frame, while both are held.
    pub(super) fn beneath_scene(&self) -> Option<(&Frame, Beneath)> {
        self.start.as_ref()?;
        Some((self.beneath.picture.as_ref()?, self.beneath.place?))
    }
}
