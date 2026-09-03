/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font, and the numbers that come out of one.
//!
//! `docs/features.md`: shaping and rasterisation are **rented**, as every
//! engine rents them. What is ours is which font is chosen, when one is given
//! up on, and how a line is put together — so a font here is a small thing:
//! some bytes, a name, and the handful of measurements the rest of the engine
//! asks for.
//!
//! Every measurement is in CSS pixels at a given size, rather than in the
//! font's own units. A caller that had to divide by `units_per_em` would be a
//! caller that could forget to.

use core::fmt;
use std::sync::Arc;

/// How heavy a face is, on CSS's nine-point scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Weight(u16);

impl Weight {
    /// `normal`.
    pub const NORMAL: Weight = Weight(400);
    /// `bold`.
    pub const BOLD: Weight = Weight(700);

    /// A weight, clamped to the range CSS allows.
    pub fn new(value: u16) -> Self {
        Self(value.clamp(1, 1000))
    }

    /// The number.
    pub fn value(self) -> u16 {
        self.0
    }
}

impl Default for Weight {
    fn default() -> Self {
        Weight::NORMAL
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether a face is upright or slanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Slant {
    /// Upright.
    #[default]
    Normal,
    /// Slanted.
    Italic,
}

/// What a caller is asking for when it asks for a font.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontRequest {
    /// The families to try, in order, as `font-family` lists them.
    pub families: Vec<String>,
    /// How heavy.
    pub weight: Weight,
    /// Upright or slanted.
    pub slant: Slant,
}

impl FontRequest {
    /// A request for one family at the ordinary weight.
    pub fn family(name: &str) -> Self {
        Self {
            families: vec![name.to_owned()],
            ..Self::default()
        }
    }

    /// The families a `font-family` value lists, in order, with quotes removed.
    ///
    /// The generic families — `sans-serif`, `serif`, `monospace` — are kept as
    /// written rather than resolved here: which font is a sans-serif is the
    /// font database's business, and it is the one thing that differs between
    /// one machine and another.
    pub fn parse_families(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|part| {
                part.trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .trim()
                    .to_owned()
            })
            .filter(|part| !part.is_empty())
            .collect()
    }
}

/// The measurements of a face, in CSS pixels at one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceMetrics {
    /// How far above the baseline the face reaches.
    pub ascender: f32,
    /// How far below it, as a positive number.
    pub descender: f32,
    /// The extra room the face asks for between lines.
    pub line_gap: f32,
    /// The height of a lowercase `x`, for the `ex` unit.
    pub x_height: f32,
    /// The width of a `0`, for the `ch` unit.
    pub zero_width: f32,
    /// How far **below** the baseline an underline sits, as a positive number.
    ///
    /// The face's own figure, because where a line goes depends on how far the
    /// letters descend — a font with long descenders puts its underline lower
    /// so the line does not cut through them.
    pub underline_offset: f32,
    /// How thick that line is.
    pub underline_thickness: f32,
}

impl FaceMetrics {
    /// The line height this face suggests: everything it reaches plus the room
    /// it asks for between lines.
    pub fn suggested_line_height(self) -> f32 {
        self.ascender + self.descender + self.line_gap
    }
}

/// A loaded font.
///
/// The bytes are held behind an [`Arc`] because a face borrows from them and
/// the same face is used from several places; cloning a `Font` is cheap and
/// does not copy a megabyte of glyphs.
#[derive(Clone)]
pub struct Font {
    family: Arc<str>,
    weight: Weight,
    slant: Slant,
    data: Arc<Vec<u8>>,
    index: u32,
}

impl Font {
    /// Load a font from its bytes.
    ///
    /// Returns [`None`] if the bytes are not a font this engine can read —
    /// which is a real answer, not an error to be swallowed: a font that will
    /// not parse should be skipped and the next one tried.
    pub fn load(family: &str, weight: Weight, slant: Slant, data: Vec<u8>) -> Option<Self> {
        let font = Self {
            family: family.into(),
            weight,
            slant,
            data: Arc::new(data),
            index: 0,
        };
        // Parsing once here means every later use can assume it parses.
        font.face()?;
        Some(font)
    }

    /// The family this font belongs to.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// How heavy it is.
    pub fn weight(&self) -> Weight {
        self.weight
    }

    /// Whether it is slanted.
    pub fn slant(&self) -> Slant {
        self.slant
    }

    /// The bytes, for whoever needs to shape or rasterise with them.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Which face within the bytes, for a collection.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Whether this font has a glyph for a character.
    ///
    /// This is the question the fallback chain asks, and it is asked of the
    /// font rather than guessed from a language tag — a font either has the
    /// glyph or it does not, and there is nothing to infer.
    pub fn has_glyph(&self, character: char) -> bool {
        self.face()
            .and_then(|face| face.glyph_index(character))
            .is_some()
    }

