/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What of a frame changed, on the wire (queue item 872, ADR 0102).
//!
//! From a renderer comes the frame's size, how many rectangles of it, and
//! each rectangle with its pixels (queue item 896), read as anything a
//! renderer says is: the frame no larger than [`crate::frame_patch`]'s asker
//! could hold, no more rectangles than it sends — said before any is read —
//! each inside the frame, and each exactly its pixels, never a byte more or
//! fewer.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::frame_patch::{MOST_PIECES, Patch, Piece};

impl Writer {
    /// What of a frame changed: its size, how many pieces, and each piece's
    /// rectangle and pixels.
    pub(super) fn patch(&mut self, patch: &Patch) {
        self.number(u64::from(patch.width));
        self.number(u64::from(patch.height));
        self.number(patch.pieces.len() as u64);
        for piece in &patch.pieces {
            for value in [piece.left, piece.top, piece.across, piece.down] {
                self.number(u64::from(value));
            }
            self.bytes(&piece.pixels);
        }
    }
}

impl Reader<'_> {
    /// What of a frame changed: at most [`MOST_PIECES`] pieces, each of whose
    /// rectangles has to lie inside the frame and whose pixels have to be
    /// the rectangle's.
    pub(super) fn patch(&mut self) -> Result<Patch, Unreadable> {
        let side = |reader: &mut Self| {
            u32::try_from(reader.number()?)
                .map_err(|_| unreadable("a patch of a frame larger than this engine will hold"))
        };
        let (width, height) = (side(self)?, side(self)?);
        let count = self.number()?;
        if count > MOST_PIECES as u64 {
            return Err(unreadable(format!(
                "a patch of {count} pieces, more than the {MOST_PIECES} a renderer sends"
            )));
        }
        let mut pieces = Vec::new();
        for _ in 0..count {
            let (left, top, across, down) = (side(self)?, side(self)?, side(self)?, side(self)?);
            let pixels = self.bytes()?;
            let piece = Piece {
                left,
                top,
                across,
                down,
                pixels,
            };
            let alone = Patch {
                width,
                height,
                pieces: vec![piece],
            };
            if !alone.is_whole() {
                return Err(unreadable(format!(
                    "a patch of {across}×{down} at {left}, {top} of a {width}×{height} frame \
                     carrying {} bytes",
                    alone.pieces.first().map_or(0, |piece| piece.pixels.len())
                )));
            }
            pieces.extend(alone.pieces);
        }
        Ok(Patch {
            width,
            height,
            pieces,
        })
    }
}
