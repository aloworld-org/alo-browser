/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What `vertical-align` says, and where it puts a box on its line.
//!
//! A box on a line stands with its baseline on its parent's unless this
//! property says otherwise. The parent is the inline box it is inside, or the
//! block that holds the lines, whose font is the strut's. Every keyword but
//! two is a distance from that parent's baseline:
//!
//! - **`baseline`** — none. The initial value.
//! - **`middle`** — the box's vertical midpoint at the parent's baseline plus
//!   half the parent's x-height, which is where the middle of a lowercase
//!   letter is. That is the keyword icons beside text are given. Both halves
//!   are put on the layout grid as Chrome puts them (queue item 598): half
//!   the x-height rounded, a half going up, and half the box's height
//!   truncated toward zero, each on its own, so that the raise is on the grid
//!   and so is the line that holds it.
//! - **`text-top`** and **`text-bottom`** — the box's top at the top of the
//!   parent's font, or its bottom at the bottom of it.
//! - **`sub`** and **`super`** — CSS leaves "the proper position" to the
//!   browser. This engine uses the numbers Chromium does, a fifth of the
//!   parent's font size plus one pixel down and a third plus one up, because
//!   those are the positions an author has seen.
//! - **a length or a percentage** — that far up, a negative one down. A
//!   percentage is of the box's own `line-height`.
//!
//! The two that are not distances are **`top`** and **`bottom`**: the box, and
//! everything inside it, against the top or the bottom of the line box itself.
//! Where that is depends on everything else on the line, so the line builder
//! places them after the rest, and a box taller than the line makes the line
//! grow on the side away from where it is held.
//!
//! What moved still counts: a box raised above everything else on its line
//! makes the line taller above the baseline, and one lowered makes it deeper
//! below, so nothing raised or lowered overlaps the line before or after.

use crate::layout_unit;
use alo_value::{FontMetrics, LengthPercentage, parse_length_percentage};

/// `vertical-align` as it was written.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum VerticalAlign {
    /// `baseline`.
    #[default]
    Baseline,
    /// `sub`.
    Sub,
    /// `super`.
    Super,
    /// `text-top`.
    TextTop,
    /// `text-bottom`.
    TextBottom,
    /// `middle`.
    Middle,
    /// `top`.
    Top,
    /// `bottom`.
    Bottom,
    /// A length or a percentage: how far up.
    Shift(LengthPercentage),
}

impl VerticalAlign {
    /// Read the property's text, or [`None`] for something this engine does
    /// not implement.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        for (value, name) in [
            (VerticalAlign::Baseline, "baseline"),
            (VerticalAlign::Sub, "sub"),
            (VerticalAlign::Super, "super"),
            (VerticalAlign::TextTop, "text-top"),
            (VerticalAlign::TextBottom, "text-bottom"),
            (VerticalAlign::Middle, "middle"),
            (VerticalAlign::Top, "top"),
            (VerticalAlign::Bottom, "bottom"),
        ] {
            if text.eq_ignore_ascii_case(name) {
                return Some(value.clone());
            }
        }
        parse_length_percentage(text).map(VerticalAlign::Shift)
    }

    /// The same value with its length a number: `metrics` are the box's own,
    /// and a percentage is of its own line height.
    pub fn resolve(&self, metrics: FontMetrics) -> LineAlign {
        match self {
            VerticalAlign::Baseline => LineAlign::Baseline,
            VerticalAlign::Sub => LineAlign::Sub,
            VerticalAlign::Super => LineAlign::Super,
            VerticalAlign::TextTop => LineAlign::TextTop,
            VerticalAlign::TextBottom => LineAlign::TextBottom,
            VerticalAlign::Middle => LineAlign::Middle,
            VerticalAlign::Top => LineAlign::Top,
            VerticalAlign::Bottom => LineAlign::Bottom,
            VerticalAlign::Shift(length) => {
                LineAlign::Raise(length.to_px(metrics, metrics.line_height))
            }
        }
    }
}

