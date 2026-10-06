/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! SVG's presentation attributes, as the declarations they are.
//!
//! `<path fill="none">` is not a property of the path that paint reads
//! somewhere else: SVG 2 says it is a **declaration**, at the author level and
//! with a specificity of zero, written ahead of every author style sheet (ADR
//! 0022 § 3). So any rule in any stylesheet overrides it — even `* { fill: red
//! }`, which is as unspecific as a selector gets — and the engine's own sheet
//! never does. Putting it anywhere other than the cascade would be a second
//! cascade with its own idea of who wins, and the first time it disagreed with
//! this one would be a page that styles its icons from a stylesheet.
//!
//! # Which attributes
//!
//! Only those whose property this engine draws, which is a list that grows
//! with the items that draw them: item 271's fills, item 273's strokes, and
//! the properties that decide whether anything is drawn at all. An attribute not in the list is an
//! attribute and nothing more, which is the state every SVG attribute was in
//! before this file. Geometry — `x`, `r`, `d`, `points` — is read by `alo-svg`
//! as attributes, and so is `transform`, whose attribute grammar is not CSS's.
//!
//! # Invalid values
//!
//! An invalid presentation attribute is ignored, as an invalid declaration is:
//! the property cascades and inherits as though the attribute had not been
//! written. It is checked here rather than by the reader, because by the time a
//! reader finds `fill="bogus"` unreadable the inherited value it should have
//! had is gone. Each one ignored is recorded.

use alo_css::{Declaration, Importance, IssueKind, Location, StyleIssue};
use alo_dom::{Element, Namespace};

/// The properties a presentation attribute may set, sorted, so a person can
/// read the list and a test can hold it to being sorted.
pub const PRESENTATION_PROPERTIES: &[&str] = &[
    "color",
    "display",
    "fill",
    "fill-opacity",
    "fill-rule",
    "opacity",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "visibility",
];

/// Where a presentation attribute was written, for an issue: nowhere in any
/// style sheet, which is what a location of nothing says.
const NOWHERE: Location = Location { line: 0, column: 0 };

/// The declarations an element's presentation attributes make, in the order
/// the list above names them, and a record of every one that was ignored.
///
/// Nothing for an element outside the SVG namespace: `<div fill="red">` is an
/// attribute nobody reads.
pub fn hints(element: &Element, issues: &mut Vec<StyleIssue>) -> Vec<Declaration> {
    if element.name.ns != Namespace::Svg {
        return Vec::new();
    }
    let mut found = Vec::new();
    for property in PRESENTATION_PROPERTIES {
        let Some(value) = element.attr(property) else {
            continue;
        };
        if valid(property, value) {
            found.push(Declaration::new(property, value, Importance::Normal));
        } else {
            issues.push(StyleIssue {
                kind: IssueKind::InvalidDeclaration,
                source: format!(
                    "presentation attribute <{} {property}={value:?}>",
                    element.name.local
                ),
                at: NOWHERE,
            });
        }
    }
    found
}

