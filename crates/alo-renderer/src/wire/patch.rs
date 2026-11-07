/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What of a frame changed, on the wire (queue item 872, ADR 0102).
//!
//! From a renderer comes the frame's size, a rectangle of it and that
//! rectangle's pixels, read as anything a renderer says is: the frame no
//! larger than [`crate::frame_patch`]'s asker could hold, the rectangle
//! inside it, and exactly its pixels — never a byte more or fewer.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::frame_patch::Patch;

impl Writer {
    /// What of a frame changed: its size, the rectangle, its pixels.
    pub(super) fn patch(&mut self, patch: &Patch) {
        for value in [
            patch.width,
            patch.height,
            patch.left,
            patch.top,
            patch.across,
            patch.down,
        ] {
            self.number(u64::from(value));
        }
        self.bytes(&patch.pixels);
    }
}

impl Reader<'_> {
    /// What of a frame changed, whose rectangle has to lie inside the frame
    /// and whose pixels have to be the rectangle's.
    pub(super) fn patch(&mut self) -> Result<Patch, Unreadable> {
        let mut side = || {
            u32::try_from(self.number()?)
                .map_err(|_| unreadable("a patch of a frame larger than this engine will hold"))
        };
        let (width, height, left, top, across, down) =
            (side()?, side()?, side()?, side()?, side()?, side()?);
        let pixels = self.bytes()?;
        let patch = Patch {
            width,
            height,
            left,
            top,
            across,
            down,
            pixels,
        };
        if !patch.is_whole() {
            return Err(unreadable(format!(
                "a patch of {across}×{down} at {left}, {top} of a {width}×{height} frame \
                 carrying {} bytes",
                patch.pixels.len()
            )));
        }
        Ok(patch)
    }
}
