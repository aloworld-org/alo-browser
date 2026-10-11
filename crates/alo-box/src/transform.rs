/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What `text-transform` does to a text box's text (ADR 0049).
//!
//! The transform is applied **where white space is collapsed**, when the box
//! is built, so the box holds the text a person sees and nothing downstream
//! knows a transform happened: shaping, measurement, line breaking, paint and
//! the agent read one string. That is CSS Text 3's order — collapsing, then
//! transforming, then breaking lines — and [`crate::whitespace`]'s argument
//! made again: two transformations would eventually disagree.
//!
//! `uppercase` and `lowercase` are built, by [`crate::case`] in the text's
//! language. Every other value — `capitalize`, `full-width`,
//! `full-size-kana`, `math-auto`, a combination with one of them — draws the
//! text as written and is recorded by the caller (ADR 0049 § 3), because an
//! approximation of it would be wrong where nobody could see it was.

use crate::case::{self, Case};
use alo_dom::{Document, NodeId};

/// What a computed `text-transform` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    /// The text as written: the initial value.
    #[default]
    None,
    /// Every letter in one case.
    Case(Case),
    /// A value this engine does not build. The text is drawn as written.
    Unbuilt,
}

impl TextTransform {
    /// What a computed `text-transform` value says.
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.eq_ignore_ascii_case("none") {
            Self::None
        } else if value.eq_ignore_ascii_case("uppercase") {
            Self::Case(Case::Upper)
        } else if value.eq_ignore_ascii_case("lowercase") {
            Self::Case(Case::Lower)
        } else {
            Self::Unbuilt
        }
    }

    /// `text` as it is drawn, for text in `language`.
    pub fn apply(self, text: String, language: Option<&str>) -> String {
        match self {
            Self::Case(case) => case::map(&text, case, language),
            Self::None | Self::Unbuilt => text,
        }
    }
}

/// The language of the element `id`: the `lang` attribute of the nearest
/// inclusive ancestor element that has one, as written (ADR 0049 § 2).
///
/// An empty `lang` is an answer — no language — and stops the walk, so a
/// `lang=""` inside a Turkish paragraph is not Turkish. `xml:lang`, an HTTP
/// header and a `<meta>` are not read.
pub fn language_of(document: &Document, id: Option<NodeId>) -> Option<&str> {
    let mut current = id;
    while let Some(node) = current {
        if let Some(language) = document
            .element(node)
            .and_then(|element| element.attr("lang"))
        {
            return Some(language);
        }
        current = document.parent(node);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_values_and_the_rest() {
        assert_eq!(TextTransform::parse("none"), TextTransform::None);
        assert_eq!(
            TextTransform::parse(" UPPERCASE "),
            TextTransform::Case(Case::Upper)
        );
        assert_eq!(
            TextTransform::parse("lowercase"),
            TextTransform::Case(Case::Lower)
        );
        for unbuilt in [
            "capitalize",
            "full-width",
            "full-size-kana",
            "math-auto",
            "uppercase full-width",
            "bogus",
        ] {
            assert_eq!(
                TextTransform::parse(unbuilt),
                TextTransform::Unbuilt,
                "{unbuilt}"
            );
        }
    }

    #[test]
    fn an_unbuilt_value_draws_the_text_as_written() {
        let written = "can't stop ab.cd".to_owned();
        assert_eq!(TextTransform::Unbuilt.apply(written.clone(), None), written);
        assert_eq!(TextTransform::None.apply(written.clone(), None), written);
    }

    #[test]
    fn the_nearest_lang_wins_and_an_empty_one_is_no_language() {
        let document = alo_dom::parse_document(
            "<html lang=en><body><p id=tr lang=tr>a<span id=in>b</span>\
             <span id=none lang=''>c</span></p><p id=en>d</p></body></html>",
        );
        let find = |wanted: &str| {
            document.descendants(document.root()).find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
        };
        assert_eq!(language_of(&document, find("in")), Some("tr"));
        assert_eq!(language_of(&document, find("tr")), Some("tr"));
        assert_eq!(language_of(&document, find("none")), Some(""));
        assert_eq!(language_of(&document, find("en")), Some("en"));
        assert_eq!(language_of(&document, None), None);
    }
}
