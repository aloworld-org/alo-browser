/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which font, and what to do when it does not have the character.
//!
//! `docs/features.md` asks for "the fallback chain when a font lacks a glyph",
//! and that is the whole of this file. It is ours rather than rented because
//! it is a policy question, not a physics one: *which* font a page gets is a
//! decision, and the decision is visible here rather than buried in a system
//! call.
//!
//! **A font is asked whether it has the glyph.** Not guessed at from a
//! language tag, not inferred from a script property — asked. A font either
//! has the character or it does not, and there is nothing there to infer.

use crate::font::{Font, FontRequest, Slant, Weight};

/// What text was set in, when a family a page named is not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instead {
    /// Nothing was substituted: either a family the page named is here, or it
    /// named none at all. What was drawn is the page's **own** fallback,
    /// written by its author, which is not a decision this engine made.
    TheirFallback,
    /// No family the page named is here, and this is the family the text was
    /// drawn in — a substitution *this engine* made, which is the kind worth
    /// saying out loud.
    Ours(String),
    /// No family the page named is here and this database has no font at all,
    /// so the text is drawn in nothing.
    Nothing,
}

/// What a database could not give a request.
///
/// Two different things, and keeping them apart is the point of the type. A
/// family that is not here is something the **browser process** may be able to
/// find on the machine and send over. A substitution is something a **person**
/// should be told about, and only happens when nothing the page named is here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absent {
    /// The families named that this database has no face for, in the order the
    /// page wrote them, stopping at the first one that *is* here.
    ///
    /// It stops there because a family listed after one that was found is a
    /// family the page never reaches — asking a machine for it would be asking
    /// for something nothing was ever going to draw with.
    pub families: Vec<String>,
    /// What was drawn instead.
    pub instead: Instead,
}

/// The fonts this engine can use, and the order it tries them in.
#[derive(Debug, Clone, Default)]
pub struct FontDatabase {
    fonts: Vec<Font>,
    generic: Vec<(String, String)>,
}

impl FontDatabase {
    /// A database with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a font. Later fonts are tried after earlier ones.
    pub fn add(&mut self, font: Font) {
        self.fonts.push(font);
    }

    /// Say which family a generic name means — `sans-serif` is whichever font
    /// this machine has decided it is.
    ///
    /// Kept as a mapping rather than a hard-coded list because it is the one
    /// thing that genuinely differs between one machine and another, and an
    /// engine that hard-coded it would render differently from the system it
    /// is running on.
    pub fn map_generic(&mut self, generic: &str, family: &str) {
        self.generic
            .push((generic.to_ascii_lowercase(), family.to_owned()));
    }

    /// How many fonts are loaded.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// The fonts a request names, in the order they should be tried.
    ///
    /// Every font in the database follows the ones the request named, so that
    /// a character no requested family has can still be drawn by something.
    /// That is the fallback chain: the requested fonts first, then everything
    /// else, and a character nobody has is a character nobody has.
    ///
    /// # Why these are fonts rather than references to them
    ///
    /// A variable font is one file holding a continuum, and the font a request
    /// gets from such a file is that file **set to the weight asked for**. So
    /// what comes back is not always something this database holds, and saying
    /// so in the type is what keeps the setting from having to be remembered by
    /// every caller. A [`Font`] is cheap to clone — the bytes are shared — and
    /// for the ordinary one-weight face this is a clone and nothing else.
    ///
    /// The fallback fonts are set too, not only the family that matched: a page
    /// asking for bold and falling through to a variable font for one character
    /// should get that character bold.
    pub fn chain(&self, request: &FontRequest) -> Vec<Font> {
        let mut order: Vec<usize> = Vec::new();
        for family in &request.families {
            for name in self.resolve_generic(family) {
                if let Some(index) = self.best_match(&name, request.weight, request.slant)
                    && !order.contains(&index)
                {
                    order.push(index);
                }
            }
        }
        for index in 0..self.fonts.len() {
            if !order.contains(&index) {
                order.push(index);
            }
        }
        order
            .iter()
            .filter_map(|index| self.fonts.get(*index))
            .map(|font| font.at_weight(request.weight))
            .collect()
    }