/// `vertical-align`, ready for the line: every length already a number.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LineAlign {
    /// On the parent's baseline.
    #[default]
    Baseline,
    /// Lowered as a subscript.
    Sub,
    /// Raised as a superscript.
    Super,
    /// Its top at the top of the parent's font.
    TextTop,
    /// Its bottom at the bottom of the parent's font.
    TextBottom,
    /// Its middle at the middle of the parent's lowercase letters.
    Middle,
    /// Against the top of the line box.
    Top,
    /// Against the bottom of the line box.
    Bottom,
    /// This many pixels above the parent's baseline.
    Raise(f32),
}

/// What a box is aligned against: its parent on the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parent {
    /// How far its font reaches above its baseline.
    pub ascent: f32,
    /// How far below.
    pub descent: f32,
    /// The height of its lowercase `x`.
    pub x_height: f32,
    /// Its font size.
    pub font_size: f32,
    /// The scale of the screen it is laid out for, whose grid a raise is put
    /// on.
    pub scale: f32,
}

/// Which way a box held by the line box itself is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// `top`.
    Top,
    /// `bottom`.
    Bottom,
}

impl LineAlign {
    /// How far above its parent's baseline a box's own baseline sits, or the
    /// edge of the line box that holds it instead.
    ///
    /// `above` and `below` are how far the box reaches above and below its own
    /// baseline: the box that is aligned, which for an atomic box is its
    /// margin box and for an inline box is its font.
    ///
    /// # Errors
    ///
    /// `top` and `bottom` are not a distance from the parent at all, and
    /// answer with the [`Edge`] that holds the box instead.
    pub fn raise(self, above: f32, below: f32, parent: Parent) -> Result<f32, Edge> {
        Ok(match self {
            LineAlign::Baseline => 0.0,
            LineAlign::Sub => -(parent.font_size / 5.0 + 1.0),
            LineAlign::Super => parent.font_size / 3.0 + 1.0,
            LineAlign::TextTop => parent.ascent - above,
            LineAlign::TextBottom => below - parent.descent,
            // The box's middle is (above - below) / 2 over its own baseline.
            LineAlign::Middle => {
                layout_unit::half_rounded(parent.x_height, parent.scale)
                    - layout_unit::half_truncated(above - below, parent.scale)
            }
            LineAlign::Raise(by) => by,
            LineAlign::Top => return Err(Edge::Top),
            LineAlign::Bottom => return Err(Edge::Bottom),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARENT: Parent = Parent {
        ascent: 12.0,
        descent: 4.0,
        x_height: 8.0,
        font_size: 16.0,
        scale: 1.0,
    };

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn every_keyword_reads_and_nothing_else_does() {
        for (text, expected) in [
            ("baseline", VerticalAlign::Baseline),
            ("SUB", VerticalAlign::Sub),
            ("super", VerticalAlign::Super),
            (" text-top ", VerticalAlign::TextTop),
            ("text-bottom", VerticalAlign::TextBottom),
            ("middle", VerticalAlign::Middle),
            ("top", VerticalAlign::Top),
            ("bottom", VerticalAlign::Bottom),
        ] {
            assert_eq!(VerticalAlign::parse(text), Some(expected), "{text}");
        }
        assert!(matches!(
            VerticalAlign::parse("-3px"),
            Some(VerticalAlign::Shift(_))
        ));
        assert!(matches!(
            VerticalAlign::parse("50%"),
            Some(VerticalAlign::Shift(_))
        ));
        for refused in ["centre", "first baseline", "auto", "", "3"] {
            assert_eq!(VerticalAlign::parse(refused), None, "{refused}");
        }
    }

    #[test]
    fn a_length_is_pixels_and_a_percentage_is_of_the_line_height() {
        let metrics = FontMetrics {
            line_height: 20.0,
            ..FontMetrics::estimated(16.0, 16.0)
        };
        let raise = |text: &str| VerticalAlign::parse(text).map(|value| value.resolve(metrics));
        assert_eq!(raise("-3px"), Some(LineAlign::Raise(-3.0)));
        assert_eq!(raise("50%"), Some(LineAlign::Raise(10.0)));
        assert_eq!(raise("0.5em"), Some(LineAlign::Raise(8.0)));
        assert_eq!(raise("middle"), Some(LineAlign::Middle));
    }

    #[test]
    fn each_keyword_is_a_distance_from_the_parents_baseline() {
        // A 20-pixel box standing on its bottom edge: all of it above its
        // baseline.
        let raise = |align: LineAlign| align.raise(20.0, 0.0, PARENT);
        assert_eq!(raise(LineAlign::Baseline), Ok(0.0));
        assert_eq!(raise(LineAlign::Sub), Ok(-4.2));
        assert!(raise(LineAlign::Super).is_ok_and(|by| close(by, 16.0 / 3.0 + 1.0)));
        // Its top at 12 over the parent's baseline: its baseline 8 under it.
        assert_eq!(raise(LineAlign::TextTop), Ok(-8.0));
        // Its bottom, which is its baseline, 4 under.
        assert_eq!(raise(LineAlign::TextBottom), Ok(-4.0));
        // Its middle, 10 over its baseline, at 4 over the parent's.
        assert_eq!(raise(LineAlign::Middle), Ok(-6.0));
        assert_eq!(raise(LineAlign::Raise(-3.0)), Ok(-3.0));
        assert_eq!(raise(LineAlign::Top), Err(Edge::Top));
        assert_eq!(raise(LineAlign::Bottom), Err(Edge::Bottom));
    }

    #[test]
    fn middle_puts_both_halves_on_the_grid_as_chrome_does() {
        // Measured in Chrome 154 (queue item 598): half the x-height rounded
        // to a unit, a half going up, and half the box's height truncated.
        let middle = |x_height: f32, height: f32, scale: f32| {
            let parent = Parent {
                x_height,
                scale,
                ..PARENT
            };
            LineAlign::Middle.raise(height, 0.0, parent)
        };
        // `DejaVu Sans` at 13 px: 455 units to its x-height, raised 228 for
        // it; a box 1,183 units tall has its middle 591 over its baseline.
        assert!(middle(455.0 / 64.0, 1183.0 / 64.0, 1.0).is_ok_and(|by| close(by, -363.0 / 64.0)));
        // At two the same are 910 and 2,366 of a smaller unit.
        assert!(middle(455.0 / 64.0, 1183.0 / 64.0, 2.0).is_ok_and(|by| close(by, -728.0 / 128.0)));
        // At 1.5, 12.25 px is set at 18.37 device pixels: 642.95 units to
        // its x-height, whose half is 321, not the 322 a sixteenth's slack
        // would take it to. A 9 px box is 864 units, half of it 432.
        assert!(middle(642.95 / 96.0, 9.0, 1.5).is_ok_and(|by| close(by, -111.0 / 96.0)));
        // Below its baseline more than above, a box's half is truncated
        // toward zero: 7 units more below is 3 more, not 4.
        assert!(
            middle(8.0, 0.0, 1.0).is_ok_and(|by| close(by, 4.0))
                && LineAlign::Middle
                    .raise(0.0, 7.0 / 64.0, PARENT)
                    .is_ok_and(|by| close(by, 4.0 + 3.0 / 64.0))
        );
    }

    #[test]
    fn middle_and_the_text_edges_count_what_hangs_below_a_baseline() {
        // Text in a 10-pixel font: 8 above its baseline, 2 below.
        let raise = |align: LineAlign| align.raise(8.0, 2.0, PARENT);
        assert_eq!(raise(LineAlign::TextTop), Ok(4.0));
        assert_eq!(raise(LineAlign::TextBottom), Ok(-2.0));
        assert_eq!(raise(LineAlign::Middle), Ok(1.0));
    }
}
