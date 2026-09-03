//! Rounded corners, and the shape a box actually is.
//!
//! `border-radius` is the difference between a box that looks like a browser
//! default and one that looks like a design system, so it is the first of
//! `docs/features.md`'s Paint line to arrive after flat colour. It changes
//! what the *shape* is — which is why clipping comes with it: `overflow:
//! hidden` clips to the box's shape, and if the shape has round corners then
//! so does the clip.
//!
//! # Why the radii can shrink
//!
//! Two corners on one edge can ask for more room than the edge has —
//! `border-radius: 100px` on a box forty pixels wide. CSS says to scale every
//! radius by the same factor until they all fit, rather than clamping them
//! one at a time: scaling keeps the shape's proportions, and clamping would
//! turn a circle into a lozenge on one side only.

use crate::path::{Path, Point};
use alo_layout::Rect;
use alo_value::{FontMetrics, parse_length_percentage};

/// How round each corner of a box is, in CSS pixels.
///
/// Each corner has two radii — across and down — because CSS allows an ellipse
/// rather than only a circle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    /// Top left, as `(horizontal, vertical)`.
    pub top_left: (f32, f32),
    /// Top right.
    pub top_right: (f32, f32),
    /// Bottom right.
    pub bottom_right: (f32, f32),
    /// Bottom left.
    pub bottom_left: (f32, f32),
}

impl Corners {
    /// Square corners.
    pub const SQUARE: Corners = Corners {
        top_left: (0.0, 0.0),
        top_right: (0.0, 0.0),
        bottom_right: (0.0, 0.0),
        bottom_left: (0.0, 0.0),
    };

    /// The same radius on every corner.
    pub fn all(radius: f32) -> Self {
        let pair = (radius.max(0.0), radius.max(0.0));
        Self {
            top_left: pair,
            top_right: pair,
            bottom_right: pair,
            bottom_left: pair,
        }
    }

    /// Whether every corner is square, and so the shape is a plain rectangle.
    pub fn are_square(self) -> bool {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
        .iter()
        .all(|(across, down)| *across <= 0.0 && *down <= 0.0)
    }

    /// These radii, scaled down until they fit inside a box.
    ///
    /// One factor for all of them, which is what CSS says: scaling keeps the
    /// shape's proportions where clamping each corner on its own would make
    /// one side rounder than another.
    #[must_use]
    pub fn fitted_to(self, size: alo_layout::Size) -> Self {
        let pairs = [
            (self.top_left.0 + self.top_right.0, size.width),
            (self.bottom_left.0 + self.bottom_right.0, size.width),
            (self.top_left.1 + self.bottom_left.1, size.height),
            (self.top_right.1 + self.bottom_right.1, size.height),
        ];
        let factor = pairs
            .iter()
            .filter(|(asked, _)| *asked > 0.0)
            .map(|(asked, available)| (available / asked).min(1.0))
            .fold(1.0f32, f32::min)
            .max(0.0);
        if factor >= 1.0 {
            return self;
        }
        let scale = |(across, down): (f32, f32)| (across * factor, down * factor);
        Self {
            top_left: scale(self.top_left),
            top_right: scale(self.top_right),
            bottom_right: scale(self.bottom_right),
            bottom_left: scale(self.bottom_left),
        }
    }

