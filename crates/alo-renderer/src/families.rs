/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which families a page asked for by name, and which of them are not here.
//!
//! # The silence this file ends
//!
//! ADR 0010 confines a renderer, so it cannot go and find a font; the browser
//! process reads a short list at startup and hands it over (`crate::fonts`).
//! A page asking for anything outside that list was drawn in whatever was to
//! hand, and **nothing anywhere said so**. The render was stable, diffable and
//! not what the page looks like in any other browser, which is the worst
//! combination a rendering difference can have: reproducible and unexplained.
//!
//! So a load now answers two questions it could not before.
//!
//! - **What did this page want that I do not have?** That list goes to the
//!   browser process, which is the one that may open a file and so the one that
//!   can find a family on this machine and send it over.
//! - **What did I draw instead?** Only when *nothing* the page named was here.
//!   A page whose second choice was found got the fallback its own author
//!   wrote, and calling that a substitution would put a message in front of
//!   somebody about a page working exactly as written.
//!
//! # Why weight and slant are not asked about
//!
//! Both questions are about a **family**. A weight only ever chooses between
//! the faces of a family this engine already has — [`FontDatabase::holds`]
//! says so — and when no named family is here at all the fallback is the
//! database's own order rather than anything a weight decides. So a request
//! built here names families and nothing else, which is one fewer place for
//! `font-weight` to be parsed slightly differently from the two that already
//! do it.

use alo_box::{BoxKind, BoxTree};
use alo_style::StyleTree;
use alo_text::{FontDatabase, FontRequest, Instead};

/// The most families one load may ask the browser process for.
///
/// A page that names more than this many families it does not have is not a
/// page the sixty-fifth was going to rescue — and each one costs the browser
/// process a look through the machine's font directories. A bound, because the
/// number is otherwise chosen by whoever wrote the page.
pub const MOST_WANTED: usize = 64;

/// What a page asked for that a renderer does not have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wanted {
    /// Every family named that is not here, in the order the page first asked
    /// for it, each once.
    ///
    /// This is the **ask**: a browser process can look for each of these on the
    /// machine and send back the ones it finds. A family already satisfied by
    /// the fonts a renderer holds never appears, and neither does one listed
    /// after a family that *was* found — nothing was ever going to draw with
    /// it.
    pub families: Vec<String>,
    /// One sentence per `font-family` list where **nothing** the page named was
    /// here, saying what it was drawn in instead.
    ///
    /// These are issues rather than the ask, and they are separate from
    /// [`Wanted::families`] because they answer a different person's question:
    /// this one is read by somebody wondering why a page looks wrong.
    pub substitutions: Vec<String>,
}

/// Every family a page's text asks for that these fonts cannot give it.
///
/// Text boxes rather than every element, because a family asked for by an
/// element holding no text is a family nothing was going to be drawn in — and
/// a substitution reported for invisible text is a message about nothing.
pub fn wanted(boxes: &BoxTree, styles: &StyleTree, fonts: &FontDatabase) -> Wanted {
    let mut found = Wanted::default();
    // Every box rather than a walk: this is looking for a kind of box rather
    // than following the tree's shape, the same reason `pipeline` gives.
    let mut asked: Vec<Vec<String>> = Vec::new();
    for id in boxes.ids() {
        let Some(node) = boxes.get(id) else {
            continue;
        };
        let BoxKind::Text { ref text, .. } = node.kind else {
            continue;
        };
        // Whitespace is drawn and has no shape anybody could recognise a font
        // by, so a run of spaces is not evidence that a family was wanted.
        if !text.chars().any(|character| !character.is_whitespace()) {
            continue;
        }
        let families = boxes
            .nearest_style(styles, id)
            .and_then(|style| style.get("font-family"))
            .map(FontRequest::parse_families)
            .unwrap_or_default();
        if families.is_empty() || asked.contains(&families) {
            continue;
        }
        asked.push(families.clone());

        let absent = fonts.absent(&FontRequest {
            families,
            ..FontRequest::default()
        });
        for family in absent.families.iter().cloned() {
            if found.families.len() >= MOST_WANTED {
                break;
            }
            if !found
                .families
                .iter()
                .any(|held| held.eq_ignore_ascii_case(&family))
            {
                found.families.push(family);
            }
        }
        match absent.instead {
            Instead::TheirFallback => {}
            Instead::Ours(family) => found.substitutions.push(format!(
                "nothing here is {}, so text asking for {} was drawn in {family:?}",
                naming(&absent.families),
                if absent.families.len() == 1 {
                    "it"
                } else {
                    "them"
                },
            )),
            Instead::Nothing => found.substitutions.push(format!(
                "nothing here is {}, and there is no font at all to draw text in",
                naming(&absent.families),
            )),
        }
    }
    found
}

