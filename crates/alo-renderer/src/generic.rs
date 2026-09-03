/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What `sans-serif` means on this machine.
//!
//! # The silence this file ends
//!
//! `alo_text::FontDatabase::map_generic` has existed since stage 1 and **only
//! tests called it**. The browser process handed a renderer faces and never said
//! which of them was this machine's `sans-serif` — so the user-agent sheet's own
//! `font-family: system-ui, sans-serif`, which reaches every page ever loaded,
//! named two families nobody had and was answered by falling off the end of the
//! fallback chain into whatever face happened to be first.
//!
//! Queue item 170 made that *audible* — a page is told in words what it was
//! drawn in — rather than fixing it, because what a generic means is a fact
//! about the **machine**, and a machine is what a renderer is not allowed to
//! look at (ADR 0010). So it has to cross the boundary, and a [`crate::face`]
//! does not carry it: `sans-serif` is not a property of any one font.
//!
//! # Why a candidate list rather than one name
//!
//! A generic keeps every candidate this machine actually has, in preference
//! order, because [`alo_text::FontDatabase`] already holds a generic as *several*
//! families and tries them in turn. So `sans-serif` on a machine with Helvetica
//! Neue and Geneva means both, in that order, and a character the first lacks is
//! still drawn by the second rather than by the first thing in the database.
//!
//! # Why only four
//!
//! `serif`, `sans-serif`, `monospace` and `system-ui` are the generics a real
//! page and our own user-agent sheet actually write. `cursive` and `fantasy`
//! have no answer on any machine that is not a guess — `WebKit` says Apple
//! Chancery and Papyrus on macOS and nothing anywhere else — and a guess here is
//! a page drawn in a typeface nobody chose. They are unanswered, which is a
//! state this engine already reports: a page asking for `cursive` is told what
//! it was drawn in instead.
//!
//! # A name here is a claim about somebody else's machine
//!
//! The candidate lists are written down rather than asked of the system, because
//! there is no portable way to ask and the platform-specific ways are three
//! different guesses in a trench coat. What *is* checked is that every name in
//! them is a family this engine can be told about: [`choose`] takes the families
//! actually found and keeps only those, so a name that is wrong for a machine
//! costs nothing there — it simply is not present, and the next candidate
//! answers.

/// The generic families this engine answers, in the order they are decided.
pub const ANSWERED: [&str; 4] = ["serif", "sans-serif", "monospace", "system-ui"];

/// The most pairs one mapping may hold.
///
/// The table below cannot produce more, so this is a bound on what may arrive
/// **across the boundary** rather than on what this file makes: a count is a
/// number the other end chose, and the decoder refuses one larger than any
/// honest mapping.
pub const MOST_PAIRS: usize = 64;

/// What each generic might be, on this machine, in preference order.
///
/// macOS: the families are the ones the system actually ships. `System Font` is
/// what `SFNS.ttf` calls itself in English and `.SF NS` is the name it carries
/// for software that hides it from font menus; both are listed because which one
/// this engine reads is decided by the font's `name` table. Since queue item 195
/// it reads `.SF NS`, which is the unlocalised record and is what CoreText
/// answers for that file — `System Font` is kept beside it because the two are
/// one decision apart and a list that named only the winner would break silently
/// if that decision ever moved.
#[cfg(target_os = "macos")]
const CANDIDATES: [(&str, &[&str]); 4] = [
    (
        "serif",
        &[
            "Times New Roman",
            "Times",
            "New York",
            ".New York",
            "Georgia",
            "Palatino",
        ],
    ),
    (
        "sans-serif",
        &[
            "Helvetica Neue",
            "Helvetica",
            "Arial",
            "SF Pro Text",
            "System Font",
            ".SF NS",
            "Lucida Grande",
            "Geneva",
        ],
    ),
    (
        "monospace",
        &[
            "Menlo",
            "SF Mono",
            ".SF NS Mono",
            "Monaco",
            "Courier New",
            "Courier",
        ],
    ),
    (
        "system-ui",
        &[
            "System Font",
            ".SF NS",
            "SF Pro Text",
            "Helvetica Neue",
            "Lucida Grande",
            "Geneva",
        ],
    ),
];

