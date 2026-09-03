//! A length, and what it is worth in pixels.
//!
//! # Why a percentage is not a length here
//!
//! `width: 50%` is half of *something*, and which something depends on the
//! property and on the box's parent — the width for `width`, the *inline* size
//! for `margin-top`, nothing at all for `border-width`. Style cannot know it;
//! only layout can. So a percentage is carried as a percentage and resolved by
//! whoever knows the basis, and a type that pretended otherwise would force
//! every caller to supply a number it does not have.
//!
//! That is the same reason `vw` and `vh` are not units here: they are relative
//! to a viewport, and a viewport belongs to layout.

use crate::calc::CalcNode;
use crate::unit::Unit;
use core::fmt;

/// What a font-relative length needs in order to become a number.
///
/// Every field is in CSS pixels and every one of them comes from the cascade
/// having already run: `em` is *this* element's font size, which is not known
/// until the element has been styled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// This element's font size.
    pub font_size: f32,
    /// The root element's font size, for `rem`.
    pub root_font_size: f32,
    /// The height of a lowercase `x` in the font in force, for `ex`.
    pub x_height: f32,
    /// The width of a `0` in the font in force, for `ch`.
    pub zero_width: f32,
    /// The line height in force, for `lh`.
    pub line_height: f32,
    /// The root element's line height, for `rlh`.
    pub root_line_height: f32,
    /// The window, for `vw`, `vh`, `vmin` and `vmax`, or [`None`] when this
    /// value is being resolved without one.
    ///
    /// An [`Option`] on purpose. A viewport is not a property of a value, and
    /// somewhere — a test, a measurement taken before there is a window —
    /// there is no window to ask. Answering `4vw` with a number in that case
    /// would be a guess; answering it with zero is at least a number nobody
    /// can mistake for a measurement.
    pub viewport: Option<Viewport>,
}

/// How big the window is, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Viewport {
    /// Across.
    pub width: f32,
    /// Down.
    pub height: f32,
}

impl Viewport {
    /// A window of this size.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl FontMetrics {
    /// The metrics a font of this size has, before there is a real font to
    /// measure.
    ///
    /// `ex` and `ch` are estimated as one half and one half of the font size —
    /// which is what every engine does before shaping, and what queue item 6
    /// replaces with the font's own numbers. It is written here rather than
    /// buried so that the day it is wrong, the wrongness is findable.
    pub fn estimated(font_size: f32, root_font_size: f32) -> Self {
        Self {
            font_size,
            root_font_size,
            x_height: font_size * 0.5,
            zero_width: font_size * 0.5,
            line_height: font_size * 1.2,
            root_line_height: root_font_size * 1.2,
            viewport: None,
        }
    }

    /// The same metrics, in a window of this size.
    #[must_use]
    pub fn in_viewport(self, viewport: Viewport) -> Self {
        Self {
            viewport: Some(viewport),
            ..self
        }
    }

    /// What one of a unit is worth, in CSS pixels.
    fn pixels_per(self, unit: Unit) -> f32 {
        if let Some(absolute) = unit.absolute_pixels() {
            return absolute;
        }
        if unit.is_viewport_relative() {
            let Some(viewport) = self.viewport else {
                // Said rather than guessed: see the field's own documentation.
                return 0.0;
            };
            return match unit {
                Unit::Vw => viewport.width / 100.0,
                Unit::Vh => viewport.height / 100.0,
                Unit::Vmin => viewport.width.min(viewport.height) / 100.0,
                _ => viewport.width.max(viewport.height) / 100.0,
            };
        }
        match unit {
            Unit::Em => self.font_size,
            Unit::Rem => self.root_font_size,
            Unit::Ex => self.x_height,
            Unit::Ch => self.zero_width,
            Unit::Lh => self.line_height,
            Unit::Rlh => self.root_line_height,
            // Every absolute unit was answered above.
            _ => 1.0,
        }
    }
}

impl Default for FontMetrics {
    /// A sixteen-pixel font, which is what a document gets when nothing says
    /// otherwise.
    fn default() -> Self {
        Self::estimated(16.0, 16.0)
    }
}

