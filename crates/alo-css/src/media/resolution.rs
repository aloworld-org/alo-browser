/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A resolution in a media query, and how one is compared with a screen's
//! scale (ADR 0057 § 3, queue item 567).
//!
//! `resolution`, `min-resolution` and `max-resolution` ask about the scale
//! the page is laid out and painted at, in dots per CSS pixel. As measured
//! in Chrome 154 under `--force-device-scale-factor` at one and at two
//! (3,032 queries, `tests/references/resolution.chrome.txt`):
//!
//! - **Four units**: `dppx` and its other name `x`, `dpi` (96 to the
//!   `dppx`) and `dpcm` (96 / 2.54 to the `dppx`), each in any case.
//! - **Read as a double, compared as an `f32`**: the number as written is
//!   turned into `dppx` in `f64` and only then rounded to an `f32`, which is
//!   what the screen's scale is compared with. So `1.99999dppx` does not
//!   equal a scale of two, and neither does `191.9dpi`, while
//!   `2.0000001dppx` and `192.00001dpi` do: each is two once it is an
//!   `f32`, and the second is not if it is made an `f32` before it is
//!   divided by 96.
//! - **Except `dpcm`, compared in hundredths of a `dppx`**: the query and
//!   the scale are each rounded to a hundredth, a half up, before they are
//!   compared, so `75.59dpcm` and `75.6dpcm` both equal two and `75.8dpcm`
//!   does not.
//! - **Zero is a resolution and a negative one is not**: `(min-resolution:
//!   0dppx)` matches every screen, and `-1dppx` makes the query one this
//!   engine does not understand, which never matches.

use core::fmt;

use super::Comparison;

/// A unit a resolution is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionUnit {
    /// `dppx`: dots per CSS pixel, which a screen's scale is.
    Dppx,
    /// `x`: `dppx` by its other name.
    X,
    /// `dpi`: dots per inch, 96 to the `dppx`.
    Dpi,
    /// `dpcm`: dots per centimetre, 96 / 2.54 to the `dppx`.
    Dpcm,
}

impl ResolutionUnit {
    /// The unit `name` names, in any case, if it names one.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Dppx, Self::X, Self::Dpi, Self::Dpcm]
            .into_iter()
            .find(|unit| name.eq_ignore_ascii_case(unit.as_str()))
    }

    /// The unit, as written in CSS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dppx => "dppx",
            Self::X => "x",
            Self::Dpi => "dpi",
            Self::Dpcm => "dpcm",
        }
    }
}

/// A resolution written in a media feature, kept in the unit it was written
/// in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueryResolution {
    /// How many, as written, never below zero.
    pub value: f64,
    /// Of what.
    pub unit: ResolutionUnit,
}

impl QueryResolution {
    /// A resolution, from its parts: [`None`] when `value` is below zero or
    /// is not a number, which a resolution cannot be.
    pub fn new(value: f64, unit: ResolutionUnit) -> Option<Self> {
        // `+ 0.0` makes a `-0` the zero it is.
        (value >= 0.0).then_some(Self {
            value: value + 0.0,
            unit,
        })
    }

    /// The resolution in dots per CSS pixel.
    ///
    /// A hostile size overflows to infinity, which no screen equals or
    /// exceeds.
    pub fn dppx(self) -> f64 {
        match self.unit {
            ResolutionUnit::Dppx | ResolutionUnit::X => self.value,
            ResolutionUnit::Dpi => self.value / 96.0,
            ResolutionUnit::Dpcm => self.value * 2.54 / 96.0,
        }
    }

    /// Whether a screen of `scale` dots per CSS pixel compares with this
    /// resolution as `comparison` asks.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a resolution is compared as an f32, as measured; one too large is infinite"
    )]
    pub fn compared(self, comparison: Comparison, scale: f32) -> bool {
        let (screen, query) = match self.unit {
            ResolutionUnit::Dpcm => (hundredths(f64::from(scale)), hundredths(self.dppx())),
            ResolutionUnit::Dppx | ResolutionUnit::X | ResolutionUnit::Dpi => {
                (f64::from(scale), f64::from(self.dppx() as f32))
            }
        };
        match comparison {
            Comparison::AtLeast => screen >= query,
            Comparison::AtMost => screen <= query,
            #[allow(clippy::float_cmp)]
            Comparison::Exactly => screen == query,
        }
    }
}

impl fmt::Display for QueryResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.unit.as_str())
    }
}