    /// Whether this database has any face filed under a family.
    ///
    /// Generic names are resolved first, so `sans-serif` is held when whatever
    /// this machine has said its sans-serif is has a face — and is **not** held
    /// when nobody has said, which is a real state and the one a renderer that
    /// was handed fonts and no mapping is in.
    ///
    /// Weight and slant are deliberately not asked about: a family with only a
    /// bold face is still a family this engine has, and a page asking for it
    /// light gets the bold one rather than a substitution.
    pub fn holds(&self, family: &str) -> bool {
        self.resolve_generic(family).iter().any(|name| {
            self.fonts
                .iter()
                .any(|font| font.family().eq_ignore_ascii_case(name))
        })
    }

    /// What this database could not give a request.
    ///
    /// The question queue item 170 exists to make askable: *which family did
    /// this page want and not get, and what did it get instead?* Before it,
    /// a page asking for `Inter` was drawn in whatever was to hand and nothing
    /// anywhere said so.
    pub fn absent(&self, request: &FontRequest) -> Absent {
        let mut families: Vec<String> = Vec::new();
        for family in &request.families {
            if self.holds(family) {
                return Absent {
                    families,
                    instead: Instead::TheirFallback,
                };
            }
            if !families
                .iter()
                .any(|held| held.eq_ignore_ascii_case(family))
            {
                families.push(family.clone());
            }
        }
        if families.is_empty() {
            // A request naming nothing is a request nothing was refused for.
            return Absent {
                families,
                instead: Instead::TheirFallback,
            };
        }
        // The first of the chain rather than every font that drew a character:
        // per-character fallback can reach further down for a rare glyph, and
        // the family ordinary text came out in is what a person would recognise
        // as "it is not in the font I asked for".
        let instead = match self.chain(request).into_iter().next() {
            Some(font) => Instead::Ours(font.family().to_owned()),
            None => Instead::Nothing,
        };
        Absent { families, instead }
    }

    /// The first font in the chain that has this character.
    ///
    /// [`None`] means no font this engine has can draw it, which is a real
    /// answer: the caller shows the missing-glyph box rather than pretending.
    pub fn font_for(&self, request: &FontRequest, character: char) -> Option<Font> {
        self.chain(request)
            .into_iter()
            .find(|font| font.has_glyph(character))
    }

    /// The names a family stands for: itself, and whatever a generic name has
    /// been mapped to.
    fn resolve_generic(&self, family: &str) -> Vec<String> {
        let lowered = family.to_ascii_lowercase();
        let mapped: Vec<String> = self
            .generic
            .iter()
            .filter(|(generic, _)| *generic == lowered)
            .map(|(_, name)| name.clone())
            .collect();
        if mapped.is_empty() {
            vec![family.to_owned()]
        } else {
            mapped
        }
    }

