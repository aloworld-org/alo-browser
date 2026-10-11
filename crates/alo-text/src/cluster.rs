/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a line of text may be cut short, and how wide it is there.
//!
//! `text-overflow: ellipsis` hides the end of a line to make room for "…",
//! and it hides whole **clusters** — what the shaper draws as one: a letter
//! with its combining marks, a ligature — because half a letter is not
//! something a reader can read. The shaper already knows where they are:
//! every glyph names the bytes it came from, and a cluster of several glyphs
//! names its bytes once, on its last. So this shapes the text once, as a line
//! is shaped, and reads the boundaries off the glyphs.
//!
//! A run set **right to left** is one cluster here. Cutting into it would
//! need the run reordered first, which is the bidirectional reordering
//! `crate` says is stage 2; until then it is kept or hidden whole, which is
//! never half a letter.

use crate::database::FontDatabase;
use crate::font::FontRequest;
use crate::run::{TextRun, split};
use crate::shape::{shape, spaced};

/// The end of every cluster of `text` set in this font, in bytes, with how
/// far the pen has moved from the start of the text there.
///
/// In order, the last the end of the text; nothing at all for no text.
/// Characters no font has take no room, as they take none on a line, and each
/// is a cluster of its own.
pub fn cluster_ends(
    text: &str,
    database: &FontDatabase,
    request: &FontRequest,
    size: f32,
    letter_spacing: f32,
) -> Vec<(usize, f32)> {
    let mut ends = Vec::new();
    let mut pen = 0.0;
    for TextRun {
        range,
        text: piece,
        direction,
        font,
    } in split(text, database, request)
    {
        let Some(font) = font else {
            ends.extend(
                piece
                    .char_indices()
                    .map(|(offset, character)| (range.start + offset + character.len_utf8(), pen)),
            );
            continue;
        };
        let shaped = spaced(shape(piece, &font, size, direction), letter_spacing);
        if direction.is_rtl() {
            pen += shaped.width;
            ends.push((range.end, pen));
            continue;
        }
        for glyph in &shaped.glyphs {
            pen += glyph.advance;
            // Every glyph of a cluster but its last names no bytes.
            if glyph.source.end > glyph.source.start {
                ends.push((range.start + glyph.source.end, pen));
            }
        }
        // A run the shaper drew nothing for still ends where it ends.
        if ends.last().map(|(end, _)| *end) != Some(range.end) {
            ends.push((range.end, pen));
        }
    }
    ends
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, Slant, Weight};
    use crate::line::measure_unwrapped;

    fn database() -> FontDatabase {
        let mut database = FontDatabase::new();
        if let Some(font) = Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        ) {
            database.add(font);
        }
        database
    }

    fn ends(text: &str, spacing: f32) -> Vec<(usize, f32)> {
        cluster_ends(
            text,
            &database(),
            &FontRequest::family("DejaVu Sans"),
            16.0,
            spacing,
        )
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn a_word_ends_a_cluster_at_every_letter_and_the_last_is_its_width() {
        let found = ends("Mira", 0.0);
        let bytes: Vec<usize> = found.iter().map(|(end, _)| *end).collect();
        assert_eq!(bytes, vec![1, 2, 3, 4]);
        let whole = measure_unwrapped(
            "Mira",
            &database(),
            &FontRequest::family("DejaVu Sans"),
            16.0,
        )
        .width();
        assert!(found.last().is_some_and(|(_, pen)| close(*pen, whole)));
        // Each one further on than the one before.
        assert!(found.windows(2).all(|pair| match pair {
            [(_, before), (_, after)] => after > before,
            _ => false,
        }));
    }

    #[test]
    fn a_letter_and_its_combining_mark_are_one_cluster() {
        // `e` and a combining acute: three bytes, one cluster, never cut
        // between the letter and its accent.
        let found = ends("e\u{301}x", 0.0);
        let bytes: Vec<usize> = found.iter().map(|(end, _)| *end).collect();
        assert_eq!(bytes, vec![3, 4]);
    }

    #[test]
    fn letter_spacing_is_in_the_pen() {
        let plain = ends("ab", 0.0);
        let spaced = ends("ab", 3.0);
        match (plain.as_slice(), spaced.as_slice()) {
            ([(_, a), (_, b)], [(_, spaced_a), (_, spaced_b)]) => {
                assert!(close(*spaced_a, a + 3.0));
                assert!(close(*spaced_b, b + 6.0));
            }
            other => panic!("two clusters each: {other:?}"),
        }
    }

    #[test]
    fn a_right_to_left_run_is_one_cluster() {
        // "ab שלום": the Hebrew word is kept or hidden whole.
        let text = "ab \u{5e9}\u{5dc}\u{5d5}\u{5dd}";
        let found = ends(text, 0.0);
        let bytes: Vec<usize> = found.iter().map(|(end, _)| *end).collect();
        assert_eq!(bytes.last(), Some(&text.len()));
        assert!(
            !bytes.iter().any(|end| *end > 3 && *end < text.len()),
            "{bytes:?}"
        );
    }

    #[test]
    fn nothing_has_no_clusters_and_a_long_line_is_shaped_once() {
        assert!(ends("", 0.0).is_empty());
        let long = "a".repeat(100_000);
        let found = ends(&long, 0.0);
        assert_eq!(found.len(), 100_000);
        assert_eq!(found.last().map(|(end, _)| *end), Some(100_000));
    }
}