/// What each generic might be, on this machine, in preference order.
///
/// Everywhere that is not macOS: the families the free desktops ship, then the
/// metric-compatible names a machine may have instead. This list is a written
/// claim about other people's machines and is **not** verified on one — what is
/// verified is that a name it gets wrong costs nothing, because [`choose`] keeps
/// only the families that are actually here.
#[cfg(not(target_os = "macos"))]
const CANDIDATES: [(&str, &[&str]); 4] = [
    (
        "serif",
        &[
            "DejaVu Serif",
            "Liberation Serif",
            "Noto Serif",
            "FreeSerif",
            "Times New Roman",
        ],
    ),
    (
        "sans-serif",
        &[
            "DejaVu Sans",
            "Liberation Sans",
            "Noto Sans",
            "FreeSans",
            "Arial",
        ],
    ),
    (
        "monospace",
        &[
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Noto Sans Mono",
            "FreeMono",
            "Courier New",
        ],
    ),
    (
        "system-ui",
        &["Cantarell", "Noto Sans", "DejaVu Sans", "Liberation Sans"],
    ),
];

/// What the generic families mean here, as it crosses the boundary.
///
/// Ordered, and a generic may appear more than once: that is the preference
/// list, and [`alo_text::FontDatabase`] tries the families of a generic in the
/// order they were given to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Generics {
    pairs: Vec<(String, String)>,
}

impl Generics {
    /// Nothing said, which is the state every renderer was in before this file.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the generics mean, given the families this machine turned out to
    /// have.
    pub fn of(families: &[String]) -> Self {
        choose(&CANDIDATES, families)
    }

    /// A mapping somebody stated outright: the wire's decoder, and a test that
    /// wants a machine it can describe in one line.
    ///
    /// A pair with an empty generic or an empty family is dropped — it maps
    /// nothing onto nothing — and the generic is lowered, because CSS keyword
    /// matching is case-insensitive and `map_generic` lowers what it is asked
    /// about. A generic this version does not name is **kept**: it is still a
    /// name a page may write, and dropping it would throw away what a newer
    /// browser process meant by it.
    pub fn stating(pairs: Vec<(String, String)>) -> Self {
        let mut held: Vec<(String, String)> = Vec::new();
        for (generic, family) in pairs {
            let generic = generic.trim().to_ascii_lowercase();
            let family = family.trim().to_owned();
            if generic.is_empty() || family.is_empty() || held.len() >= MOST_PAIRS {
                continue;
            }
            if held
                .iter()
                .any(|(held, name)| *held == generic && name.eq_ignore_ascii_case(&family))
            {
                continue;
            }
            held.push((generic, family));
        }
        Self { pairs: held }
    }

    /// Every pair, in the order a database should try them.
    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }

    /// The generic names this mapping says anything about, each once, in order.
    pub fn named(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (generic, _) in &self.pairs {
            if !out.contains(&generic.as_str()) {
                out.push(generic);
            }
        }
        out
    }

    /// The families a generic stands for here, in preference order.
    pub fn families_of(&self, generic: &str) -> Vec<&str> {
        self.pairs
            .iter()
            .filter(|(held, _)| held.eq_ignore_ascii_case(generic))
            .map(|(_, family)| family.as_str())
            .collect()
    }

    /// How many pairs.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether nothing has been said — a real state, and the one a machine with
    /// no recognisable font is in.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

/// Whether a family is one some generic would like to mean.
///
/// `crate::fonts` asks this while it is reading a machine's font directories, so
/// that a family a generic needs survives the cut down to what one renderer is
/// handed. Without it the short list is alphabetical, and `sans-serif` is
/// answered or not according to where its family sorted.
pub fn is_a_candidate(family: &str) -> bool {
    CANDIDATES.iter().any(|(_, names)| {
        names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(family.trim()))
    })
}