    /// Read `border-radius` and the four per-corner longhands from a style.
    ///
    /// The shorthand takes one to four values in the same order as every other
    /// box-model shorthand, and an optional `/` splits the horizontal radii
    /// from the vertical ones.
    /// `size` is the box the radii are for, because a percentage radius is a
    /// percentage **of the box** — horizontally of its width and vertically of
    /// its height. It used to resolve against nothing and come out as zero,
    /// which was written down as a limitation and was the reason a
    /// `border-radius: 50%` did nothing at all. Found when the user-agent sheet
    /// tried to make a radio button round.
    pub fn of(style: &alo_style::ComputedStyle, size: (f32, f32)) -> Self {
        let metrics = style.metrics();
        let (wide, tall) = size;
        let shorthand = style.get("border-radius").unwrap_or("");
        let (across, down) = if let Some((across, down)) = shorthand.split_once('/') {
            (lengths(across, metrics, wide), lengths(down, metrics, tall))
        } else {
            (
                lengths(shorthand, metrics, wide),
                lengths(shorthand, metrics, tall),
            )
        };
        let corner = |index: usize, longhand: &str| -> (f32, f32) {
            if let Some(text) = style.get(longhand) {
                // A longhand takes one or two values: the horizontal radius and
                // then the vertical one, so each resolves against its own
                // dimension.
                let pair = lengths(text, metrics, wide);
                let vertical = lengths(text, metrics, tall);
                return (
                    pair.first().copied().unwrap_or(0.0),
                    vertical
                        .get(1)
                        .or_else(|| vertical.first())
                        .copied()
                        .unwrap_or(0.0),
                );
            }
            (side(&across, index), side(&down, index))
        };
        Self {
            top_left: corner(0, "border-top-left-radius"),
            top_right: corner(1, "border-top-right-radius"),
            bottom_right: corner(2, "border-bottom-right-radius"),
            bottom_left: corner(3, "border-bottom-left-radius"),
        }
    }
}

/// The lengths in a value, as pixels.
///
/// `against` is what a percentage is a percentage *of*: the box's width for a
/// horizontal radius and its height for a vertical one.
fn lengths(text: &str, metrics: FontMetrics, against: f32) -> Vec<f32> {
    text.split_ascii_whitespace()
        .filter_map(parse_length_percentage)
        .map(|value| value.to_px(metrics, against).max(0.0))
        .collect()
}

/// One corner's value out of a one-to-four-value list, in CSS's order.
fn side(values: &[f32], corner: usize) -> f32 {
    let pick = |index: usize| values.get(index).copied().unwrap_or(0.0);
    // Two values are the two diagonals — top-left with bottom-right, and the
    // other pair — where every other box-model shorthand pairs the opposite
    // sides. Three values name the two diagonals and then one of them again.
    match (values.len(), corner) {
        (0, _) => 0.0,
        (1, _) | (2, 0 | 2) | (3, 0) => pick(0),
        (2, _) | (3, 1 | 3) => pick(1),
        (3, _) => pick(2),
        (_, corner) => pick(corner),
    }
}

/// How far along a straight line towards the corner a circular arc's control
/// point sits.
///
/// The number that makes four cubic curves look like a circle. It is not
/// arbitrary — it is the best approximation there is, and every renderer that
/// draws a circle from Béziers uses it.
const ARC: f32 = 0.552_284_7;

/// The shape between two rounded rectangles: a border, as a ring.
///
/// The outer edge is wound one way and the inner edge the other, so the
/// non-zero fill rule leaves the middle empty. That is how a border with
/// rounded corners is actually drawn — four rectangles would have square
/// corners over a rounded background, which is what it looked like before this
/// existed.
pub fn ring(outer: Rect, corners: Corners, widths: alo_layout::Edges) -> Path {
    let inner_rect = outer.shrunk_by(widths);
    // The inner shape's corners are the outer ones less the border, which is
    // what makes a border of even thickness all the way round a curve.
    let inner_corners = Corners {
        top_left: inset(corners.top_left, widths.left, widths.top),
        top_right: inset(corners.top_right, widths.right, widths.top),
        bottom_right: inset(corners.bottom_right, widths.right, widths.bottom),
        bottom_left: inset(corners.bottom_left, widths.left, widths.bottom),
    };
    between(
        &rounded_rectangle(outer, corners),
        &rounded_rectangle(inner_rect, inner_corners),
    )
}

