/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! From user space to the box: `viewBox` and `preserveAspectRatio` as one
//! transform (ADR 0022 § 2).
//!
//! The `viewBox` says which rectangle of user space to show; the box says
//! where to show it; `preserveAspectRatio` says what to do when their shapes
//! differ — scale to fit inside (`meet`), scale to cover (`slice`), or stretch
//! (`none`), and where to put the leftover. That is SVG 2's equation, in
//! § 8.2 of the specification, and this file is that equation and its
//! grammar and nothing else.

use alo_box::svg::ViewBox;
use alo_value::Matrix;

/// Where a viewBox sits along one axis when it does not fill it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// At the start.
    Min,
    /// In the middle.
    Mid,
    /// At the end.
    Max,
}

/// Whether the whole viewBox is shown, or the whole box is covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Scaled to fit inside the box: all of it is visible.
    Meet,
    /// Scaled to cover the box: some of it may be cut off.
    Slice,
}

/// A `preserveAspectRatio` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspectRatio {
    /// Stretch each axis to fill the box.
    Stretch,
    /// Keep the shape, and align it.
    Keep {
        /// Across.
        x: Align,
        /// Down.
        y: Align,
        /// Fit or cover.
        fit: Fit,
    },
}

impl Default for AspectRatio {
    /// `xMidYMid meet`, which is what nothing written means.
    fn default() -> Self {
        Self::Keep {
            x: Align::Mid,
            y: Align::Mid,
            fit: Fit::Meet,
        }
    }
}

