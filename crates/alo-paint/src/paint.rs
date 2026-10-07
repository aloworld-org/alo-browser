/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
use alo_value::{Extent, Gradient, Rgba, Shape};
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
/// **Radial**: rings around the centre the author gave, the last of them
/// through the side or corner the extent names (see [`radii`]). How far along
/// a point is, is how far out it is in proportion to that last ring.
fn along(gradient: &Gradient, area: Rect, x: f32, y: f32) -> f32 {
    match gradient {
        Gradient::Linear { angle, .. } => {
            let centre_x = area.left() + area.size.width / 2.0;
            let centre_y = area.top() + area.size.height / 2.0;
            let (dx, dy) = (x - centre_x, y - centre_y);
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
        Gradient::Radial {
            shape,
            extent,
            centre,
            ..
        } => {
            let centre_x = area.left() + centre.x.along(area.size.width);
            let centre_y = area.top() + centre.y.along(area.size.height);
            let (radius_x, radius_y) = radii(*shape, *extent, area, (centre_x, centre_y));
            if radius_x <= 0.0 || radius_y <= 0.0 {
                // A last ring of no size: every point is past it, which is
                // what CSS says such a gradient draws.
                return 1.0;
            }
            let across = (x - centre_x) / radius_x;
            let down = (y - centre_y) / radius_y;
            (across * across + down * down).sqrt().clamp(0.0, 1.0)
        }
    }
}

/// The last ring's radii, across and down, for a gradient centred at
/// `centre` in `area`.
///
/// To a side is the nearest or farthest distance to a side; a circle takes
/// one of all four, an ellipse one across and one down. To a corner, a
/// circle is the distance to that corner. An ellipse keeps the proportions
/// it would have had to the matching side and is made just big enough to
/// pass through the corner, which for a centred one is the familiar √2 times
/// the half-width and half-height.
fn radii(shape: Shape, extent: Extent, area: Rect, centre: (f32, f32)) -> (f32, f32) {
    let (centre_x, centre_y) = centre;
    let left = (centre_x - area.left()).abs();
    let right = (area.left() + area.size.width - centre_x).abs();
    let top = (centre_y - area.top()).abs();
    let bottom = (area.top() + area.size.height - centre_y).abs();
    let nearest = matches!(extent, Extent::ClosestSide | Extent::ClosestCorner);
    let pick = |a: f32, b: f32| if nearest { a.min(b) } else { a.max(b) };
    // The side distances, and the corner they meet at.
    let (side_x, side_y) = (pick(left, right), pick(top, bottom));
    let corners = [(left, top), (right, top), (left, bottom), (right, bottom)];
    let corner = corners
        .into_iter()
        .map(|(across, down)| (across, down, across.hypot(down)))
        .reduce(|held, next| {
            let better = if nearest {
                next.2 < held.2
            } else {
                next.2 > held.2
            };
            if better { next } else { held }
        })
        .map_or((0.0, 0.0), |(across, down, _)| (across, down));
    match (shape, extent) {
        (Shape::Circle, Extent::ClosestSide | Extent::FarthestSide) => {
            let radius = pick(side_x, side_y);
            (radius, radius)
        }
        (Shape::Circle, Extent::ClosestCorner | Extent::FarthestCorner) => {
            let radius = corner.0.hypot(corner.1);
            (radius, radius)
        }
        (Shape::Ellipse, Extent::ClosestSide | Extent::FarthestSide) => (side_x, side_y),
        (Shape::Ellipse, Extent::ClosestCorner | Extent::FarthestCorner) => {
            if side_x <= 0.0 || side_y <= 0.0 {
                return (0.0, 0.0);
            }
            // The ellipse with the sides' proportions through the corner:
            // x²/(k·r)² + y²/r² = 1, with k the proportion across to down.
            let proportion = side_x / side_y;
            let radius_y = ((corner.0 / proportion).powi(2) + corner.1.powi(2)).sqrt();
            (radius_y * proportion, radius_y)
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
    use alo_value::{Angle, Color, Offset, Position, Stop};

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
                shape: Shape::Ellipse,
                extent: Extent::FarthestCorner,
                centre: Position::CENTRE,
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

    /// White in the middle to black at the last ring.
    fn rings(shape: Shape, extent: Extent, centre: Position, area: Rect) -> Paint {
        Paint::Gradient {
            gradient: Gradient::Radial {
                shape,
                extent,
                centre,
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
            area,
            current: Rgba::BLACK,
        }
    }

    /// How far along its gradient a point is, read back from its grey.
    fn how_far(paint: &Paint, x: f32, y: f32) -> f32 {
        1.0 - paint.at(x, y).red
    }

    #[test]
    fn a_circle_at_a_point_is_round_and_centred_there() {
        // Meet's tint: a circle at 92% across the top of a wide box. Its
        // farthest corner is the bottom left one, √(184² + 50²) away.
        let wide = Rect::new(0.0, 0.0, 200.0, 50.0);
        let centre = Position {
            x: Offset::Fraction(0.92),
            y: Offset::Pixels(0.0),
        };
        let paint = rings(Shape::Circle, Extent::FarthestCorner, centre, wide);
        assert!(how_far(&paint, 184.0, 0.0) < 0.001, "white at the centre");
        let reach = 184.0_f32.hypot(50.0);
        assert!((how_far(&paint, 184.0 - 40.0, 0.0) - 40.0 / reach).abs() < 0.001);
        assert!(
            (how_far(&paint, 184.0, 40.0) - 40.0 / reach).abs() < 0.001,
            "the same distance down is the same colour: a circle",
        );
        assert!(how_far(&paint, 0.0, 50.0) > 0.999, "black at that corner");
    }

    #[test]
    fn each_extent_puts_the_last_ring_where_it_says() {
        // Centred 20 from the left and 10 from the top of a 100 × 50 box:
        // sides 20, 80, 10 and 40 away, corners from √(20² + 10²) to
        // √(80² + 40²).
        let area = Rect::new(0.0, 0.0, 100.0, 50.0);
        let centre = Position {
            x: Offset::Pixels(20.0),
            y: Offset::Pixels(10.0),
        };
        let circle = |extent| rings(Shape::Circle, extent, centre, area);
        let ellipse = |extent| rings(Shape::Ellipse, extent, centre, area);
        let one = |paint: &Paint, x: f32, y: f32| (how_far(paint, x, y) - 1.0).abs() < 0.001;

        assert!(one(&circle(Extent::ClosestSide), 20.0, 20.0), "10 down");
        assert!(one(&circle(Extent::FarthestSide), 100.0, 10.0), "80 across");
        assert!(one(
            &circle(Extent::ClosestCorner),
            20.0 + 500.0_f32.sqrt(),
            10.0
        ));
        assert!(one(
            &circle(Extent::FarthestCorner),
            20.0 + 8000.0_f32.sqrt(),
            10.0
        ));

        let closest = ellipse(Extent::ClosestSide);
        assert!(one(&closest, 40.0, 10.0) && one(&closest, 20.0, 20.0));
        let farthest = ellipse(Extent::FarthestSide);
        assert!(one(&farthest, 100.0, 10.0) && one(&farthest, 20.0, 50.0));
        // Through the corner, in the sides' proportions.
        assert!(one(&ellipse(Extent::ClosestCorner), 0.0, 0.0));
        assert!(one(&ellipse(Extent::FarthestCorner), 100.0, 50.0));
        let corner = ellipse(Extent::FarthestCorner);
        let across = how_far(&corner, 60.0, 10.0);
        let down = how_far(&corner, 20.0, 30.0);
        assert!(
            (across - down).abs() < 0.001,
            "40 across is 20 down, as 80 is to 40: {across} and {down}",
        );
    }

    #[test]
    fn a_last_ring_of_no_size_draws_the_last_colour() {
        let area = Rect::new(0.0, 0.0, 100.0, 50.0);
        let corner = Position {
            x: Offset::Fraction(0.0),
            y: Offset::Fraction(0.0),
        };
        let paint = rings(Shape::Circle, Extent::ClosestSide, corner, area);
        assert_eq!(paint.at(50.0, 25.0), Rgba::BLACK);
        assert_eq!(paint.at(0.0, 0.0), Rgba::BLACK);
    }

    #[test]
    fn a_centre_a_long_way_off_is_answered_rather_than_divided_by() {
        let area = Rect::new(0.0, 0.0, 100.0, 50.0);
        for (x, y) in [
            (f32::MAX, 0.0),
            (-f32::MAX, f32::MAX),
            (3e38, -3e38),
            (0.0, f32::MIN_POSITIVE),
        ] {
            let centre = Position {
                x: Offset::Pixels(x),
                y: Offset::Pixels(y),
            };
            for shape in [Shape::Circle, Shape::Ellipse] {
                for extent in [
                    Extent::ClosestSide,
                    Extent::ClosestCorner,
                    Extent::FarthestSide,
                    Extent::FarthestCorner,
                ] {
                    let found = rings(shape, extent, centre, area).at(50.0, 25.0);
                    assert!(
                        (0.0..=1.0).contains(&found.alpha),
                        "{shape:?} {extent:?} at ({x}, {y}): {found:?}",
                    );
                }
            }
        }
    }
}
