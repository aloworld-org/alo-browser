/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The card's picture drawn beneath the first screen, on the wire (queue
//! item 869, ADR 0100).
//!
//! To a renderer goes Chrome's frame of the card's page and the size of the
//! place it is drawn into: a PNG's bytes, by their signature, and a size of
//! some pixels, each side at most [`MOST_SIDE`], whose frame back fits in
//! one message. From the first screen's renderer comes where it left that
//! place clear, read as anything a renderer says is: whole device pixels, a
//! size of some pixels at most [`MOST_SIDE`] a side, no further from the
//! frame's corner than [`FURTHEST`] — a place the window draws the card's
//! picture in, inside the first screen's own room.

use super::{LARGEST_MESSAGE, Reader, Unreadable, Writer, unreadable};
use crate::beneath::{Beneath, FURTHEST, MOST_SIDE};
use crate::card_picture::CardPicture;

/// The bytes a PNG starts with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

impl Writer {
    /// A card's picture to draw: the frame's bytes and its place's size.
    pub(super) fn card_picture(&mut self, picture: &CardPicture) {
        self.bytes(&picture.png);
        self.number(u64::from(picture.width));
        self.number(u64::from(picture.height));
    }

    /// Where a part left its place clear, if it did.
    pub(super) fn picture_beneath(&mut self, place: Option<Beneath>) {
        self.tag(14);
        let Some(place) = place else {
            self.bool(false);
            return;
        };
        self.bool(true);
        for edge in [place.left, place.top] {
            self.bool(edge < 0);
            self.number(u64::from(edge.unsigned_abs()));
        }
        self.number(u64::from(place.width));
        self.number(u64::from(place.height));
    }
}

impl Reader<'_> {
    /// A card's picture to draw, as [`Writer::card_picture`] wrote it: a
    /// PNG, and a place whose frame back is one message at most.
    pub(super) fn card_picture(&mut self) -> Result<CardPicture, Unreadable> {
        let png = self.bytes()?;
        if !png.starts_with(PNG_SIGNATURE) {
            return Err(unreadable("a card's picture that is not a PNG"));
        }
        let width = self.side()?;
        let height = self.side()?;
        let pixels = u64::from(width) * u64::from(height) * 4;
        if pixels >= LARGEST_MESSAGE as u64 {
            return Err(unreadable(format!(
                "a card's picture of {width}×{height}, past what one frame carries"
            )));
        }
        Ok(CardPicture { png, width, height })
    }

    /// Where a part left its place clear, as [`Writer::picture_beneath`]
    /// wrote it: a place of some pixels inside the bounds, or none.
    pub(super) fn picture_beneath(&mut self) -> Result<Option<Beneath>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let left = self.edge()?;
        let top = self.edge()?;
        let width = self.side()?;
        let height = self.side()?;
        Ok(Some(Beneath {
            left,
            top,
            width,
            height,
        }))
    }

    /// A side of a place: some pixels, at most [`MOST_SIDE`].
    fn side(&mut self) -> Result<u32, Unreadable> {
        u32::try_from(self.number()?)
            .ok()
            .filter(|side| (1..=MOST_SIDE).contains(side))
            .ok_or_else(|| unreadable(format!("a place's side of no size or past {MOST_SIDE}")))
    }

    /// An edge of a place: whole device pixels either side of the frame's,
    /// at most [`FURTHEST`] away.
    fn edge(&mut self) -> Result<i32, Unreadable> {
        let below = self.bool()?;
        let far = i32::try_from(self.number()?)
            .ok()
            .filter(|far| *far <= FURTHEST)
            .ok_or_else(|| unreadable(format!("a place further than {FURTHEST} away")))?;
        Ok(if below { -far } else { far })
    }
}
