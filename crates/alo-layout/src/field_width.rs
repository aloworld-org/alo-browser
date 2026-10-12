/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How wide a text field is when its `width` is `auto`: Chrome's rule.
//!
//! A field is as many characters wide as its `size` says (`alo_box`'s
//! `field_size`), and a character is not the `0` that `ch` measures. Chrome
//! 154, measured over 75 sizes from 8 to 40 px in `DejaVu Sans`,
//! `DejaVu Sans Mono` and `DejaVu Serif`, each at `size` 1 to 40, with
//! padding and border off (queue item 511), makes a field of `n` characters
//!
//! ```text
//! ceil(c × n + (a − c))
//! ```
//!
//! whole pixels wide, where:
//!
//! - **`c` is the advance of the face's `x`**, rounded to a whole pixel when
//!   that makes it *wider*, and left as it is when it would make it narrower:
//!   `DejaVu Sans`' `x` is 7.89 at 13.33 px and counts as 8, and 9.47 at
//!   16 px and counts as 9.47.
//! - **`a` is the face's ascent in whole pixels**, the one a line takes
//!   (ADR 0050 § 1). One character is exactly that wide.
//!
//! Each was told apart with a copy of `DejaVu Sans` that changed one table at
//! a time. Moving the `x`'s advance moved `c`, and moving the `hhea` ascent
//! moved `a`. The `OS/2` table's average width, its `usWinAscent`, the `0`'s
//! advance and the face's widest advance and bounding box moved nothing.
//! `letter-spacing`, `word-spacing`, a placeholder and a value move nothing
//! either.
//!
//! It is a width, not a minimum: a field in a narrower box is as wide as
//! this, and `min-width` and `max-width` apply to it as to any width.

use crate::measure::{MeasureText, TextStyle};

/// The content-box width of a text field of `characters` set in `style`,
/// in CSS pixels.
pub(crate) fn of(characters: u32, style: &TextStyle, measure: &impl MeasureText) -> f32 {
    let advance = measure.x_advance(style);
    let character = advance.max(advance.round());
    let ascent = measure.ascender(style);
    // Counted as Chrome counts it, the characters first and the ascent's
    // difference after, so that the one rounding falls where Chrome's does.
    #[expect(
        clippy::cast_precision_loss,
        reason = "Chrome counts in a float too: above 2^24 characters both \
                  lose the same digits, and no page shows a field that wide"
    )]
    let count = characters as f32;
    (character * count + (ascent - character)).ceil().max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::{BlockFont, ScaledFont};

    fn at(size: f32) -> TextStyle {
        TextStyle {
            size,
            ..TextStyle::default()
        }
    }

    /// A measurer whose `x` and ascent are what a test says, and nothing
    /// else.
    struct Face {
        x: f32,
        ascent: f32,
    }

    impl MeasureText for Face {
        fn measure(
            &self,
            text: &str,
            style: &TextStyle,
            available_width: Option<f32>,
        ) -> crate::geometry::Size {
            BlockFont.measure(text, style, available_width)
        }

        fn break_opportunities(&self, text: &str) -> Vec<usize> {
            BlockFont.break_opportunities(text)
        }

        fn cluster_ends(&self, text: &str, style: &TextStyle) -> Vec<(usize, f32)> {
            BlockFont.cluster_ends(text, style)
        }

        fn ascender(&self, _style: &TextStyle) -> f32 {
            self.ascent
        }

        fn descender(&self, style: &TextStyle) -> f32 {
            BlockFont.descender(style)
        }

        fn line_gap(&self, style: &TextStyle) -> f32 {
            BlockFont.line_gap(style)
        }

        fn x_height(&self, style: &TextStyle) -> f32 {
            BlockFont.x_height(style)
        }

        fn x_advance(&self, _style: &TextStyle) -> f32 {
            self.x
        }
    }

    /// `DejaVu Sans` at a size: its `x` is 1212 and its ascent 1901 of 2048,
    /// the ascent rounded half up as a line takes it.
    fn dejavu_sans(size: f32) -> Face {
        Face {
            x: 1212.0 * size / 2048.0,
            ascent: (1901.0 * size / 2048.0 + 0.5).floor(),
        }
    }

    #[test]
    fn a_field_is_chromes_width_in_dejavu_sans() {
        // Chrome 154, padding and border off, `size` 1, 2, 3, 5, 10, 20, 30.
        for (size, widths) in [
            (13.333_333, [12.0, 20.0, 28.0, 44.0, 84.0, 164.0, 244.0]),
            (16.0, [15.0, 25.0, 34.0, 53.0, 101.0, 195.0, 290.0]),
            (10.0, [9.0, 15.0, 21.0, 33.0, 63.0, 123.0, 183.0]),
            (24.0, [22.0, 37.0, 51.0, 79.0, 150.0, 292.0, 434.0]),
            (32.0, [30.0, 49.0, 68.0, 106.0, 201.0, 391.0, 581.0]),
        ] {
            let face = dejavu_sans(size);
            for (characters, width) in [1, 2, 3, 5, 10, 20, 30].into_iter().zip(widths) {
                let got = of(characters, &at(size), &face);
                assert!(
                    (got - width).abs() < 1e-3,
                    "{size} px, {characters}: {got}, Chrome {width}"
                );
            }
        }
    }

    #[test]
    fn an_x_is_rounded_only_when_that_makes_it_wider() {
        // 7.4 counts as itself and 7.6 as 8: ten characters on an ascent of
        // 12 are 12 + 9 × 7.4 = 78.6, so 79, and 12 + 9 × 8 = 84.
        let narrow = Face {
            x: 7.4,
            ascent: 12.0,
        };
        let wide = Face {
            x: 7.6,
            ascent: 12.0,
        };
        assert!((of(10, &at(13.0), &narrow) - 79.0).abs() < 1e-3);
        assert!((of(10, &at(13.0), &wide) - 84.0).abs() < 1e-3);
    }

    #[test]
    fn one_character_is_the_ascent() {
        let face = Face {
            x: 3.0,
            ascent: 17.0,
        };
        assert!((of(1, &at(18.0), &face) - 17.0).abs() < 1e-3);
    }

    #[test]
    fn the_test_measurers_count_their_own_characters() {
        // `BlockFont`: an `x` of 8 on an ascent of 12, so 12 + 19 × 8.
        assert!((of(20, &at(16.0), &BlockFont) - 164.0).abs() < 1e-3);
        // `ScaledFont` at 20 px: 10 on 15, so 15 + 4 × 10.
        assert!((of(5, &at(20.0), &ScaledFont) - 55.0).abs() < 1e-3);
    }
}