/// `value` rounded to the nearest hundredth, a half up.
fn hundredths(value: f64) -> f64 {
    (0.5 + 100.0 * value).floor() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: f64, unit: ResolutionUnit) -> QueryResolution {
        let Some(resolution) = QueryResolution::new(value, unit) else {
            panic!("{value} is a resolution");
        };
        resolution
    }

    #[test]
    fn every_unit_is_named_in_any_case() {
        for (name, unit) in [
            ("dppx", ResolutionUnit::Dppx),
            ("DPPX", ResolutionUnit::Dppx),
            ("x", ResolutionUnit::X),
            ("X", ResolutionUnit::X),
            ("dpi", ResolutionUnit::Dpi),
            ("dpcm", ResolutionUnit::Dpcm),
            ("DpCm", ResolutionUnit::Dpcm),
        ] {
            assert_eq!(ResolutionUnit::from_name(name), Some(unit), "{name}");
        }
        for name in ["px", "dpp", "", "xx", "dppx "] {
            assert_eq!(ResolutionUnit::from_name(name), None, "{name:?}");
        }
    }

    #[test]
    fn a_negative_resolution_or_one_that_is_not_a_number_is_none() {
        assert!(QueryResolution::new(-1.0, ResolutionUnit::Dppx).is_none());
        assert!(QueryResolution::new(f64::NAN, ResolutionUnit::Dppx).is_none());
        assert!(QueryResolution::new(0.0, ResolutionUnit::Dppx).is_some());
        assert_eq!(at(-0.0, ResolutionUnit::X).to_string(), "0x");
    }

    #[test]
    fn each_unit_is_its_share_of_a_dppx() {
        assert_eq!(
            at(2.0, ResolutionUnit::X).dppx().to_bits(),
            2.0_f64.to_bits()
        );
        assert_eq!(
            at(192.0, ResolutionUnit::Dpi).dppx().to_bits(),
            2.0_f64.to_bits()
        );
        let dpcm = at(37.795_276, ResolutionUnit::Dpcm).dppx();
        assert!((dpcm - 1.0).abs() < 1.0e-6, "{dpcm}");
    }

    #[test]
    fn a_resolution_is_compared_as_written_and_dpcm_in_hundredths() {
        let equals_two = |value, unit| at(value, unit).compared(Comparison::Exactly, 2.0);
        assert!(equals_two(2.0, ResolutionUnit::Dppx));
        assert!(!equals_two(1.999_99, ResolutionUnit::Dppx));
        assert!(!equals_two(191.9, ResolutionUnit::Dpi));
        assert!(equals_two(2.000_000_1, ResolutionUnit::Dppx));
        assert!(equals_two(192.000_01, ResolutionUnit::Dpi));
        assert!(equals_two(75.59, ResolutionUnit::Dpcm));
        assert!(equals_two(75.6, ResolutionUnit::Dpcm));
        assert!(!equals_two(75.8, ResolutionUnit::Dpcm));
        assert!(at(75.77, ResolutionUnit::Dpcm).compared(Comparison::AtLeast, 2.0));
        assert!(!at(2.000_01, ResolutionUnit::Dppx).compared(Comparison::AtLeast, 2.0));
        assert!(at(0.0, ResolutionUnit::Dppx).compared(Comparison::AtLeast, 1.0));
        assert!(!at(1.5, ResolutionUnit::Dppx).compared(Comparison::AtMost, 2.0));
    }

    #[test]
    fn a_hostile_resolution_compares_and_never_panics() {
        for value in [f64::MAX, f64::INFINITY, 1.0e-300, f64::from(f32::MAX)] {
            for unit in [
                ResolutionUnit::Dppx,
                ResolutionUnit::X,
                ResolutionUnit::Dpi,
                ResolutionUnit::Dpcm,
            ] {
                let resolution = at(value, unit);
                let _ = resolution.compared(Comparison::Exactly, 2.0);
                let _ = resolution.to_string();
            }
        }
        assert!(!at(f64::MAX, ResolutionUnit::Dpcm).compared(Comparison::AtLeast, 2.0));
        assert!(!at(f64::MAX, ResolutionUnit::Dppx).compared(Comparison::AtLeast, 2.0));
    }

    #[test]
    fn a_resolution_writes_itself_back_out_in_its_own_unit() {
        assert_eq!(at(2.0, ResolutionUnit::Dppx).to_string(), "2dppx");
        assert_eq!(at(1.5, ResolutionUnit::X).to_string(), "1.5x");
        assert_eq!(at(192.0, ResolutionUnit::Dpi).to_string(), "192dpi");
        assert_eq!(at(75.6, ResolutionUnit::Dpcm).to_string(), "75.6dpcm");
    }
}
