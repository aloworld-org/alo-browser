/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a shape is stroked with, from its computed style.
//!
//! `stroke` and the `stroke-*` properties are CSS properties computed by the
//! one cascade (ADR 0022 § 3), presentation attributes and all, and every one
//! of them inherits — the offline screen's hand sets them once, on its
//! `<svg>`. This file reads the answer into a colour and paint's
//! [`alo_paint::Stroke`].
//!
//! Lengths are in **user units**, as geometry is: a stroke is outlined in the
//! shape's own space and transformed with it, so `stroke-width="2"` in a
//! 24-unit `viewBox` drawn at 56 pixels is 4⅔ pixels wide. A percentage is of
//! the viewport's diagonal over √2, as a radius is.
//!
//! A value the cascade passed that is not one — a stylesheet's
//! `stroke-width: -1` — is SVG's *in error*: the property's initial value is
//! used, and it is recorded.

use crate::bounds::MOST_DASH_LENGTHS;
use crate::dashes::{self, Pattern};
use crate::length::{Axis, Viewport, user_units};
use crate::paint::{faded, paint_of};
use alo_paint::{Dashes, LineCap, LineJoin, Stroke};
use alo_style::ComputedStyle;
use alo_value::Rgba;

/// A stroke, ready to outline.
#[derive(Debug, Clone, PartialEq)]
pub struct Painted {
    /// The colour, with `stroke-opacity` in its alpha.
    pub color: Rgba,
    /// Its width, ends, corners and dashes, in user units.
    pub stroke: Stroke,
}

/// The stroke a style asks for: [`None`] for `stroke: none` (the initial
/// value), a width of nothing, and anything else that comes to nothing.
///
/// # Errors
///
/// A dash pattern of more than [`MOST_DASH_LENGTHS`] lengths, which refuses
/// the whole drawing.
pub fn stroke_of(
    style: &ComputedStyle,
    viewport: Viewport,
    issues: &mut Vec<String>,
) -> Result<Option<Painted>, String> {
    let Some(color) = paint_of(style, "stroke", None, issues) else {
        return Ok(None);
    };
    let Some(color) = faded(color, style.get("stroke-opacity")) else {
        return Ok(None);
    };
    let metrics = style.metrics();
    let length = |property: &str, initial: f32, issues: &mut Vec<String>| {
        let Some(text) = style.get(property) else {
            return initial;
        };
        match user_units(text, Axis::Other, viewport, metrics) {
            Ok(value) => value,
            Err(why) => {
                issues.push(format!("{property}: {text}: {why}, so {initial}"));
                initial
            }
        }
    };

    let mut width = length("stroke-width", 1.0, issues);
    if width < 0.0 {
        issues.push(format!("stroke-width: {width}: negative, so 1"));
        width = 1.0;
    }
    if width == 0.0 {
        return Ok(None);
    }
    let cap = keyword(style, "stroke-linecap", LineCap::parse, issues);
    let join = keyword(style, "stroke-linejoin", LineJoin::parse, issues);
    let miter_limit = match style.get("stroke-miterlimit") {
        None => 4.0,
        Some(text) => match alo_value::parse_number(text.trim()) {
            Some(limit) if limit.is_finite() && limit >= 1.0 => limit,
            _ => {
                issues.push(format!(
                    "stroke-miterlimit: {text}: not a number of at least one, so 4"
                ));
                4.0
            }
        },
    };
    let dashes = match style.get("stroke-dasharray") {
        None => None,
        Some(text) => match dashes::pattern(text, viewport, metrics) {
            Ok(Pattern::Solid) => None,
            Ok(Pattern::Dashed(lengths)) => Some(Dashes {
                lengths,
                offset: length("stroke-dashoffset", 0.0, issues),
            }),
            Err(dashes::Refusal::Invalid(why)) => {
                issues.push(format!(
                    "stroke-dasharray: {}: {why}, so a solid stroke",
                    crate::shape::truncated(text)
                ));
                None
            }
            Err(dashes::Refusal::TooMany) => {
                return Err(format!(
                    "a stroke-dasharray of more than {MOST_DASH_LENGTHS} lengths"
                ));
            }
        },
    };
    Ok(Some(Painted {
        color,
        stroke: Stroke {
            width,
            cap,
            join,
            miter_limit,
            dashes,
        },
    }))
}

