//! Splitting text into runs a shaper can take.
//!
//! A shaper wants one font, one direction and one script at a time. A sentence
//! is rarely any of those: "the Arabic for hello is مرحبا" is two directions,
//! two scripts and — if one font does not cover both — two fonts.
//!
//! So the text is split before it is shaped, and the splitting is ours because
//! it is where the font decisions live. Where the *reordering* lives is
//! somewhere else: `docs/features.md` puts bidirectional text end to end in
//! stage 2, and what is here is the smaller, honest thing — each run knows
//! which way it goes, and the runs are laid down in the order the text is
//! written.

use crate::database::FontDatabase;
use crate::font::{Font, FontRequest};
use crate::shape::Direction;
use core::ops::Range;
use unicode_script::{Script, UnicodeScript};

/// A stretch of text that can be shaped in one go.
#[derive(Debug, Clone)]
pub struct TextRun<'a> {
    /// The bytes of the original text.
    pub range: Range<usize>,
    /// The text itself.
    pub text: &'a str,
    /// Which way it goes.
    pub direction: Direction,
    /// The font that will draw it, or [`None`] when nothing has the characters.
    pub font: Option<Font>,
}

/// Split text into runs of one direction and one font.
///
/// Two characters stay in the same run when they go the same way and the same
/// font covers both. That is deliberately not "the same script": Latin
/// punctuation inside an English sentence is a different script and does not
/// need a run of its own, and splitting on script would produce a run per
/// comma.
pub fn split<'a>(
    text: &'a str,
    database: &FontDatabase,
    request: &FontRequest,
) -> Vec<TextRun<'a>> {
    let mut runs: Vec<TextRun<'a>> = Vec::new();
    let mut start = 0usize;
    let mut current: Option<(Direction, Option<Font>)> = None;

    for (offset, character) in text.char_indices() {
        let direction = direction_of(character);
        let font = database.font_for(request, character).cloned();
        let wanted = (direction, font);

        match &current {
            Some(held) if same_run(held, &wanted, character) => {}
            Some(held) => {
                push_run(&mut runs, text, start, offset, held);
                start = offset;
                current = Some(wanted);
            }
            None => current = Some(wanted),
        }
    }
    if let Some(held) = &current {
        push_run(&mut runs, text, start, text.len(), held);
    }
    runs
}

/// Whether a character can stay in the run being built.
///
/// A character with no direction of its own — a space, a comma, a digit —
/// stays wherever it is, which is what keeps "12 items" from becoming three
/// runs. It is also what a full bidi implementation would do far more
/// carefully; this is the honest small version of it.
fn same_run(
    held: &(Direction, Option<Font>),
    wanted: &(Direction, Option<Font>),
    character: char,
) -> bool {
    let font_matches = match (&held.1, &wanted.1) {
        (Some(held), Some(wanted)) => held.family() == wanted.family(),
        (None, None) => true,
        _ => false,
    };
    if !font_matches {
        return false;
    }
    is_neutral(character) || held.0 == wanted.0
}

fn push_run<'a>(
    runs: &mut Vec<TextRun<'a>>,
    text: &'a str,
    start: usize,
    end: usize,
    held: &(Direction, Option<Font>),
) {
    let Some(slice) = text.get(start..end) else {
        return;
    };
    if slice.is_empty() {
        return;
    }
    runs.push(TextRun {
        range: start..end,
        text: slice,
        direction: held.0,
        font: held.1.clone(),
    });
}

/// Which way a character goes on its own.
///
/// Only the scripts that are written right to left say so; everything else,
/// including everything with no direction of its own, is left to right and is
/// kept in whatever run it lands in.
pub fn direction_of(character: char) -> Direction {
    match character.script() {
        Script::Arabic
        | Script::Hebrew
        | Script::Syriac
        | Script::Thaana
        | Script::Nko
        | Script::Samaritan
        | Script::Mandaic
        | Script::Adlam => Direction::RightToLeft,
        _ => Direction::LeftToRight,
    }
}