impl AspectRatio {
    /// Read a `preserveAspectRatio` attribute, or [`None`] if it is not one.
    ///
    /// `defer` is accepted and means nothing: it applies only to an `<image>`,
    /// which this engine does not draw (ADR 0022 § 7).
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_ascii_whitespace().peekable();
        if words.peek() == Some(&"defer") {
            words.next();
        }
        let align = words.next()?;
        let fit = match words.next() {
            None | Some("meet") => Fit::Meet,
            Some("slice") => Fit::Slice,
            Some(_) => return None,
        };
        if words.next().is_some() {
            return None;
        }
        if align == "none" {
            return Some(Self::Stretch);
        }
        let x = match align.get(..4)? {
            "xMin" => Align::Min,
            "xMid" => Align::Mid,
            "xMax" => Align::Max,
            _ => return None,
        };
        let y = match align.get(4..)? {
            "YMin" => Align::Min,
            "YMid" => Align::Mid,
            "YMax" => Align::Max,
            _ => return None,
        };
        Some(Self::Keep { x, y, fit })
    }

    /// The transform from a viewBox's user space to a box of this size, with
    /// its top-left corner at the origin.
    ///
    /// [`None`] when nothing can be shown: a viewBox or a box with no area.
    /// SVG says a viewBox of zero width or height disables rendering, which is
    /// a drawing of nothing rather than an error.
    pub fn transform(self, view: ViewBox, size: (f32, f32)) -> Option<Matrix> {
        let (width, height) = size;
        if view.width <= 0.0 || view.height <= 0.0 || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let (mut across, mut down) = (width / view.width, height / view.height);
        let (x, y) = match self {
            Self::Stretch => (Align::Min, Align::Min),
            Self::Keep { x, y, fit } => {
                let scale = match fit {
                    Fit::Meet => across.min(down),
                    Fit::Slice => across.max(down),
                };
                across = scale;
                down = scale;
                (x, y)
            }
        };
        let shift = |align: Align, room: f32, used: f32| match align {
            Align::Min => 0.0,
            Align::Mid => (room - used) / 2.0,
            Align::Max => room - used,
        };
        let e = shift(x, width, view.width * across) - view.x * across;
        let f = shift(y, height, view.height * down) - view.y * down;
        let matrix = Matrix {
            a: across,
            b: 0.0,
            c: 0.0,
            d: down,
            e,
            f,
        };
        [across, down, e, f]
            .iter()
            .all(|value| value.is_finite())
            .then_some(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(x: f32, y: f32, width: f32, height: f32) -> ViewBox {
        ViewBox {
            x,
            y,
            width,
            height,
        }
    }

    fn corners(ratio: AspectRatio, view: ViewBox, size: (f32, f32)) -> [(f32, f32); 2] {
        let matrix = ratio.transform(view, size).expect("a transform");
        [
            matrix.apply(view.x, view.y),
            matrix.apply(view.x + view.width, view.y + view.height),
        ]
    }

    #[test]
    fn every_value_svg_writes_is_read() {
        assert_eq!(AspectRatio::parse("none"), Some(AspectRatio::Stretch));
        assert_eq!(
            AspectRatio::parse("xMinYMax slice"),
            Some(AspectRatio::Keep {
                x: Align::Min,
                y: Align::Max,
                fit: Fit::Slice
            }),
        );
        assert_eq!(
            AspectRatio::parse("defer xMidYMid"),
            Some(AspectRatio::default())
        );
        for bad in [
            "",
            "xmidymid",
            "xMidYMid fit",
            "xMid",
            "none meet extra",
            "xMaxYTop",
        ] {
            assert_eq!(AspectRatio::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_offline_screens_viewbox_fills_its_square() {
        // A 24-unit viewBox in a 56-pixel square: everything grows by 56 / 24.
        let matrix = AspectRatio::default()
            .transform(view(0.0, 0.0, 24.0, 24.0), (56.0, 56.0))
            .expect("a transform");
        assert!((matrix.a - 56.0 / 24.0).abs() < 1.0e-6);
        assert_eq!(matrix.apply(24.0, 24.0), (56.0, 56.0));
    }

    #[test]
    fn meet_shows_all_of_it_and_aligns_the_rest() {
        let wide = (100.0, 50.0);
        let square = view(0.0, 0.0, 10.0, 10.0);
        assert_eq!(
            corners(AspectRatio::default(), square, wide),
            [(25.0, 0.0), (75.0, 50.0)],
            "centred across",
        );
        let start = AspectRatio::parse("xMinYMin").expect("a value");
        assert_eq!(corners(start, square, wide), [(0.0, 0.0), (50.0, 50.0)]);
        let end = AspectRatio::parse("xMaxYMax meet").expect("a value");
        assert_eq!(corners(end, square, wide), [(50.0, 0.0), (100.0, 50.0)]);
    }

    #[test]
    fn slice_covers_the_box_and_none_stretches() {
        let wide = (100.0, 50.0);
        let square = view(0.0, 0.0, 10.0, 10.0);
        let slice = AspectRatio::parse("xMidYMid slice").expect("a value");
        assert_eq!(corners(slice, square, wide), [(0.0, -25.0), (100.0, 75.0)]);
        assert_eq!(
            corners(AspectRatio::Stretch, square, wide),
            [(0.0, 0.0), (100.0, 50.0)]
        );
    }

    #[test]
    fn a_viewbox_that_does_not_start_at_zero_is_moved_to_the_corner() {
        let shifted = view(-5.0, 10.0, 10.0, 10.0);
        assert_eq!(
            corners(AspectRatio::default(), shifted, (20.0, 20.0)),
            [(0.0, 0.0), (20.0, 20.0)],
        );
    }

    #[test]
    fn nothing_with_no_area_is_shown() {
        let ratio = AspectRatio::default();
        assert_eq!(
            ratio.transform(view(0.0, 0.0, 0.0, 10.0), (10.0, 10.0)),
            None
        );
        assert_eq!(
            ratio.transform(view(0.0, 0.0, 10.0, 10.0), (0.0, 10.0)),
            None
        );
        assert_eq!(
            ratio.transform(view(0.0, 0.0, 1.0e-38, 1.0e-38), (1.0e38, 1.0e38)),
            None,
            "a scale that overflows is nothing, not an infinity"
        );
    }
}