/// One shape with another cut out of it.
///
/// The inner shape is wound the other way, so the non-zero fill rule leaves it
/// empty. This is what [`ring`] is made of, and it is also how an inset shadow
/// is drawn: the shadow of a hole is the shape outside it, blurred.
pub fn between(outer: &Path, inner: &Path) -> Path {
    let mut path = outer.clone();
    for segment in reversed(inner).segments() {
        push(&mut path, *segment);
    }
    path
}

fn inset((across, down): (f32, f32), horizontal: f32, vertical: f32) -> (f32, f32) {
    ((across - horizontal).max(0.0), (down - vertical).max(0.0))
}

fn push(path: &mut Path, segment: crate::path::Segment) {
    match segment {
        crate::path::Segment::MoveTo(to) => path.move_to(to),
        crate::path::Segment::LineTo(to) => path.line_to(to),
        crate::path::Segment::QuadTo(control, to) => path.quad_to(control, to),
        crate::path::Segment::CubicTo(first, second, to) => path.cubic_to(first, second, to),
        crate::path::Segment::Close => path.close(),
    }
}

/// The same shape, drawn the other way round.
///
/// Winding is what tells the fill rule which side is inside, so a hole is the
/// same outline wound backwards.
fn reversed(path: &Path) -> Path {
    use crate::path::Segment;
    // Collect the points each segment ends at, and walk them backwards.
    let mut points: Vec<Point> = Vec::new();
    let mut curves: Vec<(Point, Point)> = Vec::new();
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) | Segment::LineTo(to) => {
                points.push(to);
                curves.push((to, to));
            }
            Segment::QuadTo(control, to) => {
                points.push(to);
                curves.push((control, control));
            }
            Segment::CubicTo(first, second, to) => {
                points.push(to);
                curves.push((first, second));
            }
            Segment::Close => {}
        }
    }
    let mut out = Path::new();
    let Some(last) = points.last().copied() else {
        return out;
    };
    out.move_to(last);
    for index in (0..points.len()).rev() {
        let target = if index == 0 {
            last
        } else {
            points.get(index - 1).copied().unwrap_or(last)
        };
        match (points.get(index), curves.get(index)) {
            (Some(_), Some((first, second))) if first != second || *first != target => {
                // A curve, with its control points swapped end for end.
                out.cubic_to(*second, *first, target);
            }
            _ => out.line_to(target),
        }
    }
    out.close();
    out
}

/// The outline of a box with rounded corners.
///
/// A box with square corners comes back as a plain rectangle rather than four
/// zero-radius arcs, so that the common case costs nothing and reads clearly
/// in a display list.
pub fn rounded_rectangle(rect: Rect, corners: Corners) -> Path {
    let corners = corners.fitted_to(rect.size);
    if corners.are_square() {
        return Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height);
    }

    let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let mut path = Path::new();

    // Clockwise from just after the top-left corner.
    path.move_to(Point::new(left + corners.top_left.0, top));
    path.line_to(Point::new(right - corners.top_right.0, top));
    arc(
        &mut path,
        Point::new(right - corners.top_right.0, top),
        Point::new(right, top + corners.top_right.1),
        Point::new(right, top),
    );
    path.line_to(Point::new(right, bottom - corners.bottom_right.1));
    arc(
        &mut path,
        Point::new(right, bottom - corners.bottom_right.1),
        Point::new(right - corners.bottom_right.0, bottom),
        Point::new(right, bottom),
    );
    path.line_to(Point::new(left + corners.bottom_left.0, bottom));
    arc(
        &mut path,
        Point::new(left + corners.bottom_left.0, bottom),
        Point::new(left, bottom - corners.bottom_left.1),
        Point::new(left, bottom),
    );
    path.line_to(Point::new(left, top + corners.top_left.1));
    arc(
        &mut path,
        Point::new(left, top + corners.top_left.1),
        Point::new(left + corners.top_left.0, top),
        Point::new(left, top),
    );
    path.close();
    path
}

