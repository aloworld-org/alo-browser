/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Upper- and lowercasing, by Unicode's full mapping and a language's
//! tailorings (ADR 0049 § 2).
//!
//! **Rented**, from ICU4X's `icu_casemap`, and this is the only file that
//! names it or the language identifier it takes. Case mapping is Unicode data
//! plus a handful of tailorings — Turkish and Azeri dotted and dotless `i`,
//! Lithuanian's kept dot, Greek uppercasing that drops accents — and nobody
//! here writes that table, for the reason nobody writes a shaper.
//!
//! **Full**, so one character may become several: `ß` uppercases to `SS` and
//! the text grows. A Greek capital sigma lowercases to `ς` at a word's end.
//! Each call maps the text it is given on its own; a context in a neighbouring
//! text box is not seen (ADR 0049 § 1).

use icu_casemap::CaseMapper;
use icu_locale_core::LanguageIdentifier;

/// Which way a text's case goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Every letter that has an uppercase form takes it.
    Upper,
    /// Every letter that has a lowercase form takes it.
    Lower,
}

/// `text` in `case`, for text in `language` — a BCP 47 tag as a `lang`
/// attribute writes it, or [`None`] for text in no language.
///
/// A tag that is empty or cannot be read is no language, and the root
/// mapping is used, as it is for none: a page's `lang="??"` must not stop its
/// text being drawn.
pub fn map(text: &str, case: Case, language: Option<&str>) -> String {
    let language = language
        .filter(|tag| !tag.is_empty())
        .and_then(|tag| LanguageIdentifier::try_from_locale_bytes(tag.as_bytes()).ok())
        .unwrap_or(LanguageIdentifier::UNKNOWN);
    let mapper = CaseMapper::new();
    match case {
        Case::Upper => mapper.uppercase_to_string(text, &language).into_owned(),
        Case::Lower => mapper.lowercase_to_string(text, &language).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mapping_is_the_full_one_and_text_grows() {
        assert_eq!(map("straße", Case::Upper, None), "STRASSE");
        // Each of these is one letter that uppercases to two or three, as
        // Unicode's SpecialCasing says: decomposed, not a precomposed form.
        assert_eq!(
            map("ŉ ǰ ΐ", Case::Upper, None),
            "\u{2bc}N J\u{30c} \u{399}\u{308}\u{301}"
        );
        assert_eq!(map("MY CALENDARS", Case::Lower, None), "my calendars");
    }

    #[test]
    fn a_greek_sigma_ends_a_word_as_a_final_sigma() {
        // σ (U+03C3) begins a word and ς (U+03C2) ends one.
        assert_eq!(
            map("ΟΔΟΣ", Case::Lower, None),
            "\u{3bf}\u{3b4}\u{3bf}\u{3c2}"
        );
        assert_eq!(
            map("ΣΟΦΟΣ ΟΔΟΣ", Case::Lower, None),
            "\u{3c3}\u{3bf}\u{3c6}\u{3bf}\u{3c2} \u{3bf}\u{3b4}\u{3bf}\u{3c2}"
        );
    }

    #[test]
    fn turkish_and_azeri_dot_their_i_and_english_does_not() {
        for tag in ["tr", "TR", "tr-CY", "az-Latn"] {
            assert_eq!(map("istanbul", Case::Upper, Some(tag)), "İSTANBUL", "{tag}");
            assert_eq!(map("I", Case::Lower, Some(tag)), "ı", "{tag}");
        }
        for tag in [None, Some("en"), Some("")] {
            assert_eq!(map("istanbul", Case::Upper, tag), "ISTANBUL", "{tag:?}");
            assert_eq!(map("I", Case::Lower, tag), "i", "{tag:?}");
        }
    }

    #[test]
    fn greek_uppercases_without_its_accents_and_only_in_greek() {
        assert_eq!(map("Όσο άλλο", Case::Upper, Some("el")), "ΟΣΟ ΑΛΛΟ");
        assert_eq!(map("Όσο άλλο", Case::Upper, None), "ΌΣΟ ΆΛΛΟ");
    }

    #[test]
    fn lithuanian_keeps_the_dot_of_an_accented_i() {
        assert_eq!(map("Ì", Case::Lower, Some("lt")), "i\u{307}\u{300}");
        assert_eq!(map("Ì", Case::Lower, None), "ì");
    }

    #[test]
    fn a_tag_that_cannot_be_read_is_no_language() {
        let long = "t".repeat(10_000);
        let controls = "\0\u{1}\u{7f}\n";
        for tag in [long.as_str(), controls, "tr_TR", "-", "tr-"] {
            assert_eq!(
                map("istanbul", Case::Upper, Some(tag)),
                "ISTANBUL",
                "{tag:?}"
            );
        }
    }

    #[test]
    fn a_long_run_that_doubles_is_mapped_whole() {
        let text = "ß".repeat(100_000);
        let mapped = map(&text, Case::Upper, None);
        assert_eq!(mapped.len(), 200_000);
        assert!(mapped.chars().all(|character| character == 'S'));
    }
}
