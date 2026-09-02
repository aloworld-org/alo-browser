//! How a shape is filled: one colour, or a colour that changes across it.
//!
//! A fill used to be an `Rgba` and nothing else. A gradient is a *function of
//! position*, so the display list carries the function and the area it is
//! measured against, and the renderer asks it once per pixel. Keeping the
//! question in one place is what stops a linear gradient and a radial one
//! disagreeing about where the middle of a box is.
//!
//! # Where a gradient is measured
//!
//! Against the **padding box**, which is what `background-origin` starts at,
//! while the shape being filled is the border box — so a gradient under a
//! thick border keeps the size it would have had without one. The two
//! rectangles are the same on almost every box, and being right about the ones
//! where they differ costs one field.

use alo_layout::Rect;
use alo_value::{Gradient, Rgba};
use core::fmt;

/// What to fill a shape with.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// One colour, everywhere.
    Solid(Rgba),
    /// A colour that changes across an area.
    Gradient {
        /// The stops, and which way they run.
        gradient: Gradient,
        /// What the gradient is measured against.
        area: Rect,
        /// What `currentColor` means here, for a stop that asked for it.
        current: Rgba,
    },
}

impl Paint {
    /// The colour at a point, in the same coordinates the area is in.
    pub fn at(&self, x: f32, y: f32) -> Rgba {
        match self {
            Paint::Solid(color) => *color,
            Paint::Gradient {
                gradient,
                area,
                current,
            } => gradient.at(along(gradient, *area, x, y), *current),
        }
    }

    /// Whether filling with this would change nothing.
    ///
    /// A gradient every one of whose stops is invisible draws nothing; one
    /// with a single visible stop draws something everywhere, because the
    /// stops in between are blends of the two ends.
    pub fn is_invisible(&self) -> bool {
        match self {
            Paint::Solid(color) => color.is_invisible(),
            Paint::Gradient {
                gradient, current, ..
            } => gradient
                .stops()
                .iter()
                .all(|stop| stop.color.resolve(*current).is_invisible()),
        }
    }

    /// The one colour this fills with, if it is one colour.
    ///
    /// The renderer takes a shorter path for a flat fill: a gradient costs a
    /// blend per pixel, and most boxes are one colour.
    pub fn solid(&self) -> Option<Rgba> {
        match self {
            Paint::Solid(color) => Some(*color),
            Paint::Gradient { .. } => None,
        }
    }
}

/// How far along its gradient a point is, from zero to one.
///
/// **Linear**: the gradient line runs through the centre of the area at the
/// angle the author gave, and is long enough that its two ends sit exactly at
/// the corners' projections — which is why `linear-gradient(45deg, …)` reaches
/// its last colour at the corner rather than part way up the side.
///
/// **Radial**: an ellipse centred on the area, with the same proportions as
/// the area, passing through its farthest corner. That is CSS's default —
/// `farthest-corner` — and it is why a gradient in a wide box is an oval.
fn along(gradient: &Gradient, area: Rect, x: f32, y: f32) -> f32 {
    let (half_width, half_height) = (area.size.width / 2.0, area.size.height / 2.0);
    let centre_x = area.left() + half_width;
    let centre_y = area.top() + half_height;
    let (dx, dy) = (x - centre_x, y - centre_y);

    match gradient {
        Gradient::Linear { angle, .. } => {
            // Degrees clockwise from upwards, and `y` runs down the page.
            let radians = angle.0.to_radians();
            let (unit_x, unit_y) = (radians.sin(), -radians.cos());
            let length = (area.size.width * unit_x).abs() + (area.size.height * unit_y).abs();
            if length <= 0.0 {
                return 0.0;
            }
            let projected = dx * unit_x + dy * unit_y;
            (projected / length + 0.5).clamp(0.0, 1.0)
        }
        Gradient::Radial { .. } => {
            // The ellipse through the farthest corner has radii √2 times the
            // half-width and half-height: substitute the corner into the
            // ellipse and both terms come out the same.
            let radius_x = half_width * core::f32::consts::SQRT_2;
            let radius_y = half_height * core::f32::consts::SQRT_2;
            if radius_x <= 0.0 || radius_y <= 0.0 {
                return 0.0;
            }
            let across = dx / radius_x;
            let down = dy / radius_y;
            (across * across + down * down).sqrt().clamp(0.0, 1.0)
        }
    }
}

impl fmt::Display for Paint {
    /// A flat fill reads as its colour and nothing else, so that a display
    /// list that has no gradients in it says exactly what it always said.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Paint::Solid(color) => write!(f, "{color}"),
            Paint::Gradient { gradient, .. } => write!(f, "{gradient}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::{Angle, Color, Stop};

    /// Two colours are the same to within a rounding of the last bit.
    fn close(left: Rgba, right: Rgba) -> bool {
        (left.red - right.red).abs() < 0.001
            && (left.green - right.green).abs() < 0.001
            && (left.blue - right.blue).abs() < 0.001
            && (left.alpha - right.alpha).abs() < 0.001
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 100.0)
    }

