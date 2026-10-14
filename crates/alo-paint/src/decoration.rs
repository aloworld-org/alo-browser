/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a line under, over or through text is drawn, and how thick.
//!
//! # The rule, measured
//!
//! Measured from Chrome 154's screenshots, not read from its source (queue
//! item 566): `x` and three no-break spaces in `DejaVu Sans` at twenty sizes
//! from 8 to 64 px, each underlined, overlined and struck through, at a
//! quarter, a half and three quarters of a pixel down as well as on a whole
//! one, under `--force-device-scale-factor=1` and `=2`: 480 lines, each
//! read from the columns over the spaces, where nothing else is drawn. Every
//! one is drawn by this rule to the device pixel, and none is smoothed: each
//! row a line touches is wholly inked.
//!
//! Everything is in **device** pixels, on a screen of `s` device pixels to
//! the CSS pixel, for a font of `size` CSS pixels:
//!
//! - **The thickness** is `t = max(1, size·s / 10)`, and the line drawn is
//!   `t` cut to a whole number of device pixels: two at 20 px and at 28 px
//!   at one, five at 28 px at two. The face's own figure is not used.
//! - **The top of the text** is the top of its piece, `floor(y·s + 0.5)`,
//!   and the baseline is the ascent below that, as layout took it in whole
//!   device pixels (ADR 0050, ADR 0057 § 2). A piece a quarter of a CSS
//!   pixel down at two has its lines half a device pixel further than the
//!   piece would put them, rounded up.
//! - **An underline** starts `max(1, ceil(t / 2))` below the baseline.
//! - **An overline** starts the line's whole thickness above the top.
//! - **A line through** starts `2/3` of the ascent below the top, less half
//!   of `t`.
//! - **Each line's top** is then rounded as `floor(y + 0.5)`, and the line is
//!   its whole thickness below that.

use alo_layout::Rect;

use crate::display::DecorationLine;

/// Where a piece of text is, for the lines drawn with it: all in CSS pixels.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    /// The top of the piece: its baseline less its ascent.
    pub top: f32,
    /// The face's ascent, as layout took it, in whole device pixels.
    pub ascent: f32,
    /// The left of the line.
    pub left: f32,
    /// How far the line runs.
    pub width: f32,
    /// The font's size, as computed.
    pub size: f32,
}

/// How thick a line is for a font of `size` CSS pixels on a screen of
/// `scale` device pixels to the CSS pixel, in device pixels, before it is cut
/// to whole ones: what [`line_at`] draws cut, and what a line steps aside
/// from a letter by ([`crate::skip_ink`]). A scale that is not a scale is
/// one.
pub fn thickness(size: f32, scale: f32) -> f32 {
    (size * alo_value::device_scale(scale) / 10.0).max(1.0)
}

