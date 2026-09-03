/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Shaping: characters in, positioned glyphs out.
//!
//! **This is the only file in the repository that names `rustybuzz`.** ADR
//! 0001 rents text shaping — "nobody writes their own shaper, and not because
//! they are afraid of work" — and renting is only true if the rented thing
//! stays behind a boundary. `scripts/gate.sh` checks it.
//!
//! # Why the awkward scripts first
//!
//! `docs/features.md` asks for the awkward scripts before the easy ones,
//! because *a pipeline that assumed left-to-right and one glyph per character
//! is a pipeline that gets rewritten.* So a [`ShapedGlyph`] carries the byte
//! range of the text it came from rather than an index into it, and several
//! glyphs may name the same range or one glyph several characters:
//!
//! - **Arabic** joins: four characters become four glyphs, but *which* glyph
//!   depends on the neighbours, and a ligature turns two into one.
//! - **Hebrew and Arabic** run right to left, so the first glyph is at the
//!   right-hand end and the advances still accumulate the same way.
//! - **Combining marks** put two characters at one position with one advance.
//!
//! None of that is special-cased here. It comes out of the shaper, and this
//! file's job is to not throw it away on the way past.

use crate::font::Font;
use core::fmt;
use core::ops::Range;

/// Which way a run of text is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left to right.
    #[default]
    LeftToRight,
    /// Right to left.
    RightToLeft,
}

impl Direction {
    /// Whether this is right to left.
    pub fn is_rtl(self) -> bool {
        self == Direction::RightToLeft
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Direction::LeftToRight => "ltr",
            Direction::RightToLeft => "rtl",
        })
    }
}

/// One glyph, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    /// Which glyph in the font.
    pub glyph_id: u16,
    /// How far the pen moves after drawing it, in CSS pixels.
    pub advance: f32,
    /// How far to move the glyph before drawing it, without moving the pen.
    pub offset: (f32, f32),
    /// The bytes of the original text this glyph came from.
    ///
    /// A range rather than an index, because the relationship is not one to
    /// one in either direction: a ligature covers several characters, and a
    /// character with a combining mark produces several glyphs covering the
    /// same bytes.
    pub source: Range<usize>,
}

/// A run of text, shaped.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedRun {
    /// The glyphs, in the order they are drawn.
    pub glyphs: Vec<ShapedGlyph>,
    /// Which way the run is set.
    pub direction: Direction,
    /// How wide the whole run is, in CSS pixels.
    pub width: f32,
    /// How far above the baseline this run reaches.
    pub ascender: f32,
    /// How far below it.
    pub descender: f32,
}

impl ShapedRun {
    /// A run with nothing in it.
    pub fn empty() -> Self {
        Self {
            glyphs: Vec::new(),
            direction: Direction::LeftToRight,
            width: 0.0,
            ascender: 0.0,
            descender: 0.0,
        }
    }

    /// Whether the run has no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// The same run with `letter-spacing` added after every glyph.
///
/// A separate step rather than a parameter to the shaper, and deliberately:
/// shaping is the rented part, and letter spacing is a CSS decision about what
/// to do with the result. Adding it here keeps the boundary in
/// `rustybuzz`'s file exactly where it was.
///
/// **After every glyph, the last one included**, which is what browsers do —
/// a spaced run is wider than the text by one spacing per character, and a
/// line that measured it otherwise would break in the wrong place.
pub fn spaced(run: ShapedRun, spacing: f32) -> ShapedRun {
    if spacing == 0.0 || run.glyphs.is_empty() {
        return run;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a run is at most a few thousand glyphs"
    )]
    let count = run.glyphs.len() as f32;
    ShapedRun {
        width: run.width + spacing * count,
        glyphs: run
            .glyphs
            .into_iter()
            .map(|glyph| ShapedGlyph {
                advance: glyph.advance + spacing,
                ..glyph
            })
            .collect(),
        ..run
    }
}

