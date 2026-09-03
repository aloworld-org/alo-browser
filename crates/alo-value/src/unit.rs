/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The units a length can be written in, and what each is worth.
//!
//! Two kinds, and the difference is the whole reason this file exists:
//!
//! - **Absolute** units are worth a fixed number of CSS pixels. An inch is 96
//!   of them, by definition, and every other absolute unit is defined from the
//!   inch. There is no context to consult.
//! - **Font-relative** units are worth whatever the font in force says. `em`
//!   is this element's font size, `rem` is the root's — and that is why this
//!   whole layer had to wait for the cascade: until the cascade has run, there
//!   is no font size to be relative to.
//!
//! Absent, deliberately: `vw`, `vh` and the viewport units. They are relative
//! to a viewport, and a viewport is layout's, not style's — they belong with
//! the code that knows how big the window is, and inventing a default here
//! would be a number nobody chose.

use core::fmt;

/// A unit a length can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Unit {
    /// A CSS pixel. Every absolute unit is defined from it.
    #[default]
    Px,
    /// An inch: 96 CSS pixels, by definition.
    In,
    /// A centimetre.
    Cm,
    /// A millimetre.
    Mm,
    /// A quarter of a millimetre.
    Q,
    /// A point: a seventy-second of an inch.
    Pt,
    /// A pica: twelve points.
    Pc,
    /// This element's font size.
    Em,
    /// The root element's font size.
    Rem,
    /// The font's `x` height — the height of a lowercase letter.
    Ex,
    /// The width of the font's zero.
    Ch,
    /// The line height in force.
    Lh,
    /// The root element's line height.
    Rlh,
    /// A hundredth of the viewport's width.
    Vw,
    /// A hundredth of the viewport's height.
    Vh,
    /// A hundredth of the viewport's shorter side.
    Vmin,
    /// A hundredth of the viewport's longer side.
    Vmax,
}

impl Unit {
    /// The unit a suffix spells, if this engine has it.
    pub fn parse(suffix: &str) -> Option<Self> {
        const ALL: &[Unit] = &[
            Unit::Px,
            Unit::In,
            Unit::Cm,
            Unit::Mm,
            Unit::Q,
            Unit::Pt,
            Unit::Pc,
            Unit::Em,
            Unit::Rem,
            Unit::Ex,
            Unit::Ch,
            Unit::Lh,
            Unit::Rlh,
            Unit::Vw,
            Unit::Vh,
            Unit::Vmin,
            Unit::Vmax,
        ];
        ALL.iter()
            .copied()
            .find(|unit| unit.as_str().eq_ignore_ascii_case(suffix))
    }

