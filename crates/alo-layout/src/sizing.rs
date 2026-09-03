//! What a size property can say.
//!
//! `width` is not a length. It is a length, or `auto`, or one of the
//! content-based keywords, and the difference is not a detail: `auto` and
//! `0px` produce different layouts, and an engine that folded one into the
//! other would be wrong in the direction nobody notices until a page is empty.

use alo_value::{LengthPercentage, is_keyword, parse_length_percentage};
use core::fmt;

/// A `width`, `height`, `flex-basis` or track size.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Sizing {
    /// `auto` — let layout decide.
    #[default]
    Auto,
    /// A length or a percentage.
    Length(LengthPercentage),
    /// `min-content` — as narrow as the content allows.
    MinContent,
    /// `max-content` — as wide as the content would like.
    MaxContent,
    /// `fit-content(<length>)` — max-content, capped.
    FitContent(LengthPercentage),
}

impl Sizing {
    /// Read a size from a property's text, or [`None`] if it says something
    /// this engine does not implement.
    ///
    /// [`None`] is not `auto`. The caller records the refusal and *then* falls
    /// back, which is what keeps a value we cannot read from looking like a
    /// value the author wrote.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if is_keyword(text, "auto") {
            return Some(Sizing::Auto);
        }
        if is_keyword(text, "min-content") {
            return Some(Sizing::MinContent);
        }
        if is_keyword(text, "max-content") {
            return Some(Sizing::MaxContent);
        }
        if let Some(inner) = function_argument(text, "fit-content") {
            return Some(Sizing::FitContent(parse_length_percentage(inner)?));
        }
        Some(Sizing::Length(parse_length_percentage(text)?))
    }
}

impl fmt::Display for Sizing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sizing::Auto => f.write_str("auto"),
            Sizing::Length(value) => write!(f, "{value}"),
            Sizing::MinContent => f.write_str("min-content"),
            Sizing::MaxContent => f.write_str("max-content"),
            Sizing::FitContent(value) => write!(f, "fit-content({value})"),
        }
    }
}

/// A `margin` or an `inset`: a length, or `auto`.
///
/// `auto` on a margin is what centres a box, so it is a real value rather than
/// an absence — `margin: 0 auto` and `margin: 0` do different things.
///
/// **The default here is zero, not `auto`.** That is the initial value of
/// `margin`, and it is worth stating rather than deriving: with `auto` as the
/// default, every box in a document silently centres itself, which is a
/// layout that looks deliberate and is not. `top` and `left` do start at
/// `auto`, and [`crate::style`] says so where it reads them.
#[derive(Debug, Clone, PartialEq)]
pub enum AutoLength {
    /// `auto`.
    Auto,
    /// A length or a percentage.
    Length(LengthPercentage),
}

impl Default for AutoLength {
    fn default() -> Self {
        AutoLength::Length(LengthPercentage::ZERO)
    }
}

impl AutoLength {
    /// Read a value from a property's text.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if is_keyword(text, "auto") {
            return Some(AutoLength::Auto);
        }
        Some(AutoLength::Length(parse_length_percentage(text)?))
    }
}

impl fmt::Display for AutoLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutoLength::Auto => f.write_str("auto"),
            AutoLength::Length(value) => write!(f, "{value}"),
        }
    }
}

/// The text inside `name(…)`, if that is exactly what this value is.
pub(crate) fn function_argument<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let text = text.trim();
    let open = text.find('(')?;
    if !text.get(..open)?.trim().eq_ignore_ascii_case(name) {
        return None;
    }
    if !text.ends_with(')') {
        return None;
    }
    text.get(open + 1..text.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::{FontMetrics, Length};

    fn px(sizing: &Sizing, basis: f32) -> Option<f32> {
        match sizing {
            Sizing::Length(value) => Some(value.to_px(FontMetrics::default(), basis)),
            _ => None,
        }
    }

    #[test]
    fn auto_is_a_value_rather_than_an_absence() {
        assert_eq!(Sizing::parse("auto"), Some(Sizing::Auto));
        assert_eq!(Sizing::parse("AUTO"), Some(Sizing::Auto));
        assert_ne!(
            Sizing::parse("auto"),
            Sizing::parse("0px"),
            "auto and zero lay out differently",
        );
    }

    #[test]
    fn a_length_or_a_percentage_is_read_as_one() {
        assert_eq!(
            Sizing::parse("16px"),
            Some(Sizing::Length(LengthPercentage::Length(Length::px(16.0)))),
        );
        assert_eq!(
            px(&Sizing::parse("50%").expect("percentage"), 400.0),
            Some(200.0)
        );
        assert_eq!(
            px(&Sizing::parse("calc(50% - 10px)").expect("calc"), 400.0),
            Some(190.0),
        );
    }

    #[test]
    fn the_content_keywords_are_read_as_themselves() {
        assert_eq!(Sizing::parse("min-content"), Some(Sizing::MinContent));
        assert_eq!(Sizing::parse("max-content"), Some(Sizing::MaxContent));
        assert_eq!(
            Sizing::parse("fit-content(20em)"),
            Some(Sizing::FitContent(LengthPercentage::Length(Length {
                value: 20.0,
                unit: alo_value::Unit::Em,
            }))),
        );
    }

    #[test]
    fn a_value_this_engine_does_not_implement_is_refused_and_not_called_auto() {
        for text in [
            "stretch",
            "50dvh",
            "banana",
            "",
            "fit-content",
            "fit-content()",
        ] {
            assert_eq!(Sizing::parse(text), None, "{text} should be refused");
        }
    }

    #[test]
    fn the_default_margin_is_zero_because_that_is_what_css_says() {
        assert_eq!(
            AutoLength::default(),
            AutoLength::Length(LengthPercentage::ZERO),
            "with auto as the default, every box would centre itself",
        );
    }

    #[test]
    fn a_margin_of_auto_is_the_thing_that_centres_a_box() {
        assert_eq!(AutoLength::parse("auto"), Some(AutoLength::Auto));
        assert_eq!(
            AutoLength::parse("8px"),
            Some(AutoLength::Length(LengthPercentage::Length(Length::px(
                8.0
            )))),
        );
        assert_eq!(AutoLength::parse("stretch"), None);
    }

    #[test]
    fn every_value_writes_itself_back_out() {
        assert_eq!(Sizing::Auto.to_string(), "auto");
        assert_eq!(Sizing::MinContent.to_string(), "min-content");
        assert_eq!(
            Sizing::parse("fit-content(20px)").expect("fit").to_string(),
            "fit-content(20px)",
        );
        assert_eq!(AutoLength::Auto.to_string(), "auto");
        assert_eq!(AutoLength::parse("8px").expect("length").to_string(), "8px",);
    }

    #[test]
    fn a_function_argument_is_found_only_when_the_whole_value_is_that_function() {
        assert_eq!(
            function_argument("minmax(1px, 2px)", "minmax"),
            Some("1px, 2px")
        );
        assert_eq!(function_argument("  MINMAX( a )  ", "minmax"), Some(" a "));
        assert_eq!(function_argument("minmax(1px) extra", "minmax"), None);
        assert_eq!(function_argument("other(1px)", "minmax"), None);
        assert_eq!(function_argument("minmax", "minmax"), None);
    }
}
