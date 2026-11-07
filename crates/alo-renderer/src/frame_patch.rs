/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What of a frame changed since the last one a renderer sent (queue item
//! 872, ADR 0102).
//!
//! A letter typed beside a card repaints about a thirtieth of the first
//! screen (ADR 0101), and the renderer sent all 13.6 MB of the frame at two
//! for it, made into bytes first (5 ms), carried down the pipe and copied
//! out again. Asked with [`crate::ToRenderer::PaintOver`] by an asker that
//! holds the last frame this renderer sent, it answers with only the
//! rectangle every drawing since then painted again: the frame's size, the
//! rectangle, and its pixels row by row. The asker puts them over the frame
//! it holds ([`Patch::onto`]), and holds the frame the renderer would have
//! sent whole, byte for byte, since every pixel outside the rectangle is one
//! no drawing since changed.

use alo_paint::{Canvas, Region};

use crate::frame::Frame;

/// A rectangle of a frame's pixels, and the size of the frame it is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// How wide the whole frame is, in device pixels.
    pub width: u32,
    /// How tall.
    pub height: u32,
    /// The rectangle's left edge.
    pub left: u32,
    /// Its top edge.
    pub top: u32,
    /// How wide it is: none when nothing changed.
    pub across: u32,
    /// How tall.
    pub down: u32,
    /// Red, green, blue and alpha for every pixel of the rectangle, row by
    /// row from its top.
    pub pixels: Vec<u8>,
}

impl Patch {
    /// The pixels of `region` of `canvas`, finished as a frame's are, or
    /// none for nothing changed.
    pub fn of(canvas: &Canvas, region: Option<Region>) -> Self {
        let (width, height) = (canvas.width(), canvas.height());
        let Some(region) = region.and_then(|region| {
            Region::new(
                region.left,
                region.top,
                region.right.min(width),
                region.bottom.min(height),
            )
        }) else {
            return Self {
                width,
                height,
                left: 0,
                top: 0,
                across: 0,
                down: 0,
                pixels: Vec::new(),
            };
        };
        let across = region.right - region.left;
        let down = region.bottom - region.top;
        let mut pixels = Vec::with_capacity((across as usize) * (down as usize) * 4);
        let row = width as usize;
        for y in region.top..region.bottom {
            let from = y as usize * row;
            let Some(run) = canvas
                .pixels()
                .get(from + region.left as usize..from + region.right as usize)
            else {
                continue;
            };
            for pixel in run {
                let (red, green, blue, alpha) = pixel.to_rgba8();
                pixels.extend_from_slice(&[red, green, blue, alpha]);
            }
        }
        Self {
            width,
            height,
            left: region.left,
            top: region.top,
            across,
            down,
            pixels,
        }
    }

    /// Whether its rectangle lies inside its frame and its pixels are that
    /// rectangle's, every one: what a patch read off a wire has to be.
    pub fn is_whole(&self) -> bool {
        let inside = u64::from(self.left) + u64::from(self.across) <= u64::from(self.width)
            && u64::from(self.top) + u64::from(self.down) <= u64::from(self.height);
        let wanted = u64::from(self.across) * u64::from(self.down) * 4;
        inside && wanted == self.pixels.len() as u64
    }

    /// Put over `frame`, which has to be the frame of the size it was made
    /// for: whether it was. A patch not whole, or of a frame of another
    /// size, changes nothing.
    pub fn onto(&self, frame: &mut Frame) -> bool {
        if !self.is_whole() || (frame.width, frame.height) != (self.width, self.height) {
            return false;
        }
        let row = self.across as usize * 4;
        if row == 0 {
            return true;
        }
        for (step, run) in self.pixels.chunks_exact(row).enumerate() {
            let y = self.top as usize + step;
            let from = (y * self.width as usize + self.left as usize) * 4;
            let Some(target) = frame.pixels.get_mut(from..from + row) else {
                return false;
            };
            target.copy_from_slice(run);
        }
        true
    }

    /// How many pixels it carries.
    pub fn area(&self) -> u64 {
        u64::from(self.across) * u64::from(self.down)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    fn canvas() -> Canvas {
        let mut canvas = Canvas::new(6, 4, Rgba::WHITE);
        canvas.fill_rect(2, 1, 3, 2, Rgba::from_rgba8(10, 20, 30, 128));
        canvas
    }

    #[test]
    fn a_patch_put_over_the_frame_before_is_the_frame_after() {
        let before = Frame::from_canvas(&Canvas::new(6, 4, Rgba::WHITE));
        let after = canvas();
        let patch = Patch::of(&after, Region::new(2, 1, 5, 3));
        assert_eq!((patch.across, patch.down, patch.pixels.len()), (3, 2, 24));
        let mut held = before;
        assert!(patch.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&after));
    }

    #[test]
    fn nothing_changed_is_a_patch_of_no_pixels_and_a_region_past_the_canvas_is_cut() {
        let nothing = Patch::of(&canvas(), None);
        assert_eq!((nothing.area(), nothing.is_whole()), (0, true));
        let mut held = Frame::from_canvas(&canvas());
        assert!(nothing.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&canvas()));
        let cut = Patch::of(&canvas(), Region::new(4, 2, 40, 40));
        assert_eq!((cut.left, cut.top, cut.across, cut.down), (4, 2, 2, 2));
    }

    #[test]
    fn a_patch_of_another_frame_or_not_whole_changes_nothing() {
        let patch = Patch::of(&canvas(), Region::new(0, 0, 2, 2));
        let mut other = Frame::from_canvas(&Canvas::new(5, 4, Rgba::WHITE));
        let kept = other.clone();
        assert!(!patch.onto(&mut other));
        assert_eq!(other, kept);
        let mut short = patch.clone();
        short.pixels.pop();
        assert!(!short.is_whole());
        let past = Patch { left: 5, ..patch };
        assert!(!past.is_whole(), "a rectangle past the frame's side");
    }
}