    /// The suffix, as written.
    pub fn as_str(self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::In => "in",
            Unit::Cm => "cm",
            Unit::Mm => "mm",
            Unit::Q => "q",
            Unit::Pt => "pt",
            Unit::Pc => "pc",
            Unit::Em => "em",
            Unit::Rem => "rem",
            Unit::Ex => "ex",
            Unit::Ch => "ch",
            Unit::Lh => "lh",
            Unit::Rlh => "rlh",
            Unit::Vw => "vw",
            Unit::Vh => "vh",
            Unit::Vmin => "vmin",
            Unit::Vmax => "vmax",
        }
    }

    /// What one of this unit is worth in CSS pixels, for the units that do not
    /// need a font to answer.
    pub fn absolute_pixels(self) -> Option<f32> {
        Some(match self {
            Unit::Px => 1.0,
            // An inch is 96 CSS pixels by definition, and everything else
            // absolute follows from that rather than from a screen.
            Unit::In => 96.0,
            Unit::Cm => 96.0 / 2.54,
            Unit::Mm => 96.0 / 25.4,
            Unit::Q => 96.0 / 25.4 / 4.0,
            Unit::Pt => 96.0 / 72.0,
            Unit::Pc => 96.0 / 6.0,
            Unit::Em
            | Unit::Rem
            | Unit::Ex
            | Unit::Ch
            | Unit::Lh
            | Unit::Rlh
            | Unit::Vw
            | Unit::Vh
            | Unit::Vmin
            | Unit::Vmax => return None,
        })
    }

    /// Whether this unit needs a font in force to mean anything.
    pub fn is_font_relative(self) -> bool {
        self.absolute_pixels().is_none() && !self.is_viewport_relative()
    }

    /// Whether this unit needs a window to mean anything.
    ///
    /// The reason these were left out for so long: a viewport is not a
    /// property of a value, and a value layer that quietly assumed one would
    /// answer `4vw` with a number in a document nobody had given a size.
    /// [`crate::FontMetrics`] now carries the viewport when there is one, and
    /// says so when there is not.
    pub fn is_viewport_relative(self) -> bool {
        matches!(self, Unit::Vw | Unit::Vh | Unit::Vmin | Unit::Vmax)
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Unit] = &[
        Unit::Px,
        Unit::In,
        Unit::Cm,
        Unit::Mm,
        Unit::Q,
        Unit::Pt,
        Unit::Pc,
        Unit::Em,
        Unit::Rem,
        Unit::Ex,
        Unit::Ch,
        Unit::Lh,
        Unit::Rlh,
    ];

    #[test]
    fn every_unit_round_trips_through_its_suffix() {
        for unit in ALL {
            assert_eq!(Unit::parse(unit.as_str()), Some(*unit));
            assert_eq!(unit.to_string(), unit.as_str());
        }
        assert_eq!(Unit::parse("PX"), Some(Unit::Px));
        assert_eq!(Unit::parse("REM"), Some(Unit::Rem));
    }

    #[test]
    fn a_unit_this_engine_does_not_have_is_refused() {
        for suffix in ["dvh", "svh", "lvh", "cqw", "fr", "s", "deg", "", "pxx"] {
            assert_eq!(Unit::parse(suffix), None, "{suffix} should be refused");
        }
    }

    #[test]
    fn the_viewport_units_are_read_and_need_a_window() {
        for (suffix, unit) in [
            ("vw", Unit::Vw),
            ("vh", Unit::Vh),
            ("vmin", Unit::Vmin),
            ("VMAX", Unit::Vmax),
        ] {
            assert_eq!(Unit::parse(suffix), Some(unit));
            assert!(unit.is_viewport_relative(), "{suffix}");
            assert!(!unit.is_font_relative(), "a window is not a font");
            assert_eq!(unit.absolute_pixels(), None);
        }
    }

    #[test]
    fn the_absolute_units_are_all_defined_from_the_inch() {
        assert_eq!(Unit::Px.absolute_pixels(), Some(1.0));
        assert_eq!(Unit::In.absolute_pixels(), Some(96.0));
        assert_eq!(Unit::Pt.absolute_pixels(), Some(96.0 / 72.0));
        assert_eq!(Unit::Pc.absolute_pixels(), Some(16.0));

        // A centimetre is an inch over 2.54, and forty of them make about
        // fifteen and three quarter inches. Checked as arithmetic rather than
        // as a number somebody typed.
        let cm = Unit::Cm.absolute_pixels().expect("absolute");
        assert!((cm - 96.0 / 2.54).abs() < f32::EPSILON);
        let mm = Unit::Mm.absolute_pixels().expect("absolute");
        assert!(
            (mm * 10.0 - cm).abs() < 0.001,
            "ten millimetres are a centimetre"
        );
        let q = Unit::Q.absolute_pixels().expect("absolute");
        assert!((q * 4.0 - mm).abs() < 0.001, "four Q are a millimetre");
    }

    #[test]
    fn the_font_relative_units_have_no_answer_without_a_font() {
        for unit in [Unit::Em, Unit::Rem, Unit::Ex, Unit::Ch, Unit::Lh, Unit::Rlh] {
            assert_eq!(unit.absolute_pixels(), None, "{unit}");
            assert!(unit.is_font_relative(), "{unit}");
        }
        for unit in [Unit::Px, Unit::In, Unit::Pt] {
            assert!(!unit.is_font_relative(), "{unit}");
        }
    }
}
