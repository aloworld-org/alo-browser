/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How big a piece of text is.
//!
//! **This is a seam, not a stub.** Layout genuinely does not know how wide
//! "Invoice 12" is: that needs a font, a shaper and a fallback chain, which is
//! queue item 6. So layout asks, and whoever calls layout answers.
//!
//! There is deliberately no default. A built-in guess — eight pixels a
//! character, say — would be a wrong number that every layout quietly depended
//! on, and law 3 says a wrong pixel is a bug. Making it a parameter means a
//! test says what it is measuring with, and item 6 arrives by implementing this
//! trait rather than by replacing something.

use crate::geometry::Size;

/// The font a piece of text is set in.
///
/// Layout carries this from the computed style to the measurer, because the
/// measurer has the fonts and layout has the styles and neither has both. It
/// is per *box*, not per document: a heading and a caption on the same page
/// are different sizes, and a measurer told only one of them would lay the
/// other one out wrong — which is exactly the bug this type exists to have
/// prevented.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// The families to try, in order, as `font-family` lists them.
    pub families: Vec<String>,
    /// How big, in CSS pixels.
    pub size: f32,
    /// How heavy, on CSS's nine-point scale.
    pub weight: u16,
    /// Whether it is slanted.
    pub italic: bool,
    /// Extra room after every character, in CSS pixels, or nothing.
    ///
    /// It changes what a run *measures*, so it changes where every line
    /// breaks — which is why it is here, with the font, rather than being a
    /// paint decision applied after the lines were settled.
    pub letter_spacing: f32,
    /// How the whitespace in it is treated.
    ///
    /// Collapsing has already happened by the time a measurer sees the text
    /// (`alo_box::WhiteSpace`); what is left for the line to know is whether a
    /// newline is a break it **must** take, and whether it may break anywhere
    /// at all.
    pub white_space: alo_box::WhiteSpace,
}

impl Default for TextStyle {
    /// Sixteen-pixel upright text in whatever the sans-serif is, which is what
    /// a document gets when nothing says otherwise.
    fn default() -> Self {
        Self {
            families: vec!["sans-serif".to_owned()],
            size: 16.0,
            weight: 400,
            italic: false,
            letter_spacing: 0.0,
            white_space: alo_box::WhiteSpace::Normal,
        }
    }
}

/// What layout needs to know about a piece of text.
pub trait MeasureText {
    /// How big this text is, given how much room it has.
    ///
    /// `available_width` is [`None`] when layout is asking how wide the text
    /// would like to be rather than how tall it would be at a given width —
    /// the max-content question. An implementation that ignores the difference
    /// will lay out correctly and size badly.
    fn measure(&self, text: &str, style: &TextStyle, available_width: Option<f32>) -> Size;

    /// The byte offsets after which a line may end, in order, always including
    /// the end of the text.
    ///
    /// A line box needs this as much as it needs widths: to break a sentence
    /// across two inline boxes it has to know where the words are, and where
    /// the words are is UAX #14 rather than "wherever the spaces are" — Thai
    /// has no spaces and a hyphen is a break that is not one. Layout does not
    /// know that either, so it asks.
    fn break_opportunities(&self, text: &str) -> Vec<usize>;

    /// How far above the bottom of a line of this text the baseline sits.
    ///
    /// Two pieces of text set in different sizes sit on the same baseline, not
    /// on the same top edge, which is the whole reason a line box is not a row
    /// of boxes — and the reason this takes the style rather than answering
    /// once for the document.
    fn ascender(&self, style: &TextStyle) -> f32;

    /// How far below the baseline it reaches.
    fn descender(&self, style: &TextStyle) -> f32;
}

/// A measurer for tests and for the cases where text has no size at all.
///
/// It reports nothing for everything, which is honest: with no font, there is
/// no width. Layout with this is layout of the boxes only, and a test that
/// uses it is testing the boxes only — which is most of them.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoText;

impl MeasureText for NoText {
    fn measure(&self, _text: &str, _style: &TextStyle, _available_width: Option<f32>) -> Size {
        Size::ZERO
    }