/// Whether a presentation attribute's value is one its property can take.
///
/// `inherit` is allowed on all of them, as SVG 1.1 allowed it and every
/// browser still does. The other CSS-wide keywords are not: SVG 2 parses a
/// presentation attribute by the property's own grammar, which does not
/// include them.
fn valid(property: &str, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    if value.eq_ignore_ascii_case("inherit") {
        return true;
    }
    match property {
        "fill" | "stroke" => paint(value),
        "color" => alo_value::parse_color(value).is_some(),
        "fill-opacity" | "opacity" | "stroke-opacity" => alpha(value),
        "fill-rule" => one_of(value, &["nonzero", "evenodd"]),
        "stroke-width" => length(value).is_some_and(|length| length >= 0.0),
        "stroke-dashoffset" => length(value).is_some(),
        "stroke-dasharray" => dashes(value),
        "stroke-linecap" => one_of(value, &["butt", "round", "square"]),
        // SVG 2's `miter-clip` and `arcs` are not drawn by any browser, and a
        // page that writes one is given the join it would get elsewhere: the
        // one it inherits.
        "stroke-linejoin" => one_of(value, &["miter", "round", "bevel"]),
        "stroke-miterlimit" => alo_value::parse_number(value).is_some_and(|limit| limit >= 1.0),
        "visibility" => one_of(value, &["visible", "hidden", "collapse"]),
        "display" => value
            .split_ascii_whitespace()
            .all(|word| word.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')),
        _ => false,
    }
}

/// SVG's `<paint>`: `none`, a colour, a reference to a paint server with an
/// optional fallback, or the context keywords. Whether a reference resolves is
/// not a question of grammar, and is `alo-svg`'s to answer.
fn paint(value: &str) -> bool {
    one_of(value, &["none", "context-fill", "context-stroke"])
        || alo_value::parse_color(value).is_some()
        || value
            .get(..4)
            .is_some_and(|start| start.eq_ignore_ascii_case("url("))
}

/// A number or a percentage, which is what an opacity is written as.
fn alpha(value: &str) -> bool {
    alo_value::parse_number(value).is_some()
        || matches!(
            alo_value::parse_length_percentage(value),
            Some(alo_value::LengthPercentage::Percentage(_))
        )
}

/// A length, a percentage or a plain number, which SVG reads as user units,
/// and enough of its value to know its sign, which is all a grammar asks.
///
/// A length's sign is its number's, whatever it is measured in. A `calc()` has
/// no sign until it is used, and CSS clamps it then rather than refusing it
/// now, so it is read as zero: valid wherever a length that is not negative
/// is.
fn length(value: &str) -> Option<f32> {
    if let Some(number) = alo_value::parse_number(value) {
        return Some(number);
    }
    Some(match alo_value::parse_length_percentage(value)? {
        alo_value::LengthPercentage::Percentage(percent) => percent,
        alo_value::LengthPercentage::Length(length) => length.value,
        alo_value::LengthPercentage::Calc(_) => 0.0,
    })
}

/// `none`, or a list of lengths none of them negative, separated by commas
/// or spaces.
fn dashes(value: &str) -> bool {
    if value.eq_ignore_ascii_case("none") {
        return true;
    }
    let mut any = false;
    for part in value.split(|c: char| c == ',' || c.is_ascii_whitespace()) {
        if part.is_empty() {
            continue;
        }
        if !length(part).is_some_and(|length| length >= 0.0) {
            return false;
        }
        any = true;
    }
    any
}

fn one_of(value: &str, words: &[&str]) -> bool {
    words.iter().any(|word| value.eq_ignore_ascii_case(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::{Document, NodeId, parse_document};

    fn first(document: &Document, local: &str) -> NodeId {
        document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| &*element.name.local == local)
            })
            .expect("the element")
    }

    fn hints_of(html: &str, local: &str) -> (Vec<String>, Vec<StyleIssue>) {
        let document = parse_document(html);
        let element = document
            .element(first(&document, local))
            .expect("an element");
        let mut issues = Vec::new();
        let found = hints(element, &mut issues)
            .iter()
            .map(ToString::to_string)
            .collect();
        (found, issues)
    }

    #[test]
    fn the_list_is_sorted_and_holds_no_duplicates() {
        let mut sorted = PRESENTATION_PROPERTIES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, PRESENTATION_PROPERTIES);
    }

    #[test]
    fn an_svg_elements_attributes_are_declarations() {
        let (found, issues) = hints_of(
            r##"<svg><rect fill="#e76f51" fill-rule="evenodd" fill-opacity="50%" opacity=".5" x="3"/></svg>"##,
            "rect",
        );
        assert_eq!(
            found,
            vec![
                "fill: #e76f51",
                "fill-opacity: 50%",
                "fill-rule: evenodd",
                "opacity: .5",
            ],
            "geometry is an attribute and not a declaration",
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn an_html_elements_attributes_are_not() {
        let (found, issues) = hints_of(r#"<div fill="red" opacity="0"></div>"#, "div");
        assert!(found.is_empty());
        assert!(issues.is_empty());
    }

    #[test]
    fn every_paint_svg_writes_is_valid() {
        for value in [
            "none",
            "currentColor",
            "red",
            "rgb(1 2 3)",
            "url(#a)",
            "url(#a) red",
            "context-fill",
            "inherit",
        ] {
            assert!(valid("fill", value), "{value}");
        }
    }

    #[test]
    fn an_invalid_value_is_ignored_and_recorded() {
        let (found, issues) = hints_of(
            r##"<svg><circle fill="bogus" fill-rule="odd" opacity="half" visibility="gone" display="block" color="#f00"/></svg>"##,
            "circle",
        );
        assert_eq!(found, vec!["color: #f00", "display: block"]);
        assert_eq!(issues.len(), 4);
        assert!(
            issues
                .iter()
                .all(|issue| issue.kind == IssueKind::InvalidDeclaration)
        );
        assert!(
            issues.first().is_some_and(
                |issue| issue.source == r#"presentation attribute <circle fill="bogus">"#
            ),
            "{issues:?}",
        );
    }

    #[test]
    fn a_strokes_attributes_are_declarations() {
        let (found, issues) = hints_of(
            r##"<svg><path stroke="#e76f51" stroke-width="2" stroke-linecap="round" stroke-linejoin="bevel" stroke-miterlimit="10" stroke-opacity=".5" stroke-dasharray="4 2, 1" stroke-dashoffset="-3"/></svg>"##,
            "path",
        );
        assert_eq!(
            found,
            vec![
                "stroke: #e76f51",
                "stroke-dasharray: 4 2, 1",
                "stroke-dashoffset: -3",
                "stroke-linecap: round",
                "stroke-linejoin: bevel",
                "stroke-miterlimit: 10",
                "stroke-opacity: .5",
                "stroke-width: 2",
            ],
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn a_strokes_values_are_held_to_their_grammar() {
        for (property, value) in [
            ("stroke-width", "0"),
            ("stroke-width", "1.5px"),
            ("stroke-width", "10%"),
            ("stroke-width", "calc(100% - 4px)"),
            ("stroke-dasharray", "none"),
            ("stroke-dasharray", "5,5"),
            ("stroke-dasharray", "1em 2% 0"),
            ("stroke-dashoffset", "-2em"),
            ("stroke-miterlimit", "1"),
            ("stroke", "url(#g) none"),
        ] {
            assert!(valid(property, value), "{property}: {value}");
        }
        for (property, value) in [
            ("stroke-width", "-1"),
            ("stroke-width", "thick"),
            ("stroke-dasharray", "4 -2"),
            ("stroke-dasharray", ","),
            ("stroke-dasharray", "4 dashes"),
            ("stroke-dashoffset", "far"),
            ("stroke-linecap", "flat"),
            ("stroke-linejoin", "miter-clip"),
            ("stroke-linejoin", "arcs"),
            ("stroke-miterlimit", "0.5"),
            ("stroke-miterlimit", "4px"),
            ("stroke-opacity", "opaque"),
            ("stroke", "bogus"),
        ] {
            assert!(!valid(property, value), "{property}: {value}");
        }
    }

    #[test]
    fn a_css_wide_keyword_other_than_inherit_is_not_a_presentation_value() {
        assert!(valid("fill", "inherit"));
        assert!(!valid("fill-rule", "unset"));
        assert!(!valid("opacity", "initial"));
        assert!(!valid("fill", "  "));
    }
}
