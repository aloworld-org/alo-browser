/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! SVG's `<paint>`: what `fill` and `stroke` are written as.
//!
//! The two properties take the same values and differ only in what nothing
//! written means — a fill is black and a stroke is nothing — so they are read
//! here once. `currentColor` is the element's computed `color`, which inherits
//! from the HTML around the `<svg>`: an icon drawn in `currentColor` is the
//! colour of the text beside it, which is the point of it.

use alo_style::ComputedStyle;
use alo_value::Rgba;

/// The colour a paint property asks for, or [`None`] for `none` and for
/// anything this engine cannot draw.
///
/// `initial` is the property's value when nothing sets it. A reference to a
/// paint server — a gradient or a pattern (item 275) — is drawn in its
/// fallback colour when it has one, which is what SVG says to do with a
/// reference that cannot be used, and either way it is recorded. A context
/// keyword belongs to `<use>` and markers and draws nothing here.
pub fn paint_of(
    style: &ComputedStyle,
    property: &str,
    initial: Option<Rgba>,
    issues: &mut Vec<String>,
) -> Option<Rgba> {
    match style.get(property).map(str::trim) {
        None => initial,
        Some(text) if text.eq_ignore_ascii_case("none") => None,
        Some(text) if starts_with_ignoring_case(text, "url(") => {
            let fallback = text.find(')').and_then(|close| text.get(close + 1..));
            match fallback.map(str::trim).filter(|rest| !rest.is_empty()) {
                Some(rest) if rest.eq_ignore_ascii_case("none") => None,
                Some(rest) => {
                    issues.push(format!(
                        "{property}: {text}: a paint server is not drawn yet (item 275); its fallback is"
                    ));
                    Some(alo_value::parse_color(rest)?.resolve(style.current_color()))
                }
                None => {
                    issues.push(format!(
                        "{property}: {text}: a paint server is not drawn yet (item 275), and there is no fallback"
                    ));
                    None
                }
            }
        }
        Some(text) if starts_with_ignoring_case(text, "context-") => {
            issues.push(format!(
                "{property}: {text}: context paint belongs to <use> and markers, not drawn"
            ));
            None
        }
        Some(_) => style.color(property),
    }
}

/// An opacity: a number or a percentage, held to between nothing and one.
pub fn alpha(text: Option<&str>) -> Option<f32> {
    let text = text?.trim();
    let value = match alo_value::parse_length_percentage(text) {
        Some(alo_value::LengthPercentage::Percentage(percent)) => percent / 100.0,
        _ => alo_value::parse_number(text)?,
    };
    value.is_finite().then(|| value.clamp(0.0, 1.0))
}

/// A colour faded by an opacity property's value, or [`None`] when what is
/// left cannot be seen.
pub fn faded(color: Rgba, opacity: Option<&str>) -> Option<Rgba> {
    let color = Rgba {
        alpha: color.alpha * alpha(opacity).unwrap_or(1.0),
        ..color
    };
    (!color.is_invisible()).then_some(color)
}

fn starts_with_ignoring_case(text: &str, start: &str) -> bool {
    text.get(..start.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alpha_is_a_number_or_a_percentage_held_to_one() {
        assert_eq!(alpha(Some("0.25")), Some(0.25));
        assert_eq!(alpha(Some("25%")), Some(0.25));
        assert_eq!(alpha(Some("7")), Some(1.0));
        assert_eq!(alpha(Some("-1")), Some(0.0));
        assert_eq!(alpha(Some("half")), None);
        assert_eq!(alpha(None), None);
    }

    #[test]
    fn a_colour_faded_to_nothing_is_nothing() {
        assert_eq!(faded(Rgba::BLACK, Some("0")), None);
        assert_eq!(faded(Rgba::TRANSPARENT, None), None);
        assert_eq!(
            faded(Rgba::BLACK, Some("50%")).map(|color| color.alpha),
            Some(0.5)
        );
        assert_eq!(faded(Rgba::BLACK, Some("bogus")), Some(Rgba::BLACK));
    }
}
