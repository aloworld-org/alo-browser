//! A glyph, as a shape.
//!
//! **This is the only file that names `ttf-parser`.** A font's outlines are
//! its own format and reading them is specification work — exactly what ADR
//! 0001 says to rent — so the parser is rented and what comes out is a
//! [`crate::path::Path`] in our vocabulary, at the size asked for, with the
//! origin on the baseline where the pen sits.
//!
//! # The Y axis turns over here
//!
//! A font measures upwards from the baseline; a screen measures downwards from
//! the top. Every engine flips somewhere, and doing it here — once, at the
//! boundary — is what keeps every later stage from having to remember which
//! way a glyph's numbers go.

use crate::path::{Path, Point};
use alo_text::Font;

/// The shape of one glyph, at one size, with the pen at the origin.
///
/// Empty for a glyph that draws nothing, which a space legitimately is — and
/// which is different from a glyph that could not be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glyph {
    /// The outline, in CSS pixels, with the baseline at `y = 0` and positive
    /// `y` going down the screen.
    pub path: Path,
}

impl Glyph {
    /// Whether this glyph draws nothing — a space, or a mark with no ink.
    pub fn is_blank(&self) -> bool {
        self.path.is_empty()
    }
}

/// The shape of a glyph in a font, at a size.
///
/// [`None`] when the font cannot be read at all. A glyph that simply has no
/// outline — a space — comes back as an empty [`Glyph`], because "nothing to
/// draw" and "could not be read" are different answers and a caller that
/// confused them would draw a missing-glyph box for every space.
pub fn outline(font: &Font, glyph_id: u16, size: f32) -> Option<Glyph> {
    let face = ttf_parser::Face::parse(font.data(), font.index()).ok()?;
    let units = f32::from(face.units_per_em());
    if units <= 0.0 {
        return None;
    }
    let scale = size / units;

    let mut builder = Builder {
        path: Path::new(),
        scale,
    };
    // A glyph with no outline returns `None` here and is a blank glyph, not a
    // failure: `outline_glyph` says "nothing to draw", and a space has nothing
    // to draw.
    ttf_parser::Face::outline_glyph(&face, ttf_parser::GlyphId(glyph_id), &mut builder);
    Some(Glyph { path: builder.path })
}

/// Turns the font's outline calls into our path, scaled and flipped.
struct Builder {
    path: Path,
    scale: f32,
}

impl Builder {
    fn point(&self, x: f32, y: f32) -> Point {
        // Positive `y` is up in a font and down on a screen. This minus sign
        // is the whole of that difference, and it is here so that it is
        // nowhere else.
        Point::new(x * self.scale, -y * self.scale)
    }
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        let to = self.point(x, y);
        self.path.move_to(to);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.point(x, y);
        self.path.line_to(to);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.point(x1, y1);
        let to = self.point(x, y);
        self.path.quad_to(control, to);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let first = self.point(x1, y1);
        let second = self.point(x2, y2);
        let to = self.point(x, y);
        self.path.cubic_to(first, second, to);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{Direction, Slant, Weight, shape};

    fn sans() -> Font {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the font this crate is tested with")
    }

    /// The glyph a character shapes to in this font.
    fn glyph_of(font: &Font, character: char, size: f32) -> Glyph {
        let run = shape(&character.to_string(), font, size, Direction::LeftToRight);
        let id = run
            .glyphs
            .first()
            .map(|glyph| glyph.glyph_id)
            .expect("one glyph");
        outline(font, id, size).expect("a readable font")
    }

    #[test]
    fn a_letter_has_a_shape() {
        let glyph = glyph_of(&sans(), 'H', 100.0);
        assert!(!glyph.is_blank());
        assert!(glyph.path.segments().len() > 4, "an H is more than a line");
    }

    #[test]
    fn a_space_draws_nothing_which_is_not_the_same_as_failing() {
        let glyph = glyph_of(&sans(), ' ', 100.0);
        assert!(glyph.is_blank());
        assert!(glyph.path.bounds().is_none());
    }

    #[test]
    fn a_glyph_sits_on_the_baseline_with_the_screens_y_axis() {
        let glyph = glyph_of(&sans(), 'H', 100.0);
        let (_, top, _, bottom) = glyph.path.bounds().expect("a shape");
        assert!(
            top < 0.0,
            "a capital reaches above the baseline, which is negative y",
        );
        assert!(bottom <= 0.001, "and an H does not go below it: {bottom}");
    }

    #[test]
    fn a_descender_goes_below_the_baseline() {
        let glyph = glyph_of(&sans(), 'p', 100.0);
        let (_, _, _, bottom) = glyph.path.bounds().expect("a shape");
        assert!(bottom > 0.0, "the tail of a p is below the baseline");
    }

    #[test]
    fn a_glyph_scales_with_the_size_asked_for() {
        let font = sans();
        let small = glyph_of(&font, 'H', 50.0);
        let large = glyph_of(&font, 'H', 100.0);
        let (_, small_top, small_right, _) = small.path.bounds().expect("a shape");
        let (_, large_top, large_right, _) = large.path.bounds().expect("a shape");

        assert!((large_top - small_top * 2.0).abs() < 0.01);
        assert!((large_right - small_right * 2.0).abs() < 0.01);
    }

    #[test]
    fn a_font_that_cannot_be_read_gives_nothing_rather_than_a_blank_glyph() {
        let broken = Font::load("broken", Weight::NORMAL, Slant::Normal, vec![0; 64]);
        assert!(broken.is_none(), "and it does not load in the first place");
    }

    #[test]
    fn a_glyph_that_is_not_in_the_font_draws_nothing() {
        let font = sans();
        // Well past any glyph this face has.
        let glyph = outline(&font, u16::MAX, 100.0).expect("a readable font");
        assert!(glyph.is_blank());
    }
}