/// A keyword property, its initial value when nothing is written, and its
/// initial value — recorded — when what is written is not one of its words.
fn keyword<T: Default + std::fmt::Display>(
    style: &ComputedStyle,
    property: &str,
    parse: fn(&str) -> Option<T>,
    issues: &mut Vec<String>,
) -> T {
    let Some(text) = style.get(property) else {
        return T::default();
    };
    parse(text).unwrap_or_else(|| {
        let initial = T::default();
        issues.push(format!("{property}: {text}: not understood, so {initial}"));
        initial
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet};

    const VIEW: Viewport = Viewport {
        width: 30.0,
        height: 40.0,
    };

    /// The stroke of the element with `id=x`, in a document styled by `css`.
    fn stroke(markup: &str, css: &str) -> (Result<Option<Painted>, String>, Vec<String>) {
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
        let found = stroke_of(styles.get(id).expect("a style"), VIEW, &mut issues);
        (found, issues)
    }

    fn painted(markup: &str, css: &str) -> Painted {
        stroke(markup, css)
            .0
            .expect("not refused")
            .expect("a stroke")
    }

    #[test]
    fn nothing_written_is_no_stroke() {
        let (found, issues) = stroke("<svg><rect id=x /></svg>", "");
        assert_eq!(found, Ok(None));
        assert!(issues.is_empty());
    }

    #[test]
    fn the_offline_screens_stroke_is_set_on_its_svg_and_inherited() {
        let found = painted(
            r##"<svg viewBox="0 0 24 24" fill="none" stroke="#e76f51" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path id=x d="M0 0h1"/></svg>"##,
            "",
        );
        assert_eq!(found.color, Rgba::from_rgba8(0xe7, 0x6f, 0x51, 255));
        assert_eq!(
            found.stroke,
            Stroke {
                width: 2.0,
                cap: LineCap::Round,
                join: LineJoin::Round,
                miter_limit: 4.0,
                dashes: None,
            }
        );
    }

    #[test]
    fn a_stroke_is_svgs_initial_one_where_nothing_else_is_written() {
        let found = painted(r#"<svg><line id=x stroke="red"/></svg>"#, "");
        assert_eq!(found.stroke, Stroke::default());
    }

    #[test]
    fn current_color_and_a_stylesheet_win_as_they_do_for_a_fill() {
        let found = painted(
            r#"<p class=lead><svg><g stroke="red"><rect id=x stroke="currentColor" stroke-width="3"/></g></svg></p>"#,
            ".lead { color: rgb(0 128 0) } rect { stroke-width: 5 }",
        );
        assert_eq!(found.color, Rgba::from_rgba8(0, 128, 0, 255));
        assert_eq!(Some(found.stroke.width), Some(5.0));
    }

    #[test]
    fn widths_and_dashes_are_user_units_lengths_or_shares_of_the_viewport() {
        let found = painted(
            r#"<svg><rect id=x stroke="red" stroke-width="10%" stroke-dasharray="1em, 2" stroke-dashoffset="-1"/></svg>"#,
            "rect { font-size: 4px }",
        );
        assert!((found.stroke.width - 3.535_534).abs() < 1.0e-4);
        assert_eq!(
            found.stroke.dashes,
            Some(Dashes {
                lengths: vec![4.0, 2.0],
                offset: -1.0
            })
        );
    }

    #[test]
    fn opacity_none_and_a_width_of_nothing_are_no_stroke() {
        let found = painted(
            r#"<svg><rect id=x stroke="red" stroke-opacity="25%"/></svg>"#,
            "",
        );
        assert_eq!(Some(found.color.alpha), Some(0.25));
        for markup in [
            r#"<svg><rect id=x stroke="red" stroke-opacity="0"/></svg>"#,
            r#"<svg><rect id=x stroke="red" stroke-width="0"/></svg>"#,
            r#"<svg><g stroke="red"><rect id=x stroke="none"/></g></svg>"#,
        ] {
            assert_eq!(stroke(markup, "").0, Ok(None), "{markup}");
        }
    }

    #[test]
    fn a_value_in_error_is_the_initial_value_and_is_recorded() {
        let (found, issues) = stroke(
            r#"<svg><rect id=x stroke="red"/></svg>"#,
            "rect { stroke-width: -2; stroke-linecap: flat; stroke-linejoin: arcs; \
             stroke-miterlimit: .5; stroke-dasharray: 4 -1 }",
        );
        assert_eq!(
            found,
            Ok(Some(Painted {
                color: Rgba::from_rgba8(255, 0, 0, 255),
                stroke: Stroke::default(),
            }))
        );
        assert_eq!(issues.len(), 5, "{issues:?}");
    }

    #[test]
    fn a_dash_list_past_its_bound_refuses_the_drawing() {
        let lengths = vec!["1"; MOST_DASH_LENGTHS + 1].join(" ");
        let (found, _) = stroke(
            &format!(r#"<svg><rect id=x stroke="red" stroke-dasharray="{lengths}"/></svg>"#),
            "",
        );
        assert!(found.is_err());
    }

    #[test]
    fn a_paint_server_strokes_in_its_fallback_and_says_so() {
        let (found, issues) = stroke(r#"<svg><rect id=x stroke="url(#g) blue"/></svg>"#, "");
        assert_eq!(
            found.ok().flatten().map(|painted| painted.color),
            Some(Rgba::from_rgba8(0, 0, 255, 255))
        );
        assert_eq!(issues.len(), 1);
        assert!(issues.iter().all(|issue| issue.starts_with("stroke: ")));
    }
}