/// Shape a run of text with one font at one size.
///
/// The run must be in one direction and one script — see [`crate::run`], which
/// is what splits text into runs that satisfy that. `direction` is what the
/// caller worked out; passing it rather than guessing per call is what keeps
/// one word of Arabic inside an English sentence from turning the sentence
/// around.
pub fn shape(text: &str, font: &Font, size: f32, direction: Direction) -> ShapedRun {
    if text.is_empty() {
        return ShapedRun::empty();
    }
    let Some(face) = rustybuzz::Face::from_slice(font.data(), font.index()) else {
        return ShapedRun::empty();
    };
    let metrics = font.metrics(size);

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(match direction {
        Direction::LeftToRight => rustybuzz::Direction::LeftToRight,
        Direction::RightToLeft => rustybuzz::Direction::RightToLeft,
    });
    // The script and the language are still the shaper's to work out; only the
    // direction is decided above, because that one spans several runs.
    buffer.guess_segment_properties();

    let shaped = rustybuzz::shape(&face, &[], buffer);
    let units = f32::from(u16::try_from(face.units_per_em()).unwrap_or(1000));
    let scale = if units > 0.0 { size / units } else { 0.0 };

    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    let mut glyphs = Vec::with_capacity(infos.len());
    let mut width = 0.0;

    for (index, (info, position)) in infos.iter().zip(positions.iter()).enumerate() {
        let advance = as_f32(position.x_advance) * scale;
        width += advance;
        glyphs.push(ShapedGlyph {
            glyph_id: u16::try_from(info.glyph_id).unwrap_or(0),
            advance,
            offset: (
                as_f32(position.x_offset) * scale,
                as_f32(position.y_offset) * scale,
            ),
            source: source_range(text, infos, index, direction),
        });
    }

    ShapedRun {
        glyphs,
        direction,
        width,
        ascender: metrics.ascender,
        descender: metrics.descender,
    }
}

/// The byte range one glyph came from.
///
/// The shaper gives each glyph the byte offset of the cluster it belongs to.
/// The range runs from that offset to the next cluster's — which, in a
/// right-to-left run, is the *previous* glyph's, because the glyphs come out
/// in drawing order and the text does not.
fn source_range(
    text: &str,
    infos: &[rustybuzz::GlyphInfo],
    index: usize,
    direction: Direction,
) -> Range<usize> {
    let start = infos.get(index).map_or(0, |info| info.cluster as usize);
    let neighbour = if direction.is_rtl() {
        index.checked_sub(1)
    } else {
        Some(index + 1)
    };
    let end = neighbour
        .and_then(|at| infos.get(at))
        .map_or(text.len(), |info| info.cluster as usize);
    if end > start {
        start..end
    } else {
        // Several glyphs in one cluster: they all name the same bytes, and the
        // last one in the cluster is the one that reaches the next.
        start..start
    }
}