    /// The measurements of this font at a size, in CSS pixels.
    pub fn metrics(&self, size: f32) -> FaceMetrics {
        let Some(face) = self.face() else {
            return FaceMetrics {
                ascender: size * 0.8,
                descender: size * 0.2,
                line_gap: 0.0,
                x_height: size * 0.5,
                zero_width: size * 0.5,
                underline_offset: size * 0.1,
                underline_thickness: size * 0.06,
            };
        };
        let units = f32::from(face.units_per_em());
        let scale = if units > 0.0 { size / units } else { 0.0 };
        FaceMetrics {
            ascender: f32::from(face.ascender()) * scale,
            descender: f32::from(face.descender()).abs() * scale,
            line_gap: f32::from(face.line_gap()) * scale,
            x_height: face
                .x_height()
                .map_or(size * 0.5, |height| f32::from(height) * scale),
            zero_width: face
                .glyph_index('0')
                .and_then(|glyph| face.glyph_hor_advance(glyph))
                .map_or(size * 0.5, |advance| f32::from(advance) * scale),
            // The face reports the position as a signed offset from the
            // baseline, negative for below it, which is where an underline
            // goes. Kept as a positive distance downwards, so that everything
            // reading it does not have to remember the sign.
            underline_offset: face
                .underline_metrics()
                .map_or(size * 0.1, |line| -f32::from(line.position) * scale),
            underline_thickness: face
                .underline_metrics()
                .map(|line| f32::from(line.thickness) * scale)
                // A face that reports no thickness, or a nonsensical one, gets
                // a line that is visible at every size rather than none at all.
                .filter(|thickness| *thickness > 0.0)
                .unwrap_or(size * 0.06),
        }
    }

    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }
}

impl fmt::Debug for Font {
    /// The name and the weight — never the bytes, which are a megabyte of
    /// glyphs nobody wants in a test failure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Font({} {} {:?})", self.family, self.weight, self.slant)
    }
}

/// `ttf-parser` is the font parser `rustybuzz` itself is built on, so reading a
/// face here and shaping with it later are the same parse of the same bytes.
use rustybuzz::ttf_parser;

#[cfg(test)]
mod tests {
    use super::*;

    fn dejavu() -> Font {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the DejaVu Sans this crate is tested with")
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.01
    }

    #[test]
    fn a_font_that_is_not_a_font_is_refused_rather_than_held() {
        assert!(Font::load("nonsense", Weight::NORMAL, Slant::Normal, vec![0; 64]).is_none());
        assert!(Font::load("empty", Weight::NORMAL, Slant::Normal, Vec::new()).is_none());
    }

    #[test]
    fn a_loaded_font_reports_what_it_was_loaded_as() {
        let font = dejavu();
        assert_eq!(font.family(), "DejaVu Sans");
        assert_eq!(font.weight(), Weight::NORMAL);
        assert_eq!(font.slant(), Slant::Normal);
        assert!(!font.data().is_empty());
        assert_eq!(font.index(), 0);
        assert_eq!(format!("{font:?}"), "Font(DejaVu Sans 400 Normal)");
    }

    #[test]
    fn a_font_knows_which_characters_it_has() {
        let font = dejavu();
        assert!(font.has_glyph('a'));
        assert!(font.has_glyph('é'));
        assert!(font.has_glyph('م'), "DejaVu Sans covers Arabic");
        assert!(
            !font.has_glyph('क'),
            "and not Devanagari, which is what the fallback chain is for",
        );
    }

    #[test]
    fn the_metrics_scale_with_the_size_asked_for() {
        let font = dejavu();
        let small = font.metrics(16.0);
        let large = font.metrics(32.0);
        assert!(close(large.ascender, small.ascender * 2.0));
        assert!(close(large.x_height, small.x_height * 2.0));
        assert!(close(large.zero_width, small.zero_width * 2.0));
        assert!(small.ascender > 0.0 && small.descender > 0.0);
        assert!(close(
            small.suggested_line_height(),
            small.ascender + small.descender + small.line_gap,
        ),);
    }

    #[test]
    fn a_font_family_list_is_read_in_order_and_unquoted() {
        assert_eq!(
            FontRequest::parse_families("Inter, \"Helvetica Neue\", system-ui, sans-serif"),
            vec!["Inter", "Helvetica Neue", "system-ui", "sans-serif"],
        );
        assert_eq!(
            FontRequest::parse_families("  'One Font'  "),
            vec!["One Font"]
        );
        assert_eq!(
            FontRequest::parse_families(",,"),
            Vec::<String>::new(),
            "a list of nothing is a list of nothing",
        );
    }

    #[test]
    fn a_weight_is_kept_inside_the_range_css_allows() {
        assert_eq!(Weight::new(0).value(), 1);
        assert_eq!(Weight::new(5000).value(), 1000);
        assert_eq!(Weight::new(600).value(), 600);
        assert_eq!(Weight::default(), Weight::NORMAL);
        assert!(Weight::BOLD > Weight::NORMAL);
        assert_eq!(Weight::BOLD.to_string(), "700");
    }

    #[test]
    fn a_request_for_one_family_is_a_request_for_one_family() {
        let request = FontRequest::family("Inter");
        assert_eq!(request.families, vec!["Inter"]);
        assert_eq!(request.weight, Weight::NORMAL);
        assert_eq!(request.slant, Slant::Normal);
    }
}