/// The pairs a candidate table and a set of families come to.
///
/// Separated from [`Generics::of`] so that what this file *decides* can be
/// tested against a table written in the test, on every platform, rather than
/// against whichever of the two tables above was compiled in.
fn choose(table: &[(&str, &[&str])], families: &[String]) -> Generics {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (generic, candidates) in table {
        for candidate in *candidates {
            // The family as **this machine spells it**, rather than as the table
            // does: a database matches case-insensitively, and a person reading
            // the mapping should see the name the font gave itself.
            let Some(found) = families
                .iter()
                .find(|family| family.eq_ignore_ascii_case(candidate))
            else {
                continue;
            };
            if pairs.len() >= MOST_PAIRS {
                break;
            }
            if pairs
                .iter()
                .any(|(held, name)| held == generic && name.eq_ignore_ascii_case(found))
            {
                continue;
            }
            pairs.push(((*generic).to_owned(), found.clone()));
        }
    }
    Generics { pairs }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [(&str, &[&str]); 2] = [
        ("serif", &["Fictional Serif", "Second Serif"]),
        ("sans-serif", &["Fictional Sans", "Second Sans"]),
    ];

    fn families(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_generic_means_the_families_this_machine_actually_has() {
        let chosen = choose(&TABLE, &families(&["Second Sans", "Fictional Serif"]));
        assert_eq!(
            chosen.pairs(),
            [
                ("serif".to_owned(), "Fictional Serif".to_owned()),
                ("sans-serif".to_owned(), "Second Sans".to_owned()),
            ],
            "a candidate that is not here is not what the generic means",
        );
    }

    #[test]
    fn every_candidate_that_is_here_is_kept_in_preference_order() {
        let chosen = choose(&TABLE, &families(&["Second Serif", "Fictional Serif"]));
        assert_eq!(
            chosen.families_of("serif"),
            ["Fictional Serif", "Second Serif"],
            "the table's order rather than the machine's, so two machines with \
             the same fonts agree",
        );
    }

    #[test]
    fn a_machine_with_none_of_them_says_nothing_rather_than_guessing() {
        let chosen = choose(&TABLE, &families(&["Something Else"]));
        assert!(chosen.is_empty());
        assert_eq!(chosen.families_of("serif"), Vec::<&str>::new());
        assert_eq!(chosen.named(), Vec::<&str>::new());
    }

    #[test]
    fn the_family_is_spelt_the_way_the_font_spells_it() {
        let chosen = choose(&TABLE, &families(&["FICTIONAL SANS"]));
        assert_eq!(
            chosen.families_of("sans-serif"),
            ["FICTIONAL SANS"],
            "the table matched case-insensitively and then kept the machine's \
             spelling, because that is the name a person would recognise",
        );
    }

    #[test]
    fn a_family_this_machine_has_twice_is_named_once() {
        let chosen = choose(&TABLE, &families(&["Fictional Sans", "fictional sans"]));
        assert_eq!(chosen.families_of("sans-serif"), ["Fictional Sans"]);
    }

    #[test]
    fn the_real_table_answers_the_generics_it_claims_to() {
        let listed: Vec<&str> = CANDIDATES.iter().map(|(generic, _)| *generic).collect();
        assert_eq!(listed, ANSWERED, "the table and the promise disagree");
        for (generic, candidates) in CANDIDATES {
            assert!(!candidates.is_empty(), "{generic} has no candidate at all");
            for (at, candidate) in candidates.iter().enumerate() {
                assert!(!candidate.trim().is_empty(), "{generic} names nothing");
                assert!(
                    !candidates[..at]
                        .iter()
                        .any(|earlier| earlier.eq_ignore_ascii_case(candidate)),
                    "{generic} names {candidate:?} twice",
                );
                assert!(
                    is_a_candidate(candidate),
                    "{candidate:?} is in the table and is not recognised as being",
                );
            }
        }
        assert!(!is_a_candidate("Definitely Not A System Font"));
        assert!(!is_a_candidate(""));
    }

    #[test]
    fn a_mapping_stated_outright_is_cleaned_rather_than_believed() {
        let stated = Generics::stating(vec![
            ("SANS-SERIF".to_owned(), "  DejaVu Sans  ".to_owned()),
            ("sans-serif".to_owned(), "dejavu sans".to_owned()),
            (String::new(), "Nothing".to_owned()),
            ("serif".to_owned(), "   ".to_owned()),
            ("cursive".to_owned(), "Apple Chancery".to_owned()),
        ]);
        assert_eq!(
            stated.pairs(),
            [
                ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
                ("cursive".to_owned(), "Apple Chancery".to_owned()),
            ],
            "lowered, trimmed, deduplicated — and a generic this version does \
             not answer is still carried, because a page may write it",
        );
        assert_eq!(stated.named(), ["sans-serif", "cursive"]);
        assert_eq!(stated.len(), 2);
    }

    #[test]
    fn a_mapping_larger_than_any_honest_one_stops_at_the_bound() {
        let pairs: Vec<(String, String)> = (0..MOST_PAIRS * 2)
            .map(|at| ("sans-serif".to_owned(), format!("Family {at}")))
            .collect();
        assert_eq!(Generics::stating(pairs).len(), MOST_PAIRS);
    }

    #[test]
    fn nothing_said_is_a_state_of_its_own() {
        let nothing = Generics::new();
        assert!(nothing.is_empty());
        assert_eq!(nothing.len(), 0);
        assert_eq!(nothing.pairs(), []);
    }
}
