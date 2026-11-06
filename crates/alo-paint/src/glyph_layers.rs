/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A run's glyphs that are layers of colour, where each layer is drawn
//! (queue item 766).
//!
//! A `COLR` version 0 face — Windows' Segoe UI Emoji — draws a colour glyph
//! as outlines stacked bottom first, each filled in a colour of the font's
//! palette or in the text's own ([`crate::glyph::layers_at`]). Such a glyph
//! often keeps an outline of its own too, a plain one for programs that
//! read no colour; drawn in the text's colour under the layers it would
//! show at their smoothed edges, so the run's letters leave it out
//! ([`crate::render::outlined_glyphs_at`]) and its layers are placed here,
//! glyph by glyph, to be drawn after the run's letters as one group each,
//! at the text colour's alpha ([`crate::glyph_fade`], queue item 830).
//!
//! A run is shaped exactly as the letters are, so each layer sits where the
//! glyph's outline would have.
//!
//! # What is not drawn
//!
//! A `COLR` version 1 glyph — gradients, transforms, clips, groups, Noto's
//! vector build — and an `SVG ` glyph are drawn as their outlines in the
//! text's colour, as before (queue item 785). A layered glyph casts its
//! layers as its `text-shadow` ([`crate::glyph_shadow`], 786); a line under
//! it does not step aside for it, nor for its own plain outline, as Chrome
//! 156's does not.

use crate::glyph::{GlyphLayer, PaintedGlyph, layers_at, painted};
use crate::path::Point;
use alo_text::{Direction, Font, shape_at};

/// Every glyph of `text` in `font` that is drawn in layers of colour, each
/// as its layers, bottom first, where it is drawn with the pen at `origin`
/// on the baseline, at `size` on a screen of `scale`, the letters
/// `letter_spacing` apart — what each draws as one group
/// ([`crate::glyph_fade`], queue item 830) and casts as a shadow of its own
/// ([`crate::glyph_shadow`], 786). Nothing for a font with no such glyphs.
pub fn each_layered_glyph_at(
    text: &str,
    font: &Font,
    size: f32,
    scale: f32,
    letter_spacing: f32,
    origin: (f32, f32),
) -> Vec<Vec<GlyphLayer>> {
    if !font.draws_layers() {
        return Vec::new();
    }
    let run = alo_text::spaced(
        shape_at(text, font, size, scale, Direction::LeftToRight),
        letter_spacing,
    );
    let mut drawn = Vec::new();
    let mut pen = origin;
    for glyph in &run.glyphs {
        if let Some(layers) = layers_at(font, glyph.glyph_id, size, scale) {
            let at = Point::new(pen.0 + glyph.offset.0, pen.1 + glyph.offset.1);
            drawn.push(
                layers
                    .into_iter()
                    .map(|layer| GlyphLayer {
                        path: layer.path.translated(at),
                        color: layer.color,
                    })
                    .collect(),
            );
        }
        pen.0 += glyph.advance;
    }
    drawn
}

/// Every glyph of `text` in `font` drawn by a paint program of `COLR`
/// version 1 (queue item 785) — one [`layers_at`] does not take as flat
/// layers — with where its pen is, the text's own colour `foreground`
/// standing for `0xFFFF`. Nothing for a font with no such glyphs.
pub fn each_painted_glyph_at(
    text: &str,
    font: &Font,
    (size, scale): (f32, f32),
    letter_spacing: f32,
    origin: (f32, f32),
    foreground: alo_value::Rgba,
) -> Vec<((f32, f32), PaintedGlyph)> {
    if !font.draws_layers() {
        return Vec::new();
    }
    let run = alo_text::spaced(
        shape_at(text, font, size, scale, Direction::LeftToRight),
        letter_spacing,
    );
    let mut drawn = Vec::new();
    let mut pen = origin;
    for glyph in &run.glyphs {
        if layers_at(font, glyph.glyph_id, size, scale).is_none()
            && let Some(painted) = painted(font, glyph.glyph_id, (size, scale), foreground)
        {
            drawn.push(((pen.0 + glyph.offset.0, pen.1 + glyph.offset.1), painted));
        }
        pen.0 += glyph.advance;
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{Slant, Weight};

    fn layered() -> Font {
        Font::load(
            "alo Layered Test",
            Weight::NORMAL,
            Slant::Normal,
            include_bytes!("../tests/fonts/layered.ttf").to_vec(),
        )
        .expect("the layered test face")
    }

    #[test]
    fn each_layered_glyph_is_placed_at_its_pen() {
        // `A` is plain and a whole em wide; `B` is layered: its three
        // layers start one em along, here 20 px, and 4 down.
        let glyphs = each_layered_glyph_at("AB", &layered(), 20.0, 1.0, 0.0, (5.0, 4.0));
        assert_eq!(glyphs.len(), 1, "B alone is layered");
        let layers = glyphs.concat();
        assert_eq!(layers.len(), 3);
        let back = layers.first().and_then(|layer| layer.path.bounds());
        let Some((left, top, right, bottom)) = back else {
            panic!("the back layer has an outline");
        };
        assert!((left - 27.0).abs() < 1e-3, "{left}");
        assert!((right - 43.0).abs() < 1e-3, "{right}");
        assert!((top - (4.0 - 16.0)).abs() < 1e-3, "{top}");
        assert!((bottom - 4.0).abs() < 1e-3, "{bottom}");
    }

    #[test]
    fn a_face_with_no_layers_places_none() {
        let sans = Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the font this crate is tested with");
        assert!(each_layered_glyph_at("AB", &sans, 20.0, 1.0, 0.0, (0.0, 0.0)).is_empty());
    }
}
