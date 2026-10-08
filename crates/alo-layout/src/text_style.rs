/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The font an element's text is set in, read from its computed style.
//!
//! One function, because two readers need the same answer. Layout measures
//! text in this font, and the cascade's `ex` and `ch` are measured in the
//! face it picks (`alo_style::MeasureFace`). Two copies of the reading would
//! be a `ch` taken from one face and a line set in another.

use crate::measure::TextStyle;
use alo_style::ComputedStyle;

/// The font a style's text is set in.
///
/// Only the style's font size, family, weight and slant choose the face;
/// the rest says how the text is laid out in it.
pub fn text_style_of(style: &ComputedStyle) -> TextStyle {
    TextStyle {
        families: style
            .get("font-family")
            .map(|value| {
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
            })
            .unwrap_or_default(),
        size: style.font_size(),
        weight: weight_of(style),
        italic: style
            .get("font-style")
            .is_some_and(|value| !value.eq_ignore_ascii_case("normal")),
        // `normal` and a value this engine cannot read are both no extra room,
        // which is what CSS says the initial value is.
        letter_spacing: style
            .get("letter-spacing")
            .filter(|value| !value.eq_ignore_ascii_case("normal"))
            .and_then(|value| {
                style
                    .px("letter-spacing", 0.0)
                    .filter(|_| !value.is_empty())
            })
            .unwrap_or(0.0),
        white_space: style
            .get("white-space")
            .and_then(alo_box::WhiteSpace::parse)
            .unwrap_or_default(),
        line_height: style.set_line_height(),
    }
}

/// `font-weight` as a number, taking the two keywords that are numbers in
/// disguise.
fn weight_of(style: &ComputedStyle) -> u16 {
    if let Some(number) = style.number("font-weight") {
        let clamped = number.clamp(1.0, 1000.0).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to one..=1000 and rounded"
        )]
        let weight = clamped as u16;
        return weight;
    }
    match style.get("font-weight") {
        Some(value) if value.eq_ignore_ascii_case("bold") => 700,
        _ => 400,
    }
}
