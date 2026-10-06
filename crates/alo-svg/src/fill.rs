/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a shape is filled with, from its computed style.
//!
//! `fill`, `fill-opacity` and `fill-rule` are CSS properties computed by the
//! one cascade (ADR 0022 § 3), presentation attributes and all; this file only
//! reads the answer. `currentColor` is the element's computed `color`, which
//! inherits from the HTML around the `<svg>` — so an icon drawn in
//! `currentColor` is the colour of the text beside it, which is the point.
//!
//! `opacity` is not here: it fades a group, not a fill, and the walk opens
//! the group.

use alo_paint::FillRule;
use alo_style::ComputedStyle;
use alo_value::Rgba;

/// A fill, ready to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    /// The colour, with `fill-opacity` in its alpha.
    pub color: Rgba,
    /// What counts as inside.
    pub rule: FillRule,
}

/// The fill a style asks for, or [`None`] for `fill: none` and for anything
/// that comes to nothing.
///
/// A paint this engine does not draw — a reference to a gradient or a pattern
/// (item 275), or a context keyword — is recorded. A reference with a fallback
/// colour is drawn in the fallback, which is what SVG says to do when a
/// reference cannot be used.
pub fn fill_of(style: &ComputedStyle, issues: &mut Vec<String>) -> Option<Fill> {
    let color = match style.get("fill").map(str::trim) {
        // The initial value: black.
        None => Rgba::BLACK,
        Some(text) if text.eq_ignore_ascii_case("none") => return None,
        Some(text) if starts_with_ignoring_case(text, "url(") => {
            let fallback = text.find(')').and_then(|close| text.get(close + 1..));
            match fallback.map(str::trim).filter(|rest| !rest.is_empty()) {
                Some(rest) if rest.eq_ignore_ascii_case("none") => return None,
                Some(rest) => {
                    issues.push(format!(
                        "fill: {text}: a paint server is not drawn yet (item 275); its fallback is"
                    ));
                    alo_value::parse_color(rest)?.resolve(style.current_color())
                }
                None => {
                    issues.push(format!(
                        "fill: {text}: a paint server is not drawn yet (item 275), and there is no fallback"
                    ));
                    return None;
                }
            }
        }
        Some(text) if starts_with_ignoring_case(text, "context-") => {
            issues.push(format!(
                "fill: {text}: context paint belongs to <use> and markers, not drawn"
            ));
            return None;
        }
        Some(_) => style.color("fill")?,
    };
    let opacity = alpha(style.get("fill-opacity")).unwrap_or(1.0);
    let color = Rgba {
        alpha: color.alpha * opacity,
        ..color
    };
    if color.is_invisible() {
        return None;
    }
    let rule = style
        .get("fill-rule")
        .and_then(FillRule::parse)
        .unwrap_or_default();
    Some(Fill { color, rule })
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

fn starts_with_ignoring_case(text: &str, start: &str) -> bool {
    text.get(..start.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet};

    /// The fill of the element with `id=x`, in a document styled by `css`.
    fn fill(markup: &str, css: &str) -> (Option<Fill>, Vec<String>) {
        let document = parse_document(markup);
        let sheet = parse_stylesheet(css);
        let styles = alo_style::resolve(
            &document,
            &[SourcedSheet::new(Origin::Author, &sheet)],
            &MediaContext::default(),
        );
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the element");
        let mut issues = Vec::new();
        let found = fill_of(styles.get(id).expect("a style"), &mut issues);
        (found, issues)
    }

    #[test]
    fn nothing_written_is_black_by_the_non_zero_rule() {
        let (found, issues) = fill("<svg><rect id=x /></svg>", "");
        assert_eq!(
            found,
            Some(Fill {
                color: Rgba::BLACK,
                rule: FillRule::NonZero
            })
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn current_color_is_the_colour_of_the_paragraph_around_the_svg() {
        let (found, _) = fill(
            r#"<p class=lead><svg><g><rect id=x fill="currentColor"/></g></svg></p>"#,
            ".lead { color: rgb(0 128 0) }",
        );
        assert_eq!(
            found.map(|fill| fill.color),
            Some(Rgba::from_rgba8(0, 128, 0, 255))
        );
    }

    #[test]
    fn a_stylesheet_overrides_a_presentation_attribute_and_a_group_passes_it_down() {
        let (found, _) = fill(
            r#"<svg><g fill="red" fill-rule="evenodd"><rect id=x /></g></svg>"#,
            "rect { fill: blue }",
        );
        assert_eq!(
            found,
            Some(Fill {
                color: Rgba::from_rgba8(0, 0, 255, 255),
                rule: FillRule::EvenOdd
            }),
            "the colour from the sheet, the rule inherited from the group",
        );
    }

    #[test]
    fn fill_opacity_fades_the_colour_and_none_is_nothing() {
        let (found, _) = fill(
            r#"<svg><rect id=x fill="red" fill-opacity="50%"/></svg>"#,
            "",
        );
        assert_eq!(found.map(|fill| fill.color.alpha), Some(0.5));
        assert_eq!(fill(r#"<svg><rect id=x fill="none"/></svg>"#, "").0, None);
        assert_eq!(
            fill(r#"<svg><rect id=x fill-opacity="0"/></svg>"#, "").0,
            None
        );
    }

    #[test]
    fn a_paint_server_draws_its_fallback_and_says_so() {
        let (found, issues) = fill(r#"<svg><rect id=x fill="url(#g) red"/></svg>"#, "");
        assert_eq!(
            found.map(|fill| fill.color),
            Some(Rgba::from_rgba8(255, 0, 0, 255))
        );
        assert_eq!(issues.len(), 1);
        let (found, issues) = fill(r#"<svg><rect id=x fill="url(#g)"/></svg>"#, "");
        assert_eq!(found, None);
        assert_eq!(issues.len(), 1);
        let (found, issues) = fill(r#"<svg><rect id=x fill="context-fill"/></svg>"#, "");
        assert_eq!(found, None);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn an_alpha_is_a_number_or_a_percentage_held_to_one() {
        assert_eq!(alpha(Some("0.25")), Some(0.25));
        assert_eq!(alpha(Some("25%")), Some(0.25));
        assert_eq!(alpha(Some("7")), Some(1.0));
        assert_eq!(alpha(Some("-1")), Some(0.0));
        assert_eq!(alpha(Some("half")), None);
        assert_eq!(alpha(None), None);
    }
}