/// The rectangle `line` is drawn in for `piece`, on a screen of `scale`
/// device pixels to the CSS pixel, in CSS pixels. A scale that is not a
/// scale is one.
pub fn line_at(line: DecorationLine, piece: Piece, scale: f32) -> Rect {
    let thickness = thickness(piece.size, scale);
    let scale = alo_value::device_scale(scale);
    let drawn = thickness.trunc();
    let top = (piece.top * scale + 0.5).floor();
    let ascent = piece.ascent * scale;
    let at = match line {
        DecorationLine::Underline => top + ascent + (thickness / 2.0).ceil().max(1.0),
        DecorationLine::Overline => top - drawn,
        DecorationLine::LineThrough => top + ascent * 2.0 / 3.0 - thickness / 2.0,
    };
    let at = (at + 0.5).floor();
    Rect::new(piece.left, at / scale, piece.width, drawn / scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// The rows a line covers, in device pixels: its first, and how many.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number of device pixels on a small page"
    )]
    fn rows(rect: Rect, scale: f32) -> (i32, i32) {
        let top = rect.top() * scale;
        let height = rect.size.height * scale;
        assert!(close(top, top.round()), "{top} is a whole device pixel");
        assert!(close(height, height.round()), "{height} is whole");
        (top.round() as i32, height.round() as i32)
    }

    fn piece(top: f32, ascent: f32, size: f32) -> Piece {
        Piece {
            top,
            ascent,
            left: 3.0,
            width: 40.0,
            size,
        }
    }

    // Each expectation below is a line read from Chrome 154's screenshots
    // (the module's documentation): the first device row it inks, and how
    // many rows. The ascent is `DejaVu Sans`' as Chrome's layout took it.

    #[test]
    fn at_one_a_line_is_whole_pixels_of_a_tenth_of_the_size_cut_down() {
        // 18 px: a tenth is 1.8, drawn as one row; 64 px: 6.4, drawn as six.
        let small = piece(910.0, 17.0, 18.0);
        assert_eq!(
            rows(line_at(DecorationLine::Underline, small, 1.0), 1.0),
            (928, 1)
        );
        assert_eq!(
            rows(line_at(DecorationLine::Overline, small, 1.0), 1.0),
            (909, 1)
        );
        assert_eq!(
            rows(line_at(DecorationLine::LineThrough, small, 1.0), 1.0),
            (920, 1)
        );
        let large = piece(1720.0, 59.0, 64.0);
        assert_eq!(
            rows(line_at(DecorationLine::Underline, large, 1.0), 1.0),
            (1783, 6)
        );
        assert_eq!(
            rows(line_at(DecorationLine::Overline, large, 1.0), 1.0),
            (1714, 6)
        );
        assert_eq!(
            rows(line_at(DecorationLine::LineThrough, large, 1.0), 1.0),
            (1756, 6)
        );
    }

    #[test]
    fn an_underline_leaves_half_its_thickness_rounded_up_below_the_baseline() {
        // 20 px at one: 2.0 thick, a gap of one. 22 px: 2.2, a gap of two.
        assert_eq!(
            rows(
                line_at(DecorationLine::Underline, piece(1000.0, 19.0, 20.0), 1.0),
                1.0
            ),
            (1020, 2)
        );
        assert_eq!(
            rows(
                line_at(DecorationLine::Underline, piece(1090.0, 20.0, 22.0), 1.0),
                1.0
            ),
            (1112, 2)
        );
    }

    #[test]
    fn at_two_the_thickness_and_gap_are_counted_in_device_pixels() {
        // 15 px at two: 3.0 device pixels thick, where at one it is one CSS
        // pixel, which would be two device pixels.
        let at = piece(640.0, 14.0, 15.0);
        assert_eq!(
            rows(line_at(DecorationLine::Underline, at, 2.0), 2.0),
            (1310, 3)
        );
        assert_eq!(
            rows(line_at(DecorationLine::Overline, at, 2.0), 2.0),
            (1277, 3)
        );
        assert_eq!(
            rows(line_at(DecorationLine::LineThrough, at, 2.0), 2.0),
            (1297, 3)
        );
        // 28 px: 5.6 thick, drawn as five, three below the baseline.
        assert_eq!(
            rows(
                line_at(DecorationLine::Underline, piece(1270.0, 26.0, 28.0), 2.0),
                2.0
            ),
            (2595, 5)
        );
    }

    #[test]
    fn a_piece_between_device_pixels_has_its_top_rounded_first() {
        // 13 px at two, the piece a quarter of a CSS pixel down: its top is
        // device pixel 740.5, drawn from 741, so the lines are where they
        // would be a half device pixel further down, not a half up.
        let at = piece(370.25, 12.0, 13.0);
        assert_eq!(
            rows(line_at(DecorationLine::Underline, at, 2.0), 2.0),
            (767, 2)
        );
        assert_eq!(
            rows(line_at(DecorationLine::Overline, at, 2.0), 2.0),
            (739, 2)
        );
        assert_eq!(
            rows(line_at(DecorationLine::LineThrough, at, 2.0), 2.0),
            (756, 2)
        );
    }

    #[test]
    fn the_line_runs_along_the_piece_unsnapped() {
        let rect = line_at(DecorationLine::Underline, piece(0.0, 13.0, 14.0), 2.0);
        assert!(close(rect.left(), 3.0));
        assert!(close(rect.size.width, 40.0));
    }

    #[test]
    fn a_scale_that_is_not_a_scale_is_one() {
        let at = piece(910.0, 17.0, 18.0);
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                rows(line_at(DecorationLine::Underline, at, scale), 1.0),
                (928, 1)
            );
        }
    }
}