/// Whether a character has no direction of its own — spaces, punctuation,
/// digits — and so takes the direction of what is around it.
fn is_neutral(character: char) -> bool {
    matches!(
        character.script(),
        Script::Common | Script::Inherited | Script::Unknown
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Slant, Weight};

    fn database() -> FontDatabase {
        let mut database = FontDatabase::new();
        for (family, data) in [
            ("DejaVu Sans", dejavu::sans::regular()),
            ("DejaVu Serif", dejavu::serif::regular()),
        ] {
            if let Some(font) = Font::load(family, Weight::NORMAL, Slant::Normal, data.to_vec()) {
                database.add(font);
            }
        }
        database
    }

    fn described(text: &str) -> Vec<String> {
        split(text, &database(), &FontRequest::family("DejaVu Sans"))
            .into_iter()
            .map(|run| format!("{} {:?}", run.direction, run.text))
            .collect()
    }

    #[test]
    fn text_of_one_direction_is_one_run() {
        assert_eq!(described("hello there"), vec![r#"ltr "hello there""#]);
        assert_eq!(described("مرحبا"), vec![r#"rtl "مرحبا""#]);
    }

    #[test]
    fn nothing_at_all_is_no_runs() {
        assert!(described("").is_empty());
    }

    #[test]
    fn two_directions_are_two_runs_in_the_order_they_were_written() {
        let runs = described("hello مرحبا");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], r#"ltr "hello ""#);
        assert_eq!(runs[1], r#"rtl "مرحبا""#);
    }

    #[test]
    fn punctuation_and_digits_stay_in_the_run_they_are_in() {
        assert_eq!(
            described("12 items, please"),
            vec![r#"ltr "12 items, please""#],
            "splitting on script would have made this five runs",
        );
        assert_eq!(
            described("مرحبا، 12"),
            vec![r#"rtl "مرحبا، 12""#],
            "and the same the other way round",
        );
    }

    #[test]
    fn every_run_names_the_bytes_it_covers_and_they_join_up() {
        let text = "hello مرحبا there";
        let runs = split(text, &database(), &FontRequest::family("DejaVu Sans"));
        let mut expected_start = 0;
        for run in &runs {
            assert_eq!(run.range.start, expected_start);
            assert_eq!(text.get(run.range.clone()), Some(run.text));
            expected_start = run.range.end;
        }
        assert_eq!(expected_start, text.len(), "the runs cover the whole text");
    }

    #[test]
    fn a_character_no_font_has_gets_a_run_with_no_font() {
        let runs = split("aकb", &database(), &FontRequest::family("DejaVu Sans"));
        let without: Vec<&TextRun<'_>> = runs.iter().filter(|run| run.font.is_none()).collect();
        assert_eq!(without.len(), 1);
        assert_eq!(without.first().map(|run| run.text), Some("क"));
        assert_eq!(runs.len(), 3, "and the letters around it keep their font");
    }

    #[test]
    fn a_run_changes_when_the_font_does() {
        let mut database = FontDatabase::new();
        // Mono has no Hebrew, so Hebrew falls through to Sans and the run
        // splits on the font rather than only on the direction.
        for (family, data) in [
            ("DejaVu Sans Mono", dejavu::sans_mono::regular()),
            ("DejaVu Sans", dejavu::sans::regular()),
        ] {
            if let Some(font) = Font::load(family, Weight::NORMAL, Slant::Normal, data.to_vec()) {
                database.add(font);
            }
        }
        let runs = split("aשb", &database, &FontRequest::family("DejaVu Sans Mono"));
        assert_eq!(runs.len(), 3);
        assert_eq!(
            runs.iter()
                .filter_map(|run| run.font.as_ref().map(Font::family))
                .collect::<Vec<_>>(),
            vec!["DejaVu Sans Mono", "DejaVu Sans", "DejaVu Sans Mono"],
        );
    }

    #[test]
    fn the_right_to_left_scripts_are_the_ones_that_say_so() {
        for character in ['م', 'א', 'ܐ', 'ހ', 'ߊ'] {
            assert_eq!(
                direction_of(character),
                Direction::RightToLeft,
                "{character:?}",
            );
        }
        for character in ['a', 'あ', 'Я', '1', ' '] {
            assert_eq!(
                direction_of(character),
                Direction::LeftToRight,
                "{character:?}",
            );
        }
    }
}
