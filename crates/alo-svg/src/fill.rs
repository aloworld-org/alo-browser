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
//! the group. What a `fill` value can be is [`crate::paint`]'s, because a
//! `stroke` is written the same way.

use crate::paint::{faded, paint_of};
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
/// Nothing written is black ([`paint_of`] says what else a paint can be, and
/// what is recorded about it).
pub fn fill_of(style: &ComputedStyle, issues: &mut Vec<String>) -> Option<Fill> {
    let color = paint_of(style, "fill", Some(Rgba::BLACK), issues)?;
    let color = faded(color, style.get("fill-opacity"))?;
    let rule = style
        .get("fill-rule")
        .and_then(FillRule::parse)
        .unwrap_or_default();
    Some(Fill { color, rule })
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
}
