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
    pub fn chain(&self, request: &FontRequest) -> Vec<&Font> {
        let mut chain: Vec<&Font> = Vec::new();
        for family in &request.families {
            for name in self.resolve_generic(family) {
                if let Some(font) = self.best_match(&name, request.weight, request.slant) {
                    if !chain.iter().any(|held| core::ptr::eq(*held, font)) {
                        chain.push(font);
                    }
                }
            }
        }
        for font in &self.fonts {
            if !chain.iter().any(|held| core::ptr::eq(*held, font)) {
                chain.push(font);
            }
        }
        chain
    }

    /// The first font in the chain that has this character.
    ///
    /// [`None`] means no font this engine has can draw it, which is a real
    /// answer: the caller shows the missing-glyph box rather than pretending.
    pub fn font_for(&self, request: &FontRequest, character: char) -> Option<&Font> {
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

    /// The face of a family that best matches a weight and a slant.
    ///
    /// The rule is CSS's own, simplified to what a stage 1 database holds:
    /// prefer the right slant, then the nearest weight. A family with one face
    /// gets that face, which is the case nearly every design system is.
    fn best_match(&self, family: &str, weight: Weight, slant: Slant) -> Option<&Font> {
        self.fonts
            .iter()
            .filter(|font| font.family().eq_ignore_ascii_case(family))
            .min_by_key(|font| {
                let wrong_slant = u32::from(font.slant() != slant);
                let weight_distance = u32::from(font.weight().value().abs_diff(weight.value()));
                // Slant first, then weight: an upright face at the wrong
                // weight reads better than a slanted one at the right weight.
                (wrong_slant, weight_distance)
            })
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

    fn names(fonts: &[&Font]) -> Vec<String> {
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
        assert_eq!(chosen.first().map(|font| font.slant()), Some(Slant::Italic));
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
            database.font_for(&request, 'a').map(Font::family),
            Some("DejaVu Sans Mono"),
            "a character the first font has stays with the first font",
        );
        assert_eq!(
            database.font_for(&request, 'א').map(Font::family),
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
