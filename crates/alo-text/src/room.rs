/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How much room a line takes from a font: whole pixels (ADR 0050 § 1).
//!
//! A font's ascent, descent and line gap are real numbers: `DejaVu Sans` at
//! 16 px reaches 14.85 above its baseline. A line does not take them as they
//! are. Each of the three is rounded to a whole pixel, half up, on its own,
//! before a line uses it, which is what Chrome 154 does and what makes a
//! line of text a whole number of pixels tall. Rounding them as a sum is a
//! different answer: a face with a line gap at 9 px is 11 tall measured, and
//! its sum rounded would be 12.
//!
//! [`FaceMetrics`] keeps the font's own exact figures, because underlines,
//! `ex`, `ch` and `vertical-align: middle` are measured with them (ADR 0050
//! § 5). This is the other reading of the same face, the one only a line
//! asks for.

use crate::font::FaceMetrics;

/// The whole pixels a line takes from a font at one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineRoom {
    /// How far above the baseline the font's content area reaches.
    pub ascent: f32,
    /// How far below it, as a positive number.
    pub descent: f32,
    /// The extra room the font asks for on a `normal` line.
    pub line_gap: f32,
}

impl LineRoom {
    /// A face's figures, each rounded half up to a whole pixel.
    pub fn of(metrics: FaceMetrics) -> Self {
        Self {
            ascent: whole(metrics.ascender),
            descent: whole(metrics.descender),
            line_gap: whole(metrics.line_gap),
        }
    }

    /// How far above the baseline a `normal` line of this font takes room:
    /// its ascent and the top half of its line gap, floored, as any other
    /// leading's is (ADR 0050 §§ 2 and 4).
    pub fn normal_above(self) -> f32 {
        self.ascent + (self.line_gap / 2.0).floor()
    }

    /// How far below the baseline a `normal` line of this font takes room:
    /// its descent and whatever of its line gap the top did not take.
    pub fn normal_below(self) -> f32 {
        self.descent + self.line_gap - (self.line_gap / 2.0).floor()
    }
}

/// A figure rounded to the nearest whole pixel, a half going up: 2.5 is 3,
/// where rounding to even would make it 2 and Chrome does not.
fn whole(pixels: f32) -> f32 {
    (pixels + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A face's figures in units of 2048, at `size`.
    fn face(ascent: f32, descent: f32, gap: f32, size: f32) -> FaceMetrics {
        let scale = size / 2048.0;
        FaceMetrics {
            ascender: ascent * scale,
            descender: descent * scale,
            line_gap: gap * scale,
            x_height: 0.0,
            zero_width: 0.0,
            underline_offset: 0.0,
            underline_thickness: 0.0,
        }
    }

    fn room(ascent: f32, descent: f32, line_gap: f32) -> LineRoom {
        LineRoom {
            ascent,
            descent,
            line_gap,
        }
    }

    #[test]
    fn dejavu_sans_is_whole_pixels_as_chrome_measures_it() {
        // ADR 0050 § 1's table: 14.85 and 3.77 at 16 px, 7.43 and 1.89 at 8.
        assert_eq!(
            LineRoom::of(face(1901.0, 483.0, 0.0, 16.0)),
            room(15.0, 4.0, 0.0)
        );
        assert_eq!(
            LineRoom::of(face(1901.0, 483.0, 0.0, 8.0)),
            room(7.0, 2.0, 0.0)
        );
    }

    #[test]
    fn each_figure_is_rounded_on_its_own_and_not_as_a_sum() {
        // Gap at 9 px: 8.35, 2.12 and 1.32, which are 8, 2 and 1. The sum,
        // 11.79, rounded would be 12; Chrome's `normal` line is 11.
        let gap = LineRoom::of(face(1901.0, 483.0, 300.0, 9.0));
        assert_eq!(gap, room(8.0, 2.0, 1.0));
        assert_eq!(
            (gap.normal_above(), gap.normal_below()),
            (8.0, 3.0),
            "11 in all"
        );
        // At 20 px, 18.56, 4.72 and 2.93: 27, where the sum rounded is 26.
        let gap = LineRoom::of(face(1901.0, 483.0, 300.0, 20.0));
        assert_eq!(
            (gap.normal_above(), gap.normal_below()),
            (20.0, 7.0),
            "27 in all"
        );
    }

    #[test]
    fn a_half_goes_up() {
        // Half at 10 px is 7.5, 2.5 and 2.5 exactly: 8, 3 and 3, a content
        // area of 11 and a `normal` line of 14.
        let half = LineRoom::of(face(1536.0, 512.0, 512.0, 10.0));
        assert_eq!(half, room(8.0, 3.0, 3.0));
        assert_eq!(
            LineRoom::of(face(1536.0, 512.0, 512.0, 2.0)),
            room(2.0, 1.0, 1.0)
        );
    }

    #[test]
    fn a_normal_line_puts_the_odd_pixel_of_its_gap_below() {
        // Gap at 16 px: 15, 4 and 2.34 → 2; one above the letters, one below,
        // so the baseline is at 16 of 21.
        let gap = LineRoom::of(face(1901.0, 483.0, 300.0, 16.0));
        assert_eq!((gap.normal_above(), gap.normal_below()), (16.0, 5.0));
        let odd = room(8.0, 2.0, 1.0);
        assert_eq!((odd.normal_above(), odd.normal_below()), (8.0, 3.0));
        let none = room(15.0, 4.0, 0.0);
        assert_eq!((none.normal_above(), none.normal_below()), (15.0, 4.0));
    }
}
