/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How wide a piece of a text box's run is, as the run is shaped **whole**
//! (queue item 797).
//!
//! The line builder lays text down a word at a time, because a line breaks
//! between words. Measuring each word on its own loses whatever the shaper
//! does across the space between two of them: a kerning pair of a space and
//! the letter after it — Newsreader kerns `" A"` −1.48 px at 40 — and any
//! contextual form. Chrome 156 shapes an item's text once and measures a
//! line's part of it as the distance between two places in that one shaping,
//! so `Good afternoon, Ada.` at 40 is 363.83 there and was 365.42 here, the
//! sum of its three words.
//!
//! So the run is shaped once ([`crate::MeasureText::cluster_ends`]) and a
//! piece's width is where the pen is at its end less where it was at its
//! start. A place that is not the end of a cluster — inside a run set right
//! to left, which is one cluster until it is reordered, or inside a
//! ligature — has no pen of its own, and a piece starting or ending there is
//! measured alone, as every piece was before.

use crate::measure::{MeasureText, TextStyle};

/// The pen along one run of text, shaped whole: where it stands after each
/// cluster, by byte.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunPen {
    /// Each cluster's end in bytes and how far the pen has moved there, in
    /// order, as [`MeasureText::cluster_ends`] gives them.
    ends: Vec<(usize, f32)>,
}

impl RunPen {
    /// The pen along `text` set in `style`, shaped once.
    pub(crate) fn of(text: &str, style: &TextStyle, measurer: &impl MeasureText) -> Self {
        Self {
            ends: measurer.cluster_ends(text, style),
        }
    }

    /// Where the pen stands at byte `at`: nothing moved at the start, and
    /// at a cluster's end where the shaping put it. [`None`] anywhere else.
    fn at(&self, at: usize) -> Option<f32> {
        if at == 0 {
            return Some(0.0);
        }
        self.ends
            .binary_search_by(|(end, _)| end.cmp(&at))
            .ok()
            .and_then(|found| self.ends.get(found))
            .map(|(_, pen)| *pen)
    }

    /// How wide the bytes `start..end` of `text` are within the run: the
    /// pen's move between them, or — where either is no cluster's end —
    /// the piece measured alone.
    pub(crate) fn width(
        &self,
        text: &str,
        (start, end): (usize, usize),
        style: &TextStyle,
        measurer: &impl MeasureText,
    ) -> f32 {
        if end <= start {
            return 0.0;
        }
        match (self.at(start), self.at(end)) {
            (Some(from), Some(to)) => to - from,
            _ => text
                .get(start..end)
                .map_or(0.0, |piece| measurer.measure(piece, style, None).width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    /// A face whose every character is 10 wide, but a space before an `A`
    /// is kerned 3 narrower, and `fi` is one cluster: the two things a word
    /// measured alone gets wrong or cannot be cut inside.
    struct Kerned;

    impl Kerned {
        fn pens(text: &str) -> Vec<(usize, f32)> {
            let mut ends = Vec::new();
            let mut pen = 0.0;
            let mut characters = text.char_indices().peekable();
            while let Some((at, character)) = characters.next() {
                pen += 10.0;
                if character == ' ' && characters.peek().is_some_and(|(_, next)| *next == 'A') {
                    pen -= 3.0;
                }
                if character == 'f' && characters.peek().is_some_and(|(_, next)| *next == 'i') {
                    continue;
                }
                ends.push((at + character.len_utf8(), pen));
            }
            ends
        }
    }

    impl MeasureText for Kerned {
        fn measure(&self, text: &str, _style: &TextStyle, _available: Option<f32>) -> Size {
            let width = Self::pens(text).last().map_or(0.0, |(_, pen)| *pen);
            Size::new(width, 10.0)
        }

        fn break_opportunities(&self, text: &str) -> Vec<usize> {
            vec![text.len()]
        }

        fn cluster_ends(&self, text: &str, _style: &TextStyle) -> Vec<(usize, f32)> {
            Self::pens(text)
        }

        fn ascender(&self, _style: &TextStyle) -> f32 {
            8.0
        }

        fn descender(&self, _style: &TextStyle) -> f32 {
            2.0
        }

        fn line_gap(&self, _style: &TextStyle) -> f32 {
            0.0
        }

        fn x_height(&self, _style: &TextStyle) -> f32 {
            5.0
        }

        fn x_advance(&self, _style: &TextStyle) -> f32 {
            10.0
        }
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-4
    }

    #[test]
    fn a_word_and_its_space_keep_the_kerning_with_the_word_after_it() {
        let style = TextStyle::default();
        let text = "Go A.";
        let pen = RunPen::of(text, &style, &Kerned);
        // `Go ` is 27 in the run, its space kerned against the `A`; alone
        // it would be 30.
        assert!(close(pen.width(text, (0, 3), &style, &Kerned), 27.0));
        assert!(close(Kerned.measure("Go ", &style, None).width, 30.0));
        // The pieces add up to the run, which a sum of words did not.
        let pieces =
            pen.width(text, (0, 3), &style, &Kerned) + pen.width(text, (3, 5), &style, &Kerned);
        assert!(close(pieces, Kerned.measure(text, &style, None).width));
        assert!(close(pieces, 47.0));
        // Its letters without the space: 20.
        assert!(close(pen.width(text, (0, 2), &style, &Kerned), 20.0));
    }

    #[test]
    fn inside_a_cluster_a_piece_is_measured_alone() {
        let style = TextStyle::default();
        let text = "afix";
        let pen = RunPen::of(text, &style, &Kerned);
        // Byte 2 is inside `fi`, one cluster: no pen there.
        assert_eq!(pen.at(2), None);
        assert!(close(pen.width(text, (0, 2), &style, &Kerned), 20.0));
        assert!(close(pen.width(text, (1, 4), &style, &Kerned), 30.0));
        // Nothing for nothing, or backwards.
        assert!(close(pen.width(text, (3, 3), &style, &Kerned), 0.0));
        assert!(close(pen.width(text, (3, 1), &style, &Kerned), 0.0));
    }
}
