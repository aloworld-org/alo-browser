/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Closing the seam layout left open.
//!
//! `alo_layout::MeasureText` is deliberately unimplemented in the layout
//! crate: layout does not know how wide "Invoice 12" is, and a built-in guess
//! would have been a wrong number every layout quietly depended on. This is
//! the implementation, and it arrives by filling the seam rather than by
//! replacing anything.

use crate::database::FontDatabase;
use crate::font::{FontRequest, Slant, Weight};
use crate::line::lay_out;
use alo_layout::{MeasureText, Size, TextStyle};

/// Measures text with real fonts.
///
/// Holds only the font database. **What font a piece of text is set in comes
/// with the text**, from `alo_layout::TextStyle`, because it is a fact about
/// the box rather than about the document — a heading and a caption on the
/// same page are different sizes, and a measurer that answered once for the
/// whole document laid one of them out wrong.
#[derive(Debug, Clone, Copy)]
pub struct TextMeasurer<'a> {
    database: &'a FontDatabase,
}

impl<'a> TextMeasurer<'a> {
    /// A measurer drawing from these fonts.
    pub fn new(database: &'a FontDatabase) -> Self {
        Self { database }
    }

    /// The fonts this measurer draws from.
    pub fn database(&self) -> &FontDatabase {
        self.database
    }

    /// What a `TextStyle` is asking for, in this crate's vocabulary.
    fn request(style: &TextStyle) -> FontRequest {
        FontRequest {
            families: style.families.clone(),
            weight: Weight::new(style.weight),
            slant: if style.italic {
                Slant::Italic
            } else {
                Slant::Normal
            },
        }
    }
}

impl MeasureText for TextMeasurer<'_> {
    fn measure(&self, text: &str, style: &TextStyle, available_width: Option<f32>) -> Size {
        let paragraph = lay_out(
            text,
            self.database,
            &Self::request(style),
            style.size,
            available_width,
            style.letter_spacing,
        );
        Size::new(paragraph.width(), paragraph.height())
    }

    fn break_opportunities(&self, text: &str) -> Vec<usize> {
        let mut points: Vec<usize> = crate::linebreak::opportunities(text)
            .into_iter()
            .map(|point| point.offset)
            .collect();
        // Layout relies on the last opportunity being the end of the text.
        // UAX #14 gives that for any text with something in it, and this is
        // what makes the empty case behave the same way.
        if points.last() != Some(&text.len()) {
            points.push(text.len());
        }
        points
    }

    fn ascender(&self, style: &TextStyle) -> f32 {
        self.face(style)
            .map_or(style.size * 0.8, |metrics| metrics.ascender)
    }

    fn descender(&self, style: &TextStyle) -> f32 {
        self.face(style)
            .map_or(style.size * 0.2, |metrics| metrics.descender)
    }
}

impl TextMeasurer<'_> {
    /// The metrics of the first font this style would use.
    ///
    /// A line's baseline comes from the font the text is set in; when a line
    /// mixes fonts the tallest wins, and that is the line box's business
    /// rather than this one's.
    fn face(self, style: &TextStyle) -> Option<crate::font::FaceMetrics> {
        self.database
            .chain(&Self::request(style))
            .first()
            .map(|font| font.metrics(style.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, Slant, Weight};

    fn database() -> FontDatabase {
        let mut database = FontDatabase::new();
        if let Some(font) = Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        ) {
            database.add(font);
        }
        database
    }

    fn style(size: f32) -> TextStyle {
        TextStyle {
            families: vec!["DejaVu Sans".to_owned()],
            size,
            ..TextStyle::default()
        }
    }

    #[test]
    fn nothing_measures_as_nothing() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts);
        assert_eq!(measurer.measure("", &style(16.0), None), Size::ZERO);
    }

    #[test]
    fn text_has_a_width_and_a_height_that_come_from_the_font() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts);
        let size = measurer.measure("Invoice 12", &style(16.0), None);
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
        assert_eq!(measurer.database().len(), 1);
    }

    #[test]
    fn the_size_comes_with_the_text_rather_than_with_the_measurer() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts);
        let small = measurer.measure("Invoices", &style(14.0), None);
        let large = measurer.measure("Invoices", &style(28.0), None);
        assert!(
            (large.width - small.width * 2.0).abs() < 0.01,
            "one measurer, two sizes: {} against {}",
            large.width,
            small.width,
        );
        assert!(measurer.ascender(&style(28.0)) > measurer.ascender(&style(14.0)));
    }

    #[test]
    fn asking_for_a_width_wraps_and_makes_it_taller() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts);
        let one_line = measurer.measure("the quick brown fox jumps", &style(16.0), None);
        let wrapped = measurer.measure(
            "the quick brown fox jumps",
            &style(16.0),
            Some(one_line.width / 3.0),
        );

        assert!(wrapped.width <= one_line.width);
        assert!(
            wrapped.height > one_line.height,
            "a third of the width takes more than one line",
        );
    }

    #[test]
    fn a_larger_size_measures_proportionally_larger() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts);
        let small = measurer.measure("hello", &style(16.0), None);
        let large = measurer.measure("hello", &style(32.0), None);
        assert!((large.width - small.width * 2.0).abs() < 0.01);
    }

    #[test]
    fn with_no_fonts_at_all_text_takes_no_room_and_nothing_breaks() {
        let empty = FontDatabase::new();
        let measurer = TextMeasurer::new(&empty);
        assert_eq!(
            measurer.measure("hello", &style(16.0), Some(100.0)),
            Size::ZERO
        );
    }
}
