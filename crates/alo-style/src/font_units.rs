/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the cascade has to ask a font.
//!
//! `ex` is the height of the font's `x` and `ch` the advance of its `0`, so
//! neither is a number until somebody has a font to measure. The cascade does
//! not: fonts are `alo-text`'s, and a style crate that opened font files would
//! be two crates in one. So the cascade asks, the way layout asks
//! `alo_layout::MeasureText` how wide a piece of text is, and whoever has the
//! fonts answers.
//!
//! **This is a seam, not a stub.** [`NoFaces`] is a real answer for a real
//! case — a style resolved where there are no fonts, such as an SVG file's
//! fill — and what it answers is what CSS Values 4 says to assume when the
//! measurement is impossible: half an em for both.

use crate::computed::ComputedStyle;

/// The two measurements of a face the cascade's lengths need, in CSS pixels
/// at one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceUnits {
    /// The height of the face's lowercase `x`: one `ex`.
    pub x_height: f32,
    /// The advance of the face's `0`: one `ch`.
    pub zero_width: f32,
}

impl FaceUnits {
    /// What CSS Values 4 says to assume when a face cannot be measured: half
    /// the font size for each.
    pub fn assumed(font_size: f32) -> Self {
        Self {
            x_height: font_size * 0.5,
            zero_width: font_size * 0.5,
        }
    }

    /// These units where they are usable, and the assumed ones where they are
    /// not.
    ///
    /// The answer comes from a font file, and a font file is bytes from
    /// outside. A negative, infinite or not-a-number advance is no unit, and
    /// handing one to layout would be a page that cannot be laid out, so each
    /// measurement is checked on its own and replaced alone.
    #[must_use]
    pub fn or_assumed(self, font_size: f32) -> Self {
        let assumed = Self::assumed(font_size);
        let usable = |value: f32| value.is_finite() && value >= 0.0;
        Self {
            x_height: if usable(self.x_height) {
                self.x_height
            } else {
                assumed.x_height
            },
            zero_width: if usable(self.zero_width) {
                self.zero_width
            } else {
                assumed.zero_width
            },
        }
    }
}

/// Whoever can measure the face an element's text is set in.
pub trait MeasureFace {
    /// The `ex` and `ch` of the first face this style's text would be set in,
    /// or [`None`] when there is no face to ask.
    ///
    /// The style is part-way through being computed when it is asked: its
    /// `font-family`, `font-weight`, `font-style` and font size are final,
    /// and its `ex`, `ch` and line height are not yet — they are what this is
    /// for. A face without an `x` or a `0` answers half its size for that
    /// unit, as CSS says.
    fn face_units(&self, style: &ComputedStyle) -> Option<FaceUnits>;
}

/// No fonts at all, so every face is unmeasurable and every `ex` and `ch` is
/// half an em.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFaces;

impl MeasureFace for NoFaces {
    fn face_units(&self, _style: &ComputedStyle) -> Option<FaceUnits> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_faces_measures_nothing() {
        assert_eq!(NoFaces.face_units(&ComputedStyle::new()), None);
    }

    #[test]
    fn the_assumed_units_are_half_an_em() {
        assert_eq!(
            FaceUnits::assumed(20.0),
            FaceUnits {
                x_height: 10.0,
                zero_width: 10.0
            }
        );
    }

    #[test]
    fn an_unusable_measurement_is_replaced_alone() {
        let measured = FaceUnits {
            x_height: 9.0,
            zero_width: 12.0,
        };
        assert_eq!(measured.or_assumed(20.0), measured);
        assert_eq!(
            FaceUnits {
                x_height: f32::NAN,
                zero_width: 12.0
            }
            .or_assumed(20.0),
            FaceUnits {
                x_height: 10.0,
                zero_width: 12.0
            }
        );
        for hostile in [f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::NAN] {
            let units = FaceUnits {
                x_height: 9.0,
                zero_width: hostile,
            }
            .or_assumed(20.0);
            assert_eq!(
                units,
                FaceUnits {
                    x_height: 9.0,
                    zero_width: 10.0
                },
                "{hostile}"
            );
        }
        // Zero is a measurement: a face may have a `0` that does not advance.
        let zero = FaceUnits {
            x_height: 0.0,
            zero_width: 0.0,
        };
        assert_eq!(zero.or_assumed(20.0), zero);
    }
}