fn as_f32(value: i32) -> f32 {
    // A font's units are small numbers; anything outside this range is a
    // broken font rather than a wide glyph.
    let clamped = value.clamp(-(1 << 23), 1 << 23);
    #[expect(
        clippy::cast_precision_loss,
        reason = "clamped above to a range f32 represents exactly"
    )]
    let converted = clamped as f32;
    converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Slant, Weight};

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
    fn shaping_nothing_gives_nothing() {
        let run = shape("", &sans(), 16.0, Direction::LeftToRight);
        assert!(run.is_empty());
        assert!((run.width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn latin_gives_one_glyph_a_character_and_a_width_that_adds_up() {
        let run = shape("abc", &sans(), 16.0, Direction::LeftToRight);
        assert_eq!(run.glyphs.len(), 3);
        assert_eq!(run.direction, Direction::LeftToRight);

        let summed: f32 = run.glyphs.iter().map(|glyph| glyph.advance).sum();
        assert!((run.width - summed).abs() < 0.001);
        assert!(run.width > 0.0);
        assert!(run.ascender > 0.0 && run.descender > 0.0);
    }

    #[test]
    fn a_wider_size_is_a_proportionally_wider_run() {
        let small = shape("abc", &sans(), 16.0, Direction::LeftToRight);
        let large = shape("abc", &sans(), 32.0, Direction::LeftToRight);
        assert!((large.width - small.width * 2.0).abs() < 0.01);
    }

    #[test]
    fn every_glyph_names_the_bytes_it_came_from() {
        let text = "abc";
        let run = shape(text, &sans(), 16.0, Direction::LeftToRight);
        let sources: Vec<Range<usize>> = run
            .glyphs
            .iter()
            .map(|glyph| glyph.source.clone())
            .collect();
        assert_eq!(sources, vec![0..1, 1..2, 2..3]);
        for glyph in &run.glyphs {
            assert!(text.get(glyph.source.clone()).is_some(), "a real range");
        }
    }

    #[test]
    fn arabic_is_shaped_right_to_left_and_the_glyphs_are_not_the_characters() {
        // "مرحبا" — five characters, joined, and the shaper decides which form
        // each takes from its neighbours.
        let text = "مرحبا";
        let run = shape(text, &sans(), 16.0, Direction::RightToLeft);

        assert_eq!(run.direction, Direction::RightToLeft);
        assert!(!run.is_empty());
        assert!(run.width > 0.0);
        assert!(
            run.glyphs.iter().all(|glyph| glyph.glyph_id != 0),
            "no missing glyphs: this font covers Arabic",
        );

        // The glyphs come out in drawing order, so their source offsets run
        // *down* the text rather than up. A pipeline that assumed otherwise
        // would map every click to the wrong letter.
        let starts: Vec<usize> = run.glyphs.iter().map(|glyph| glyph.source.start).collect();
        assert!(
            starts.windows(2).all(|pair| pair[0] >= pair[1]),
            "expected descending source offsets, got {starts:?}",
        );
    }

    #[test]
    fn shaping_the_same_text_the_wrong_way_round_gives_a_different_answer() {
        let text = "مرحبا";
        let rtl = shape(text, &sans(), 16.0, Direction::RightToLeft);
        let ltr = shape(text, &sans(), 16.0, Direction::LeftToRight);
        assert_ne!(
            rtl.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>(),
            ltr.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>(),
            "direction is not decoration: it changes which glyphs are drawn",
        );
    }

    #[test]
    fn two_characters_can_be_one_glyph() {
        // "e" followed by a combining acute. The shaper composes them into the
        // single glyph this font has for "é" — two characters, one glyph, one
        // advance. A pipeline that indexed glyphs by character would have put
        // the caret in the wrong place from here on.
        let text = "e\u{0301}";
        assert_eq!(text.chars().count(), 2);

        let run = shape(text, &sans(), 16.0, Direction::LeftToRight);
        assert_eq!(run.glyphs.len(), 1, "one glyph for two characters");

        let glyph = run.glyphs.first().expect("the composed glyph");
        assert_eq!(
            glyph.source,
            0..text.len(),
            "and it names both characters' bytes, not the first one's",
        );
        assert_eq!(
            run.glyphs.len(),
            shape("é", &sans(), 16.0, Direction::LeftToRight)
                .glyphs
                .len(),
            "which is the same glyph the precomposed character gives",
        );
    }

    #[test]
    fn a_character_the_font_lacks_shapes_to_the_missing_glyph() {
        let run = shape("क", &sans(), 16.0, Direction::LeftToRight);
        assert_eq!(run.glyphs.len(), 1);
        assert_eq!(
            run.glyphs.first().map(|glyph| glyph.glyph_id),
            Some(0),
            "glyph zero is the box a person sees when nothing has the character",
        );
    }

    #[test]
    fn a_direction_says_which_way_it_goes() {
        assert!(Direction::RightToLeft.is_rtl());
        assert!(!Direction::LeftToRight.is_rtl());
        assert_eq!(Direction::LeftToRight.to_string(), "ltr");
        assert_eq!(Direction::RightToLeft.to_string(), "rtl");
        assert_eq!(Direction::default(), Direction::LeftToRight);
    }
}
