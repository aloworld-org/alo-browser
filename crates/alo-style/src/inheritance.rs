/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which properties inherit.
//!
//! This is a table because CSS is a table: there is no rule that derives
//! whether `color` inherits and `margin` does not, and any engine that guessed
//! would be wrong about half of them. What is here is the properties stage 1
//! can meet — the ones `docs/features.md` lists under Style, Layout, Text and
//! Paint — and the honest default for anything else.
//!
//! **A property that is not in this table does not inherit.** That is right
//! for the great majority of CSS, and for a property this engine does not
//! implement it is the answer that changes nothing: an unknown property is
//! kept and ignored, and inheriting it would spread something nobody reads.
//!
//! **Custom properties always inherit**, whatever their name, which is what
//! makes a design system defined once on `:root` reach the whole document.

use alo_css::PropertyName;

/// The properties that inherit, sorted so that a lookup is a binary search and
/// so that a person can read the list.
///
/// Longhands only. Stage 1 has no shorthand expansion — `font: 12px/1.5 serif`
/// is kept as the declaration it was written as — so `font` itself is here as
/// well, and the day shorthands are expanded this is the list they expand into.
const INHERITED: &[&str] = &[
    "accent-color",
    "caret-color",
    "color",
    "color-scheme",
    "cursor",
    "direction",
    "font",
    "font-family",
    "font-feature-settings",
    "font-kerning",
    "font-optical-sizing",
    "font-size",
    "font-size-adjust",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-variant-caps",
    "font-variant-east-asian",
    "font-variant-ligatures",
    "font-variant-numeric",
    "font-variation-settings",
    "font-weight",
    "hyphens",
    "letter-spacing",
    "line-break",
    "line-height",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "overflow-wrap",
    "quotes",
    "tab-size",
    "text-align",
    "text-align-last",
    "text-indent",
    "text-justify",
    "text-rendering",
    "text-shadow",
    "text-transform",
    "text-underline-offset",
    "text-underline-position",
    "text-wrap",
    "visibility",
    "white-space",
    "white-space-collapse",
    "widows",
    "word-break",
    "word-spacing",
    "writing-mode",
];

/// Whether a property's value passes to an element's children when the child
/// does not set it.
pub fn inherits(name: &PropertyName) -> bool {
    match name {
        // A design system defined on `:root` has to reach the whole document,
        // and this is the mechanism that lets it.
        PropertyName::Custom(_) => true,
        PropertyName::Ident(ident) => INHERITED.binary_search(&&**ident).is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inherits_named(name: &str) -> bool {
        inherits(&PropertyName::parse(name))
    }

    #[test]
    fn the_table_is_sorted_so_the_search_in_it_is_valid() {
        let mut sorted = INHERITED.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, INHERITED, "the table must stay sorted");
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), INHERITED.len(), "and hold no duplicates");
    }

    #[test]
    fn the_text_properties_inherit() {
        for name in [
            "color",
            "font-size",
            "line-height",
            "text-align",
            "visibility",
        ] {
            assert!(inherits_named(name), "{name} should inherit");
        }
    }

    #[test]
    fn the_box_properties_do_not() {
        for name in [
            "margin",
            "padding",
            "display",
            "width",
            "background-color",
            "border-top-width",
            "gap",
            "position",
            "z-index",
            "opacity",
            "transform",
        ] {
            assert!(!inherits_named(name), "{name} should not inherit");
        }
    }

    #[test]
    fn a_custom_property_always_inherits_whatever_it_is_called() {
        assert!(inherits_named("--surface"));
        assert!(inherits_named("--x"));
        assert!(inherits_named("--Margin"));
    }

    #[test]
    fn a_property_we_do_not_implement_does_not_inherit() {
        assert!(!inherits_named("-alo-nonexistent"));
        assert!(!inherits_named("container-type"));
    }

    #[test]
    fn a_property_name_is_matched_after_it_has_been_lowercased() {
        assert!(inherits_named("COLOR"));
        assert!(inherits_named("Font-Size"));
    }
}
