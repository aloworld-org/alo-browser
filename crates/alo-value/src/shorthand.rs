//! Shorthands, split into their parts.
//!
//! Stage 1 does not expand shorthands in the cascade — a declaration is kept
//! as it was written, which is what lets an unknown property be kept and
//! ignored. So a reader that wants `border-top-width` has to be able to find
//! it inside `border: 1px solid #ccc`, and this is where that is done **once**
//! rather than in each reader.
//!
//! # Why `border` and not the rest
//!
//! `margin` and `padding` split by position — first value, second value — and
//! the reader that wants one side knows which position it is. `border` splits
//! by **kind**: `1px solid red`, `solid red 1px` and `red 1px solid` are the
//! same border, because a length is a width wherever it sits and a colour is a
//! colour. That cannot be done by counting, so it is done here.

use crate::color::Color;
use crate::length::LengthPercentage;
use crate::parse::{parse_color, parse_length_percentage};

/// The border-style keywords CSS has.
///
/// All of them are recognised even though only `solid` is drawn: recognising
/// `dashed` is what lets it be reported as not implemented, where failing to
/// recognise it would make `border: 1px dashed red` look like a border with a
/// broken colour.
const STYLES: &[&str] = &[
    "none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset",
];

/// The three parts of a `border`, whichever order they were written in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Border {
    /// How thick.
    pub width: Option<LengthPercentage>,
    /// What kind of line, lowercased.
    pub style: Option<String>,
    /// What colour.
    pub color: Option<Color>,
}

impl Border {
    /// Whether nothing in the value was recognised.
    pub fn is_empty(&self) -> bool {
        self.width.is_none() && self.style.is_none() && self.color.is_none()
    }
}

/// Split a `border`, `border-top` or `border-inline` value into its parts.
///
/// A part this engine does not recognise is left out rather than guessed at,
/// and the parts it does recognise still come through — `border: 1px wobbly
/// red` is a one-pixel red border with a style nobody has, which is more
/// useful than nothing at all.
///
/// The keywords `thin`, `medium` and `thick` are the widths CSS names, and
/// they are the values a sheet uses when it says `border: thin solid`.
pub fn parse_border(text: &str) -> Border {
    let mut border = Border::default();
    for part in text.split_ascii_whitespace() {
        if border.style.is_none()
            && let Some(style) = STYLES.iter().find(|style| part.eq_ignore_ascii_case(style))
        {
            border.style = Some((*style).to_owned());
            continue;
        }
        if border.width.is_none()
            && let Some(width) = named_width(part).or_else(|| parse_length_percentage(part))
        {
            border.width = Some(width);
            continue;
        }
        if border.color.is_none()
            && let Some(color) = parse_color(part)
        {
            border.color = Some(color);
        }
    }
    border
}

/// `thin`, `medium` and `thick`, which are the three widths CSS gives names.
fn named_width(text: &str) -> Option<LengthPercentage> {
    let pixels = if text.eq_ignore_ascii_case("thin") {
        1.0
    } else if text.eq_ignore_ascii_case("medium") {
        3.0
    } else if text.eq_ignore_ascii_case("thick") {
        5.0
    } else {
        return None;
    };
    Some(LengthPercentage::Length(crate::length::Length::px(pixels)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;
    use crate::length::{FontMetrics, Length};

    fn width_px(border: &Border) -> Option<f32> {
        Some(border.width.clone()?.to_px(FontMetrics::default(), 0.0))
    }

    fn color(border: &Border) -> Option<(u8, u8, u8, u8)> {
        Some(border.color?.resolve(Rgba::BLACK).to_rgba8())
    }

    #[test]
    fn the_ordinary_way_of_writing_a_border_is_read() {
        let border = parse_border("1px solid #cccccc");
        assert_eq!(width_px(&border), Some(1.0));
        assert_eq!(border.style.as_deref(), Some("solid"));
        assert_eq!(color(&border), Some((204, 204, 204, 255)));
    }

    #[test]
    fn the_parts_may_be_written_in_any_order() {
        let expected = parse_border("1px solid red");
        for text in ["solid 1px red", "red solid 1px", "red 1px solid"] {
            let border = parse_border(text);
            assert_eq!(width_px(&border), width_px(&expected), "{text}");
            assert_eq!(border.style, expected.style, "{text}");
            assert_eq!(color(&border), color(&expected), "{text}");
        }
    }

    #[test]
    fn a_border_may_say_only_some_of_the_three() {
        let just_style = parse_border("solid");
        assert_eq!(just_style.style.as_deref(), Some("solid"));
        assert!(just_style.width.is_none() && just_style.color.is_none());

        let width_and_colour = parse_border("2px blue");
        assert_eq!(width_px(&width_and_colour), Some(2.0));
        assert!(width_and_colour.style.is_none());
    }

    #[test]
    fn the_three_named_widths_are_the_ones_css_names() {
        assert_eq!(width_px(&parse_border("thin solid")), Some(1.0));
        assert_eq!(width_px(&parse_border("medium solid")), Some(3.0));
        assert_eq!(width_px(&parse_border("thick solid")), Some(5.0));
    }

    #[test]
    fn a_style_this_engine_does_not_draw_is_still_read() {
        let border = parse_border("1px dashed red");
        assert_eq!(
            border.style.as_deref(),
            Some("dashed"),
            "recognising it is what lets it be reported as not drawn",
        );
        assert_eq!(width_px(&border), Some(1.0));
    }

    #[test]
    fn a_part_nobody_recognises_is_left_out_and_the_rest_survives() {
        let border = parse_border("1px wobbly red");
        assert_eq!(width_px(&border), Some(1.0));
        assert_eq!(color(&border), Some((255, 0, 0, 255)));
        assert!(border.style.is_none());
    }

    #[test]
    fn nothing_is_nothing() {
        assert!(parse_border("").is_empty());
        assert!(parse_border("   ").is_empty());
        assert!(Border::default().is_empty());
    }

    #[test]
    fn a_length_in_any_unit_is_a_width() {
        assert_eq!(
            parse_border("0.125rem solid").width,
            Some(LengthPercentage::Length(Length {
                value: 0.125,
                unit: crate::unit::Unit::Rem,
            })),
        );
    }
}
