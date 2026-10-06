/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Geometry attributes as user units.
//!
//! `x="10"`, `r="2em"`, `width="50%"`: a plain number is user units, a length
//! is a CSS length, and a percentage is a share of the **viewport** — its width
//! for something horizontal, its height for something vertical, and for
//! anything else (a circle's radius) the viewport's diagonal over √2, which is
//! SVG's way of giving a percentage one meaning whichever way the viewport is
//! turned.

use alo_value::svg_number as number;
use alo_value::{FontMetrics, LengthPercentage};

/// Which side of the viewport a percentage is a share of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Across: `x`, `cx`, `rx`, `width`.
    Horizontal,
    /// Down: `y`, `cy`, `ry`, `height`.
    Vertical,
    /// Neither: a circle's `r`.
    Other,
}

/// The rectangle a percentage is measured against, in user units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// How wide.
    pub width: f32,
    /// How tall.
    pub height: f32,
}

impl Viewport {
    /// What 100% is along an axis.
    pub fn basis(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
            Axis::Other => (self.width.mul_add(self.width, self.height * self.height) / 2.0).sqrt(),
        }
    }
}

/// An attribute's value in user units, or why it is not one.
///
/// # Errors
///
/// Text that is not a number, a length or a percentage, and a value that does
/// not come out finite.
pub fn user_units(
    text: &str,
    axis: Axis,
    viewport: Viewport,
    metrics: FontMetrics,
) -> Result<f32, &'static str> {
    let value = match number::number(text) {
        Some(value) => value,
        None => alo_value::parse_length_percentage(text.trim())
            .ok_or("not a number, a length or a percentage")?
            .to_px(metrics, viewport.basis(axis)),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err("not a finite length")
    }
}

/// Whether a length was written as a percentage, which some attributes care
/// about only to say so.
pub fn is_percentage(text: &str) -> bool {
    matches!(
        alo_value::parse_length_percentage(text.trim()),
        Some(LengthPercentage::Percentage(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> FontMetrics {
        FontMetrics {
            font_size: 10.0,
            ..FontMetrics::default()
        }
    }

    const VIEW: Viewport = Viewport {
        width: 30.0,
        height: 40.0,
    };

    #[test]
    fn a_number_is_user_units_and_a_length_is_converted() {
        assert_eq!(
            user_units("12", Axis::Horizontal, VIEW, metrics()),
            Ok(12.0)
        );
        assert_eq!(
            user_units("-3.5", Axis::Vertical, VIEW, metrics()),
            Ok(-3.5)
        );
        assert_eq!(user_units("1in", Axis::Other, VIEW, metrics()), Ok(96.0));
        assert_eq!(user_units("2em", Axis::Other, VIEW, metrics()), Ok(20.0));
        assert_eq!(user_units(" 4px ", Axis::Other, VIEW, metrics()), Ok(4.0));
    }

    #[test]
    fn a_percentage_is_a_share_of_the_viewport_along_its_axis() {
        assert_eq!(
            user_units("50%", Axis::Horizontal, VIEW, metrics()),
            Ok(15.0)
        );
        assert_eq!(user_units("50%", Axis::Vertical, VIEW, metrics()), Ok(20.0));
        // √((30² + 40²) / 2) = √1250 ≈ 35.355; ten per cent of it.
        let radius = user_units("10%", Axis::Other, VIEW, metrics()).expect("a length");
        assert!((radius - 3.535_534).abs() < 1.0e-4, "{radius}");
        assert!(is_percentage("10%") && !is_percentage("10"));
    }

    #[test]
    fn what_is_not_a_length_is_refused() {
        for text in ["", "ten", "1e99999", "inf", "10 20", "red"] {
            assert!(
                user_units(text, Axis::Horizontal, VIEW, metrics()).is_err(),
                "{text}"
            );
        }
    }
}