    fn break_opportunities(&self, text: &str) -> Vec<usize> {
        // The end of the text, which is where it stops rather than a break.
        // With no font there is nothing to break.
        vec![text.len()]
    }

    fn ascender(&self, _style: &TextStyle) -> f32 {
        0.0
    }

    fn descender(&self, _style: &TextStyle) -> f32 {
        0.0
    }
}

/// A measurer for tests: eight pixels a character on a line of sixteen, with
/// a twelve-pixel ascender, breaking at spaces.
///
/// It is a fake and it says so. Real fonts are `alo-text`, and a test that used
/// one would be testing the font rather than the layout.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockFont;

impl MeasureText for BlockFont {
    fn measure(&self, text: &str, style: &TextStyle, available_width: Option<f32>) -> Size {
        if text.is_empty() {
            return Size::ZERO;
        }
        let characters = f32::from(u16::try_from(text.chars().count()).unwrap_or(u16::MAX));
        // Even a fake font has to honour letter spacing, or a layout test
        // cannot tell whether spacing reached the measurer at all.
        let per_character = 8.0 + style.letter_spacing;
        let widest = characters * per_character;
        match available_width {
            Some(room) if room > 0.0 && widest > room => {
                let per_line = (room / per_character.max(0.001)).floor().max(1.0);
                let lines = (characters / per_line).ceil();
                Size::new(room, lines * 16.0)
            }
            _ => Size::new(widest, 16.0),
        }
    }

    fn break_opportunities(&self, text: &str) -> Vec<usize> {
        let mut points: Vec<usize> = text
            .char_indices()
            .filter(|(_, character)| *character == ' ')
            .map(|(offset, character)| offset + character.len_utf8())
            .collect();
        if points.last() != Some(&text.len()) {
            points.push(text.len());
        }
        points
    }

    fn ascender(&self, _style: &TextStyle) -> f32 {
        12.0
    }

    fn descender(&self, _style: &TextStyle) -> f32 {
        4.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_font_measures_eight_pixels_a_character() {
        let style = TextStyle::default();
        assert_eq!(
            BlockFont.measure("abc", &style, None),
            Size::new(24.0, 16.0)
        );
        assert_eq!(BlockFont.measure("", &style, None), Size::ZERO);
    }

    #[test]
    fn even_the_test_font_honours_letter_spacing() {
        let spaced = TextStyle {
            letter_spacing: 2.0,
            ..TextStyle::default()
        };
        assert_eq!(
            BlockFont.measure("abc", &spaced, None),
            Size::new(30.0, 16.0),
            "three characters of ten",
        );
    }

    #[test]
    fn a_measurer_is_told_whether_there_is_a_width_to_fit_into() {
        let style = TextStyle::default();
        assert_eq!(
            BlockFont.measure("abcdefgh", &style, Some(32.0)),
            Size::new(32.0, 32.0),
        );
        assert_eq!(
            BlockFont.measure("abcdefgh", &style, None),
            Size::new(64.0, 16.0),
        );
    }

    #[test]
    fn the_break_opportunities_always_end_at_the_end_of_the_text() {
        assert_eq!(BlockFont.break_opportunities("one two"), vec![4, 7]);
        assert_eq!(BlockFont.break_opportunities("one "), vec![4]);
        assert_eq!(BlockFont.break_opportunities(""), vec![0]);
        assert_eq!(NoText.break_opportunities("anything"), vec![8]);
    }

    #[test]
    fn with_no_font_there_is_no_size_and_no_baseline() {
        let style = TextStyle::default();
        assert_eq!(
            NoText.measure("anything at all", &style, Some(100.0)),
            Size::ZERO
        );
        assert!((NoText.ascender(&style) - 0.0).abs() < f32::EPSILON);
        assert!((NoText.descender(&style) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_line_of_the_test_font_is_its_ascender_plus_its_descender() {
        let style = TextStyle::default();
        assert!(
            (BlockFont.ascender(&style) + BlockFont.descender(&style) - 16.0).abs() < f32::EPSILON,
        );
    }
}