/// A list of families as a person would read it out.
fn naming(families: &[String]) -> String {
    let quoted: Vec<String> = families
        .iter()
        .map(|family| format!("{family:?}"))
        .collect();
    match quoted.split_last() {
        None => "nothing in particular".to_owned(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::Size;
    use alo_text::{Font, Slant, Weight};

    fn database(families: &[&str]) -> FontDatabase {
        let mut database = FontDatabase::new();
        for family in families {
            if let Some(font) = Font::load(
                family,
                Weight::NORMAL,
                Slant::Normal,
                dejavu::sans::regular().to_vec(),
            ) {
                database.add(font);
            }
        }
        database
    }

    fn asked_for(css: &str, fonts: &FontDatabase) -> Wanted {
        let rendered = crate::pipeline::render(
            "<!DOCTYPE html><html><body><p>text</p></body></html>",
            css,
            Size::new(200.0, 100.0),
            fonts,
        );
        wanted(&rendered.boxes, &rendered.styles, fonts)
    }

    #[test]
    fn a_family_that_is_here_is_wanted_by_nobody() {
        let fonts = database(&["Inter"]);
        let found = asked_for("p { font-family: Inter }", &fonts);
        assert!(found.families.is_empty(), "{:?}", found.families);
        assert!(found.substitutions.is_empty());
    }

    #[test]
    fn a_family_that_is_not_here_is_named_and_so_is_what_replaced_it() {
        let fonts = database(&["DejaVu Sans"]);
        let found = asked_for("p { font-family: Inter }", &fonts);
        assert_eq!(found.families, vec!["Inter".to_owned()]);
        assert_eq!(
            found.substitutions,
            vec![
                "nothing here is \"Inter\", so text asking for it was drawn in \"DejaVu Sans\""
                    .to_owned()
            ],
        );
    }

    #[test]
    fn the_page_s_own_fallback_is_wanted_without_being_a_substitution() {
        let fonts = database(&["DejaVu Sans"]);
        let found = asked_for("p { font-family: Inter, 'DejaVu Sans' }", &fonts);
        assert_eq!(
            found.families,
            vec!["Inter".to_owned()],
            "the machine may have Inter, so it is worth asking for",
        );
        assert!(
            found.substitutions.is_empty(),
            "but the page was drawn in the font its own author named second",
        );
    }

    #[test]
    fn a_list_of_families_none_of_which_is_here_reads_as_a_list() {
        let fonts = database(&["DejaVu Sans"]);
        let found = asked_for("p { font-family: Inter, Helvetica, monospace }", &fonts);
        assert_eq!(
            found.families,
            vec![
                "Inter".to_owned(),
                "Helvetica".to_owned(),
                "monospace".to_owned(),
            ],
        );
        assert_eq!(
            found.substitutions,
            vec![
                concat!(
                    "nothing here is \"Inter\", \"Helvetica\" or \"monospace\", ",
                    "so text asking for them was drawn in \"DejaVu Sans\"",
                )
                .to_owned()
            ],
        );
    }

    #[test]
    fn a_renderer_with_no_fonts_says_so_rather_than_naming_one() {
        let fonts = FontDatabase::new();
        let found = asked_for("p { font-family: Inter }", &fonts);
        assert_eq!(found.families, vec!["Inter".to_owned()]);
        assert_eq!(
            found.substitutions,
            vec![
                concat!(
                    "nothing here is \"Inter\", and there is ",
                    "no font at all to draw text in",
                )
                .to_owned()
            ],
        );
    }

    #[test]
    fn a_family_asked_for_twice_is_asked_for_once() {
        let fonts = database(&["DejaVu Sans"]);
        let rendered = crate::pipeline::render(
            "<!DOCTYPE html><html><body><p>one</p><p>two</p><h1>three</h1></body></html>",
            "p { font-family: Inter } h1 { font-family: Inter, Helvetica }",
            Size::new(200.0, 100.0),
            &fonts,
        );
        let found = wanted(&rendered.boxes, &rendered.styles, &fonts);
        assert_eq!(
            found.families,
            vec!["Inter".to_owned(), "Helvetica".to_owned()],
            "two paragraphs asking for the same thing ask once",
        );
        assert_eq!(
            found.substitutions.len(),
            2,
            "but two different lists were each substituted for: {:?}",
            found.substitutions,
        );
    }

    #[test]
    fn text_nobody_can_see_the_shape_of_wants_nothing() {
        let fonts = database(&["DejaVu Sans"]);
        let rendered = crate::pipeline::render(
            "<!DOCTYPE html><html><body><p> </p></body></html>",
            "p { font-family: Inter; white-space: pre }",
            Size::new(200.0, 100.0),
            &fonts,
        );
        let found = wanted(&rendered.boxes, &rendered.styles, &fonts);
        assert!(
            found.families.is_empty(),
            "a run of spaces is not evidence a family was wanted: {found:?}",
        );
    }

    #[test]
    fn a_page_with_no_text_at_all_wants_nothing() {
        let fonts = database(&["DejaVu Sans"]);
        let rendered = crate::pipeline::render(
            "<!DOCTYPE html><html><body><div></div></body></html>",
            "div { font-family: Inter; width: 10px; height: 10px }",
            Size::new(200.0, 100.0),
            &fonts,
        );
        let found = wanted(&rendered.boxes, &rendered.styles, &fonts);
        assert_eq!(found, Wanted::default());
    }

    #[test]
    fn a_list_is_read_out_the_way_a_person_would() {
        assert_eq!(naming(&[]), "nothing in particular");
        assert_eq!(naming(&["One".to_owned()]), "\"One\"");
        assert_eq!(
            naming(&["One".to_owned(), "Two".to_owned()]),
            "\"One\" or \"Two\"",
        );
        assert_eq!(
            naming(&["One".to_owned(), "Two".to_owned(), "Three".to_owned()]),
            "\"One\", \"Two\" or \"Three\"",
        );
    }
}