/// One quarter-ellipse, from `from` to `to`, bending around `corner`.
fn arc(path: &mut Path, from: Point, to: Point, corner: Point) {
    path.cubic_to(
        Point::new(
            from.x + (corner.x - from.x) * ARC,
            from.y + (corner.y - from.y) * ARC,
        ),
        Point::new(
            to.x + (corner.x - to.x) * ARC,
            to.y + (corner.y - to.y) * ARC,
        ),
        to,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::Size;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn square_corners_make_a_plain_rectangle() {
        let path = rounded_rectangle(Rect::new(0.0, 0.0, 10.0, 10.0), Corners::SQUARE);
        assert_eq!(path.segments().len(), 5, "four lines and a close");
        assert!(Corners::SQUARE.are_square());
    }

    #[test]
    fn a_rounded_box_is_four_lines_and_four_arcs() {
        let path = rounded_rectangle(Rect::new(0.0, 0.0, 20.0, 20.0), Corners::all(4.0));
        assert_eq!(
            path.segments().len(),
            10,
            "a move, four lines, four curves and a close",
        );
        assert_eq!(path.bounds(), Some((0.0, 0.0, 20.0, 20.0)));
    }

    #[test]
    fn a_radius_larger_than_the_box_is_scaled_down_rather_than_clamped() {
        // Both corners of the top edge want 100 of a 40-pixel edge, so
        // everything scales by a fifth — including the corners on the tall
        // edges, which is what keeps the shape's proportions.
        let asked = Corners::all(100.0);
        let fitted = asked.fitted_to(Size::new(40.0, 40.0));
        assert!(close(fitted.top_left.0, 20.0));
        assert!(close(fitted.bottom_right.1, 20.0));
    }

    #[test]
    fn radii_that_fit_are_left_alone() {
        let asked = Corners::all(4.0);
        assert_eq!(asked.fitted_to(Size::new(40.0, 40.0)), asked);
    }

    #[test]
    fn one_side_asking_too_much_scales_the_other_side_too() {
        let asked = Corners {
            top_left: (30.0, 30.0),
            top_right: (30.0, 30.0),
            bottom_right: (2.0, 2.0),
            bottom_left: (2.0, 2.0),
        };
        let fitted = asked.fitted_to(Size::new(40.0, 100.0));
        assert!(close(fitted.top_left.0, 20.0));
        assert!(
            close(fitted.bottom_right.0, 4.0 / 3.0),
            "scaled by the same factor: {}",
            fitted.bottom_right.0,
        );
    }

    #[test]
    fn a_box_of_no_size_has_no_corners_rather_than_a_division_by_nothing() {
        let fitted = Corners::all(10.0).fitted_to(Size::ZERO);
        assert!(fitted.are_square());
    }

    #[test]
    fn the_shorthand_is_split_the_way_css_splits_it() {
        assert!(close(side(&[4.0], 0), 4.0));
        assert!(close(side(&[4.0], 3), 4.0));

        // Two values are the two diagonals.
        assert!(close(side(&[1.0, 2.0], 0), 1.0));
        assert!(close(side(&[1.0, 2.0], 1), 2.0));
        assert!(close(side(&[1.0, 2.0], 2), 1.0));
        assert!(close(side(&[1.0, 2.0], 3), 2.0));

        assert!(close(side(&[1.0, 2.0, 3.0], 2), 3.0));
        assert!(close(side(&[1.0, 2.0, 3.0], 3), 2.0));
        assert!(close(side(&[1.0, 2.0, 3.0, 4.0], 3), 4.0));
        assert!(close(side(&[], 0), 0.0));
    }

    #[test]
    fn an_arc_stays_inside_the_corner_it_bends_around() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 10.0));
        arc(
            &mut path,
            Point::new(0.0, 10.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 0.0),
        );
        let (left, top, right, bottom) = path.bounds().expect("a shape");
        assert!(close(left, 0.0) && close(top, 0.0));
        assert!(close(right, 10.0) && close(bottom, 10.0));
    }
}
