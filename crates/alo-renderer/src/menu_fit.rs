/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The words of alo's menu's *Search for “…”* cut to the menu's line by
//! their measured width, so that the closing quote follows the `…`
//! directly (queue item 857, found by the hand run of 7 November 2026,
//! `handrun35/17`).
//!
//! # Why the stylesheet could not
//!
//! Item 855 had the stylesheet end the words with `text-overflow:
//! ellipsis` where the line met the menu's edge. A box that clips its
//! words is as wide as the room it is given, and the `…` stands right after
//! the last letter that fits with it — so between the `…` and the box's end
//! there was the part of a letter that did not fit, up to a whole `W`, and
//! the quote after the box stood that far from the `…`. That is CSS as
//! Chrome draws it too: no stylesheet sizes a box to the letters an
//! ellipsis left. So the renderer measures, with the very fonts and the
//! very measurer its layout sets the line in ([`alo_text::TextMeasurer`]),
//! how many of the words fit with `…` and the quotes in the line's room,
//! and writes the words already cut. The stylesheet's ellipsis stays as
//! what it was, a bound that is never reached.
//!
//! # What is measured
//!
//! The line's three pieces as the layout sets them — each its own run, at
//! [`SIZE`] px and weight [`WEIGHT`] in the interface's face, then
//! `system-ui`, then `sans-serif`, as the stylesheet asks — and the words
//! kept whole when the three fit [`ROOM`], else the longest beginning of
//! them, its trailing space dropped, that fits with `…` after it.

use crate::card_menu::{BORDER, SEARCH_FOR, SEARCHED_END, WIDE};
use alo_dom::{Document, NodeKind};
use alo_layout::{MeasureText, TextStyle};
use alo_text::{FontDatabase, TextMeasurer};

/// How far a line's words stand in from the menu's border, either side
/// (`.mitem`'s padding).
pub const INSET: f32 = 12.0;

/// The room a line's words have: the menu's width less its border and
/// the line's inset, either side — 194.
pub const ROOM: f32 = WIDE - 2.0 * BORDER - 2.0 * INSET;

/// The size of a line's words, in CSS pixels (`.mitem`).
pub const SIZE: f32 = 13.0;

/// Their weight (`.mitem`).
pub const WEIGHT: u16 = 500;

/// The class the words are drawn with, between the quotes.
pub const WORDS: &str = "mwords";

/// What the words are cut with.
const ELLIPSIS: char = '\u{2026}';

/// Cut the words of every *Search for “…”* in `document` to the line's
/// room, set in `family` on a screen of `scale`, measured with `fonts`.
pub fn fit(document: &mut Document, fonts: &FontDatabase, family: &str, scale: f32) {
    let style = TextStyle {
        families: vec![
            family.to_owned(),
            "system-ui".to_owned(),
            "sans-serif".to_owned(),
        ],
        size: SIZE,
        weight: WEIGHT,
        white_space: alo_box::WhiteSpace::Pre,
        scale,
        ..TextStyle::default()
    };
    let measurer = TextMeasurer::new(fonts);
    let width = |text: &str| measurer.measure(text, &style, None).width;
    let texts: Vec<_> = document
        .descendants(document.root())
        .filter(|&node| document.attribute_by_name(node, "class") == Some(WORDS))
        .filter_map(|words| document.first_child(words))
        .filter(|&text| matches!(document.kind(text), Some(NodeKind::Text(_))))
        .collect();
    if texts.is_empty() {
        return;
    }
    let room = ROOM - width(SEARCH_FOR) - width(SEARCHED_END);
    for text in texts {
        let Some(NodeKind::Text(words)) = document.kind(text) else {
            continue;
        };
        let cut = cut(words, room, &width);
        if &cut != words {
            let _ = document.set_data(text, &cut);
        }
    }
}

/// `words` whole when they fit `room` as `width` measures them, else the
/// longest beginning of them that fits with `…` after it — `…` alone when
/// none does.
pub fn cut(words: &str, room: f32, width: &impl Fn(&str) -> f32) -> String {
    if width(words) <= room {
        return words.to_owned();
    }
    let ends: Vec<usize> = words
        .char_indices()
        .map(|(at, _)| at)
        .skip(1)
        .chain(std::iter::once(words.len()))
        .collect();
    let kept_and_cut = |count: usize| -> String {
        let kept = ends
            .get(count.wrapping_sub(1))
            .and_then(|&end| words.get(..end))
            .filter(|_| count > 0)
            .unwrap_or_default();
        format!("{}{ELLIPSIS}", kept.trim_end())
    };
    // The most letters that fit, found by halving: a longer beginning is
    // never narrower.
    let (mut fits, mut over) = (0usize, ends.len());
    while over - fits > 1 {
        let middle = fits + (over - fits) / 2;
        if width(&kept_and_cut(middle)) <= room {
            fits = middle;
        } else {
            over = middle;
        }
    }
    kept_and_cut(fits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every letter 10 wide.
    fn tens(text: &str) -> f32 {
        let letters = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
        f32::from(letters) * 10.0
    }

    #[test]
    fn words_that_fit_are_kept_whole() {
        assert_eq!(cut("Kampala", 70.0, &tens), "Kampala");
    }

    #[test]
    fn words_too_wide_keep_the_most_letters_that_fit_with_the_ellipsis() {
        // 75 holds seven letters: six and `…`.
        assert_eq!(cut("WWWWWWWWWW", 75.0, &tens), "WWWWWW\u{2026}");
        assert_eq!(cut("WWWWWWWWWW", 70.0, &tens), "WWWWWW\u{2026}");
        assert_eq!(cut("WWWWWWWWWW", 69.0, &tens), "WWWWW\u{2026}");
    }

    #[test]
    fn a_space_before_the_ellipsis_is_dropped_and_no_room_is_the_ellipsis() {
        assert_eq!(cut("ab cdefgh", 40.0, &tens), "ab\u{2026}");
        assert_eq!(cut("abcdef", 5.0, &tens), "\u{2026}");
    }

    #[test]
    fn letters_of_more_than_one_byte_are_cut_between_them() {
        assert_eq!(
            cut("\u{e9}t\u{e9}\u{e9}t\u{e9}", 40.0, &tens),
            "\u{e9}t\u{e9}\u{2026}"
        );
    }

    #[test]
    fn the_room_is_the_menus_less_its_border_and_inset() {
        assert!((ROOM - 194.0).abs() < f32::EPSILON);
    }
}
