/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a piece of text stands on a line, on the grid of 1/64 of a device
//! pixel (ADR 0054 § 4, queue item 574).
//!
//! The line builder lays text down a word at a time, but Chrome 154 rounds
//! by the **item**: one text box's text on one line, between whatever else
//! the line holds — an inline box opening or closing, an atomic box, a box
//! out of flow, a break. Measured with the corpus's `DejaVu Sans` at five
//! sizes, at one and at two:
//!
//! - **An item's advance is its exact width rounded up onto the grid**, and
//!   what comes after it starts there. `x<b></b>xx<i></i>` puts the `<i>` at
//!   `ceil(x) + ceil(xx)`; `x xx<i></i>` puts it at `ceil(x xx)`, not at the
//!   sum of each word rounded. 300 runs of words split by `&nbsp;`, spaces
//!   and an empty `<b>`, and 120 wrapped blocks' last lines, all fit; rounding
//!   a word at a time, or a whole line at once, does not.
//! - **What the item draws is its letters' width rounded up**, without the
//!   space a line ends in: 620 lines' `Range` rectangles, at one and at two.
//! - So **a padded `<span>` holding one `&nbsp;`** is its two paddings
//!   truncated and its letter rounded up: 20.3 px each side at 13 px is
//!   2,598 and 265 units, 44.734375 px, which is the width item 574 found
//!   this engine missing by a fraction because it rounded only the line.
//!
//! Only the item's place and size are rounded. Inside it the letters are
//! where shaping put them, which is ADR 0054 § 5.

use crate::layout_unit;
use alo_box::BoxId;

/// One text box's text laid down on the current line so far, since the last
/// thing that was not that text.
///
/// Only the piece starting at the byte the run reached, in the same box,
/// carries it on, so anything else laid down between two pieces — an inline
/// box's edge, an atomic box, another box's text — ends it without being
/// told. A new line is the one thing that does not lay anything down, and
/// the line builder starts a new run there itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TextRun {
    /// The box whose text it is.
    box_id: BoxId,
    /// The byte its text has reached, so that only the piece straight after
    /// it carries it on.
    reached: usize,
    /// Where it started on the line, which is on the grid.
    start: f32,
    /// How far its pieces advance, exactly, as shaping measured them.
    advance: f32,
    /// The screen's scale, whose device pixel the grid divides.
    scale: f32,
}

impl TextRun {
    /// The run a piece of `box_id`'s text starting at byte `at` goes on:
    /// `last` when the piece carries it on, or a new one starting at `pen`.
    pub(crate) fn for_piece(
        last: Option<TextRun>,
        box_id: BoxId,
        at: usize,
        pen: f32,
        scale: f32,
    ) -> TextRun {
        match last {
            Some(run) if run.box_id == box_id && run.reached == at => run,
            _ => TextRun {
                box_id,
                reached: at,
                start: pen,
                advance: 0.0,
                scale,
            },
        }
    }

    /// Where the next piece's letters begin: exactly where shaping put them.
    pub(crate) fn left(&self) -> f32 {
        self.start + self.advance
    }

    /// Where the run's letters would end with `visible` more of them: the
    /// run's letters rounded up onto the grid.
    pub(crate) fn reach(&self, visible: f32) -> f32 {
        self.start + layout_unit::rounded_up(self.advance + visible, self.scale)
    }

    /// The run once a piece advancing `by` and ending at byte `end` is on it.
    pub(crate) fn after(self, by: f32, end: usize) -> TextRun {
        TextRun {
            reached: end,
            advance: self.advance + by,
            ..self
        }
    }

    /// Where what comes after the run starts: its advance rounded up onto the
    /// grid.
    pub(crate) fn end(&self) -> f32 {
        self.reach(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_at(pen: f32, scale: f32) -> TextRun {
        TextRun::for_piece(None, BoxId::from_index_for_tests(1), 0, pen, scale)
    }

    fn units(pixels: f32) -> f32 {
        pixels * 64.0
    }

    #[test]
    fn a_no_break_space_is_its_width_rounded_up_onto_the_grid() {
        // `&nbsp;` at 13 px in DejaVu Sans is 4.132324 px wide, 264.47
        // units; Chrome 154 gives it 265, after a padding of 1,299.
        let run = run_at(1299.0 / 64.0, 1.0).after(4.132_324, 2);
        assert!((units(run.end()) - (1299.0 + 265.0)).abs() < 1e-3);
    }

    #[test]
    fn at_a_scale_of_two_the_grid_is_a_hundred_and_twenty_eighth() {
        // 4.132324 px is 528.94 units of 1/128: 529.
        let run = run_at(0.0, 2.0).after(4.132_324, 2);
        assert!((run.end() * 128.0 - 529.0).abs() < 1e-3);
    }

    #[test]
    fn a_run_is_rounded_whole_and_not_a_word_at_a_time() {
        // Two words of 10.3 units each: 21 together, where each rounded
        // would be 22.
        let word = 10.3 / 64.0;
        let run = run_at(0.0, 1.0).after(word, 2);
        let carried =
            TextRun::for_piece(Some(run), BoxId::from_index_for_tests(1), 2, run.end(), 1.0);
        assert!((units(carried.left()) - 10.3).abs() < 1e-3);
        assert!((units(carried.after(word, 4).end()) - 21.0).abs() < 1e-3);
    }

    #[test]
    fn another_box_or_a_gap_in_the_text_starts_a_new_run_at_the_pen() {
        let run = run_at(0.0, 1.0).after(10.3 / 64.0, 2);
        let pen = run.end();
        let other = TextRun::for_piece(Some(run), BoxId::from_index_for_tests(2), 2, pen, 1.0);
        let skipped = TextRun::for_piece(Some(run), BoxId::from_index_for_tests(1), 3, pen, 1.0);
        for fresh in [other, skipped] {
            assert!((units(fresh.left()) - 11.0).abs() < 1e-3);
            assert!((units(fresh.after(10.3 / 64.0, 9).end()) - 22.0).abs() < 1e-3);
        }
    }

    #[test]
    fn its_letters_end_rounded_up_without_the_space_after_them() {
        // `x ` whose letter is 7.69 px and whose space is 4.13 more.
        let run = run_at(0.0, 1.0);
        assert!((units(run.reach(7.693_359)) - 493.0).abs() < 1e-3);
        assert!((units(run.after(7.693_359 + 4.132_324, 2).end()) - 757.0).abs() < 1e-3);
    }

    #[test]
    fn a_width_already_on_the_grid_stays_where_it_is() {
        let run = run_at(0.0, 1.0).after(0.5, 1);
        assert!((units(run.end()) - 32.0).abs() < 1e-3);
    }
}
