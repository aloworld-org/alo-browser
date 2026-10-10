/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which longhands a shorthand covers — the question a script's
//! `setProperty` and `removeProperty` ask, not the cascade.
//!
//! [`crate::shorthand`] splits the shorthands whose parts are told apart by
//! **counting**, and its tables say which longhands each one becomes. The
//! shorthands whose parts are told apart by **kind** — `background`,
//! `border`, `font` — are never split: a block holds them whole and the
//! stage that uses them reads them, longhand first. Their longhands are
//! listed here, because CSSOM still asks for them (ADR 0033 § 5): setting
//! `background` must take a written `background-color` out of the block, or
//! `alo-paint`, which lets `background-color` beat the shorthand's colour,
//! would go on drawing the old one.
//!
//! The lists are the longhands each shorthand **sets or resets**, as the
//! specification defining it says — CSS Backgrounds 3 for `background` and
//! `border`, CSS Fonts 4 for `font` — including the shorthands nested in
//! them (`background-position`, `border-width`, `border-top`,
//! `font-variant`), since a written one of those is a declaration of the
//! same longhands. Whether this engine acts on a longhand does not matter
//! here: a written declaration of it is removed either way.

use crate::shorthand::{BLOCK_AXIS, GAPS, PAIRED, SIDED};

/// The shorthands read by kind, and every longhand each one sets or resets.
static BY_KIND: [(&str, &[&str]); 3] = [
    (
        "background",
        &[
            "background-attachment",
            "background-clip",
            "background-color",
            "background-image",
            "background-origin",
            "background-position",
            "background-position-x",
            "background-position-y",
            "background-repeat",
            "background-size",
        ],
    ),
    (
        "border",
        &[
            "border-bottom",
            "border-bottom-color",
            "border-bottom-style",
            "border-bottom-width",
            "border-color",
            "border-image",
            "border-image-outset",
            "border-image-repeat",
            "border-image-slice",
            "border-image-source",
            "border-image-width",
            "border-left",
            "border-left-color",
            "border-left-style",
            "border-left-width",
            "border-right",
            "border-right-color",
            "border-right-style",
            "border-right-width",
            "border-style",
            "border-top",
            "border-top-color",
            "border-top-style",
            "border-top-width",
            "border-width",
        ],
    ),
    (
        "font",
        &[
            "font-family",
            "font-feature-settings",
            "font-kerning",
            "font-language-override",
            "font-optical-sizing",
            "font-size",
            "font-size-adjust",
            "font-stretch",
            "font-style",
            "font-variant",
            "font-variant-alternates",
            "font-variant-caps",
            "font-variant-east-asian",
            "font-variant-emoji",
            "font-variant-ligatures",
            "font-variant-numeric",
            "font-variant-position",
            "font-variation-settings",
            "font-weight",
            "font-width",
            "line-height",
        ],
    ),
];

/// Every longhand the shorthand `name` sets — whether it is split by
/// counting or read by kind — or [`None`] when `name` is not a shorthand
/// this engine knows. `name` is compared as written, so a caller lowercases
/// it first.
pub fn longhands(name: &str) -> Option<&'static [&'static str]> {
    if let Some((_, longhands)) = SIDED.iter().find(|(shorthand, _)| *shorthand == name) {
        return Some(longhands.as_slice());
    }
    if let Some((_, longhands)) = PAIRED.iter().find(|(shorthand, _)| *shorthand == name) {
        return Some(longhands.as_slice());
    }
    if let Some((_, longhands)) = GAPS.iter().find(|(shorthand, _)| *shorthand == name) {
        return Some(longhands.as_slice());
    }
    // A block-axis shorthand sets its logical longhands, as CSS Logical says,
    // and not the physical sides they are laid out as: removing
    // `padding-block` must not take a written `padding-top` with it.
    if let Some((_, longhands, _)) = BLOCK_AXIS
        .iter()
        .find(|(shorthand, _, _)| *shorthand == name)
    {
        return Some(longhands.as_slice());
    }
    BY_KIND
        .iter()
        .find(|(shorthand, _)| *shorthand == name)
        .map(|(_, longhands)| *longhands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counted_shorthand_covers_the_longhands_it_is_split_into() {
        assert_eq!(
            longhands("margin"),
            Some(["margin-top", "margin-right", "margin-bottom", "margin-left"].as_slice()),
        );
        assert_eq!(
            longhands("place-items"),
            Some(["align-items", "justify-items"].as_slice()),
        );
        assert_eq!(
            longhands("padding-block"),
            Some(["padding-block-start", "padding-block-end"].as_slice()),
            "its logical longhands, not the sides they are laid out as",
        );
        assert_eq!(longhands("gap"), Some(["row-gap", "column-gap"].as_slice()),);
    }

    #[test]
    fn a_shorthand_read_by_kind_covers_every_longhand_it_resets() {
        let background = longhands("background").unwrap_or_default();
        assert!(background.contains(&"background-color"));
        assert!(background.contains(&"background-image"));
        assert!(
            longhands("border")
                .unwrap_or_default()
                .contains(&"border-left-color")
        );
        assert!(
            longhands("font")
                .unwrap_or_default()
                .contains(&"line-height")
        );
    }

    #[test]
    fn anything_else_is_not_a_shorthand() {
        assert_eq!(longhands("color"), None);
        assert_eq!(longhands("background-color"), None);
        assert_eq!(longhands("--margin"), None);
        assert_eq!(longhands("MARGIN"), None, "the caller lowercases");
    }

    #[test]
    fn every_list_is_sorted_or_in_the_order_its_shorthand_splits() {
        for (shorthand, longhands) in &BY_KIND {
            assert!(
                longhands.windows(2).all(|pair| pair[0] < pair[1]),
                "{shorthand}'s longhands are sorted and unique"
            );
            assert!(!longhands.contains(shorthand));
        }
    }
}