    fn red_to_blue(angle: Angle) -> Paint {
        Paint::Gradient {
            gradient: Gradient::Linear {
                angle,
                stops: vec![
                    Stop {
                        color: Color::Rgba(Rgba::new(1.0, 0.0, 0.0, 1.0)),
                        position: None,
                    },
                    Stop {
                        color: Color::Rgba(Rgba::new(0.0, 0.0, 1.0, 1.0)),
                        position: None,
                    },
                ],
            },
            area: area(),
            current: Rgba::BLACK,
        }
    }

    #[test]
    fn a_flat_fill_is_the_same_colour_everywhere() {
        let paint = Paint::Solid(Rgba::WHITE);
        assert_eq!(paint.at(0.0, 0.0), Rgba::WHITE);
        assert_eq!(paint.at(999.0, -50.0), Rgba::WHITE);
        assert_eq!(paint.solid(), Some(Rgba::WHITE));
        assert_eq!(paint.to_string(), Rgba::WHITE.to_string());
    }

    #[test]
    fn a_gradient_runs_from_its_first_colour_to_its_last() {
        let paint = red_to_blue(Angle::DOWN);
        let top = paint.at(50.0, 0.0);
        let bottom = paint.at(50.0, 100.0);
        assert!(top.red > 0.99 && top.blue < 0.01, "the top is red: {top}");
        assert!(
            bottom.blue > 0.99 && bottom.red < 0.01,
            "the bottom is blue: {bottom}",
        );
        assert_eq!(paint.solid(), None);
    }

    #[test]
    fn the_middle_of_a_two_stop_gradient_is_half_way() {
        let middle = red_to_blue(Angle::DOWN).at(50.0, 50.0);
        assert!(
            (middle.red - 0.5).abs() < 0.01 && (middle.blue - 0.5).abs() < 0.01,
            "expected half of each, got {middle}",
        );
    }

    #[test]
    fn an_angle_turns_the_gradient_rather_than_the_box() {
        let across = red_to_blue(Angle(90.0));
        let left = across.at(0.0, 50.0);
        let right = across.at(100.0, 50.0);
        assert!(left.red > 0.99, "ninety degrees runs left to right: {left}");
        assert!(right.blue > 0.99, "{right}");

        // Down the page, at ninety degrees, nothing changes.
        assert!(close(across.at(50.0, 10.0), across.at(50.0, 90.0)));
    }

    #[test]
    fn past_the_ends_a_gradient_holds_its_end_colours() {
        let paint = red_to_blue(Angle::DOWN);
        assert!(close(paint.at(50.0, -500.0), paint.at(50.0, 0.0)));
        assert!(close(paint.at(50.0, 500.0), paint.at(50.0, 100.0)));
    }

    #[test]
    fn a_radial_gradient_starts_in_the_middle_and_reaches_the_corner() {
        let paint = Paint::Gradient {
            gradient: Gradient::Radial {
                stops: vec![
                    Stop {
                        color: Color::Rgba(Rgba::WHITE),
                        position: None,
                    },
                    Stop {
                        color: Color::Rgba(Rgba::BLACK),
                        position: None,
                    },
                ],
            },
            area: area(),
            current: Rgba::BLACK,
        };
        assert_eq!(paint.at(50.0, 50.0), Rgba::WHITE);
        let corner = paint.at(100.0, 100.0);
        assert!(
            corner.red < 0.01,
            "the farthest corner is the last stop: {corner}"
        );
        // The same distance in any direction is the same colour.
        assert!(close(paint.at(50.0, 0.0), paint.at(0.0, 50.0)));
    }

    #[test]
    fn a_box_with_no_area_is_answered_rather_than_divided_by() {
        let paint = Paint::Gradient {
            gradient: Gradient::Linear {
                angle: Angle::DOWN,
                stops: vec![
                    Stop {
                        color: Color::Rgba(Rgba::WHITE),
                        position: None,
                    },
                    Stop {
                        color: Color::Rgba(Rgba::BLACK),
                        position: None,
                    },
                ],
            },
            area: Rect::new(0.0, 0.0, 0.0, 0.0),
            current: Rgba::BLACK,
        };
        assert_eq!(paint.at(0.0, 0.0), Rgba::WHITE);
    }

    #[test]
    fn a_fill_nobody_would_see_says_so() {
        assert!(Paint::Solid(Rgba::TRANSPARENT).is_invisible());
        assert!(!Paint::Solid(Rgba::BLACK).is_invisible());

        let invisible = Paint::Gradient {
            gradient: Gradient::Linear {
                angle: Angle::DOWN,
                stops: vec![
                    Stop {
                        color: Color::Rgba(Rgba::TRANSPARENT),
                        position: None,
                    },
                    Stop {
                        color: Color::Rgba(Rgba::TRANSPARENT),
                        position: None,
                    },
                ],
            },
            area: area(),
            current: Rgba::BLACK,
        };
        assert!(invisible.is_invisible());
        assert!(!red_to_blue(Angle::DOWN).is_invisible());
    }
}