    /// Which face of a family best matches a weight and a slant, by position in
    /// the database.
    ///
    /// The rule is CSS's own, simplified to what this database holds: prefer
    /// the right slant, then the nearest weight. A family with one face gets
    /// that face, which is the case nearly every design system is.
    ///
    /// **Nearest to what the face can be**, rather than to what it is. For an
    /// ordinary face those are the same number. For a variable one the distance
    /// is nothing wherever its axis reaches the weight asked for, which is what
    /// makes one file answer a request for 400 and a request for 700 — and what
    /// stopped a font whose `OS/2` states 1000 from being the only black face
    /// this machine could offer.
    ///
    /// An index rather than a reference, so that the caller can hand back a
    /// font *set to* the weight rather than one this database holds.
    fn best_match(&self, family: &str, weight: Weight, slant: Slant) -> Option<usize> {
        self.fonts
            .iter()
            .enumerate()
            .filter(|(_, font)| font.family().eq_ignore_ascii_case(family))
            .min_by_key(|(_, font)| {
                let wrong_slant = u32::from(font.slant() != slant);
                let distance =
                    u32::from(font.nearest_weight(weight).value().abs_diff(weight.value()));
                // Slant first, then weight: an upright face at the wrong
                // weight reads better than a slanted one at the right weight.
                (wrong_slant, distance)
            })
            .map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(family: &str, weight: Weight, slant: Slant, data: &[u8]) -> Font {
        Font::load(family, weight, slant, data.to_vec()).expect("a font this crate ships with")
    }

    fn database() -> FontDatabase {
        let mut database = FontDatabase::new();
        database.add(font(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular(),
        ));
        database.add(font(
            "DejaVu Sans",
            Weight::BOLD,
            Slant::Normal,
            dejavu::sans::bold(),
        ));
        database.add(font(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Italic,
            dejavu::sans::oblique(),
        ));
        database.add(font(
            "DejaVu Serif",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::serif::regular(),
        ));
        database.map_generic("sans-serif", "DejaVu Sans");
        database.map_generic("serif", "DejaVu Serif");
        database
    }

    fn names(fonts: &[Font]) -> Vec<String> {
        fonts
            .iter()
            .map(|font| format!("{} {}", font.family(), font.weight()))
            .collect()
    }

    #[test]
    fn an_empty_database_has_nothing_and_says_so() {
        let empty = FontDatabase::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(empty.chain(&FontRequest::family("Anything")).is_empty());
        assert!(
            empty
                .font_for(&FontRequest::family("Anything"), 'a')
                .is_none()
        );
    }

    #[test]
    fn the_requested_family_comes_first_and_everything_else_follows() {
        let database = database();
        let chain = database.chain(&FontRequest::family("DejaVu Serif"));
        assert_eq!(
            names(&chain).first().map(String::as_str),
            Some("DejaVu Serif 400"),
        );
        assert_eq!(
            chain.len(),
            database.len(),
            "every font is in the chain, so a rare character still has somewhere to go",
        );
    }

    #[test]
    fn a_generic_family_is_whatever_this_machine_says_it_is() {
        let database = database();
        let chain = database.chain(&FontRequest::family("sans-serif"));
        assert_eq!(
            names(&chain).first().map(String::as_str),
            Some("DejaVu Sans 400"),
        );

        let bare = FontDatabase::new();
        assert!(
            bare.chain(&FontRequest::family("sans-serif")).is_empty(),
            "and nothing at all when nobody has said",
        );
    }

    #[test]
    fn the_weight_and_the_slant_asked_for_are_the_ones_chosen() {
        let database = database();
        let bold = FontRequest {
            families: vec!["DejaVu Sans".to_owned()],
            weight: Weight::BOLD,
            slant: Slant::Normal,
        };
        assert_eq!(
            names(&database.chain(&bold)).first().map(String::as_str),
            Some("DejaVu Sans 700"),
        );

        let italic = FontRequest {
            families: vec!["DejaVu Sans".to_owned()],
            weight: Weight::NORMAL,
            slant: Slant::Italic,
        };
        let chosen = database.chain(&italic);
        assert_eq!(chosen.first().map(Font::slant), Some(Slant::Italic));
    }

    #[test]
    fn the_nearest_weight_is_taken_when_the_exact_one_is_missing() {
        let database = database();
        let request = FontRequest {
            families: vec!["DejaVu Sans".to_owned()],
            weight: Weight::new(500),
            slant: Slant::Normal,
        };
        assert_eq!(
            names(&database.chain(&request)).first().map(String::as_str),
            Some("DejaVu Sans 400"),
            "five hundred is nearer four hundred than seven hundred",
        );

        let heavier = FontRequest {
            weight: Weight::new(800),
            ..request
        };
        assert_eq!(
            names(&database.chain(&heavier)).first().map(String::as_str),
            Some("DejaVu Sans 700"),
        );
    }

    #[test]
    fn the_families_are_tried_in_the_order_they_were_written() {
        let database = database();
        let request = FontRequest {
            families: vec!["Nowhere".to_owned(), "DejaVu Serif".to_owned()],
            ..FontRequest::default()
        };
        assert_eq!(
            names(&database.chain(&request)).first().map(String::as_str),
            Some("DejaVu Serif 400"),
            "a family nobody has is skipped rather than fatal",
        );
    }

    #[test]
    fn a_character_the_first_font_lacks_is_found_further_down_the_chain() {
        let mut database = FontDatabase::new();
        // DejaVu Sans Mono has no Hebrew; DejaVu Sans does.
        database.add(font(
            "DejaVu Sans Mono",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans_mono::regular(),
        ));
        database.add(font(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular(),
        ));
        let request = FontRequest::family("DejaVu Sans Mono");

        assert_eq!(
            database.font_for(&request, 'a').as_ref().map(Font::family),
            Some("DejaVu Sans Mono"),
            "a character the first font has stays with the first font",
        );
        assert_eq!(
            database.font_for(&request, 'א').as_ref().map(Font::family),
            Some("DejaVu Sans"),
            "and one it lacks moves down the chain",
        );
    }

    #[test]
    fn a_character_no_font_has_is_a_character_no_font_has() {
        let database = database();
        assert!(
            database
                .font_for(&FontRequest::family("DejaVu Sans"), 'क')
                .is_none(),
            "no Devanagari here, and saying so beats drawing the wrong thing",
        );
    }

    #[test]
    fn a_family_that_is_here_is_held_however_it_is_spelt() {
        let database = database();
        assert!(database.holds("DejaVu Sans"));
        assert!(
            database.holds("dejavu sans"),
            "families are case-insensitive"
        );
        assert!(
            database.holds("sans-serif"),
            "a generic this machine mapped"
        );
        assert!(!database.holds("Inter"));
        assert!(
            !database.holds("monospace"),
            "a generic nobody has said the meaning of is not held",
        );
    }

    #[test]
    fn a_family_the_page_named_and_got_is_not_a_substitution() {
        let database = database();
        let request = FontRequest {
            families: vec!["Inter".to_owned(), "DejaVu Sans".to_owned()],
            ..FontRequest::default()
        };
        let absent = database.absent(&request);
        assert_eq!(
            absent.families,
            vec!["Inter".to_owned()],
            "the machine may have Inter, so it is still worth asking for",
        );
        assert_eq!(
            absent.instead,
            Instead::TheirFallback,
            "and what was drawn is the fallback the author wrote",
        );
    }

    #[test]
    fn a_family_listed_after_one_that_was_found_is_never_asked_for() {
        let database = database();
        let request = FontRequest {
            families: vec![
                "Inter".to_owned(),
                "DejaVu Sans".to_owned(),
                "Helvetica Neue".to_owned(),
            ],
            ..FontRequest::default()
        };
        assert_eq!(
            database.absent(&request).families,
            vec!["Inter".to_owned()],
            "nothing was ever going to be drawn in Helvetica Neue",
        );
    }

    #[test]
    fn a_page_that_got_none_of_what_it_named_is_told_what_it_got() {
        let database = database();
        let request = FontRequest {
            families: vec!["Inter".to_owned(), "monospace".to_owned()],
            ..FontRequest::default()
        };
        let absent = database.absent(&request);
        assert_eq!(
            absent.families,
            vec!["Inter".to_owned(), "monospace".to_owned()],
        );
        assert_eq!(absent.instead, Instead::Ours("DejaVu Sans".to_owned()));
    }

    #[test]
    fn a_database_with_nothing_in_it_substitutes_nothing() {
        let empty = FontDatabase::new();
        let absent = empty.absent(&FontRequest::family("Inter"));
        assert_eq!(absent.families, vec!["Inter".to_owned()]);
        assert_eq!(
            absent.instead,
            Instead::Nothing,
            "there is no family to name, and saying so beats naming one",
        );
    }

    #[test]
    fn a_request_naming_nothing_wanted_nothing() {
        let database = database();
        let absent = database.absent(&FontRequest::default());
        assert!(absent.families.is_empty());
        assert_eq!(absent.instead, Instead::TheirFallback);
    }

    #[test]
    fn a_family_named_twice_is_wanted_once() {
        let database = database();
        let request = FontRequest {
            families: vec!["Inter".to_owned(), "inter".to_owned()],
            ..FontRequest::default()
        };
        assert_eq!(database.absent(&request).families, vec!["Inter".to_owned()]);
    }

    #[test]
    fn a_font_is_never_in_the_chain_twice() {
        let database = database();
        let request = FontRequest {
            families: vec![
                "DejaVu Sans".to_owned(),
                "sans-serif".to_owned(),
                "DejaVu Sans".to_owned(),
            ],
            ..FontRequest::default()
        };
        let chain = database.chain(&request);
        assert_eq!(chain.len(), database.len());
    }
}
