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

/// What layout needs to know about a piece of text.
pub trait MeasureText {
    /// How big this text is, given how much room it has.
    ///
    /// `available_width` is [`None`] when layout is asking how wide the text
    /// would like to be rather than how tall it would be at a given width —
    /// the max-content question. An implementation that ignores the difference
    /// will lay out correctly and size badly.
    fn measure(&self, text: &str, available_width: Option<f32>) -> Size;
}

impl<F> MeasureText for F
where
    F: Fn(&str, Option<f32>) -> Size,
{
    fn measure(&self, text: &str, available_width: Option<f32>) -> Size {
        self(text, available_width)
    }
}

/// A measurer for tests and for the cases where text has no size at all.
///
/// It reports nothing for everything, which is honest: with no font, there is
/// no width. Layout with this is layout of the boxes only, and a test that
/// uses it is testing the boxes only — which is most of them.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoText;

impl MeasureText for NoText {
    fn measure(&self, _text: &str, _available_width: Option<f32>) -> Size {
        Size::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many characters, as a number a width can be built from.
    fn characters(text: &str) -> f32 {
        f32::from(u16::try_from(text.chars().count()).unwrap_or(u16::MAX))
    }

    #[test]
    fn a_closure_is_a_measurer() {
        let measure = |text: &str, _: Option<f32>| Size::new(characters(text) * 8.0, 16.0);
        assert_eq!(measure.measure("abc", None), Size::new(24.0, 16.0));
    }

    #[test]
    fn a_measurer_is_told_whether_there_is_a_width_to_fit_into() {
        let measure = |text: &str, available: Option<f32>| match available {
            Some(width) => Size::new(width, if text.len() > 4 { 32.0 } else { 16.0 }),
            None => Size::new(characters(text) * 8.0, 16.0),
        };
        assert_eq!(measure.measure("hello", Some(40.0)), Size::new(40.0, 32.0));
        assert_eq!(measure.measure("hello", None), Size::new(40.0, 16.0));
    }

    #[test]
    fn with_no_font_there_is_no_size() {
        assert_eq!(NoText.measure("anything at all", Some(100.0)), Size::ZERO);
        assert_eq!(NoText.measure("", None), Size::ZERO);
    }
}
