/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's mark at the start of the line saying what alo did (queue item 984;
//! ADR 0112, *Answered* 1; ADR 0074 § 2 as amended).
//!
//! Deep teal means alo is acting, and it is never drawn alone: always with
//! alo's mark and a word. On `alo PAGE` the ring around the box a verb acted
//! on ([`crate::mark`]) is the teal, and the line along the bottom saying
//! what the verb did is its word; through `alo --chrome` Chrome's highlight
//! is the teal ([`crate::chrome_mark`]) and the frame of ours around
//! Chrome's window carries the line. Both lines are drawn by this process,
//! not from a document, so the mark the interface's documents draw as an
//! `<img>` (`alo_renderer::acting`) is drawn here too: [`SIDE`] CSS pixels
//! square, [`crate::notice::INSET`] in from the line's left edge and in the
//! middle of its height, the words [`AFTER`] past it. A refusal's line is
//! not alo acting and has no mark.
//!
//! # Drawn by this engine, from the owner's drawing compiled in
//!
//! The mark is the owner's small drawing, `docs/brand/alo-browser-icon-small.svg`
//! ([`alo_renderer::acting::picture`]), laid out and painted by this engine
//! as one `<img>` of fixed markup of ours, at the window's scale — the same
//! bytes, the same drawing and the same rasteriser as the panel's mark. No
//! stranger's byte is in it (ADR 0024 § 4 keeps those out of this
//! process), as none is in the face the line's words are drawn in. It is
//! drawn once per scale and kept.

use crate::compose::copy;
use crate::notice::INSET;
use crate::place::PixelRect;
use crate::scale::Scale;
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Frame, Screen, render_at};
use alo_text::FontDatabase;
use alo_value::Rgba;
use std::cell::RefCell;

/// How big the mark is, in CSS pixels: as big as the mark before the
/// working line's words over a card (queue item 974).
pub const SIDE: u32 = 16;

/// How far past the mark the line's words begin, in CSS pixels: as far as
/// over a card.
pub const AFTER: u32 = 8;

/// The markup the mark is laid out from: ours, fixed, one picture.
const MARKUP: &str = "<!doctype html><img src=\"alo:mark\" alt=\"\">";

/// Its sheet: the picture at the page's corner, [`SIDE`] square.
const SHEET: &str = "* { margin: 0; padding: 0 } img { display: block; width: 16px; height: 16px }";

thread_local! {
    /// The mark as last drawn, and the scale's factor it was drawn at.
    static KEPT: RefCell<Option<(u32, Frame)>> = const { RefCell::new(None) };
}

/// alo's mark at `scale`, [`Scale::device`] of [`SIDE`] device pixels
/// square, its corners clear: drawn the first time a scale asks for it, and
/// kept.
pub fn drawn(scale: Scale) -> Frame {
    let key = scale.factor().to_bits();
    KEPT.with(|kept| {
        if let Some((at, frame)) = kept.borrow().as_ref()
            && *at == key
        {
            return frame.clone();
        }
        let frame = painted(scale);
        *kept.borrow_mut() = Some((key, frame.clone()));
        frame
    })
}

/// The mark laid out and painted by the engine at `scale`.
fn painted(scale: Scale) -> Frame {
    let side = f32::from(u16::try_from(SIDE).unwrap_or(u16::MAX));
    let rendered = render_at(
        MARKUP,
        SHEET,
        Screen::new(Size::new(side, side)).at_scale(scale.factor()),
        &FontDatabase::new(),
        &[],
        &[alo_renderer::acting::picture()],
    );
    let device = scale.device(SIDE);
    let mut canvas = Canvas::new(device, device, Rgba::TRANSPARENT);
    alo_paint::render(&rendered.drawing.display, &mut canvas);
    Frame::from_canvas(&canvas)
}

/// How many device pixels further in than the inset a line's words begin
/// when the line carries the mark: the mark and the room after it.
pub fn lead(scale: Scale) -> u32 {
    scale.device(SIDE) + scale.device(AFTER)
}

/// Where the mark stands on a line placed at `line`, at `scale`: the inset
/// in from its left edge, in the middle of its height — or nowhere, when
/// the line is too narrow to hold the mark and the inset either side of it,
/// or too short to hold it.
pub fn place(line: PixelRect, scale: Scale) -> Option<PixelRect> {
    let side = scale.device(SIDE);
    let inset = inset(scale);
    let wanted = side.checked_add(inset.checked_mul(2)?)?;
    if line.width < wanted || line.height < side {
        return None;
    }
    Some(PixelRect {
        x: line.x.checked_add(inset)?,
        y: line.y.checked_add((line.height - side) / 2)?,
        width: side,
        height: side,
    })
}

/// [`INSET`] in device pixels at `scale`, rounded as [`Scale::device`]
/// rounds.
fn inset(scale: Scale) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "INSET is a whole 12, exact in a u32"
    )]
    let whole = INSET as u32;
    scale.device(whole)
}

/// Draw alo's mark on a line placed at `line` of `canvas`, where
/// [`place`] puts it, over what the line already drew there.
pub fn draw(canvas: &mut Canvas, line: PixelRect, scale: Scale) {
    if let Some(at) = place(line, scale) {
        copy(canvas, &drawn(scale), at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_mark_is_sixteen_css_pixels_square_at_each_scale() {
        for (scale, side) in [
            (Scale::ONE, 16),
            (Scale::ONE_AND_A_HALF, 24),
            (Scale::TWO, 32),
        ] {
            let frame = drawn(scale);
            assert_eq!((frame.width, frame.height), (side, side), "{scale:?}");
        }
    }

    #[test]
    fn it_is_the_owners_drawing_its_tile_cream_its_arcs_navy_its_corner_clear() {
        let frame = drawn(Scale::TWO);
        // The tile's cream, in from its rounded corner; nothing in the
        // corner itself.
        assert_eq!(frame.at(16, 2), Some((0xfb, 0xfa, 0xf8, 0xff)));
        assert_eq!(frame.at(0, 0).map(|(.., alpha)| alpha), Some(0));
        // The navy arcs drawn somewhere, and the teal swoosh.
        let colours: Vec<(u8, u8, u8)> = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .filter_map(|(x, y)| frame.at(x, y))
            .filter(|&(.., alpha)| alpha == 0xff)
            .map(|(red, green, blue, _)| (red, green, blue))
            .collect();
        assert!(colours.contains(&(0x1b, 0x3e, 0x5e)), "navy arcs");
        assert!(
            colours
                .iter()
                .any(|&(red, green, blue)| red < 0x30 && green > 0x60 && blue > 0x70),
            "a teal swoosh"
        );
    }

    #[test]
    fn it_stands_at_the_inset_in_the_middle_of_the_line() {
        // A line of 32 at one, from 400: the mark 16 square 12 in, 8 down.
        assert_eq!(
            place(line(0, 400, 480, 32), Scale::ONE),
            Some(line(12, 408, 16, 16))
        );
        // At 1.5 the line is 48 tall: 24 square, 18 in, 12 down.
        assert_eq!(
            place(line(100, 0, 600, 48), Scale::ONE_AND_A_HALF),
            Some(line(118, 12, 24, 24))
        );
        assert_eq!(lead(Scale::ONE), 24);
        assert_eq!(lead(Scale::TWO), 48);
    }

    #[test]
    fn a_line_too_narrow_or_too_short_for_it_has_none() {
        assert_eq!(place(line(0, 0, 39, 32), Scale::ONE), None);
        assert!(place(line(0, 0, 40, 32), Scale::ONE).is_some());
        assert_eq!(place(line(0, 0, 480, 15), Scale::ONE), None);
    }
}
