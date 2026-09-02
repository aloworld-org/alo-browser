//! Closing the seam layout left open.
//!
//! `alo_layout::MeasureText` is deliberately unimplemented in the layout
//! crate: layout does not know how wide "Invoice 12" is, and a built-in guess
//! would have been a wrong number every layout quietly depended on. This is
//! the implementation, and it arrives by filling the seam rather than by
//! replacing anything.

use crate::database::FontDatabase;
use crate::font::FontRequest;
use crate::line::lay_out;
use alo_layout::{MeasureText, Size};

/// Measures text with real fonts.
///
/// Holds the font database and what the text is being set in — a family, a
/// weight, a size — because those are what layout does not carry. When
/// different parts of a document are set differently, each gets one of these;
/// building one is cheap, and the fonts inside are shared.
#[derive(Debug, Clone)]
pub struct TextMeasurer<'a> {
    database: &'a FontDatabase,
    request: FontRequest,
    size: f32,
}

impl<'a> TextMeasurer<'a> {
    /// A measurer for text set in this font at this size.
    pub fn new(database: &'a FontDatabase, request: FontRequest, size: f32) -> Self {
        Self {
            database,
            request,
            size,
        }
    }

    /// The size text is being set at, in CSS pixels.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// The fonts this measurer draws from.
    pub fn database(&self) -> &FontDatabase {
        self.database
    }
}

impl MeasureText for TextMeasurer<'_> {
    fn measure(&self, text: &str, available_width: Option<f32>) -> Size {
        let paragraph = lay_out(
            text,
            self.database,
            &self.request,
            self.size,
            available_width,
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

    fn ascender(&self) -> f32 {
        self.first_face()
            .map_or(self.size * 0.8, |metrics| metrics.ascender)
    }

    fn descender(&self) -> f32 {
        self.first_face()
            .map_or(self.size * 0.2, |metrics| metrics.descender)
    }
}

impl TextMeasurer<'_> {
    /// The metrics of the first font this request would use.
    ///
    /// A line's baseline comes from the font the text is set in; when a line
    /// mixes fonts the tallest wins, and that is the line box's business
    /// rather than this one's.
    fn first_face(&self) -> Option<crate::font::FaceMetrics> {
        self.database
            .chain(&self.request)
            .first()
            .map(|font| font.metrics(self.size))
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

    #[test]
    fn nothing_measures_as_nothing() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts, FontRequest::family("DejaVu Sans"), 16.0);
        assert_eq!(measurer.measure("", None), Size::ZERO);
    }

    #[test]
    fn text_has_a_width_and_a_height_that_come_from_the_font() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts, FontRequest::family("DejaVu Sans"), 16.0);
        let size = measurer.measure("Invoice 12", None);
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
        assert!((measurer.size() - 16.0).abs() < f32::EPSILON);
        assert_eq!(measurer.database().len(), 1);
    }

    #[test]
    fn asking_for_a_width_wraps_and_makes_it_taller() {
        let fonts = database();
        let measurer = TextMeasurer::new(&fonts, FontRequest::family("DejaVu Sans"), 16.0);
        let one_line = measurer.measure("the quick brown fox jumps", None);
        let wrapped = measurer.measure("the quick brown fox jumps", Some(one_line.width / 3.0));

        assert!(wrapped.width <= one_line.width);
        assert!(
            wrapped.height > one_line.height,
            "a third of the width takes more than one line",
        );
    }

    #[test]
    fn a_larger_size_measures_proportionally_larger() {
        let fonts = database();
        let small = TextMeasurer::new(&fonts, FontRequest::family("DejaVu Sans"), 16.0)
            .measure("hello", None);
        let large = TextMeasurer::new(&fonts, FontRequest::family("DejaVu Sans"), 32.0)
            .measure("hello", None);
        assert!((large.width - small.width * 2.0).abs() < 0.01);
    }

    #[test]
    fn with_no_fonts_at_all_text_takes_no_room_and_nothing_breaks() {
        let empty = FontDatabase::new();
        let measurer = TextMeasurer::new(&empty, FontRequest::family("Anything"), 16.0);
        assert_eq!(measurer.measure("hello", Some(100.0)), Size::ZERO);
    }
}
