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
//!
//! **A rectangle per piece** (queue item 896). A drawing is painted again
//! in a few pieces ([`alo_paint::pieces`]): two rings far apart in
//! *Settings* are two pieces of about 198,000 pixels at two, which crossed
//! as the 725,000 of the rectangle holding both. So a patch carries each
//! piece every drawing since the last frame painted, at most
//! [`MOST_PIECES`], each its own rectangle of pixels — pieces joined, by the
//! same rule a drawing's are cut by, where there would be more. Read off a
//! wire, every piece is read as hostile on its own: inside the frame, its
//! pixels exactly its rectangle's, and no more pieces than this file sends.

use alo_paint::{Canvas, Region};

use crate::frame::Frame;

/// The most pieces a patch carries: what [`alo_paint::pieces`] cuts a
/// drawing into, and no more read off a wire.
pub const MOST_PIECES: usize = alo_paint::pieces::MOST_PIECES;

/// Rectangles of a frame's pixels, and the size of the frame they are of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// How wide the whole frame is, in device pixels.
    pub width: u32,
    /// How tall.
    pub height: u32,
    /// The rectangles: none when nothing changed.
    pub pieces: Vec<Piece>,
}

/// One rectangle of a frame's pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The rectangle's left edge.
    pub left: u32,
    /// Its top edge.
    pub top: u32,
    /// How wide it is.
    pub across: u32,
    /// How tall.
    pub down: u32,
    /// Red, green, blue and alpha for every pixel of the rectangle, row by
    /// row from its top.
    pub pixels: Vec<u8>,
}

impl Piece {
    /// The pixels of `region` of `canvas`, finished as a frame's are; none
    /// for a region with no pixel on it.
    fn of(canvas: &Canvas, region: Region) -> Option<Self> {
        let (width, height) = (canvas.width(), canvas.height());
        let region = Region::new(
            region.left,
            region.top,
            region.right.min(width),
            region.bottom.min(height),
        )?;
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
        Some(Self {
            left: region.left,
            top: region.top,
            across,
            down,
            pixels,
        })
    }

    /// Whether it lies inside a frame of `(width, height)` and its pixels
    /// are its rectangle's, every one.
    fn is_whole(&self, (width, height): (u32, u32)) -> bool {
        let inside = u64::from(self.left) + u64::from(self.across) <= u64::from(width)
            && u64::from(self.top) + u64::from(self.down) <= u64::from(height);
        let wanted = u64::from(self.across) * u64::from(self.down) * 4;
        inside && wanted == self.pixels.len() as u64
    }

    /// How many pixels it carries.
    pub fn area(&self) -> u64 {
        u64::from(self.across) * u64::from(self.down)
    }
}

impl Patch {
    /// The pixels of each of `regions` of `canvas`, finished as a frame's
    /// are: none for nothing changed. Past [`MOST_PIECES`] regions, they
    /// are joined as a drawing's places are ([`alo_paint::pieces::cut`]).
    pub fn of(canvas: &Canvas, regions: &[Region]) -> Self {
        let joined;
        let regions = if regions.len() > MOST_PIECES {
            joined = alo_paint::pieces::cut(regions, &[]);
            &joined[..]
        } else {
            regions
        };
        Self {
            width: canvas.width(),
            height: canvas.height(),
            pieces: regions
                .iter()
                .filter_map(|region| Piece::of(canvas, *region))
                .collect(),
        }
    }

    /// Whether every piece lies inside its frame and carries its own
    /// rectangle's pixels, and there are no more than [`MOST_PIECES`]: what
    /// a patch read off a wire has to be.
    pub fn is_whole(&self) -> bool {
        self.pieces.len() <= MOST_PIECES
            && self
                .pieces
                .iter()
                .all(|piece| piece.is_whole((self.width, self.height)))
    }

    /// Put over `frame`, which has to be the frame of the size it was made
    /// for: whether it was. A patch not whole, or of a frame of another
    /// size, changes nothing.
    pub fn onto(&self, frame: &mut Frame) -> bool {
        if !self.is_whole() || (frame.width, frame.height) != (self.width, self.height) {
            return false;
        }
        for piece in &self.pieces {
            let row = piece.across as usize * 4;
            if row == 0 {
                continue;
            }
            for (step, run) in piece.pixels.chunks_exact(row).enumerate() {
                let y = piece.top as usize + step;
                let from = (y * self.width as usize + piece.left as usize) * 4;
                if let Some(target) = frame.pixels.get_mut(from..from + row) {
                    target.copy_from_slice(run);
                }
            }
        }
        true
    }

