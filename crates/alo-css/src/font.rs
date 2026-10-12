/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `font`, split into its longhands as it is written, when it is one of the
//! keywords every property takes.
//!
//! Split here for the reason
//! [`DeclarationBlock::push`](crate::DeclarationBlock::push) gives, which
//! arrived with queue item 506. The user-agent sheet now gives a control
//! Chrome's own font, `font-size: 13.333333px` and the rest, and an alo
//! screen takes it back with `button { font: inherit }`. Left whole, that
//! `font` and the sheet's `font-size` are different properties that never
//! meet in the cascade, nothing reads `font` itself, and every button alo
//! Sites draws would have been set in 13.33 px where Chrome sets it in the
//! page's own size.
//!
//! # What is split, and what is left whole
//!
//! A CSS-wide keyword alone (`inherit`, `initial`, `unset`, `revert`,
//! `revert-layer`) sets every longhand `font` covers to it, as CSS Cascade
//! says of a shorthand, and that list is [`crate::longhand`]'s, `line-height`
//! included: measured in headless Chrome 154 on 2026-10-12, a field with
//! `font: inherit` inside a paragraph of `line-height: 3` computes `60px`
//! where its neighbour without it computes `normal`.
//!
//! Any other value — `font: 12px/1.5 serif`, a system font such as
//! `caption`, a `var()` — is left whole and split into nothing, which is what
//! this engine did with every `font` before: the longhands keep whatever else
//! set them. Its parts are told apart by kind, a size and a family and up to
//! four `normal`s, and reading them is queue item 513.

use crate::list_style::is_wide_keyword;

/// Every longhand `font` covers with the keyword it was given, or nothing
/// when the value is not one keyword alone.
pub(crate) fn split(value: &str) -> Vec<(String, String)> {
    let value = value.trim();
    if !is_wide_keyword(value) {
        return Vec::new();
    }
    crate::longhand::longhands("font")
        .unwrap_or_default()
        .iter()
        .map(|longhand| ((*longhand).to_owned(), value.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of(split: &[(String, String)], longhand: &str) -> Option<String> {
        split
            .iter()
            .find(|(name, _)| name == longhand)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn a_wide_keyword_is_every_longhand_line_height_included() {
        let split = split("inherit");
        for longhand in ["font-family", "font-size", "font-style", "font-weight"] {
            assert_eq!(value_of(&split, longhand).as_deref(), Some("inherit"));
        }
        assert_eq!(
            value_of(&split, "line-height").as_deref(),
            Some("inherit"),
            "a field with `font: inherit` takes its paragraph's line, measured",
        );
        assert_eq!(split.len(), longhands_of_font());
    }

    #[test]
    fn every_wide_keyword_is_split_as_it_was_written() {
        for keyword in ["initial", "unset", "revert", "revert-layer", "INHERIT"] {
            assert_eq!(
                value_of(&split(keyword), "font-size").as_deref(),
                Some(keyword),
            );
        }
        assert_eq!(
            value_of(&split("  inherit "), "font-size").as_deref(),
            Some("inherit"),
        );
    }

    #[test]
    fn any_other_value_is_left_whole() {
        for value in [
            "12px/1.5 serif",
            "bold 14px system-ui",
            "caption",
            "var(--font)",
            "inherit 12px",
            "",
        ] {
            assert!(split(value).is_empty(), "{value}");
        }
    }

    fn longhands_of_font() -> usize {
        crate::longhand::longhands("font").unwrap_or_default().len()
    }
}