/// A length: a number and a unit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Length {
    /// How many.
    pub value: f32,
    /// Of what.
    pub unit: Unit,
}

impl Length {
    /// A length in CSS pixels.
    pub fn px(value: f32) -> Self {
        Self {
            value,
            unit: Unit::Px,
        }
    }

    /// Zero, in the unit that needs no context.
    pub const ZERO: Length = Length {
        value: 0.0,
        unit: Unit::Px,
    };

    /// This length in CSS pixels.
    pub fn to_px(self, metrics: FontMetrics) -> f32 {
        self.value * metrics.pixels_per(self.unit)
    }

    /// This length in CSS pixels, if it needs no font to say so.
    ///
    /// Useful where there is no element to be relative to — a media query, or
    /// the root's own font size before it has one.
    pub fn to_absolute_px(self) -> Option<f32> {
        Some(self.value * self.unit.absolute_pixels()?)
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

/// A length, a percentage of something only layout knows, or an expression
/// that has to wait for both.
///
/// Not `Copy`: an expression is a tree, and making the common cases copyable
/// at the cost of a second type for the third one would push the difference
/// onto every caller.
#[derive(Debug, Clone, PartialEq)]
pub enum LengthPercentage {
    /// A length.
    Length(Length),
    /// A percentage, as written: `50%` is `50.0`, not `0.5`. Kept in the form
    /// the author wrote so that a diagnostic reads back the same way.
    Percentage(f32),
    /// A `calc()` expression, already type-checked.
    Calc(Box<CalcNode>),
}

impl LengthPercentage {
    /// Zero.
    pub const ZERO: LengthPercentage = LengthPercentage::Length(Length::ZERO);

    /// This value in CSS pixels, given what a percentage would be a percentage
    /// *of*.
    ///
    /// The basis is the caller's to supply because only the caller knows it:
    /// the containing block's width for `width`, and — the one that surprises
    /// people — also for `margin-top`, because CSS resolves vertical
    /// percentages against the inline size.
    pub fn to_px(&self, metrics: FontMetrics, basis: f32) -> f32 {
        match self {
            LengthPercentage::Length(length) => length.to_px(metrics),
            LengthPercentage::Percentage(percent) => percent / 100.0 * basis,
            LengthPercentage::Calc(node) => node.evaluate(metrics, basis),
        }
    }

    /// This value in CSS pixels, if no percentage is involved anywhere in it.
    pub fn to_px_without_basis(&self, metrics: FontMetrics) -> Option<f32> {
        if self.is_percentage() {
            return None;
        }
        Some(self.to_px(metrics, 0.0))
    }

    /// Whether a percentage appears anywhere in this value, and so a basis is
    /// needed before it is a number.
    pub fn is_percentage(&self) -> bool {
        match self {
            LengthPercentage::Length(_) => false,
            LengthPercentage::Percentage(_) => true,
            LengthPercentage::Calc(node) => node.has_percentage(),
        }
    }
}

impl Default for LengthPercentage {
    /// Zero — which is what a padding, a border width and a gap all are before
    /// anybody sets them.
    fn default() -> Self {
        LengthPercentage::ZERO
    }
}

impl fmt::Display for LengthPercentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthPercentage::Length(length) => write!(f, "{length}"),
            LengthPercentage::Percentage(percent) => write!(f, "{percent}%"),
            LengthPercentage::Calc(node) => write!(f, "calc{node}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn an_absolute_length_needs_no_font_to_be_a_number() {
        assert!(close(Length::px(16.0).to_absolute_px().expect("px"), 16.0));
        assert!(close(
            Length {
                value: 1.0,
                unit: Unit::In,
            }
            .to_absolute_px()
            .expect("in"),
            96.0,
        ));
        assert!(close(
            Length {
                value: 12.0,
                unit: Unit::Pt,
            }
            .to_absolute_px()
            .expect("pt"),
            16.0,
        ));
    }

    #[test]
    fn a_font_relative_length_has_no_answer_without_one() {
        assert_eq!(
            Length {
                value: 2.0,
                unit: Unit::Em,
            }
            .to_absolute_px(),
            None,
        );
    }

    #[test]
    fn em_is_this_elements_font_size_and_rem_is_the_roots() {
        let metrics = FontMetrics::estimated(24.0, 16.0);
        assert!(close(
            Length {
                value: 2.0,
                unit: Unit::Em,
            }
            .to_px(metrics),
            48.0,
        ));
        assert!(close(
            Length {
                value: 2.0,
                unit: Unit::Rem,
            }
            .to_px(metrics),
            32.0,
        ));
    }

    #[test]
    fn the_estimated_metrics_are_written_down_rather_than_hidden() {
        let metrics = FontMetrics::estimated(20.0, 16.0);
        assert!(close(metrics.x_height, 10.0));
        assert!(close(metrics.zero_width, 10.0));
        assert!(close(metrics.line_height, 24.0));
        assert!(close(metrics.root_line_height, 19.2));
    }

    #[test]
    fn the_default_font_is_sixteen_pixels() {
        let metrics = FontMetrics::default();
        assert!(close(metrics.font_size, 16.0));
        assert!(close(
            Length {
                value: 1.0,
                unit: Unit::Em,
            }
            .to_px(metrics),
            16.0,
        ));
    }

    #[test]
    fn a_percentage_is_carried_until_somebody_knows_what_of() {
        let half = LengthPercentage::Percentage(50.0);
        assert!(half.is_percentage());
        assert_eq!(half.to_px_without_basis(FontMetrics::default()), None);
        assert!(close(half.to_px(FontMetrics::default(), 400.0), 200.0));
        assert_eq!(half.to_string(), "50%");
    }

    #[test]
    fn a_calc_waits_for_both_the_font_and_the_basis() {
        use crate::calc::CalcNode;
        let half_less_ten = LengthPercentage::Calc(Box::new(CalcNode::Sum(vec![
            CalcNode::Percentage(50.0),
            CalcNode::Negate(Box::new(CalcNode::Length(Length::px(10.0)))),
        ])));
        assert!(half_less_ten.is_percentage());
        assert_eq!(
            half_less_ten.to_px_without_basis(FontMetrics::default()),
            None
        );
        assert!(close(
            half_less_ten.to_px(FontMetrics::default(), 400.0),
            190.0
        ));

        let twice_the_font = LengthPercentage::Calc(Box::new(CalcNode::Product(vec![
            CalcNode::Length(Length {
                value: 1.0,
                unit: Unit::Em,
            }),
            CalcNode::Number(2.0),
        ])));
        assert!(!twice_the_font.is_percentage());
        assert!(close(
            twice_the_font
                .to_px_without_basis(FontMetrics::estimated(20.0, 16.0))
                .expect("no percentage in it"),
            40.0,
        ));
    }

    #[test]
    fn a_length_ignores_the_basis_it_is_given() {
        let sixteen = LengthPercentage::Length(Length::px(16.0));
        assert!(!sixteen.is_percentage());
        assert!(close(sixteen.to_px(FontMetrics::default(), 400.0), 16.0));
        assert!(close(sixteen.to_px(FontMetrics::default(), 0.0), 16.0));
    }

    #[test]
    fn zero_is_zero_whatever_it_is_asked() {
        assert!(close(Length::ZERO.to_px(FontMetrics::default()), 0.0));
        assert!(close(
            LengthPercentage::ZERO.to_px(FontMetrics::default(), 999.0),
            0.0,
        ));
    }

    #[test]
    fn a_length_writes_itself_back_out() {
        assert_eq!(Length::px(16.0).to_string(), "16px");
        assert_eq!(
            Length {
                value: 1.5,
                unit: Unit::Rem,
            }
            .to_string(),
            "1.5rem",
        );
        assert_eq!(LengthPercentage::Length(Length::px(4.0)).to_string(), "4px",);
    }
}