    /// How many pixels it carries, every piece's together.
    pub fn area(&self) -> u64 {
        self.pieces.iter().map(Piece::area).sum()
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

    fn region(left: u32, top: u32, right: u32, bottom: u32) -> Region {
        Region::new(left, top, right, bottom).expect("a region")
    }

    #[test]
    fn a_patch_put_over_the_frame_before_is_the_frame_after() {
        let before = Frame::from_canvas(&Canvas::new(6, 4, Rgba::WHITE));
        let after = canvas();
        let patch = Patch::of(&after, &[region(2, 1, 5, 3)]);
        let [piece] = &patch.pieces[..] else {
            panic!("one piece: {patch:?}");
        };
        assert_eq!((piece.across, piece.down, piece.pixels.len()), (3, 2, 24));
        let mut held = before;
        assert!(patch.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&after));
    }

    #[test]
    fn a_patch_of_pieces_carries_each_and_nothing_between() {
        // Queue item 896: two marks far apart cross as two rectangles, not
        // the one holding both.
        let mut before = Canvas::new(40, 30, Rgba::WHITE);
        before.fill_rect(30, 20, 4, 4, Rgba::BLACK);
        let mut after = Canvas::new(40, 30, Rgba::WHITE);
        after.fill_rect(1, 1, 3, 2, Rgba::from_rgba8(200, 30, 30, 255));
        let patch = Patch::of(&after, &[region(1, 1, 4, 3), region(30, 20, 34, 24)]);
        assert_eq!((patch.pieces.len(), patch.area()), (2, 6 + 16));
        let mut held = Frame::from_canvas(&before);
        assert!(patch.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&after));
        // Overlapping pieces carry the same pixels twice, alike.
        let twice = Patch::of(&after, &[region(0, 0, 5, 5), region(2, 2, 6, 6)]);
        let mut held = Frame::from_canvas(&Canvas::new(40, 30, Rgba::WHITE));
        assert!(twice.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&after));
    }

    #[test]
    fn more_pieces_than_a_patch_carries_are_joined() {
        let after = Canvas::new(400, 30, Rgba::WHITE);
        let many: Vec<Region> = (0..20).map(|k| region(k * 20, 0, k * 20 + 2, 2)).collect();
        let patch = Patch::of(&after, &many);
        assert!(patch.pieces.len() <= MOST_PIECES, "{}", patch.pieces.len());
        assert!(patch.is_whole());
        for each in &many {
            assert!(
                patch.pieces.iter().any(|piece| piece.left <= each.left
                    && piece.top <= each.top
                    && piece.left + piece.across >= each.right
                    && piece.top + piece.down >= each.bottom),
                "{each:?} carried"
            );
        }
    }

    #[test]
    fn nothing_changed_is_a_patch_of_no_pixels_and_a_region_past_the_canvas_is_cut() {
        let nothing = Patch::of(&canvas(), &[]);
        assert_eq!((nothing.area(), nothing.is_whole()), (0, true));
        let mut held = Frame::from_canvas(&canvas());
        assert!(nothing.onto(&mut held));
        assert_eq!(held, Frame::from_canvas(&canvas()));
        let cut = Patch::of(&canvas(), &[region(4, 2, 40, 40)]);
        let [piece] = &cut.pieces[..] else {
            panic!("one piece: {cut:?}");
        };
        assert_eq!(
            (piece.left, piece.top, piece.across, piece.down),
            (4, 2, 2, 2)
        );
        assert!(
            Patch::of(&canvas(), &[region(10, 10, 20, 20)])
                .pieces
                .is_empty()
        );
    }

    #[test]
    fn a_patch_of_another_frame_or_not_whole_changes_nothing() {
        let patch = Patch::of(&canvas(), &[region(0, 0, 2, 2), region(3, 2, 5, 4)]);
        let mut other = Frame::from_canvas(&Canvas::new(5, 4, Rgba::WHITE));
        let kept = other.clone();
        assert!(!patch.onto(&mut other));
        assert_eq!(other, kept);
        let mut held = Frame::from_canvas(&Canvas::new(6, 4, Rgba::WHITE));
        let untouched = held.clone();
        let mut short = patch.clone();
        if let Some(piece) = short.pieces.last_mut() {
            piece.pixels.pop();
        }
        assert!(!short.is_whole());
        assert!(!short.onto(&mut held), "one piece short");
        assert_eq!(held, untouched, "and not even the whole piece put");
        let mut past = patch.clone();
        if let Some(piece) = past.pieces.first_mut() {
            piece.left = 5;
        }
        assert!(!past.is_whole(), "a rectangle past the frame's side");
        let mut many = patch;
        let one = many.pieces.first().cloned().expect("a piece");
        many.pieces = vec![one; MOST_PIECES + 1];
        assert!(!many.is_whole(), "more pieces than are sent");
    }
}
