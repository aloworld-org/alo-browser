/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A run of glyphs that are pictures, where each is drawn (ADR 0088).
//!
//! A colour emoji face — Noto Color Emoji, which the interface carries; Apple
//! Color Emoji, were a renderer handed it — holds each glyph as a PNG in a
//! `CBDT` or `sbix` table and has no outline for it, so the letters
//! [`crate::render`] fills draw nothing. Paint draws each such glyph as what
//! it is: a picture, at the place and size the font gives it, through the
//! same road every `<img>` takes ([`crate::drawn_picture`]) — so it is
//! scaled down, clipped and faded exactly as a picture is. The PNG is read
//! by the picture readers every picture goes through
//! ([`crate::picture::read`]), with their one bound on pixels, in the
//! process that paints — a renderer's.
//!
//! A run is shaped exactly as [`crate::render::outlined_glyphs_at`] shapes
//! it, so a picture sits where the outline would have, and the text after it
//! starts where layout measured.
//!
//! # What is not drawn
//!
//! A glyph whose picture does not read is drawn as nothing, as a glyph with
//! no outline is. A colour face whose glyphs are layers of outlines (`COLR`)
//! or SVG documents is not drawn in colour here: its outlines, where it has
//! them, are drawn as text (queue item 766).

use crate::canvas::Canvas;
use crate::glyph::picture_at;
use alo_layout::Rect;
use alo_text::{Direction, Font, shape_at};
use std::sync::Arc;

/// Every glyph of `text` in `font` that is a picture, where it is drawn with
/// the pen starting at `origin` on the baseline, at `size` on a screen of
/// `scale`, the letters `letter_spacing` apart: each one's place in CSS
/// pixels and its pixels. Nothing for a font whose glyphs are not pictures.
pub fn pictured_glyphs_at(
    text: &str,
    font: &Font,
    size: f32,
    scale: f32,
    letter_spacing: f32,
    origin: (f32, f32),
) -> Vec<(Rect, Arc<Canvas>)> {
    if !font.draws_pictures() {
        return Vec::new();
    }
    let run = alo_text::spaced(
        shape_at(text, font, size, scale, Direction::LeftToRight),
        letter_spacing,
    );
    // A run of one emoji repeated reads its picture once.
    let mut read: Vec<(u16, Arc<Canvas>)> = Vec::new();
    let mut drawn = Vec::new();
    let mut pen = origin;
    for glyph in &run.glyphs {
        if let Some(picture) = picture_at(font, glyph.glyph_id, size, scale) {
            let pixels = match read.iter().find(|(id, _)| *id == glyph.glyph_id) {
                Some((_, pixels)) => Some(Arc::clone(pixels)),
                None => crate::picture::read(&picture.png).ok().map(|canvas| {
                    let pixels = Arc::new(canvas);
                    read.push((glyph.glyph_id, Arc::clone(&pixels)));
                    pixels
                }),
            };
            if let Some(pixels) = pixels {
                let at = Rect::new(
                    pen.0 + glyph.offset.0 + picture.left,
                    pen.1 + glyph.offset.1 + picture.top,
                    picture.width,
                    picture.height,
                );
                drawn.push((at, pixels));
            }
        }
        pen.0 += glyph.advance;
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{Slant, Weight};

    /// The interface's emoji face, as `alo-renderer` carries it.
    fn emoji() -> Font {
        Font::load(
            "Noto Color Emoji",
            Weight::NORMAL,
            Slant::Normal,
            include_bytes!("../../alo-renderer/interface/fonts/NotoColorEmoji.ttf").to_vec(),
        )
        .expect("the face the interface carries")
    }

    fn sans() -> Font {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the font this crate is tested with")
    }

    #[test]
    fn an_emoji_is_a_picture_where_its_glyph_goes() {
        let font = emoji();
        assert!(font.draws_pictures());
        let drawn = pictured_glyphs_at("\u{1f600}", &font, 16.0, 1.0, 0.0, (10.0, 20.0));
        assert_eq!(drawn.len(), 1);
        let Some((at, pixels)) = drawn.first() else {
            return;
        };
        // Noto's strike is 109 to the em and the face 136 × 128: at 16 px,
        // 16/109 of that, with its foot 101 − 128 = −27 strike pixels under
        // the baseline at 20 and its left edge at the pen.
        let to_css = 16.0 / 109.0;
        assert!((at.size.width - 136.0 * to_css).abs() < 1.0e-4, "{at:?}");
        assert!((at.size.height - 128.0 * to_css).abs() < 1.0e-4, "{at:?}");
        assert!((at.left() - 10.0).abs() < 1.0e-4, "{at:?}");
        assert!(
            (at.top() - (20.0 - 101.0 * to_css)).abs() < 1.0e-3,
            "{at:?}"
        );
        assert_eq!((pixels.width(), pixels.height()), (136, 128));
        // It is in colour: the face's yellow, not the text's ink.
        let middle = pixels.at(68, 30).expect("inside the picture");
        assert!(
            middle.red > 0.8 && middle.green > 0.6 && middle.blue < 0.4,
            "{middle:?}"
        );
    }

    #[test]
    fn a_joined_sequence_is_one_picture_and_a_repeat_is_read_once() {
        let font = emoji();
        // A woman, a joiner and a laptop: one glyph in Noto.
        let one = pictured_glyphs_at(
            "\u{1f469}\u{200d}\u{1f4bb}",
            &font,
            16.0,
            1.0,
            0.0,
            (0.0, 0.0),
        );
        assert_eq!(one.len(), 1);
        let twice = pictured_glyphs_at("\u{1f600}\u{1f600}", &font, 16.0, 1.0, 0.0, (0.0, 0.0));
        assert_eq!(twice.len(), 2);
        if let [(first, one), (second, two)] = twice.as_slice() {
            assert!(Arc::ptr_eq(one, two), "the picture is read once");
            assert!(second.left() > first.left());
        }
    }

    #[test]
    fn a_face_of_outlines_has_no_pictures() {
        assert!(pictured_glyphs_at("\u{263a} a", &sans(), 16.0, 1.0, 0.0, (0.0, 0.0)).is_empty());
    }
}
